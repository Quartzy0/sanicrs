use crate::icon_names;
use crate::dbus::player::MprisPlayer;
use crate::opensonic::client::OpenSubsonicClient;
use crate::opensonic::types::Song;
use crate::player::TrackList;
use crate::ui::cover_picture;
use crate::ui::cover_picture::CoverSize;
use relm4::adw::glib;
use relm4::adw::glib::ControlFlow;
use relm4::adw::gtk::glib::Propagation;
use relm4::adw::gtk::prelude::OrientableExt;
use relm4::adw::gtk::{Align, Orientation};
use relm4::adw::prelude::*;
use relm4::component::{AsyncComponent, AsyncComponentParts, AsyncComponentSender};
use relm4::prelude::*;
use std::ops::Deref;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use zbus::object_server::InterfaceRef;
use crate::ui::app::Init;

#[derive(Debug)]
pub struct SongInfo {
    pub id: String,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub cover_art_id: Option<String>,
    pub duration: Duration,
}

impl SongInfo {
    pub fn dbus_path(&self) -> String {
        format!("/me/quartzy/sanicrs/track/{}", self.id.replace("-", "/"))
    }
}

impl From<&Song> for SongInfo {
    fn from(value: &Song) -> Self {
        SongInfo {
            id: value.id.clone(),
            title: value.title.clone(),
            artist: value
                .display_artists
                .clone()
                .unwrap_or(value.artist.clone().unwrap_or("Unknown Artist".to_string())),
            album: value.album.clone().unwrap_or("Unknown Album".to_string()),
            cover_art_id: value.cover_art.clone(),
            duration: value.duration.unwrap(),
        }
    }
}

impl Default for SongInfo {
    fn default() -> Self {
        SongInfo {
            id: "".to_string(),
            album: "".to_string(),
            artist: "".to_string(),
            title: "".to_string(),
            cover_art_id: None,
            duration: Duration::from_secs(0),
        }
    }
}

pub struct CurrentSong {
    player_reference: InterfaceRef<MprisPlayer>,
    track_list: Arc<RwLock<TrackList>>,
    sender: AsyncComponentSender<Self>,
    client: Arc<OpenSubsonicClient>,

    // UI data
    song_info: SongInfo,
    playback_state_icon: &'static str,
    playback_position: f64,
    playback_rate: f64,
}

#[derive(Debug)]
pub enum CurrentSongMsg {
    PlayPause,
    Start,
    Next,
    Previous,
    SongUpdate(SongInfo),
    PlaybackStateChange(String),
    VolumeChanged(f64),
    VolumeChangedExternal(f64),
    ProgressUpdate,
    ProgressUpdateSync(Option<f64>),
    RateChange(f64),
    RateChangeUI(f64),
    Seek(f64),
}

#[relm4::component(pub async)]
impl AsyncComponent for CurrentSong {
    type CommandOutput = ();
    type Input = CurrentSongMsg;
    type Output = ();
    type Init = Init;

    view! {
        gtk::Box {
            set_orientation: Orientation::Vertical,
            set_halign: Align::Center,
            set_valign: Align::Center,
            set_spacing: 5,

            #[name = "cover_image"]
            cover_picture::CoverPicture {
                set_cover_size: CoverSize::Huge,
            },
            gtk::Label {
                #[watch]
                set_label: &model.song_info.title,
                add_css_class: "bold",
            },
            gtk::Label {
                #[watch]
                set_label: &model.song_info.artist,
            },
            gtk::Label {
                #[watch]
                set_label: &model.song_info.album,
                add_css_class: "italic",
            },

            gtk::Box {
                set_orientation: Orientation::Horizontal,
                set_halign: Align::Center,
                set_spacing: 10,

                gtk::Button {
                    set_icon_name: icon_names::PREVIOUS_REGULAR,
                    connect_clicked => CurrentSongMsg::Previous,
                    add_css_class: "track-action-btn"
                },
                #[name = "play_pause"]
                gtk::Button {
                    #[watch]
                    set_icon_name: model.playback_state_icon,
                    connect_clicked => CurrentSongMsg::PlayPause,
                    add_css_class: "track-action-btn",
                    add_css_class: "track-playpause-btn"
                },
                gtk::Button {
                    set_icon_name: icon_names::NEXT_REGULAR,
                    connect_clicked => CurrentSongMsg::Next,
                    add_css_class: "track-action-btn"
                }
            },

            gtk::CenterBox {
                set_orientation: Orientation::Horizontal,
                set_halign: Align::Center,

                #[wrap(Some)]
                set_center_widget = &gtk::Box{
                    set_orientation: Orientation::Horizontal,
                    set_spacing: 3,

                    gtk::Label {
                        #[watch]
                        set_label: &*format!("{}:{:02}", (model.playback_position / 60.0) as u64, model.playback_position as u64 % 60),
                        set_width_chars: 4
                    },
                    gtk::Scale {
                        set_orientation: Orientation::Horizontal,
                        #[watch]
                        set_adjustment: &gtk::Adjustment::new(model.playback_position, 0.0, model.song_info.duration.as_secs_f64(), 0.5, 0.0, 0.0),
                        #[watch]
                        set_value: model.playback_position,
                        set_hexpand: true,
                        set_width_request: 400,
                        connect_change_value[sender] => move |_range, scroll_type, val| {
                            sender.input(CurrentSongMsg::Seek(val));
                            Propagation::Proceed
                        },
                    },
                    gtk::Label {
                        #[watch]
                        set_label: &*format!("{}:{:02}", model.song_info.duration.as_secs() / 60, model.song_info.duration.as_secs() % 60),
                        set_width_chars: 4
                    }
                },


                #[wrap(Some)]
                set_end_widget = &gtk::Box{
                    set_orientation: Orientation::Horizontal,
                    set_spacing: 3,

                    #[name = "volume_btn"]
                    gtk::ScaleButton {
                        set_icons: &[icon_names::SPEAKER_0, icon_names::SPEAKER_3, icon_names::SPEAKER_1, icon_names::SPEAKER_2],
                        set_adjustment: &gtk::Adjustment::new(1.0, 0.0, 1.0, 0.05, 0.0, 0.0),
                        connect_value_changed[sender] => move |_btn, val| {
                            sender.input(CurrentSongMsg::VolumeChanged(val));
                        },
                    },

                    #[name = "rate_dropdown"]
                    gtk::DropDown {
                        set_enable_search: false,
                        set_model: Some(&gtk::StringList::new(&["0.5x", "0.75x", "1.0x", "1.25x", "1.5x", "1.75x", "2x"])),
                        set_selected: 2,
                        connect_selected_notify[sender] => move |dd| {
                            sender.input(CurrentSongMsg::RateChangeUI(match dd.selected() { // Rates from string list above
                                0 => 0.5,
                                1 => 0.75,
                                2 => 1.0,
                                3 => 1.25,
                                4 => 1.5,
                                5 => 1.75,
                                6 => 2.0,
                                _ => 1.0,
                            }));
                        }
                    },
                }
            }
        }
    }

