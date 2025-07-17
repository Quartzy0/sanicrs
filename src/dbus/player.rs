use std::cell::{Cell, RefCell};
use crate::opensonic::client::OpenSubsonicClient;
use crate::player::TrackList;
use crate::ui::current_song::{CurrentSong, CurrentSongMsg, SongInfo};
use relm4::AsyncComponentSender;
use rodio::{OutputStream, Sink};
use std::collections::HashMap;
use std::error::Error;
use std::io::Cursor;
use std::ops::{Add, Deref};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use zbus::interface;
use zvariant::{Array, ObjectPath, Str, Value};
use crate::opensonic::types::Song;

const MAX_PLAYBACK_RATE: f64 = 10.0;
const MIN_PLAYBACK_RATE: f64 = 0.0;

pub struct MprisPlayer {
    client: Arc<OpenSubsonicClient>,
    sink: Sink,
    current_song_id: RwLock<String>,
    // stream_handle: Mixer,
    track_list: Arc<RwLock<TrackList>>,

    model_sender: Option<AsyncComponentSender<CurrentSong>>,
}

impl MprisPlayer {
    pub fn new(
        client: Arc<OpenSubsonicClient>,
        stream_handle: &OutputStream,
        track_list: Arc<RwLock<TrackList>>,
    ) -> Self {
        // let sink = ;
        MprisPlayer {
            client,
            sink: Sink::connect_new(stream_handle.mixer()),
            current_song_id: RwLock::new("".to_string()),
            // stream_handle,
            track_list,
            model_sender: None
        }
    }
}

impl MprisPlayer {
    pub fn set_model(&mut self, model_sender: AsyncComponentSender<CurrentSong>) {
        self.model_sender = Some(model_sender);
    }

    pub async fn start_current(&self) -> Result<(), Box<dyn Error + Send + Sync>> {
        let track_list = self.track_list.read().await;
        let song = match track_list.current() {
            None => {return Ok(())}
            Some(s) => s
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
            .stream(&*song.id, None, None, None, None, None, None);

        let x1 = stream
            .await
            .expect("Error when reading bytes")
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

        let decoder = match &song.suffix {
            None => rodio::Decoder::new(reader)?,
            Some(suffix) => match suffix.as_str() {
                "wav" => rodio::Decoder::new_wav(reader)?,
                "ogg" => rodio::Decoder::new_vorbis(reader)?,
                "flac" => rodio::Decoder::new_flac(reader)?,
                _ => rodio::Decoder::new(reader)?,
            },
        };

        self.sink.clear();
        self.sink.append(decoder);
        self.sink.play();
        {
            let mut x = self.current_song_id.write().await;
            *x = song.id.clone();
        }

        self.send_signal(CurrentSongMsg::SongUpdate(SongInfo::from(song)));
        self.update_playback_state();

        Ok(())
    }

    fn update_playback_state(&self) {
        self.send_signal(CurrentSongMsg::PlaybackStateChange(String::from(self.playback_status())));
    }
    
    pub fn set_volume_no_notify(&self, volume: f64){
        self.sink.set_volume(volume as f32);
    }

    pub fn send_signal(&self, signal: CurrentSongMsg) {
        if let Some(model_sender) = &self.model_sender{
            model_sender.input(signal);
        }
    }
}

pub async fn get_song_metadata<'a>(song: &&Song, client: Arc<OpenSubsonicClient>) -> Result<HashMap<&'a str, Value<'a>>, zbus::fdo::Error> {
    let mut map: HashMap<&str, Value> = HashMap::new();
    map.insert("mpris:trackid", Value::ObjectPath(ObjectPath::try_from(song.dbus_path()).expect("Invalid object path")));
    map.insert("xesam:title", Value::Str(Str::from(song.title.clone())));
    if let Some(cover_art) = &song.cover_art{
        let url = client.get_cover_image_url(cover_art.as_str()).await;
        if let Some(url) = url {
            map.insert("mpris:artUrl", Value::Str(Str::from(url)));
        }
    }
    if let Some(duration) = song.duration{
        map.insert("mpris:length", Value::I64(duration.as_micros() as i64));
    }
    if let Some(album) = &song.album {
        map.insert("xesam:album", Value::Str(Str::from(album.clone())));
    }
    if let Some(artists) = &song.artists {
        let a: Vec<String> = artists.iter().map(|x| x.name.clone()).collect();
        map.insert("xesam:artist", Value::Array(Array::from(a)));
    }
    if let Some(artists) = &song.album_artists {
        let al: Vec<String> = artists.iter().map(|x| x.name.clone()).collect();
        map.insert("xesam:albumArtist", Value::Array(Array::from(al)));
    }
    if let Some(artists) = &song.genres {
        let g: Vec<String> = artists.iter().map(|x| x.name.clone()).collect();
        map.insert("xesam:genre", Value::Array(Array::from(g)));
    }
    if let Some(comment) = &song.comment {
        map.insert("xesam:comment", Value::Str(Str::from(comment.clone())));
    }
    if let Some(composer) = &song.display_composer {
        map.insert("xesam:composer", Value::Str(Str::from(composer.clone())));
    }
    if let Some(played) = &song.played {
        map.insert("xesam:lastUsed", Value::Str(Str::from(played.clone())));
    }
    if let Some(played) = song.play_count {
        map.insert("xesam:useCount", Value::I64(played as i64));
    }
    if let Some(track) = song.track {
        map.insert("xesam:trackNumber", Value::I64(track as i64));
    }
    if let Some(rating) = song.user_rating {
        map.insert("xesam:trackNumber", Value::F64(rating as f64 / 5.0));
    }
    if let Some(disc_number) = song.disc_number {
        map.insert("xesam:discNumber", Value::I64(disc_number as i64));
    }
    if let Some(bpm) = song.bpm {
        map.insert("xesam:audioBPM", Value::I64(bpm as i64));
    }

