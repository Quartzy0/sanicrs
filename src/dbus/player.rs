use std::cell::RefCell;
use std::error::Error;
use crate::opensonic::client::OpenSubsonicClient;
use crate::player::{PlayerInfo, SongEntry, TrackList};
use crate::PlayerCommand;
use std::ops::Add;
use std::rc::Rc;
use std::sync::Arc;
use async_channel::Sender;
use std::time::Duration;
use mpris_server::{LocalPlayerInterface, Metadata, Time, TrackId, LoopStatus, PlaybackStatus, Property, LocalServer, TrackListSignal, Signal, zbus::fdo};
use relm4::adw::gio::Settings;
use relm4::adw::prelude::SettingsExt;
use relm4::AsyncComponentSender;
use crate::opensonic::cache::{AlbumCache, SongCache};
use crate::ui::app::{AppMsg, Model};
use crate::ui::bottom_bar::BottomBar;
use crate::ui::current_song::{CurrentSong, CurrentSongMsg};
use crate::ui::track_list::{TrackListMsg, TrackListWidget};

pub struct MprisPlayer {
    pub client: Rc<OpenSubsonicClient>,
    pub cmd_channel: Arc<Sender<PlayerCommand>>,
    pub player_ref: PlayerInfo,

    pub app_sender: RefCell<Option<AsyncComponentSender<Model>>>,
    pub tl_sender: RefCell<Option<AsyncComponentSender<TrackListWidget>>>,
    pub cs_sender: RefCell<Option<AsyncComponentSender<CurrentSong>>>,
    pub bb_sender: RefCell<Option<AsyncComponentSender<BottomBar>>>,
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
            let r = sender.input_sender().send(msg.clone());
            if r.is_err() {
                self.app_sender.replace(None);
            }
        }
        if let Some(sender) = self.bb_sender.borrow().as_ref() {
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

    pub fn properties_changed(&self, properties: impl IntoIterator<Item = Property>){
        if let Some(server) = self.server.borrow().clone() {
            let properties: Vec<Property> = properties.into_iter().collect();
            relm4::spawn_local(async move {
                let _ = server.properties_changed(properties).await;
            });
        }
    }

    pub fn send_error(&self, error: Box<dyn Error>) {
        self.send_app_msg(AppMsg::ShowError(format!("{}", error), format!("{:?}", error)));
    }

    pub fn send_res(&self, result: Result<(), Box<dyn Error>>) {
        if let Err(error) = result {
            self.send_app_msg(AppMsg::ShowError(format!("{}", error), format!("{:?}", error)));
        }
    }

    pub fn send_res_fdo(&self, result: fdo::Result<()>) {
        if let Err(error) = result {
            self.send_app_msg(AppMsg::ShowError(format!("{}", error), format!("{:?}", error)));
        }
    }

    pub fn track_list_emit(&self, signal: TrackListSignal) {
        if let Some(server) = self.server.borrow().clone() {
            relm4::spawn_local(async move {
                let _ = server.track_list_emit(signal).await;
            });
        }
    }

    pub fn emit(&self, signal: Signal) {
        if let Some(server) = self.server.borrow().clone() {
            relm4::spawn_local(async move {
                let _ = server.emit(signal).await;
            });
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

    pub fn set_position(&self, p: Duration) -> Result<(), Box<dyn Error>> {
        self.player_ref.set_position(p)?;
        self.send_cs_msg(CurrentSongMsg::ProgressUpdateSync(Some(p.as_secs_f64())));
        self.emit(Signal::Seeked {
            position: Time::from_micros(p.as_micros() as i64),
        });
        Ok(())
    }

    pub fn reload_settings(&self) -> Result<(), Box<dyn Error>> {
        self.player_ref.load_settings(&self.settings)
    }

    pub async fn current_song_metadata(&self) -> Metadata {
        let guard = self.track_list().borrow();
        get_song_metadata(guard.current(), self.client.clone()).await
    }

    pub fn track_list(&self) -> &RefCell<TrackList> {
        self.player_ref.track_list()
    }

    pub fn info(&self) -> &PlayerInfo {
        &self.player_ref
    }
}


// Non-async version of DBus function which don't need to be async
impl MprisPlayer {
    pub fn pause(&self){
        self.player_ref.pause();
        self.send_cs_msg(CurrentSongMsg::Update);
        self.properties_changed([
            Property::PlaybackStatus(self.player_ref.playback_status())
        ]);
    }

    pub async fn stop(&self) {
        self.player_ref.stop();
        self.send_cs_msg(CurrentSongMsg::SongUpdate(None));
        self.send_tl_msg(TrackListMsg::ReloadList);
        self.properties_changed([
            Property::PlaybackStatus(self.player_ref.playback_status()),
            Property::Metadata(self.current_song_metadata().await),
        ]);
    }

    pub fn set_loop_status(&self, loop_status: LoopStatus){
        self.player_ref.set_loop_status(loop_status);
        self.send_cs_msg(CurrentSongMsg::Update);
        self.properties_changed([
            Property::LoopStatus(loop_status)
        ]);
    }

    // Changing the rate is currently unsupported because of issues in Rodio's API
    // https://github.com/RustAudio/rodio/issues/638
    // https://github.com/RustAudio/rodio/pull/768 (maybe will fix this)
    /*pub fn set_rate(&self, rate: f64)  {
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
            self.player_ref.set_rate(rate);
            self.send_cs_msg(CurrentSongMsg::RateChange(rate));
            self.properties_changed([
                Property::Rate(rate)
            ]);
        }
    }*/

    pub fn set_shuffle(&self, shuffle: bool) {
        self.player_ref.set_shuffled(shuffle);
        self.send_cs_msg(CurrentSongMsg::Update);
        self.properties_changed([
            Property::Shuffle(shuffle)
        ]);
    }

    pub fn set_volume(&self, v: f64) {
        self.player_ref.set_volume(v);
        self.send_res_fdo(self.settings.set_double("volume", v).map_err(|e| fdo::Error::Failed(e.to_string())));
        self.send_cs_msg(CurrentSongMsg::Update);
        self.properties_changed([
            Property::Volume(v)
        ]);
    }
}

impl LocalPlayerInterface for MprisPlayer {
    async fn next(&self) -> fdo::Result<()> {
        let s = self.player_ref.next().await;
        if s.is_none() { // Track list over
            self.pause();
        }
        self.send_cs_msg(CurrentSongMsg::SongUpdate(s));
        self.send_tl_msg(TrackListMsg::TrackChanged(None));
        self.properties_changed([
            Property::Metadata(self.current_song_metadata().await),
            Property::PlaybackStatus(self.player_ref.playback_status())
        ]);
        Ok(())
    }

    async fn previous(&self) -> fdo::Result<()> {
        let s = self.player_ref.previous().await;
        self.send_cs_msg(CurrentSongMsg::SongUpdate(s));
        self.send_tl_msg(TrackListMsg::TrackChanged(None));
        self.properties_changed([
            Property::Metadata(self.current_song_metadata().await),
            Property::PlaybackStatus(self.player_ref.playback_status())
        ]);
        Ok(())
    }

    async fn pause(&self) -> fdo::Result<()> {
        self.pause();
        Ok(())
    }

    async fn play_pause(&self) -> fdo::Result<()> {
        self.player_ref.playpause().await;
        self.send_cs_msg(CurrentSongMsg::Update);
        self.properties_changed([
            Property::PlaybackStatus(self.player_ref.playback_status())
        ]);
        Ok(())
    }

    async fn stop(&self) -> fdo::Result<()> {
        self.stop().await;
        Ok(())
    }

    async fn play(&self) -> fdo::Result<()> {
        self.player_ref.play().await;
        self.send_cs_msg(CurrentSongMsg::Update);
        self.properties_changed([
            Property::PlaybackStatus(self.player_ref.playback_status())
        ]);
        Ok(())
    }

    async fn seek(&self, offset: Time) -> Result<(), fdo::Error> {
        let current_position = Duration::from_micros(PlayerInfo::position(&self.player_ref) as u64);
        let new_positon = if offset.is_positive() {
            current_position.add(Duration::from_micros(offset.as_micros() as u64))
        } else {
            current_position.checked_sub(Duration::from_micros((-offset.as_micros()) as u64)).unwrap_or(Duration::from_secs(0))
        };
        let mut seek_next = false;
        {
            let track_list = self.track_list().borrow();
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
            self.set_position(new_positon).map_err(|e| fdo::Error::Failed(e.to_string()))?;
        }
        Ok(())
    }

    async fn set_position(&self, track_id: TrackId, position: Time) -> Result<(), fdo::Error> {
        if position.is_negative(){
            return Ok(());
        }
        let position = Duration::from_micros(position.as_micros() as u64);
        {
            let track_list = self.track_list().borrow();
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
        self.set_position(position).map_err(|e| fdo::Error::Failed(e.to_string()))?;
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
        Ok(self.player_ref.loop_status())
    }

    async fn set_loop_status(&self, loop_status: LoopStatus) -> Result<(), zbus::Error> {
        self.set_loop_status(loop_status);
        Ok(())
    }

    async fn rate(&self) -> fdo::Result<f64> {
        Ok(self.player_ref.rate())
    }

    async fn set_rate(&self, _rate: f64) -> zbus::Result<()> {
        Err(zbus::Error::Unsupported)
        /*self.set_rate(rate);
        Ok(())*/
    }

    async fn shuffle(&self) -> fdo::Result<bool> {
        Ok(self.player_ref.shuffled())
    }

    async fn set_shuffle(&self, shuffle: bool) -> zbus::Result<()> {
        self.set_shuffle(shuffle);
        Ok(())
    }

    async fn metadata(&self) -> Result<Metadata, fdo::Error> {
        let track_list = self.track_list().borrow();
        Ok(get_song_metadata(track_list.current(), self.client.clone()).await)
    }

    async fn volume(&self) -> fdo::Result<f64> {
        Ok(self.player_ref.volume())
    }

    async fn set_volume(&self, v: f64) -> zbus::Result<()> {
        self.set_volume(v);
        Ok(())
    }

    async fn position(&self) -> fdo::Result<Time> {
        Ok(Time::from_micros(PlayerInfo::position(&self.player_ref)))
    }

    async fn minimum_rate(&self) -> fdo::Result<f64> {
        Ok(1.0)
    }

    async fn maximum_rate(&self) -> fdo::Result<f64> {
        Ok(1.0)
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
