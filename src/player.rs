use std::cmp::PartialEq;
use std::error::Error;
use std::io::Cursor;
use std::sync::Arc;
use async_channel::Sender;
use rand::Rng;
use relm4::gtk::gio::prelude::SettingsExt;
use relm4::gtk::gio::Settings;
use std::time::Duration;
use rand::prelude::SliceRandom;
use rodio::{Decoder, OutputStream, Sink};
use rodio::source::EmptyCallback;
use tokio::sync::RwLock;
use uuid::Uuid;
use zvariant::ObjectPath;
use crate::opensonic::cache::SongCache;
use crate::opensonic::client::OpenSubsonicClient;
use crate::opensonic::types::{InvalidResponseError, Song};
use crate::PlayerCommand;

pub const MAX_PLAYBACK_RATE: f64 = 10.0;
pub const MIN_PLAYBACK_RATE: f64 = 0.0;

#[derive(Debug)]
pub enum PlaybackStatus {
    Playing,
    Paused,
    Stopped
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum LoopStatus {
    None,
    Track,
    Playlist,
}

impl Into<String> for LoopStatus {
    fn into(self) -> String {
        match self {
            LoopStatus::None => "None",
            LoopStatus::Track => "Track",
            LoopStatus::Playlist => "Playlist"
        }.to_string()
    }
}

impl TryFrom<String> for LoopStatus {
    type Error = zbus::fdo::Error;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        match value.as_str() {
            "None" => Ok(LoopStatus::None),
            "Track" => Ok(LoopStatus::Track),
            "Playlist" => Ok(LoopStatus::Playlist),
            _ => Err(zbus::fdo::Error::Failed(format!("Unknown loop status: {}", value)))
        }
    }
}

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
    volume: f64
}

impl Default for PlayerSettings {
    fn default() -> Self {
        Self { replay_gain_mode: Default::default(), volume: 1.0 }
    }
}

impl PlayerSettings {
    pub fn load_settings(&mut self, settings: &Settings) -> Result<(), Box<dyn Error + Send + Sync>>{
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

        Ok(())
    }
}

pub struct PlayerInfo {
    client: Arc<OpenSubsonicClient>,
    sink: Sink,
    track_list: Arc<RwLock<TrackList>>,
    cmd_channel: Arc<Sender<PlayerCommand>>,

    settings: RwLock<PlayerSettings>,
}

impl PlayerInfo {
    pub fn new(
        client: Arc<OpenSubsonicClient>,
        stream_handle: &OutputStream,
        track_list: Arc<RwLock<TrackList>>,
        cmd_channel: Arc<Sender<PlayerCommand>>,
    ) -> Self {
        PlayerInfo {
            client,
            sink: Sink::connect_new(stream_handle.mixer()),
            track_list,
            cmd_channel,
            settings: RwLock::new(Default::default())
        }
    }

    pub fn load_settings_blocking(&self, settings: &Settings) -> Result<(), Box<dyn Error + Send + Sync>>{
        let mut s = self.settings.blocking_write();
        s.load_settings(settings)
    }

    pub async fn load_settings(&self, settings: &Settings) -> Result<(), Box<dyn Error + Send + Sync>>{
        let mut s = self.settings.write().await;
        s.load_settings(settings)
    }

    pub async fn loop_status(&self) -> LoopStatus {
        self.track_list.read().await.loop_status
    }

    pub async fn goto(&self, index: usize) -> Result<Option<SongEntry>, Box<dyn Error + Send + Sync>> {
        self.track_list.write().await.set_current(index);
        self.start_current().await
    }

    pub async fn remove_song(&self, index: usize) -> Result<SongEntry, Box<dyn Error + Send + Sync>> {
        let mut guard = self.track_list.write().await;
        let c = guard.current_index();
        let e = guard.remove_song(index);
        if c != guard.current_index() {
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

    pub async fn start_current(&self) -> Result<Option<SongEntry>, Box<dyn Error + Send + Sync>> {
        let track_list = self.track_list.read().await;
        let song = match track_list.current() {
            None => {return Ok(None)}
            Some(s) => s
        };
        /*{
            let x = self.current_song_id.read().await;
            if song.1.id == x.as_str() {
                return Ok(Some(song.clone()));
            }
        }*/
        println!("Playing: {}", song.1.title);
        let stream = self
            .client
            .stream(&*song.1.id, None, None, None, None, Some(true), None);

        let response = stream
            .await
            .expect("Error when reading bytes");
        let len = response.headers().get("Content-Length").and_then(|t| match t.to_str() {
            Ok(v) => match v.parse::<u64>() {
                Ok(v) => Some(v),
                Err(_) => None
            },
            Err(_) => None
        });
        let x1 = response
            .bytes()
            .await?; // TODO: Figure this out without downloading entire file first
        let reader = Cursor::new(x1);
        /*let x = stream.await.expect("Error reading stream").bytes_stream();
        let reader =
            StreamReader::new(x.map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e)));


        let reader = StreamDownload::new_async_read(
            AsyncReadStreamParams::new(reader),
            MemoryStorageProvider::default(),
            Settings::default(),
        ).await?;*/

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
            cmd_channel.send_blocking(PlayerCommand::Next).expect("Error sending message Next");
        }));
        self.sink.append(callback_source);
        self.sink.play();