    Ok(map)
}

#[interface(name = "org.mpris.MediaPlayer2.Player")]
impl MprisPlayer {
    pub async fn open_uri(&self, uri: &str) -> Result<(), zbus::fdo::Error> {
        let mut track_list = self.track_list.write().await;
        let err = track_list.add_song_from_uri(uri, self.client.clone(), None).await;
        match err {
            None => Ok(()),
            Some(err) => Err(zbus::fdo::Error::Failed(format!("Error when adding song: {}", err)))
        }
    }

    pub async fn play(&self) {
        if !self.sink.empty() {
            self.sink.play();
        } else {
            self.start_current().await.expect("Error playing");
        }
        self.update_playback_state();
    }

    pub async fn pause(&self) {
        if !self.sink.empty() {
            self.sink.pause();
        }
        self.update_playback_state();
    }

    pub async fn play_pause(&self) {
        if self.sink.is_paused() || self.sink.empty() {
            self.play().await;
        } else {
            self.pause().await;
        }
    }

    pub async fn next(&mut self) {
        {
            let mut track_list = self.track_list.write().await;
            track_list.next();
        }
        self.start_current()
            .await
            .expect("Error starting next track");
    }

    pub async fn previous(&mut self) {
        {
            let mut track_list = self.track_list.write().await;
            track_list.previous();
        }
        self.start_current()
            .await
            .expect("Error starting next track");
    }

    pub async fn stop(&mut self) {
        self.sink.clear();
        let mut track_list = self.track_list.write().await;
        track_list.clear();
        self.update_playback_state();
    }

    pub async fn set_position(&mut self, track_id: &str, position: i64) -> Result<(), zbus::fdo::Error> {
        if position < 0{
            return Ok(());
        }
        let position = Duration::from_micros(position as u64);
        {
            let track_list = self.track_list.read().await;
            let song = match track_list.current() {
                Some(t) => t,
                None => return Ok(())
            };
            if song.dbus_path() != track_id { 
                return Ok(());
            }
            if let Some(duration) = song.duration && position > duration {
                return Ok(());
            }
        }
        self.sink.try_seek(position).map_err(|e| zbus::fdo::Error::IOError(format!("Error when seeking: {}", e)))?;
        self.send_signal(CurrentSongMsg::ProgressUpdateSync(Some(position.as_secs_f64())));
        Ok(())
    }

