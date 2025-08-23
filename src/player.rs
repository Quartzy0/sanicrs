use std::cell::RefCell;
use crate::opensonic::cache::SongCache;
use crate::opensonic::client::OpenSubsonicClient;
use crate::opensonic::types::{InvalidResponseError, Song};
use crate::ui::track_list::MoveDirection;
use crate::PlayerCommand;
use async_channel::Sender;
use futures_util::TryStreamExt;
use mpris_server::{LoopStatus, PlaybackStatus, TrackId};
use rand::prelude::SliceRandom;
use rand::Rng;
use relm4::gtk::gio::prelude::SettingsExt;
use relm4::gtk::gio::Settings;
use rodio::source::EmptyCallback;
use rodio::{Decoder, OutputStream, Sink};
use std::error::Error;
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;
use stream_download::async_read::AsyncReadStreamParams;
use stream_download::storage::memory::MemoryStorageProvider;
use stream_download::StreamDownload;
use tokio_util::io::StreamReader;
use uuid::Uuid;

#[derive(Debug, Default)]
#[repr(u8)]
pub enum ReplayGainMode {
    #[default]
    None = 0,
    Track = 1,
    Album = 2
}

#[derive(Debug)]
pub struct PlayerSettings {
    replay_gain_mode: ReplayGainMode,
    volume: f64,
    should_scrobble: bool,
}

impl Default for PlayerSettings {
    fn default() -> Self {
        Self { replay_gain_mode: Default::default(), volume: 1.0, should_scrobble: true }
    }
}

impl PlayerSettings {
    pub fn load_settings(&mut self, settings: &Settings) -> Result<(), Box<dyn Error>>{
        let mode: u8 = settings.value("replay-gain-mode").try_get()?;
        self.replay_gain_mode = match mode {
            0 => ReplayGainMode::None,
            1 => ReplayGainMode::Track,
            2 => ReplayGainMode::Album,
            v => {
                eprintln!("Unknown replay-gain-mode setting: {}", v);
                ReplayGainMode::None
            }
        };
        self.volume = settings.value("volume").try_get()?;
        self.should_scrobble = settings.boolean("should-scrobble");

        Ok(())
    }
}

pub struct PlayerInfo {
    client: Rc<OpenSubsonicClient>,
    sink: Sink,
    track_list: RefCell<TrackList>,
    cmd_channel: Arc<Sender<PlayerCommand>>,

    settings: RefCell<PlayerSettings>,
}

impl PlayerInfo {
    pub fn new(
        client: Rc<OpenSubsonicClient>,
        stream_handle: &OutputStream,
        track_list: TrackList,
        cmd_channel: Arc<Sender<PlayerCommand>>,
    ) -> Self {
        PlayerInfo {
            client,
            sink: Sink::connect_new(stream_handle.mixer()),
            track_list: RefCell::new(track_list),
            cmd_channel,
            settings: RefCell::default()
        }
    }

    pub fn track_list(&self) -> &RefCell<TrackList> {
        &self.track_list
    }

    pub fn load_settings(&self, settings: &Settings) -> Result<(), Box<dyn Error>>{
        let mut s = self.settings.borrow_mut();
        s.load_settings(settings)
    }

    pub fn loop_status(&self) -> LoopStatus {
        self.track_list.borrow().loop_status
    }

    pub fn set_loop_status(&self, loop_status: LoopStatus) {
        self.track_list.borrow_mut().loop_status = loop_status;
    }

    pub async fn goto(&self, index: usize) -> Result<Option<SongEntry>, Box<dyn Error>> {
        self.track_list.borrow_mut().set_current(index);
        self.start_current().await
    }

    pub async fn remove_song(&self, index: usize) -> Result<SongEntry, Box<dyn Error>> {
        let mut guard = self.track_list.borrow_mut();
        let c = guard.current().and_then(|s| Some(s.0.clone()));
        let e = guard.remove_song(index);
        if let Some(c) = c && c == e.0 { // Check if previously playing entry is the same as the removed one
            drop(guard);
            self.start_current().await?;
        }
        Ok(e)
    }

