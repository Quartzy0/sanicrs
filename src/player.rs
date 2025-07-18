use std::error::Error;
use std::io::Cursor;
use std::sync::Arc;
use std::time::Duration;
use rodio::{Decoder, OutputStream, Sink};
use rodio::source::EmptyCallback;
use tokio::sync::mpsc::UnboundedSender;
use tokio::sync::RwLock;
use uuid::Uuid;
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

pub struct PlayerInfo {
    client: Arc<OpenSubsonicClient>,
    sink: Sink,
    current_song_id: RwLock<String>,
    track_list: Arc<RwLock<TrackList>>,
    cmd_channel: Arc<UnboundedSender<PlayerCommand>>,
}

impl PlayerInfo {
    pub fn new(
        client: Arc<OpenSubsonicClient>,
        stream_handle: &OutputStream,
        track_list: Arc<RwLock<TrackList>>,
        cmd_channel: Arc<UnboundedSender<PlayerCommand>>,
    ) -> Self {
        PlayerInfo {
            client,
            sink: Sink::connect_new(stream_handle.mixer()),
            current_song_id: RwLock::new("".to_string()),
            track_list,
            cmd_channel
        }
    }

    pub async fn goto(&self, index: usize) -> Result<(), Box<dyn Error + Send + Sync>> {
        self.track_list.write().await.set_current(index);
        self.start_current().await
    }

    pub async fn remove_song(&self, index: usize) -> Result<(), Box<dyn Error + Send + Sync>> {
        let mut guard = self.track_list.write().await;
        let c = guard.current_index();
        guard.remove_song(index);
        if c != guard.current_index() {
            self.start_current().await?;
        }
        Ok(())
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

    pub async fn start_current(&self) -> Result<(), Box<dyn Error + Send + Sync>> {
        let track_list = self.track_list.read().await;
        let song = match track_list.current() {
            None => {return Ok(())}
            Some(s) => &s.1
        };
        {
            let x = self.current_song_id.read().await;
            if song.id == x.as_str() {
                return Ok(());
            }
        }
        println!("Playing: {}", song.title);
        let stream = self
            .client
            .stream(&*song.id, None, None, None, None, Some(true), None);

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
        if let Some(suffix) = &song.suffix {
            decoder = decoder.with_hint(suffix);
        }
        if let Some(mime) = &song.media_type {
            decoder = decoder.with_mime_type(mime);
        }

        self.sink.clear();
        self.sink.append(decoder.build()?);
        let cmd_channel = self.cmd_channel.clone();
        let callback_source = EmptyCallback::new(Box::new(move || {
            cmd_channel.send(PlayerCommand::Next).expect("Error sending message Next");
        }));
        self.sink.append(callback_source);
        self.sink.play();
        {
            let mut x = self.current_song_id.write().await;
            *x = song.id.clone();
        }

        Ok(())
    }

    pub async fn next(&self) {
        {
            let mut track_list = self.track_list.write().await;
            track_list.next();
        }
        self.start_current()
            .await
            .expect("Error starting next track");
    }

    pub async fn previous(&self) {
        {
            let mut track_list = self.track_list.write().await;
            track_list.previous();
        }
        self.start_current()
            .await
            .expect("Error starting next track");
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

    pub fn volume(&self) -> f64 {
        self.sink.volume() as f64
    }

    pub fn set_volume(&self, volume: f64) {
        self.sink.set_volume(volume as f32);
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

    pub shuffled: bool,
    pub looping: bool,
}

impl TrackList {
    pub fn new() -> Self {
        TrackList{
            songs: Vec::new(),
            current: 0,
            shuffled: false,
            looping: false
        }
    }

    pub async fn add_song_from_uri(&mut self, uri: &str, client: Arc<OpenSubsonicClient>, index: Option<usize>) -> Option<Box<dyn Error + Send + Sync>> {
        if !uri.starts_with("sanic://song/"){
            return Some(InvalidResponseError::new_boxed("Invalid URI, should be sanic://song/<song-id>"));
        }
        let id = &uri[13..]; // 13 is length of "sanic://song/"
        self.add_song_from_id(id, client, index).await
    }
    
    pub async fn add_song_from_id(&mut self, id: &str, client: Arc<OpenSubsonicClient>, index: Option<usize>) -> Option<Box<dyn Error + Send + Sync>> {
        let result = client.get_song(id).await;
        if result.is_err(){
            return result.err();
        }
        let song = result.unwrap();
        self.add_song(song, index);
        None
    }
    
    pub fn set_current(&mut self, index: usize) {
        self.current = index;
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
        self.looping = false;
    }

    pub fn empty(&self) -> bool {
        self.songs.is_empty()
    }

    pub fn next(&mut self) {
        if self.current != self.songs.len()-1 {
            self.current += 1;
        } else {
            self.current = 0;
        }
    }

    pub fn previous(&mut self) {
        if self.current != 0 {
            self.current -= 1;
        }
    }

    pub fn current(&self) -> Option<&SongEntry> {
        self.songs.get(self.current)
    }

    pub fn current_index(&self) -> Option<usize> {
        if self.songs.len() > 0{
            Some(self.current)
        } else {
            None
        }
    }

    pub fn add_song(&mut self, song: Arc<Song>, index: Option<usize>) {
        match index {
            None => self.songs.push((Uuid::new_v4(), song).into()),
            Some(i) => {
                if i <= self.current {
                    self.current += 1;
                }
                self.songs.insert(i, (Uuid::new_v4(), song).into());
            }
        }
    }

    pub fn add_songs(&mut self, songs: &Vec<Arc<Song>>) {
        let mut x: Vec<SongEntry> = songs.iter().map(|song| {
            (Uuid::new_v4(), song.clone()).into()
        }).collect();
        self.songs.append(&mut x);
    }

    pub fn get_songs(&self) -> &Vec<SongEntry> {
        &self.songs
    }
}