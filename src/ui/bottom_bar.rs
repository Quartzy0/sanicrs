use std::rc::Rc;
use std::time::{Duration, SystemTime};
use mpris_server::{LocalPlayerInterface, LocalServer, PlaybackStatus, Time};
use relm4::adw::glib::{clone, Propagation};
use relm4::adw::gtk::prelude::*;
use relm4::prelude::*;
use relm4::adw::{gdk, glib, gtk};
use relm4::gtk::{Align, Justification, Orientation};
use crate::dbus::player::MprisPlayer;
use crate::icon_names;
use crate::opensonic::types::Song;
use crate::ui::app::Init;
use crate::ui::cover_picture::{CoverPicture, CoverSize};
use crate::ui::current_song::CurrentSongMsg;

pub struct BottomBar {
    mpris_player: Rc<LocalServer<MprisPlayer>>,

    // UI data
    song_info: Option<Rc<Song>>,
    playback_state_icon: &'static str,
    loop_status_icon: &'static str,
    playback_position: f64,
    playback_rate: f64,
    previous_progress_check: SystemTime,
}

#[derive(Debug)]
pub enum BottomBarOut {
    ShowSong,
}

#[relm4::component(pub async)]
impl AsyncComponent for BottomBar {
    type CommandOutput = ();
    type Input = CurrentSongMsg; // Reuse CurrentSongMsg because so many of the messages are the same
    type Output = BottomBarOut;
    type Init = Init;

