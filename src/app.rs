use std::ops::Deref;
use crate::icon_names;
use crate::mpris::MprisPlayer;
use crate::opensonic::client::OpenSubsonicClient;
use crate::opensonic::types::Song;
use crate::player::TrackList;
use gtk::prelude::GtkWindowExt;
use relm4::adw::gdk::Texture;
use relm4::adw::prelude::*;
use relm4::adw::glib;
use relm4::gtk::prelude::OrientableExt;
use relm4::gtk::{Align, Orientation};
use relm4::prelude::*;
use relm4::SimpleComponent;
use relm4::{
    adw, component::{AsyncComponent, AsyncComponentParts, AsyncComponentSender},
    gtk,
    RelmApp,
};
use std::sync::Arc;
use std::thread;
use std::time::Duration;
use relm4::adw::glib::ControlFlow;
use relm4::factory::Position;
use relm4::gtk::glib::Propagation;
use tokio::sync::RwLock;
use zbus::object_server::InterfaceRef;

#[derive(Debug)]
pub struct SongInfo {
    id: String,
    title: String,
    artist: String,
    album: String,
    cover_art_id: Option<String>,
    duration: Duration,
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
            duration: value.duration.unwrap()
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

pub struct Model {
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
pub enum AppMsg {
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

type Init = (
    InterfaceRef<MprisPlayer>,
    Arc<RwLock<TrackList>>,
    Arc<OpenSubsonicClient>,
);

pub fn start_app(init: Init) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        let app = RelmApp::new("me.quartzy.sanicrs");
        relm4_icons::initialize_icons(icon_names::GRESOURCE_BYTES, icon_names::RESOURCE_PREFIX);
        relm4::set_global_css(include_str!("css/style.css"));
        app.run_async::<Model>(init);
    })
}

#[relm4::component(pub async)]
impl AsyncComponent for Model {
    type CommandOutput = ();
    type Input = AppMsg;
    type Output = ();
    type Init = Init;