    pub async fn play(&self) {
        if !self.sink.empty() {
            self.sink.play();
        } else {
            self.start_current().await.expect("Error playing");
        }
    }

    pub fn pause(&self) {
        self.sink.pause();
    }

    pub async fn playpause(&self) {
        if self.sink.is_paused() || self.sink.empty() {
            self.play().await;
        } else {
            self.pause();
        }
    }

    pub async fn start_current(&self) -> Result<Option<SongEntry>, Box<dyn Error>> {
        let track_list = self.track_list.borrow();
        let song = match track_list.current() {
            None => {return Ok(None)}
            Some(s) => s
        };

        println!("Playing: {}", song.1.title);
        let stream = self
            .client
            .stream(&*song.1.id, None, None, None, None, Some(true), None);

        let response = stream
            .await?;
        let len = response.headers().get("Content-Length").and_then(|t| match t.to_str() {
            Ok(v) => match v.parse::<u64>() {
                Ok(v) => Some(v),
                Err(_) => None
            },
            Err(_) => None
        });
        let x = response.bytes_stream();
        let reader =
            StreamReader::new(x.map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e)));


        let reader = StreamDownload::new_async_read(
            AsyncReadStreamParams::new(reader),
            MemoryStorageProvider::default(),
            stream_download::Settings::default(),
        ).await?;

        let mut decoder = Decoder::builder().with_data(reader).with_seekable(true);
        if let Some(len) = len {
            decoder = decoder.with_byte_len(len);
        }
        if let Some(suffix) = &song.1.suffix {
            decoder = decoder.with_hint(suffix);
        }
        if let Some(mime) = &song.1.media_type {
            decoder = decoder.with_mime_type(mime);
        }

        self.sink.clear();
        self.sink.append(decoder.build()?);
        let cmd_channel = self.cmd_channel.clone();
        let callback_source = EmptyCallback::new(Box::new(move || {
            cmd_channel.send_blocking(PlayerCommand::TrackOver).expect("Error sending message TrackOver");
        }));
        self.sink.append(callback_source);
        self.sink.play();

        let v = self.settings.borrow().volume;
        let mul = v * 10.0_f64.powf(self.gain_from_track(Some(song))/20.0);
        self.sink.set_volume(mul as f32);

        if self.settings.borrow().should_scrobble {
            self.client.scrobble(song.1.id.as_str(), Some(false)).await?;
        }

        Ok(Some(song.clone()))
    }

    pub async fn next(&self) -> Option<SongEntry> {
        let over;
        {
            let mut track_list = self.track_list.borrow_mut();
            over = track_list.next();
        }
        if over {
            None
        } else {
            self.start_current()
                .await
                .expect("Error starting next track")
        }
    }

    pub async fn previous(&self) -> Option<SongEntry> {
        {
            let mut track_list = self.track_list.borrow_mut();
            track_list.previous();
        }
        self.start_current()
            .await
            .expect("Error starting next track")
    }

    pub fn stop(&self) {
        self.sink.clear();
        let mut track_list = self.track_list.borrow_mut();
        track_list.clear();
    }

    pub fn set_position(&self, position: Duration) -> Result<(), Box<dyn Error>> {
        let position = position;
        {
            let track_list = self.track_list.borrow();
            let song = match track_list.current() {
                Some(t) => t,
                None => return Ok(())
            };
            if let Some(duration) = song.1.duration && position > duration {
                return Ok(());
            }
        }
        self.sink.try_seek(position)?;
        Ok(())
    }

    pub fn gain_from_track(&self, song: Option<&SongEntry>) -> f64 {
        match self.settings.borrow().replay_gain_mode {
            ReplayGainMode::None => 0.0,
            ReplayGainMode::Track => match song {
                Some(entry) => entry.1.replay_gain.as_ref().and_then(|x| x.track_gain).unwrap_or(0.0) as f64,
                None => 0.0,
            },
            ReplayGainMode::Album => match song {
                Some(entry) => entry.1.replay_gain.as_ref().and_then(|x| x.album_gain).unwrap_or(0.0) as f64,
                None => 0.0,
            },
        }
    }

    pub fn gain(&self) -> f64 {
        self.gain_from_track(self.track_list.borrow().current())
    }

    fn set_set_volume(&self) {
        let v = self.settings.borrow().volume;
        let mul = v * 10.0_f64.powf(self.gain()/20.0);
        self.sink.set_volume(mul as f32);
    }

    pub fn volume(&self) -> f64 {
        self.settings.borrow().volume
    }

    pub fn set_volume(&self, volume: f64) {
        {
            let mut settings = self.settings.borrow_mut();
            settings.volume = volume;
        }
        self.set_set_volume();
    }

    pub fn playback_status(&self) -> PlaybackStatus {
        if self.sink.empty() {
            PlaybackStatus::Stopped
        } else {
            if self.sink.is_paused() {
                PlaybackStatus::Paused
            } else {
                PlaybackStatus::Playing
            }
        }
    }

    pub fn rate(&self) -> f64 {
        self.sink.speed() as f64
    }

    // See comment in dbus/player.rs on set_rate function
    /*pub fn set_rate(&self, rate: f64) {
        let rate = if rate > MAX_PLAYBACK_RATE {
            MAX_PLAYBACK_RATE
        } else if rate < MIN_PLAYBACK_RATE {
            MIN_PLAYBACK_RATE
        } else {
            rate
        };
        if rate == 0.0 {
            self.pause();
        } else {
            self.sink.set_speed(rate as f32);
        }
    }*/

    pub fn position(&self) -> i64 {
        self.sink.get_pos().mul_f64(self.rate()).as_micros() as i64
    }

    pub fn shuffled(&self) -> bool {
        self.track_list.borrow().shuffled
    }

    pub fn set_shuffled(&self, shuffled: bool) {
        self.track_list.borrow_mut().set_shuffle(shuffled);
    }
}

