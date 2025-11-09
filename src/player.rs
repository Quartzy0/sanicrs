use std::cell::{Cell, RefCell};
use crate::opensonic::cache::SongCache;
use crate::opensonic::client::{OpenSubsonicClient};
use crate::opensonic::types::{InvalidResponseError, Song};
use crate::ui::track_list::MoveDirection;
use crate::PlayerCommand;
use async_channel::Sender;
use mpris_server::{LoopStatus, TrackId};
use rand::prelude::SliceRandom;
use rand::Rng;
use relm4::gtk::gio::prelude::SettingsExt;
use relm4::gtk::gio::Settings;
use std::error::Error;
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;
use gstreamer::glib::clone;
use gstreamer::prelude::{Cast, ElementExt, ElementExtManual, GstBinExt, ObjectExt, PadExt};
use gstreamer_play::PlayState;
use uuid::Uuid;

pub const MAX_PLAYBACK_RATE: f64 = 2.0;
pub const MIN_PLAYBACK_RATE: f64 = 0.25;

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
    client: &'static OpenSubsonicClient,
    // sink: Sink,
    track_list: RefCell<TrackList>,

    gst_player: gstreamer_play::Play,
    rg_filter_bin: gstreamer::Element,
    rg_volume: gstreamer::Element,
    play_state: Cell<PlayState>,

    settings: RefCell<PlayerSettings>,
}

impl PlayerInfo {
    pub fn new(
        client: &'static OpenSubsonicClient,
        track_list: TrackList,
        cmd_channel: Arc<Sender<PlayerCommand>>,
    ) -> Result<Self, Box<dyn Error>> {
        // GStreamer initialization code adapted from Amberol (https://gitlab.gnome.org/World/amberol/-/blob/main/src/audio/gst_backend.rs)
        gstreamer::init()?;

        let gst_player = gstreamer_play::Play::default();
        gst_player.set_video_track_enabled(false);

        let mut config = gst_player.config();
        config.set_position_update_interval(250);
        gst_player.set_config(config).unwrap();

        gst_player.message_bus().set_sync_handler(clone!(
            move |_bus, msg| {
                let Ok(play_msg) = gstreamer_play::PlayMessage::parse(&msg) else {
                    return gstreamer::BusSyncReply::Drop;
                };

                match play_msg {
                    gstreamer_play::PlayMessage::Error(error) => {
                        let err_str = format!("{:?}", error);
                        eprintln!("GStreamer error: {}", err_str);
                        if let Err(_) = cmd_channel.send_blocking(PlayerCommand::Error("Error from GStreamer".to_string(), err_str)) {
                            eprintln!("Error sending error string to main");
                        }
                    }
                    gstreamer_play::PlayMessage::Warning(warning) => {
                        eprintln!("GStreamer warning: {:?}", warning);
                    }
                    gstreamer_play::PlayMessage::EndOfStream(_) => {
                        if let Err(e) = cmd_channel.send_blocking(PlayerCommand::TrackOver) {
                            eprintln!("Failed to send TrackOver: {e}");
                        }
                    }
                    gstreamer_play::PlayMessage::PositionUpdated(pos) => {
                        if let Some(position) = pos.position() {
                            if let Err(e) = cmd_channel.send_blocking(PlayerCommand::PositionUpdate(position.seconds_f64())) {
                                eprintln!("Failed to send PositionUpdate: {e}");
                            }
                        }
                    },
                    gstreamer_play::PlayMessage::SeekDone(sd) => {
                        if let Some(position) = sd.position() {
                            if let Err(e) = cmd_channel.send_blocking(PlayerCommand::PositionUpdate(position.seconds_f64())) {
                                eprintln!("Failed to send PositionUpdate: {e}");
                            }
                        }
                    }
                    gstreamer_play::PlayMessage::StateChanged(state) => {
                        if let Err(e) = cmd_channel.send_blocking(PlayerCommand::PlayStateUpdate(state.state())) {
                            eprintln!("Failed to send PlayStateUpdate: {e}");
                        }
                    }
                    _ => {}
                }

                gstreamer::BusSyncReply::Drop
            }
        ));

        let rg_volume = gstreamer::ElementFactory::make_with_name("rgvolume", Some("rg volume"))?;
        let rg_limiter = gstreamer::ElementFactory::make_with_name("rglimiter", Some("rg limiter"))?;

        let filter_bin = gstreamer::Bin::builder().name("filter bin").build();
        filter_bin.add(&rg_volume)?;
        filter_bin.add(&rg_limiter)?;
        rg_volume.link(&rg_limiter)?;

        let pad_src = rg_limiter.static_pad("src").unwrap();
        pad_src.set_active(true).unwrap();
        let ghost_src = gstreamer::GhostPad::with_target(&pad_src)?;
        filter_bin.add_pad(&ghost_src)?;

        let pad_sink = rg_volume.static_pad("sink").unwrap();
        pad_sink.set_active(true).unwrap();
        let ghost_sink = gstreamer::GhostPad::with_target(&pad_sink)?;
        filter_bin.add_pad(&ghost_sink)?;


        Ok(PlayerInfo {
            client,
            track_list: RefCell::new(track_list),
            gst_player,
            rg_filter_bin: filter_bin.upcast(),
            rg_volume,
            play_state: Cell::new(PlayState::Stopped),
            settings: RefCell::default()
        })
    }