    view! {
        adw::ApplicationWindow {
            set_title: Some("Sanic-rs"),
            set_default_width: 400,
            set_default_height: 400,

            adw::ToolbarView {
                #[wrap(Some)]
                set_content = &adw::ViewStack {
                    add = &gtk::Box {
                        set_orientation: Orientation::Vertical,
                        set_halign: Align::Center,
                        set_valign: Align::Center,
                        set_spacing: 5,

                        gtk::Frame{
                            add_css_class: "cover-img-frame",
                            #[name = "cover_image"]
                            gtk::Image {
                                set_pixel_size: 512,
                            },
                        },
                        gtk::Label {
                            #[watch]
                            set_label: &model.song_info.title,
                            add_css_class: "track-title",
                            add_css_class: "track-info",
                        },
                        gtk::Label {
                            #[watch]
                            set_label: &model.song_info.artist,
                            add_css_class: "track-artist",
                            add_css_class: "track-info",
                        },
                        gtk::Label {
                            #[watch]
                            set_label: &model.song_info.album,
                            add_css_class: "track-album",
                            add_css_class: "track-info",
                        },

                        gtk::Box {
                            set_orientation: Orientation::Horizontal,
                            set_halign: Align::Center,
                            set_spacing: 10,

                            gtk::Button {
                                set_icon_name: icon_names::PREVIOUS_REGULAR,
                                connect_clicked => AppMsg::Previous,
                                add_css_class: "track-action-btn"
                            },
                            #[name = "play_pause"]
                            gtk::Button {
                                #[watch]
                                set_icon_name: model.playback_state_icon,
                                connect_clicked => AppMsg::PlayPause,
                                add_css_class: "track-action-btn",
                                add_css_class: "track-playpause-btn"
                            },
                            gtk::Button {
                                set_icon_name: icon_names::NEXT_REGULAR,
                                connect_clicked => AppMsg::Next,
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
                                    set_label: &*format!("{}:{:02}", (model.playback_position / 60.0) as u64, model.playback_position as u64 % 60)
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
                                        sender.input(AppMsg::Seek(val));
                                        Propagation::Proceed
                                    },
                                },
                                gtk::Label {
                                    #[watch]
                                    set_label: &*format!("{}:{:02}", model.song_info.duration.as_secs() / 60, model.song_info.duration.as_secs() % 60)
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
                                        sender.input(AppMsg::VolumeChanged(val));
                                    },
                                },

                                #[name = "rate_dropdown"]
                                gtk::DropDown {
                                    set_enable_search: false,
                                    set_model: Some(&gtk::StringList::new(&["0.5x", "0.75x", "1.0x", "1.25x", "1.5x", "1.75x", "2x"])),
                                    set_selected: 2,
                                    connect_selected_notify[sender] => move |dd| {
                                        sender.input(AppMsg::RateChangeUI(match dd.selected() { // Rates from string list above
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
                    } -> {
                        set_title: Some("Song"),
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
                Some(song) => sender.input(AppMsg::SongUpdate(SongInfo::from(song))),
            };
        }
        let model = Model {
            player_reference: init.0,
            track_list: init.1,
            client: init.2,
            playback_state_icon: icon_names::PLAY,
            sender: sender.clone(),
            song_info: Default::default(),
            playback_position: 0.0,
            playback_rate: 1.0,
        };
        let widgets = view_output!();

        let sender1 = sender.clone();
        glib::timeout_add_local(Duration::from_millis(500), move || {
            sender1.input(AppMsg::ProgressUpdate);
            return ControlFlow::Continue;
        });
        glib::timeout_add_seconds(10, move || {
            sender.input(AppMsg::ProgressUpdateSync(None));
            return ControlFlow::Continue;
        });

        AsyncComponentParts { model, widgets }
    }

    async fn update_with_view(
        &mut self,
        widgets: &mut Self::Widgets,
        message: Self::Input,
        sender: AsyncComponentSender<Self>,
        root: &Self::Root,
    ) {
        match message {
            AppMsg::Start => {
                self.player_reference
                    .get()
                    .await
                    .start_current()
                    .await
                    .expect("Error starting!");
            }
            AppMsg::PlayPause => {
                self.player_reference.get().await.play_pause().await;
            }
            AppMsg::Next => {
                self.player_reference.get_mut().await.next().await;
            }
            AppMsg::Previous => self.player_reference.get_mut().await.previous().await,
            AppMsg::PlaybackStateChange(new_state) => match new_state.as_str() {
                "Paused" => self.playback_state_icon = icon_names::PLAY,
                "Playing" => self.playback_state_icon = icon_names::PAUSE,
                _ => self.playback_state_icon = icon_names::STOP,
            },
            AppMsg::SongUpdate(info) => {
                self.song_info = info;
                self.playback_position = 0.0;
                if let Some(cover_art_id) = &self.song_info.cover_art_id {
                    let img_resp = self
                        .client
                        .get_cover_image(cover_art_id.as_str(), Some("512"))
                        .await
                        .expect("Error getting cover image");
                    let bytes = img_resp.bytes().await.unwrap().to_vec();
                    let bytes = glib::Bytes::from(&bytes.to_vec());
                    let texture = Texture::from_bytes(&bytes).expect("Error loading textre");
                    widgets.cover_image.set_paintable(Some(&texture));
                }
            },
            AppMsg::VolumeChanged(volume) => {
                self.player_reference.get().await.set_volume_no_notify(volume);
            },
            AppMsg::VolumeChangedExternal(volume) => {
                widgets.volume_btn.set_value(volume);
            },
            AppMsg::ProgressUpdate => {
                if self.playback_state_icon == icon_names::PAUSE { // If icon is PAUSE, then its currently playing
                    self.playback_position += (0.5 * self.playback_rate);
                }
            },
            AppMsg::RateChange(rate) => {
                self.playback_rate = rate;
                sender.input(AppMsg::ProgressUpdateSync(None));
            },
            AppMsg::RateChangeUI(rate) => {
                self.player_reference.get().await.set_rate(rate).await;
            },
            AppMsg::ProgressUpdateSync(pos) => {
                if let Some(pos) = pos {
                    self.playback_position = pos;
                } else {
                    let mpris_ref = self.player_reference.get().await;
                    self.playback_position = Duration::from_micros(MprisPlayer::position(mpris_ref.deref()) as u64).as_secs_f64();
                }
            },
            AppMsg::Seek(pos) => {
                let mut mpris_player = self.player_reference.get_mut().await;
                mpris_player.set_position(&*self.song_info.dbus_path(), Duration::from_secs_f64(pos).as_micros() as i64).await.expect("Error seeking");
            }
        }
        self.update_view(widgets, sender);
    }
}