    pub async fn seek(&mut self, offset: i64) -> Result<(), zbus::fdo::Error> {
        let current_position = self.sink.get_pos();
        let new_positon = if offset > 0 {
            current_position.add(Duration::from_micros(offset as u64))
        } else {
            current_position.checked_sub(Duration::from_micros((-offset) as u64)).unwrap_or_else(|| Duration::from_secs(0))
        };
        let mut seek_next = false;
        {
            let track_list = self.track_list.read().await;
            let song = match track_list.current() {
                Some(t) => t,
                None => return Ok(())
            };
            let song_duration = song.duration;
            if let Some(song_duration) = song_duration {
                seek_next = song_duration <= new_positon;
            }
        }
        if seek_next {
            self.next().await;
        } else {
            self.sink.try_seek(new_positon).map_err(|e| zbus::fdo::Error::IOError(format!("Error when seeking: {}", e)))?;
            self.send_signal(CurrentSongMsg::ProgressUpdateSync(Some(new_positon.as_secs_f64())));
        }
        Ok(())
    }

    #[zbus(property)]
    pub async fn metadata(&self) -> Result<HashMap<&str, Value>, zbus::fdo::Error> {
        let track_list = self.track_list.read().await;
        let song = match track_list.current() {
            Some(t) => t,
            None => return Err(zbus::fdo::Error::Failed("No song currently playing".to_string()))
        };

        get_song_metadata(&song, self.client.clone()).await
    }

    #[zbus(property)]
    pub fn volume(&self) -> f64 {
        self.sink.volume() as f64
    }

    #[zbus(property)]
    pub fn set_volume(&self, volume: f64) {
        self.sink.set_volume(volume as f32);
        self.send_signal(CurrentSongMsg::VolumeChangedExternal(volume));
    }

    #[zbus(property)]
    pub fn playback_status(&self) -> &str {
        if self.sink.empty() {
            "Stopped"
        } else {
            if self.sink.is_paused() {
                "Paused"
            } else {
                "Playing"
            }
        }
    }

    #[zbus(property)]
    pub fn rate(&self) -> f64 {
        self.sink.speed() as f64
    }

    #[zbus(property)]
    pub async fn set_rate(&self, rate: f64) {
        let rate = if rate > MAX_PLAYBACK_RATE {
            MAX_PLAYBACK_RATE
        } else if rate < MIN_PLAYBACK_RATE {
            MIN_PLAYBACK_RATE
        } else {
            rate
        };
        if rate == 0.0 {
            self.pause().await;
        } else {
            self.sink.set_speed(rate as f32);
            self.send_signal(CurrentSongMsg::RateChange(rate));
        }
    }

    #[zbus(property)]
    pub fn position(&self) -> i64 {
        (self.sink.get_pos().as_micros() as f64 * self.sink.speed() as f64) as i64
    }

    #[zbus(property)]
    pub fn maximum_rate(&self) -> f64 {
        MAX_PLAYBACK_RATE
    }

    #[zbus(property)]
    pub fn minimum_rate(&self) -> f64 {
        MIN_PLAYBACK_RATE
    }

    #[zbus(property)]
    pub async fn shuffle(&self) -> bool {
        let track_list = self.track_list.read().await;
        track_list.shuffled
    }

    #[zbus(property)]
    pub async fn set_shuffle(&self, shuffle: bool) {
        let mut track_list = self.track_list.write().await;
        track_list.shuffled = shuffle;
    }

    #[zbus(property)]
    pub fn can_go_next(&self) -> bool {
        true
    }

    #[zbus(property)]
    pub fn can_go_previous(&self) -> bool {
        true
    }

    #[zbus(property)]
    pub fn can_play(&self) -> bool {
        true
    }

    #[zbus(property)]
    pub fn can_pause(&self) -> bool {
        true
    }

    #[zbus(property)]
    pub fn can_seek(&self) -> bool {
        false // TODO: Implement
    }

    #[zbus(property)]
    pub fn can_control(&self) -> bool {
        true
    }
}


