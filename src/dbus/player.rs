use std::cell::RefCell;
use std::error::Error;
use crate::opensonic::client::OpenSubsonicClient;
use crate::player::{PlayerInfo, SongEntry, TrackList, MAX_PLAYBACK_RATE, MIN_PLAYBACK_RATE};
use crate::PlayerCommand;
use std::ops::{Add, Deref};
use std::rc::Rc;
use std::sync::Arc;
use async_channel::Sender;
use std::time::Duration;
use mpris_server::{LocalPlayerInterface, Metadata, Time, TrackId, LoopStatus, PlaybackStatus, Property, LocalServer, TrackListSignal, Signal, zbus::fdo};
use relm4::adw::gio::Settings;
use relm4::adw::prelude::SettingsExt;
use relm4::AsyncComponentSender;
use tokio::sync::RwLock;
use crate::opensonic::cache::{AlbumCache, SongCache};
use crate::ui::app::{AppMsg, Model};
use crate::ui::current_song::{CurrentSong, CurrentSongMsg};
use crate::ui::track_list::{TrackListMsg, TrackListWidget};

pub struct MprisPlayer {
    pub client: Rc<OpenSubsonicClient>,
    pub track_list: Arc<RwLock<TrackList>>,
    pub cmd_channel: Arc<Sender<PlayerCommand>>,
    pub player_ref: Rc<PlayerInfo>,

    pub app_sender: RefCell<Option<AsyncComponentSender<Model>>>,
    pub tl_sender: RefCell<Option<AsyncComponentSender<TrackListWidget>>>,
    pub cs_sender: RefCell<Option<AsyncComponentSender<CurrentSong>>>,
    pub server: RefCell<Option<Rc<LocalServer<MprisPlayer>>>>,
    
    pub song_cache: SongCache,
    pub album_cache: AlbumCache,
    pub settings: Settings,
}

pub async fn get_song_metadata<'a>(song: Option<&SongEntry>, client: Rc<OpenSubsonicClient>) -> Metadata {
    let mut map: Metadata = Metadata::new();
    if song.is_none() {
        map.set_trackid(Some(TrackId::NO_TRACK));
        return map;
    }
    let song = song.unwrap();
    map.set_trackid(Some(song.dbus_obj()));
    let song = &song.1;
    map.set_title(Some(song.title.clone()));
    if let Some(cover_art) = &song.cover_art{
        let url = client.get_cover_image_url(cover_art.as_str()).await;
        if let Some(url) = url {
            map.set_art_url(Some(url));
        }
    }
    map.set_length(song.duration.and_then(|d| Some(Time::from_millis(d.as_millis() as i64))));
    map.set_album(song.album.as_ref());
    if let Some(artists) = &song.artists {
        let a: Vec<String> = artists.iter().map(|x| x.name.clone()).collect();
        map.set_artist(Some(a));
    }
    if let Some(artists) = &song.album_artists {
        let al: Vec<String> = artists.iter().map(|x| x.name.clone()).collect();
        map.set_album_artist(Some(al));
    }
    if let Some(artists) = &song.genres {
        let g: Vec<String> = artists.iter().map(|x| x.name.clone()).collect();
        map.set_genre(Some(g));
    }
    map.set_comment(song.comment.as_ref().and_then(|c| Some(vec![c.clone()])));
    map.set_composer(song.display_composer.as_ref().and_then(|c| Some(vec![c.clone()])));
    map.set_last_used(song.played.as_ref());
    map.set_use_count(song.play_count.and_then(|n| Some(n as i32)));
    map.set_track_number(song.track);
    map.set_user_rating(song.user_rating.and_then(|r| Some(r as f64 / 5.0)));
    map.set_disc_number(song.disc_number.and_then(|d| Some(d as i32)));
    map.set_audio_bpm(song.bpm.and_then(|b| Some(b as i32)));

    map
}

impl MprisPlayer {
    pub fn send_app_msg(&self, msg: AppMsg) {
        if let Some(sender) = self.app_sender.borrow().as_ref() {
            let r = sender.input_sender().send(msg);
            if r.is_err() {
                self.app_sender.replace(None);
            }
        }
    }

    pub fn send_cs_msg(&self, msg: CurrentSongMsg) {
        if let Some(sender) = self.cs_sender.borrow().as_ref() {
            let r = sender.input_sender().send(msg);
            if r.is_err() {
                self.app_sender.replace(None);
            }
        }
    }

    pub fn send_tl_msg(&self, msg: TrackListMsg) {
        if let Some(sender) = self.tl_sender.borrow().as_ref() {
            let r = sender.input_sender().send(msg);
            if r.is_err() {
                self.app_sender.replace(None);
            }
        }
    }

    pub async fn properties_changed(&self, properties: impl IntoIterator<Item = Property>) -> zbus::Result<()>{
        if let Some(server) = self.server.borrow().as_ref() {
            server.properties_changed(properties).await
        } else {
            Ok(())
        }
    }
    