        let v = self.settings.read().await.volume;
        let mul = v * 10.0_f64.powf(self.gain_from_track(Some(song)).await/20.0);
        self.sink.set_volume(mul as f32);

        Ok(Some(song.clone()))
    }

    pub async fn next(&self) -> Option<SongEntry> {
        let over;
        {
            let mut track_list = self.track_list.write().await;
            over = track_list.next();
        }
        if over {
            self.cmd_channel.send(PlayerCommand::Pause).await.expect("Error sending message to player");
            None
        } else {
            self.start_current()
                .await
                .expect("Error starting next track")
        }
    }

    pub async fn previous(&self) -> Option<SongEntry> {
        {
            let mut track_list = self.track_list.write().await;
            track_list.previous();
        }
        self.start_current()
            .await
            .expect("Error starting next track")
    }

    pub async fn stop(&self) {
        self.sink.clear();
        let mut track_list = self.track_list.write().await;
        track_list.clear();
    }

    pub async fn set_position(&self, position: Duration) -> Result<(), Box<dyn Error>> {
        let position = position;
        {
            let track_list = self.track_list.read().await;
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

    pub async fn gain_from_track(&self, song: Option<&SongEntry>) -> f64 {
        match self.settings.read().await.replay_gain_mode {
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

    pub async fn gain(&self) -> f64 {
        self.gain_from_track(self.track_list.read().await.current()).await
    }

    async fn set_set_volume(&self) {
        let v = self.settings.read().await.volume;
        let mul = v * 10.0_f64.powf(self.gain().await/20.0);
        self.sink.set_volume(mul as f32);
    }

    pub async fn volume(&self) -> f64 {
        self.settings.read().await.volume
    }

    pub async fn set_volume(&self, volume: f64) {
        {
            let mut settings = self.settings.write().await;
            settings.volume = volume;
        }
        self.set_set_volume().await;
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

    pub fn set_rate(&self, rate: f64) {
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
            // self.send_signal(CurrentSongMsg::RateChange(rate));
        }
    }

    pub fn position(&self) -> i64 {
        (self.sink.get_pos().as_micros() as f64 /* * self.sink.speed() as f64*/) as i64
    }

    pub async fn shuffled(&self) -> bool {
        self.track_list.read().await.shuffled
    }
}

#[derive(Clone, Debug)]
pub struct SongEntry(
    pub Uuid,
    pub Arc<Song>
);

impl SongEntry {
    pub fn dbus_path(&self) -> String {
        format!("/me/quartzy/sanicrs/song/{}", self.0.as_simple().to_string())
    }

    pub fn dbus_obj<'a>(&self) -> ObjectPath<'a> {
        ObjectPath::try_from(self.dbus_path().clone()).expect("Error when making object path")
    }
}

impl From<(Uuid, Arc<Song>)> for SongEntry {
    fn from(value: (Uuid, Arc<Song>)) -> Self {
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

    pub async fn add_song_from_uri(&mut self, uri: &str, client: &SongCache, index: Option<usize>) -> Option<Box<dyn Error + Send + Sync>> {
        if !uri.starts_with("sanic://song/"){
            return Some(InvalidResponseError::new_boxed("Invalid URI, should be sanic://song/<song-id>"));
        }
        let id = &uri[13..]; // 13 is length of "sanic://song/"
        self.add_song_from_id(id, client, index).await
    }

    pub async fn add_song_from_id(&mut self, id: &str, client: &SongCache, index: Option<usize>) -> Option<Box<dyn Error + Send + Sync>> {
        let result = client.get_song(id).await;
        if result.is_err(){
            return result.err();
        }
        let song = result.unwrap();
        self.add_song(song, index);
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
        if self.current < index && self.current != 0 {
            self.current -= 1;
        }
        self.songs.remove(index)
    }

    pub fn clear(&mut self) {
        self.songs.clear();
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

    pub fn current(&self) -> Option<&SongEntry> {
        if self.shuffled {
            self.songs.get(self.shuffled_order[self.current])
        } else {
            self.songs.get(self.current)
        }
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

    pub fn add_song(&mut self, song: Arc<Song>, index: Option<usize>) {
        let index = index.unwrap_or(self.songs.len());
        if index <= self.current {
            self.current += 1;
        }
        self.songs.insert(index, (Uuid::new_v4(), song).into());
        if self.shuffled {
            let mut rng = rand::rng();
            self.shuffled_order.insert(rng.random_range(self.current..=self.songs.len()), index);
        }
    }

    pub fn add_songs(&mut self, songs: Vec<Arc<Song>>) {
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
}
