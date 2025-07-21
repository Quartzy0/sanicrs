use crate::opensonic::client::OpenSubsonicClient;
use crate::player::{LoopStatus, PlaybackStatus, PlayerInfo, SongEntry, TrackList, MAX_PLAYBACK_RATE, MIN_PLAYBACK_RATE};
use crate::PlayerCommand;
use std::collections::HashMap;
use std::ops::{Add, Deref};
use std::sync::Arc;
use async_channel::Sender;
use std::time::Duration;
use readlock_tokio::SharedReadLock;
use tokio::sync::RwLock;
use zbus::interface;
use zbus::object_server::SignalEmitter;
use zvariant::{Array, ObjectPath, Str, Value};

pub struct MprisPlayer {
    pub client: Arc<OpenSubsonicClient>,
    pub track_list: Arc<RwLock<TrackList>>,
    pub cmd_channel: Arc<Sender<PlayerCommand>>,
    pub player_ref: SharedReadLock<PlayerInfo>,
}

pub async fn get_song_metadata<'a>(song: Option<&SongEntry>, client: Arc<OpenSubsonicClient>) -> HashMap<&'a str, Value<'a>> {
    let mut map: HashMap<&str, Value> = HashMap::new();
    if song.is_none() {
        map.insert("mpris:trackid", Value::ObjectPath(ObjectPath::from_static_str_unchecked("/org/mpris/MediaPlayer2/TrackList/NoTrack")));
        return map;
    }
    let song = song.unwrap();
    map.insert("mpris:trackid", Value::ObjectPath(song.dbus_obj()));
    let song = &song.1;
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

    map
}

#[interface(name = "org.mpris.MediaPlayer2.Player")]
impl MprisPlayer {

    #[zbus(signal)]
    pub async fn seeked(emitter: &SignalEmitter<'_>, position: i64) -> Result<(), zbus::Error>;
    
    #[zbus(property)]
    pub async fn loop_status(&self) -> String {
        self.track_list.read().await.loop_status.clone().into()
    }

    #[zbus(property)]
    pub async fn set_loop_status(&self, loop_status: String) -> Result<(), zbus::Error> {
        let loop_status = LoopStatus::try_from(loop_status)?;
        self.cmd_channel.send(PlayerCommand::SetLoopStatus(loop_status)).await.expect("Error sending message to player");
        Ok(())
    }

    pub async fn open_uri(&self, uri: &str) -> Result<(), zbus::fdo::Error> {
        let mut track_list = self.track_list.write().await;
        let err = track_list.add_song_from_uri(uri, self.client.clone(), None).await;
        match err {
            None => Ok(()),
            Some(err) => Err(zbus::fdo::Error::Failed(format!("Error when adding song: {}", err)))
        }
    }

    pub async fn play(&self) {
        self.cmd_channel.send(PlayerCommand::Play).await.expect("Error sending message to player")
    }

    pub async fn pause(&self) {
        self.cmd_channel.send(PlayerCommand::Pause).await.expect("Error sending message to player")
    }

    pub async fn play_pause(&self) {
        self.cmd_channel.send(PlayerCommand::PlayPause).await.expect("Error sending message to player")
    }

    pub async fn next(&self) {
        self.cmd_channel.send(PlayerCommand::Next).await.expect("Error sending message to player")
    }

    pub async fn previous(&self) {
        self.cmd_channel.send(PlayerCommand::Previous).await.expect("Error sending message to player")
    }

    pub async fn stop(&self) {
        self.cmd_channel.send(PlayerCommand::Stop).await.expect("Error sending message to player")
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
            if let Some(duration) = song.1.duration && position > duration {
                return Ok(());
            }
        }
        self.cmd_channel.send(PlayerCommand::SetPosition(position)).await.expect("Error sending message to player");
        Ok(())
    }

    pub async fn seek(&mut self, offset: i64) -> Result<(), zbus::fdo::Error> {
        let current_position = Duration::from_micros(PlayerInfo::position(&self.player_ref.lock().await.deref()) as u64);
        let new_positon = if offset > 0 {
            current_position.add(Duration::from_micros(offset as u64))
        } else {
            current_position.checked_sub(Duration::from_micros((-offset) as u64)).unwrap_or(Duration::from_secs(0))
        };
        let mut seek_next = false;
        {
            let track_list = self.track_list.read().await;
            let song = match track_list.current() {
                Some(t) => &t.1,
                None => return Ok(())
            };
            let song_duration = song.duration;
            if let Some(song_duration) = song_duration {
                seek_next = song_duration <= new_positon;
            }
        }
        if seek_next {
            self.cmd_channel.send(PlayerCommand::Next).await.expect("Error sending message to player");
        } else {
            self.cmd_channel.send(PlayerCommand::SetPosition(new_positon)).await.expect("Error sending message to player");
        }
        Ok(())
    }

    #[zbus(property)]
    pub async fn metadata(&self) -> Result<HashMap<&str, Value>, zbus::fdo::Error> {
        let track_list = self.track_list.read().await;
        Ok(get_song_metadata(track_list.current(), self.client.clone()).await)
    }

    #[zbus(property)]
    pub async fn volume(&self) -> f64 {
        self.player_ref.lock().await.volume()
    }

    #[zbus(property)]
    pub async fn set_volume(&self, volume: f64) {
        self.cmd_channel.send(PlayerCommand::SetVolume(volume)).await.expect("Error sending message to player");
    }

    #[zbus(property)]
    pub async fn playback_status(&self) -> &str {
        match self.player_ref.lock().await.playback_status() {
            PlaybackStatus::Playing => "Playing",
            PlaybackStatus::Paused => "Paused",
            PlaybackStatus::Stopped => "Stopped"
        }
    }

    #[zbus(property)]
    pub async fn rate(&self) -> f64 {
        self.player_ref.lock().await.rate()
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
            self.cmd_channel.send(PlayerCommand::SetRate(rate)).await.expect("Error sending message to player");
        }
    }

    #[zbus(property)]
    pub async fn position(&self) -> i64 {
        PlayerInfo::position(&self.player_ref.lock().await.deref())
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
        true
    }

    #[zbus(property)]
    pub fn can_control(&self) -> bool {
        true
    }
}