    view!{
        gtk::CenterBox {
            set_orientation: Orientation::Horizontal,
            add_css_class: "spaced",

            #[name = "start_box"]
            #[wrap(Some)]
            set_start_widget = &gtk::Box {
                set_orientation: Orientation::Horizontal,
                set_halign: Align::Start,
                set_spacing: 5,
                set_cursor: gdk::Cursor::from_name("pointer", None).as_ref(),

                #[name = "cover_image"]
                CoverPicture {
                    set_cache: init.0,
                    set_cover_size: CoverSize::Small,
                },
                gtk::Box {
                    set_orientation: Orientation::Vertical,
                    set_spacing: 5,
                    set_halign: Align::Start,

                    gtk::Label {
                        #[watch]
                        set_label: &model.song_info.as_ref()
                                        .and_then(|x| Some(x.title.clone()))
                                        .unwrap_or("No song".to_string()),
                        add_css_class: "bold",
                        add_css_class: "t2",
                        set_max_width_chars: 30,
                        set_justify: Justification::Left,
                        set_halign: Align::Start,
                    },
                    gtk::Label {
                        #[watch]
                        set_label: &model.song_info.as_ref()
                                        .and_then(|x| Some(x.artists()))
                                        .unwrap_or("".to_string()),
                        set_max_width_chars: 30,
                        set_justify: Justification::Left,
                        set_halign: Align::Start,
                    },
                }
            },

            #[wrap(Some)]
            set_center_widget = &gtk::Box {
                set_orientation: Orientation::Vertical,
                set_spacing: 5,

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
                gtk::Box {
                    set_orientation: Orientation::Horizontal,
                    set_halign: Align::Center,
                    set_spacing: 3,

                    append = &gtk::Label {
                        #[watch]
                        set_label: &*format!("{}:{:02}", (model.playback_position / 60.0) as u64, model.playback_position as u64 % 60),
                        set_width_chars: 4
                    },
                    append = &gtk::Scale {
                        set_orientation: Orientation::Horizontal,
                        #[watch]
                        set_adjustment: &gtk::Adjustment::new(model.playback_position, 0.0, model.song_info.as_ref()
                            .and_then(|x| x.duration)
                            .unwrap_or(Duration::from_secs(1))
                            .as_secs_f64(), 0.5, 0.0, 0.0),
                        #[watch]
                        set_value: model.playback_position,
                        set_hexpand: true,
                        set_width_request: 400,
                        connect_change_value[sender] => move |_range, _scroll_type, val| {
                            sender.input(CurrentSongMsg::Seek(val));
                            Propagation::Proceed
                        },
                    },
                    append = &gtk::Label {
                        #[watch]
                        set_label: &*format!("{}:{:02}",
                            model.song_info.as_ref()
                            .and_then(|x| x.duration)
                            .unwrap_or(Duration::from_secs(0))
                            .as_secs() / 60,
                            model.song_info.as_ref()
                            .and_then(|x| x.duration)
                            .unwrap_or(Duration::from_secs(0))
                            .as_secs() % 60),
                        set_width_chars: 4
                    }
                }
            },

            #[wrap(Some)]
            set_end_widget = &gtk::Box {
                set_orientation: Orientation::Horizontal,
                set_halign: Align::End,
                set_spacing: 5,

                #[name = "volume_btn"]
                gtk::ScaleButton {
                    set_icons: &[icon_names::SPEAKER_0, icon_names::SPEAKER_3, icon_names::SPEAKER_1, icon_names::SPEAKER_2],
                    set_adjustment: &gtk::Adjustment::new(1.0, 0.0, 1.0, 0.05, 0.0, 0.0),
                    set_value: model.mpris_player.imp().volume().await.unwrap(),
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

    async fn init(
        init: Self::Init,
        root: Self::Root,
        sender: AsyncComponentSender<Self>,
    ) -> AsyncComponentParts<Self> {
        let model = Self {
            mpris_player: init.6,
            playback_state_icon: icon_names::PLAY,
            loop_status_icon: icon_names::PLAYLIST_CONSECUTIVE,
            song_info: Default::default(),
            playback_position: 0.0,
            playback_rate: 1.0,
            previous_progress_check: SystemTime::now(),
        };
        model.mpris_player.imp().bb_sender.replace(Some(sender.clone()));

        let widgets: BottomBarWidgets = view_output!();

        let gesture = gtk::GestureClick::new();
        gesture.connect_released(clone!(
            #[strong]
            sender,
            move |_this, _n: i32, _x: f64, _y: f64| {
                sender.output(BottomBarOut::ShowSong).expect("Error sending output");
            }
        ));
        widgets.start_box.add_controller(gesture);

        glib::timeout_add_local(Duration::from_millis(500), clone!(
            #[strong]
            sender,
            move || {
                match sender.input_sender().send(CurrentSongMsg::ProgressUpdate) {
                    Ok(_) => glib::ControlFlow::Continue,
                    Err(_) => glib::ControlFlow::Break
                }
            }
        ));
        glib::timeout_add_seconds_local(10, clone!(
            #[strong]
            sender,
            move || {
                match sender.input_sender().send(CurrentSongMsg::ProgressUpdateSync(None)) {
                    Ok(_) => glib::ControlFlow::Continue,
                    Err(_) => glib::ControlFlow::Break
                }
            }
        ));

        AsyncComponentParts { model, widgets }
    }

    async fn update_with_view(
        &mut self,
        widgets: &mut Self::Widgets,
        message: Self::Input,
        sender: AsyncComponentSender<Self>,
        root: &gtk::CenterBox,
    ) {
        let player = self.mpris_player.imp();
        match message {
            CurrentSongMsg::PlayPause => player.play_pause().await.unwrap(),
            CurrentSongMsg::Next => player.next().await.unwrap(),
            CurrentSongMsg::Previous => player.previous().await.unwrap(),
            CurrentSongMsg::PlaybackStateChange(new_state) => match new_state {
                PlaybackStatus::Paused => self.playback_state_icon = icon_names::PLAY,
                PlaybackStatus::Playing => self.playback_state_icon = icon_names::PAUSE,
                PlaybackStatus::Stopped => self.playback_state_icon = icon_names::STOP,
            },
            CurrentSongMsg::SongUpdate(info) => {
                self.playback_position = Duration::from_micros(player.position().await.unwrap().as_micros() as u64)
                    .as_secs_f64();
                widgets.cover_image.set_cover_id(
                    info.as_ref().and_then(|t| t.1.cover_art.clone())
                );
                self.song_info = match info {
                    None => None,
                    Some(i) => {
                        Some(i.1)
                    },
                };

                self.previous_progress_check = SystemTime::now();
                sender.input(CurrentSongMsg::PlaybackStateChange(
                    player.playback_status().await.unwrap(),
                ));
            }
            CurrentSongMsg::VolumeChanged(v) => player.set_volume(v).await.unwrap(),
            CurrentSongMsg::VolumeChangedExternal(v) => widgets.volume_btn.set_value(v),
            CurrentSongMsg::ProgressUpdate => {
                if self.playback_state_icon == icon_names::PAUSE {
                    // If icon is PAUSE, then its currently playing
                    self.playback_position += SystemTime::now()
                        .duration_since(self.previous_progress_check)
                        .expect("Error calculating progress tiem")
                        .as_secs_f64()
                        * self.playback_rate;
                }
                self.previous_progress_check = SystemTime::now();
            }
            CurrentSongMsg::ProgressUpdateSync(pos) => {
                if let Some(pos) = pos {
                    self.playback_position = pos;
                } else {
                    self.playback_position = Duration::from_micros(player.position().await.unwrap().as_micros() as u64)
                        .as_secs_f64();
                }
            }
            CurrentSongMsg::Seek(pos) => player.seek(Time::from_micros(Duration::from_secs_f64(pos).as_micros() as i64)).await.unwrap(),
            CurrentSongMsg::RateChange(rate) => {
                self.playback_rate = rate;
                sender.input(CurrentSongMsg::ProgressUpdateSync(None));
            }
            CurrentSongMsg::RateChangeUI(r) => player.set_rate(r).await.unwrap(),
            _ => {},
        }
        self.update_view(widgets, sender);
    }
}