    pub async fn send_error(&self, error: Box<dyn Error>) {
        self.send_app_msg(AppMsg::ShowError(format!("{}", error), format!("{:?}", error)));
    }

    pub async fn send_res(&self, result: Result<(), Box<dyn Error>>) {
        if let Err(error) = result {
            self.send_app_msg(AppMsg::ShowError(format!("{}", error), format!("{:?}", error)));
        }
    }

    pub async fn send_res_fdo(&self, result: fdo::Result<()>) {
        if let Err(error) = result {
            self.send_app_msg(AppMsg::ShowError(format!("{}", error), format!("{:?}", error)));
        }
    }

    pub async fn track_list_emit(&self, signal: TrackListSignal) -> zbus::Result<()> {
        if let Some(server) = self.server.borrow().as_ref() {
            server.track_list_emit(signal).await
        } else {
            Ok(())
        }
    }
    
    pub async fn emit(&self, signal: Signal) -> zbus::Result<()> {
        if let Some(server) = self.server.borrow().as_ref() {
            server.emit(signal).await
        } else {
            Ok(())
        }
    }

    pub async fn track_list_replaced(&self, songs: &Vec<SongEntry>, current_i: Option<usize>) -> Result<(), zbus::Error> {
        if let Some(server) = self.server.borrow().as_ref() {
            server.track_list_emit(TrackListSignal::TrackListReplaced {
                tracks: songs.iter().map(|s| s.dbus_obj()).collect(),
                current_track: if let Some(i) = current_i && let Some(current) = songs.get(i) {
                    current.dbus_obj()
                } else {
                    TrackId::NO_TRACK
                }
            }
            ).await
        }else {
            Ok(())
        }
    }
    
    pub async fn set_position(&self, p: Duration) -> Result<(), Box<dyn Error>> {
        self.player_ref.set_position(p).await?;
        self.send_cs_msg(CurrentSongMsg::ProgressUpdateSync(Some(p.as_secs_f64())));
        self.emit(Signal::Seeked {
            position: Time::from_micros(p.as_micros() as i64),
        }).await?;
        Ok(())
    }
    
    pub async fn reload_settings(&self) -> Result<(), Box<dyn Error>> {
        self.player_ref.load_settings(&self.settings).await
    }

    pub async fn current_song_metadata(&self) -> Metadata {
        let guard = self.track_list.read().await;
        get_song_metadata(guard.current(), self.client.clone()).await
    }
}

impl LocalPlayerInterface for MprisPlayer {
    async fn next(&self) -> fdo::Result<()> {
        let s = self.player_ref.next().await;
        if s.is_none() { // Track list over
            self.pause().await?;
        }
        self.send_cs_msg(CurrentSongMsg::SongUpdate(s));
        self.send_tl_msg(TrackListMsg::TrackChanged(None));
        self.properties_changed([
            Property::Metadata(self.current_song_metadata().await),
            Property::PlaybackStatus(self.player_ref.playback_status())
        ]).await?;
        Ok(())
    }

    async fn previous(&self) -> fdo::Result<()> {
        let s = self.player_ref.previous().await;
        self.send_cs_msg(CurrentSongMsg::SongUpdate(s));
        self.send_tl_msg(TrackListMsg::TrackChanged(None));
        self.properties_changed([
            Property::Metadata(self.current_song_metadata().await),
            Property::PlaybackStatus(self.player_ref.playback_status())
        ]).await?;
        Ok(())
    }

    async fn pause(&self) -> fdo::Result<()> {
        self.player_ref.pause();
        self.send_cs_msg(CurrentSongMsg::PlaybackStateChange(self.player_ref.playback_status()));
        self.properties_changed([
            Property::PlaybackStatus(self.player_ref.playback_status())
        ]).await?;
        Ok(())
    }

    async fn play_pause(&self) -> fdo::Result<()> {
        self.player_ref.playpause().await;
        self.send_cs_msg(CurrentSongMsg::PlaybackStateChange(self.player_ref.playback_status()));
        self.properties_changed([
            Property::PlaybackStatus(self.player_ref.playback_status())
        ]).await?;
        Ok(())
    }

    async fn stop(&self) -> fdo::Result<()> {
        self.player_ref.stop().await;
        self.send_cs_msg(CurrentSongMsg::PlaybackStateChange(self.player_ref.playback_status()));
        self.properties_changed([
            Property::PlaybackStatus(self.player_ref.playback_status())
        ]).await?;
        Ok(())
    }

    async fn play(&self) -> fdo::Result<()> {
        self.player_ref.play().await;
        self.send_cs_msg(CurrentSongMsg::PlaybackStateChange(self.player_ref.playback_status()));
        self.properties_changed([
            Property::PlaybackStatus(self.player_ref.playback_status())
        ]).await?;
        Ok(())
    }