    async fn init(
        init: Self::Init,
        root: Self::Root,
        sender: AsyncComponentSender<Self>,
    ) -> AsyncComponentParts<Self> {
        let x1 = init.0.clone();
        let mut x = x1.get_mut().await;
        x.set_model(sender.clone());

        {
            let track_list = init.1.read().await;
            match track_list.current() {
                None => Default::default(),
                Some(song) => sender.input(CurrentSongMsg::SongUpdate(SongInfo::from(song))),
            };
        }
        let model = CurrentSong {
            player_reference: init.0,
            track_list: init.1,
            client: init.2,
            playback_state_icon: icon_names::PLAY,
            sender: sender.clone(),
            song_info: Default::default(),
            playback_position: 0.0,
            playback_rate: 1.0,
        };
        let widgets: Self::Widgets = view_output!();

        let sender1 = sender.clone();
        glib::timeout_add_local(Duration::from_millis(500), move || {
            sender1.input(CurrentSongMsg::ProgressUpdate);
            return ControlFlow::Continue;
        });
        glib::timeout_add_seconds(10, move || {
            sender.input(CurrentSongMsg::ProgressUpdateSync(None));
            return ControlFlow::Continue;
        });

        AsyncComponentParts { model, widgets }
    }

    async fn update_with_view(
        &mut self,
        widgets: &mut Self::Widgets,
        message: Self::Input,
        sender: AsyncComponentSender<Self>,
        _root: &Self::Root,
    ) {
        match message {
            CurrentSongMsg::Start => {
                self.player_reference
                    .get()
                    .await
                    .start_current()
                    .await
                    .expect("Error starting!");
            }
            CurrentSongMsg::PlayPause => {
                self.player_reference.get().await.play_pause().await;
            }
            CurrentSongMsg::Next => {
                self.player_reference.get_mut().await.next().await;
            }
            CurrentSongMsg::Previous => self.player_reference.get_mut().await.previous().await,
            CurrentSongMsg::PlaybackStateChange(new_state) => match new_state.as_str() {
                "Paused" => self.playback_state_icon = icon_names::PLAY,
                "Playing" => self.playback_state_icon = icon_names::PAUSE,
                _ => self.playback_state_icon = icon_names::STOP,
            },
            CurrentSongMsg::SongUpdate(info) => {
                if self.song_info.id != info.id {
                    widgets
                        .cover_image
                        .set_cover_from_id(info.cover_art_id.as_ref(), self.client.clone())
                        .await;
                }
                self.song_info = info;
                let mpris_ref = self.player_reference.get().await;
                self.playback_position =
                    Duration::from_micros(MprisPlayer::position(mpris_ref.deref()) as u64)
                        .as_secs_f64();
            }
            CurrentSongMsg::VolumeChanged(volume) => {
                self.player_reference
                    .get()
                    .await
                    .set_volume_no_notify(volume);
            }
            CurrentSongMsg::VolumeChangedExternal(volume) => {
                widgets.volume_btn.set_value(volume);
            }
            CurrentSongMsg::ProgressUpdate => {
                if self.playback_state_icon == icon_names::PAUSE {
                    // If icon is PAUSE, then its currently playing
                    self.playback_position += (0.5 * self.playback_rate);
                }
            }
            CurrentSongMsg::RateChange(rate) => {
                self.playback_rate = rate;
                sender.input(CurrentSongMsg::ProgressUpdateSync(None));
            }
            CurrentSongMsg::RateChangeUI(rate) => {
                self.player_reference.get().await.set_rate(rate).await;
            }
            CurrentSongMsg::ProgressUpdateSync(pos) => {
                if let Some(pos) = pos {
                    self.playback_position = pos;
                } else {
                    let mpris_ref = self.player_reference.get().await;
                    self.playback_position =
                        Duration::from_micros(MprisPlayer::position(mpris_ref.deref()) as u64)
                            .as_secs_f64();
                }
            }
            CurrentSongMsg::Seek(pos) => {
                let mut mpris_player = self.player_reference.get_mut().await;
                mpris_player
                    .set_position(
                        &*self.song_info.dbus_path(),
                        Duration::from_secs_f64(pos).as_micros() as i64,
                    )
                    .await
                    .expect("Error seeking");
            }
        }
        self.update_view(widgets, sender);
    }
}