    pub fn set_playstate(&self, new_state: PlayState) {
        self.play_state.set(new_state);
    }

    pub fn load_rg_from_settings(&self) {
        let identity = gstreamer::ElementFactory::make_with_name("identity", None).unwrap();

        let (filter, album_mode) = match self.settings.borrow().replay_gain_mode {
            ReplayGainMode::Album => (self.rg_filter_bin.as_ref(), true),
            ReplayGainMode::Track => (self.rg_filter_bin.as_ref(), false),
            ReplayGainMode::None => (&identity, true),
        };

        self.rg_volume.set_property("album-mode", album_mode);
        self.gst_player.pipeline().set_property("audio-filter", filter);
    }


    pub fn track_list(&self) -> &RefCell<TrackList> {
        &self.track_list
    }

    pub fn load_settings(&self, settings: &Settings) -> Result<(), Box<dyn Error>>{
        {
            let mut s = self.settings.borrow_mut();
            s.load_settings(settings)?;
        }
        self.set_set_volume();
        self.load_rg_from_settings();
        Ok(())
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
        let c = guard.current().and_then(|s| Some(s.uuid.clone()));
        let e = guard.remove_song(index);
        if let Some(c) = c && c == e.uuid { // Check if previously playing entry is the same as the removed one
            drop(guard);
            self.start_current().await?;
        }
        Ok(e)
    }

    pub async fn play(&self) {
        match self.play_state.get() {
            PlayState::Stopped => {
                self.start_current().await.expect("Error playing");
            }
            PlayState::Paused => {
                self.gst_player.play();
            }
            _ => {}
        }
    }

    pub fn pause(&self) {
        self.gst_player.pause();
    }

    pub async fn playpause(&self) {
        match self.play_state.get() {
            PlayState::Stopped => {
                self.start_current().await.expect("Error playing");
            }
            PlayState::Paused => {
                self.gst_player.play();
            }
            _ => {
                self.gst_player.pause();
            }
        }
    }

    pub async fn start_current(&self) -> Result<Option<SongEntry>, Box<dyn Error>> {
        let track_list = self.track_list.borrow();
        let song = match track_list.current() {
            None => {return Ok(None)}
            Some(s) => s
        };

        println!("Playing: {}", song.song.title);
        self.gst_player.set_uri(Some(&self.client.stream_get_url(&song.song.id, None, None, None, None, Some(true), None)));
        self.gst_player.play();

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
        self.gst_player.stop();
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
            if let Some(duration) = song.song.duration && position > duration {
                return Ok(());
            }
        }
        self.gst_player.seek(gstreamer::ClockTime::from_seconds_f64(position.as_secs_f64()));
        Ok(())
    }

    fn set_set_volume(&self) {
        let v = self.settings.borrow().volume;
        self.gst_player.set_volume(v);
    }

    pub fn volume(&self) -> f64 {
        self.settings.borrow().volume
    }

    pub fn set_volume(&self, volume: f64) {
        {
            let mut settings = self.settings.borrow_mut();
            settings.volume = volume;
        }
        self.gst_player.set_volume(volume);
    }

    pub fn playback_status(&self) -> PlayState {
        self.play_state.get()
    }

    pub fn rate(&self) -> f64 {
        self.gst_player.rate()
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
            self.gst_player.set_rate(rate);
        }
    }

    pub fn position(&self) -> i64 {
        self.gst_player.position().and_then(|c| Some(c.useconds() as i64)).unwrap_or(0)
    }

    pub fn shuffled(&self) -> bool {
        self.track_list.borrow().shuffled
    }

    pub fn set_shuffled(&self, shuffled: bool) {
        self.track_list.borrow_mut().set_shuffle(shuffled);
    }
}

#[derive(Clone, Debug)]
pub struct SongEntry {
    pub uuid: Uuid,
    pub song: Rc<Song>
}

impl SongEntry {
    pub fn dbus_path(&self) -> String {
        format!("/me/quartzy/sanicrs/song/{}", self.uuid.as_simple().to_string())
    }

    pub fn dbus_obj<'a>(&self) -> TrackId {
        TrackId::try_from(self.dbus_path().clone()).expect("Error when making object path")
    }
}

impl From<(Uuid, Rc<Song>)> for SongEntry {
    fn from(value: (Uuid, Rc<Song>)) -> Self {
        Self {
            uuid: value.0,
            song: value.1
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