#[derive(Clone, Debug)]
pub struct SongEntry(
    pub Uuid,
    pub Rc<Song>
);

impl SongEntry {
    pub fn dbus_path(&self) -> String {
        format!("/me/quartzy/sanicrs/song/{}", self.0.as_simple().to_string())
    }

    pub fn dbus_obj<'a>(&self) -> TrackId {
        TrackId::try_from(self.dbus_path().clone()).expect("Error when making object path")
    }
}

impl From<(Uuid, Rc<Song>)> for SongEntry {
    fn from(value: (Uuid, Rc<Song>)) -> Self {
        Self {
            0: value.0,
            1: value.1
        }
    }
}

pub struct TrackList {
    songs: Vec<SongEntry>,
    current: usize,
    shuffled_order: Vec<usize>,

    shuffled: bool,
    pub loop_status: LoopStatus,
}

impl TrackList {
    pub fn new() -> Self {
        TrackList{
            songs: Vec::new(),
            current: 0,
            shuffled: false,
            loop_status: LoopStatus::None,
            shuffled_order: Vec::new(),
        }
    }

    pub fn is_suffled(&self) -> bool {
        self.shuffled
    }

    pub fn set_shuffle(&mut self, shuffle: bool) {
        if shuffle {
            let start = if self.current + 1 >= self.songs.len() {
                0
            } else {
                self.current + 1
            };
            let mut shuffled_order: Vec<usize> = (start..self.songs.len()).collect();
            shuffled_order.shuffle(&mut rand::rng());
            self.shuffled_order = (0..start).collect();
            self.shuffled_order.append(&mut shuffled_order);
        }
        self.shuffled = shuffle;
    }

    pub async fn add_song_from_uri(&mut self, uri: &str, client: &SongCache, index: Option<usize>) -> Option<Box<dyn Error>> {
        if !uri.starts_with("sanic://song/"){
            return Some(InvalidResponseError::new_boxed("Invalid URI, should be sanic://song/<song-id>"));
        }
        let id = &uri[13..]; // 13 is length of "sanic://song/"
        self.add_song_from_id(id, client, index).await
    }

    pub async fn add_song_from_id(&mut self, id: &str, client: &SongCache, index: Option<usize>) -> Option<Box<dyn Error>> {
        let result = client.get_song(id).await;
        if result.is_err(){
            return result.err();
        }
        let song = result.unwrap();
        self.insert_song(song, index);
        None
    }