    async fn seek(&self, offset: Time) -> Result<(), fdo::Error> {
        let current_position = Duration::from_micros(PlayerInfo::position(&self.player_ref.deref()) as u64);
        let new_positon = if offset.is_positive() {
            current_position.add(Duration::from_micros(offset.as_micros() as u64))
        } else {
            current_position.checked_sub(Duration::from_micros((-offset.as_micros()) as u64)).unwrap_or(Duration::from_secs(0))
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
            self.next().await?;
        } else {
            self.set_position(new_positon).await.map_err(|e| fdo::Error::Failed(e.to_string()))?;
        }
        Ok(())
    }

    async fn set_position(&self, track_id: TrackId, position: Time) -> Result<(), fdo::Error> {
        if position.is_negative(){
            return Ok(());
        }
        let position = Duration::from_micros(position.as_micros() as u64);
        {
            let track_list = self.track_list.read().await;
            let song = match track_list.current() {
                Some(t) => t,
                None => return Ok(())
            };
            if song.dbus_path() != track_id.as_str() {
                return Ok(());
            }
            if let Some(duration) = song.1.duration && position > duration {
                return Ok(());
            }
        }
        self.set_position(position).await.map_err(|e| fdo::Error::Failed(e.to_string()))?;
        Ok(())
    }

    async fn open_uri(&self, uri: String) -> fdo::Result<()> {
        self.add_track_to_index(uri, None, false).await.map_err(|e| fdo::Error::Failed(e.to_string()))?;
        Ok(())
    }

    async fn playback_status(&self) -> fdo::Result<PlaybackStatus> {
        Ok(self.player_ref.playback_status())
    }

    async fn loop_status(&self) -> fdo::Result<LoopStatus> {
        Ok(self.track_list.read().await.loop_status)
    }

    async fn set_loop_status(&self, loop_status: LoopStatus) -> Result<(), zbus::Error> {
        {
            let mut guard = self.track_list.write().await;
            guard.loop_status = loop_status;
        }
        self.send_cs_msg(CurrentSongMsg::SetLoopStatus(loop_status));
        self.properties_changed([
            Property::LoopStatus(loop_status)
        ]).await?;
        Ok(())
    }

    async fn rate(&self) -> fdo::Result<f64> {
        Ok(self.player_ref.rate())
    }

    async fn set_rate(&self, rate: f64) -> zbus::Result<()> {
        let rate = if rate > MAX_PLAYBACK_RATE {
            MAX_PLAYBACK_RATE
        } else if rate < MIN_PLAYBACK_RATE {
            MIN_PLAYBACK_RATE
        } else {
            rate
        };
        if rate == 0.0 {
            self.pause().await?;
        } else {
            self.player_ref.set_rate(rate);
            self.send_cs_msg(CurrentSongMsg::RateChange(rate));
            self.properties_changed([
                Property::Rate(rate)
            ]).await?;
        }
        Ok(())
    }

    async fn shuffle(&self) -> fdo::Result<bool> {
        let track_list = self.track_list.read().await;
        Ok(track_list.is_suffled())
    }

    async fn set_shuffle(&self, shuffle: bool) -> zbus::Result<()> {
        {
            let mut guard = self.track_list.write().await;
            guard.set_shuffle(shuffle);
        }
        self.send_cs_msg(CurrentSongMsg::SetShuffle(shuffle));
        self.properties_changed([
            Property::Shuffle(shuffle)
        ]).await?;
        Ok(())
    }

    async fn metadata(&self) -> Result<Metadata, fdo::Error> {
        let track_list = self.track_list.read().await;
        Ok(get_song_metadata(track_list.current(), self.client.clone()).await)
    }

    async fn volume(&self) -> fdo::Result<f64> {
        Ok(self.player_ref.volume().await)
    }

    async fn set_volume(&self, v: f64) -> zbus::Result<()> {
        self.player_ref.set_volume(v).await;
        self.settings.set_double("volume", v).map_err(|e| fdo::Error::Failed(e.to_string()))?;
        self.send_cs_msg(CurrentSongMsg::VolumeChangedExternal(v));
        self.properties_changed([
            Property::Volume(v)
        ]).await?;
        Ok(())
    }

    async fn position(&self) -> fdo::Result<Time> {
        Ok(Time::from_micros(PlayerInfo::position(&self.player_ref.deref())))
    }

    async fn minimum_rate(&self) -> fdo::Result<f64> {
        Ok(MIN_PLAYBACK_RATE)
    }

    async fn maximum_rate(&self) -> fdo::Result<f64> {
        Ok(MAX_PLAYBACK_RATE)
    }

    async fn can_go_next(&self) -> fdo::Result<bool> {
        Ok(true)
    }

    async fn can_go_previous(&self) -> fdo::Result<bool> {
        Ok(true)
    }

    async fn can_play(&self) -> fdo::Result<bool> {
        Ok(true)
    }

    async fn can_pause(&self) -> fdo::Result<bool> {
        Ok(true)
    }

    async fn can_seek(&self) -> fdo::Result<bool> {
        Ok(true)
    }

    async fn can_control(&self) -> fdo::Result<bool> {
        Ok(true)
    }
}