    pub fn set_current(&mut self, index: usize) {
        if !self.shuffled {
            self.current = index;
        } else {
            self.current = self.shuffled_order.iter().position(|i| *i == index).unwrap_or(index);
        }
    }

    pub fn remove_song(&mut self, index: usize) -> SongEntry {
        if self.current > index && self.current != 0 {
            self.current -= 1;
        }
        self.songs.remove(index)
    }

    pub fn clear(&mut self) {
        self.songs.clear();
        self.shuffled_order.clear();
        self.current = 0;
        self.shuffled = false;
        self.loop_status = LoopStatus::None;
    }

    pub fn empty(&self) -> bool {
        self.songs.is_empty()
    }

    pub fn next(&mut self) -> bool {
        match self.loop_status {
            LoopStatus::None => {
                self.current += 1;

                self.current >= self.songs.len()
            },
            LoopStatus::Track => false,
            LoopStatus::Playlist => {
                if self.current != self.songs.len()-1 {
                    self.current += 1;
                } else {
                    self.current = 0;
                }
                false
            }
        }
    }

    pub fn previous(&mut self) {
        if self.current != 0 {
            self.current -= 1;
        }
    }

    pub fn song_at_index(&self, i: usize) -> Option<&SongEntry> {
        if self.shuffled {
            self.songs.get(self.shuffled_order[i])
        } else {
            self.songs.get(i)
        }
    }

    pub fn current(&self) -> Option<&SongEntry> {
        self.song_at_index(self.current)
    }

    pub fn current_index(&self) -> Option<usize> {
        if self.songs.len() > 0{
            if self.shuffled {
                Some(self.shuffled_order[self.current])
            } else {
                Some(self.current)
            }
        } else {
            None
        }
    }

    pub fn insert_song(&mut self, song: Rc<Song>, index: Option<usize>) {
        let index = if let Some(index) = index {
            if index <= self.current {
                self.current += 1;
            }
            self.songs.insert(index, (Uuid::new_v4(), song).into());
            index
        } else {
            self.songs.push((Uuid::new_v4(), song).into());
            self.songs.len()-1
        };
        if self.shuffled {
            let mut rng = rand::rng();
            self.shuffled_order.insert(rng.random_range(self.current..=self.songs.len()), index);
        }
    }

    pub fn add_songs(&mut self, songs: Vec<Rc<Song>>) {
        let mut x: Vec<SongEntry> = songs.into_iter().map(|song| {
            (Uuid::new_v4(), song).into()
        }).collect();
        if self.shuffled {
            let prev_songs_n = self.songs.len();
            let songs_n = x.len() + prev_songs_n;
            let mut rng = rand::rng();
            for i in prev_songs_n..songs_n {
                self.shuffled_order.insert(rng.random_range(self.current..=self.shuffled_order.len()), i);
            }
        }
        self.songs.append(&mut x);
    }

    pub fn get_songs(&self) -> &Vec<SongEntry> {
        &self.songs
    }

    // Returns the index of the current song after the move. This may be the same as before
    // or it may be different due to the move. If None is returned, no move was performed.
    pub fn move_song(&mut self, index: usize, direction: MoveDirection) -> Option<usize> {
        let new_i = match direction {
            MoveDirection::Up => if index == 0 {
                return None;
            } else {
                index - 1
            },
            MoveDirection::Down => if index == self.songs.len()-1 {
                return None;
            } else {
                index + 1
            },
        };
        self.songs.swap(index, new_i);
        // When shuffled also swap the two indexes in the shuffled_order vec. This should leave the shuffled order unaffected
        if self.shuffled {
            let oldi = self.shuffled_order.iter().find(|i| **i==index).expect("Couldn't find old index in shuffle map").clone();
            let newi = self.shuffled_order.iter().find(|i| **i==new_i).expect("Couldn't find new index in shuffle map").clone();
            self.shuffled_order.swap(oldi, newi);
        } else {
            if index == self.current {
                self.current = new_i;
            } else if new_i == self.current {
                self.current = index;
            }
        }
        self.current_index()
    }
}
