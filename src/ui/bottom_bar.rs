use std::rc::Rc;
use std::time::Duration;
use gstreamer_play::PlayState;
use mpris_server::{LocalPlayerInterface, LocalServer};
use relm4::actions::ActionablePlus;
use relm4::adw::glib::{clone, Propagation};
use relm4::adw::gtk::prelude::*;
use relm4::gtk::glib::{closure, Object};
use relm4::prelude::*;
use relm4::adw::{gdk, glib, gtk, LengthUnit};
use relm4::gtk::{Align, Justification, Orientation, Widget};
use relm4::gtk::pango::EllipsizeMode;
use uuid::Uuid;
use crate::dbus::player::MprisPlayer;
use crate::icon_names;
use crate::opensonic::cache::SongCache;
use crate::opensonic::types::Song;
use crate::player::SongEntry;
use crate::ui::app::{Init, NextAction, PlayPauseAction, PreviousAction, ShowRandomSongsAction, ShowTracklistAction};
use crate::ui::cover_picture::{CoverPicture, CoverSize};
use crate::ui::current_song::CurrentSongMsg;

pub struct BottomBar {
    mpris_player: Rc<LocalServer<MprisPlayer>>,
    song_cache: SongCache,

    // UI data
    song_info: Option<Rc<Song>>,
    playback_position: f64,
}

#[derive(Debug)]
pub enum BottomBarOut {
    ShowSong,
}

// Needed because Relm4 view! macro doesn't work otherwise (or idk how to make it work otherwise)
fn add_layout_conv(view: &adw::MultiLayoutView, layout: &impl IsA<gtk::Widget>, name: &str) {
    let layout = adw::Layout::new(layout);
    layout.set_name(Some(name));
    view.add_layout(layout);
}

#[relm4::component(pub async)]
impl AsyncComponent for BottomBar {
    type CommandOutput = ();
    type Input = CurrentSongMsg; // Reuse CurrentSongMsg because so many of the messages are the same
    type Output = BottomBarOut;
    type Init = Init;

    view!{
        gtk::Overlay {
            #[name = "progress_bar_overlay"]
            add_overlay = &gtk::ProgressBar {
                set_orientation: Orientation::Horizontal,
                #[watch]
                set_fraction: model.playback_position / model.song_info.as_ref()
                    .and_then(|x| x.duration)
                    .unwrap_or(Duration::from_secs(1))
                    .as_secs_f64(),
                set_hexpand: true,
                set_visible: false,
                set_can_focus: false,
                set_can_target: false,
                add_css_class: "osd",
            },

            #[name = "main_box"]
            #[wrap(Some)]
            set_child = &gtk::CenterBox {
                set_orientation: Orientation::Horizontal,
                add_css_class: "padded",
                add_css_class: "current-song",
                set_shrink_center_last: false,

                #[name = "start_box"]
                #[wrap(Some)]
                set_start_widget = &gtk::Box {
                    set_orientation: Orientation::Horizontal,
                    set_halign: Align::Start,
                    set_valign: Align::Center,
                    set_spacing: 5,
                    set_vexpand: false,
                    set_vexpand_set: true,
                    set_cursor: gdk::Cursor::from_name("pointer", None).as_ref(),

                    #[name = "cover_image"]
                    CoverPicture {
                        set_cache: init.0.clone(),
                        set_cover_size: CoverSize::Small,
                        set_halign: Align::Center,
                        set_valign: Align::Center,
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
                            set_ellipsize: EllipsizeMode::End,
                            set_tooltip: "Song title",
                        },
                        gtk::Label {
                            #[watch]
                            set_markup: &model.song_info.as_ref()
                                            .and_then(|x| Some(x.artists()))
                                            .unwrap_or("".to_string()),
                            set_max_width_chars: 30,
                            set_justify: Justification::Left,
                            set_halign: Align::Start,
                            set_ellipsize: EllipsizeMode::End,
                            set_tooltip: "Artists",
                            connect_activate_link => move |this, url| {
                                this.activate_action("win.artist", Some(&url.to_variant())).expect("Error executing action");
                                glib::Propagation::Stop
                            },
                        },
                        gtk::Label {
                            #[watch]
                            set_markup: &model.song_info.as_ref()
                                            .and_then(|x| x.album.as_ref().and_then(|a|
                                                Some(format!("<a href=\"{}\" title=\"View album\" class=\"normal-link\">{}</a>", x.id, a))
                                            ))
                                            .unwrap_or("".to_string()),
                            add_css_class: "italic",
                            set_justify: Justification::Left,
                            set_halign: Align::Start,
                            set_max_width_chars: 30,
                            set_ellipsize: EllipsizeMode::End,
                            set_tooltip: "Album",
                            connect_activate_link => move |this, url| {
                                if url.len() != 0 {
                                    this.activate_action("win.song", Some(&url.to_variant())).expect("Error executing action");
                                }
                                glib::Propagation::Stop
                            },
                        }
                    },
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
                            add_css_class: "track-action-btn",
                            set_tooltip: "Play previous",
                            ActionablePlus::set_stateless_action::<PreviousAction>: &(),
                        },
                        append = if !model.mpris_player.imp().is_buffering() {
                            &gtk::Button {
                                #[watch]
                                set_icon_name: match model.mpris_player.imp().info().playback_status() {
                                    PlayState::Paused => icon_names::PLAY,
                                    PlayState::Playing => icon_names::PAUSE,
                                    _ => icon_names::STOP,
                                },
                                set_tooltip: "Toggle playback",
                                add_css_class: "track-action-btn",
                                add_css_class: "track-playpause-btn",
                                ActionablePlus::set_stateless_action::<PlayPauseAction>: &(),
                            }
                        } else {
                            adw::Spinner {
                                set_tooltip: "Loading...",
                            }
                        },
                        gtk::Button {
                            set_icon_name: icon_names::NEXT_REGULAR,
                            add_css_class: "track-action-btn",
                            set_tooltip: "Play next",
                            ActionablePlus::set_stateless_action::<NextAction>: &(),
                        }
                    },
                    adw::Clamp{
                        set_orientation: Orientation::Horizontal,
                        set_maximum_size: 1000,
                        set_tightening_threshold: 600,
                        set_unit: LengthUnit::Px,

                        gtk::Box {
                            set_orientation: Orientation::Horizontal,
                            set_halign: Align::Fill,
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
                                // set_width_request: 400,
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
                    }
                },

                #[wrap(Some)]
                set_end_widget = &gtk::Box {
                    set_orientation: Orientation::Horizontal,
                    set_halign: Align::End,
                    set_spacing: 5,

                    #[name = "like_btn"]
                    gtk::ToggleButton {
                        #[watch]
                        set_active: model.song_info.as_ref().and_then(|s| Some(s.is_starred())).unwrap_or(false),
                        add_css_class: "flat",
                        set_tooltip: "Star song",
                        connect_clicked => CurrentSongMsg::ToggleStarred,
                        set_valign: Align::Center,
                    },
                    #[name = "end_layout"]
                    adw::MultiLayoutView {
                        crate::ui::bottom_bar::add_layout_conv["default"] = &gtk::Box {
                            set_orientation: Orientation::Horizontal,
                            set_halign: Align::End,
                            set_spacing: 5,

                            gtk::Button {
                                set_icon_name: icon_names::LIST,
                                set_tooltip: "Show queue",
                                set_valign: Align::Center,
                                set_halign: Align::Center,
                                ActionablePlus::set_stateless_action::<ShowTracklistAction>: &(),
                            },
                            gtk::Button {
                                set_icon_name: icon_names::ADD_REGULAR,
                                set_tooltip: "Add random songs",
                                set_valign: Align::Center,
                                set_halign: Align::Center,
                                ActionablePlus::set_stateless_action::<ShowRandomSongsAction>: &(),
                            },
                            #[name = "volume_btn"]
                            gtk::ScaleButton {
                                #[watch]
                                set_value: model.mpris_player.imp().info().volume(),
                                set_icons: &[icon_names::SPEAKER_0, icon_names::SPEAKER_3, icon_names::SPEAKER_1, icon_names::SPEAKER_2],
                                set_adjustment: &gtk::Adjustment::new(1.0, 0.0, 1.0, 0.05, 0.0, 0.0),
                                set_valign: Align::Center,
                                set_halign: Align::Center,
                                set_tooltip: "Adjust volume",
                                connect_value_changed[mplayer] => move |_btn, val| {
                                            mplayer.imp().set_volume(val);
                                },
                            },
                        },
                        crate::ui::bottom_bar::add_layout_conv["small"] = &gtk::Box {
                            set_orientation: Orientation::Horizontal,
                            set_halign: Align::End,
                            set_spacing: 5,

                            if !model.mpris_player.imp().is_buffering() {
                                &gtk::Button {
                                    #[watch]
                                    set_icon_name: match model.mpris_player.imp().info().playback_status() {
                                        PlayState::Paused => icon_names::PLAY,
                                        PlayState::Playing => icon_names::PAUSE,
                                        _ => icon_names::STOP,
                                    },
                                    set_tooltip: "Toggle playback",
                                    add_css_class: "track-action-btn",
                                    add_css_class: "track-playpause-btn",
                                    set_valign: Align::Center,
                                    set_halign: Align::Center,
                                    ActionablePlus::set_stateless_action::<PlayPauseAction>: &(),
                                }
                            } else {
                                adw::Spinner {
                                    set_tooltip: "Loading...",
                                }
                            },
                            gtk::Button {
                                set_icon_name: icon_names::NEXT_REGULAR,
                                set_tooltip: "Play next",
                                add_css_class: "track-action-btn",
                                set_valign: Align::Center,
                                set_halign: Align::Center,
                                ActionablePlus::set_stateless_action::<NextAction>: &(),
                            },
                        }
                    }
                }
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
            song_info: Default::default(),
            playback_position: 0.0,
            song_cache: init.1,
        };
        model.mpris_player.imp().bb_sender.replace(Some(sender.clone()));

        let mplayer = &model.mpris_player;
        let widgets: BottomBarWidgets = view_output!();

        {
            let track_list = model.mpris_player.imp().track_list().borrow();
            match track_list.current() {
                None => Default::default(),
                Some(song) => sender.input(CurrentSongMsg::SongUpdate(Some(SongEntry {
                    uuid: Uuid::new_v4(),
                    song: song.song.clone(),
                }))),
            };
        }

        let gesture = gtk::GestureClick::new();
        gesture.connect_released(clone!(
            #[strong]
            sender,
            move |_this, _n: i32, _x: f64, _y: f64| {
                sender.output(BottomBarOut::ShowSong).expect("Error sending output");
            }
        ));
        widgets.start_box.add_controller(gesture);

        widgets.like_btn
            .property_expression("active")
            .chain_closure::<String>(closure!(
                move |_: Option<Object>, active: bool| {
                    if active {
                        icon_names::HEART_FILLED
                    } else {
                        icon_names::HEART_OUTLINE_THIN
                    }
                }
            ))
            .bind(&widgets.like_btn, "icon-name", Widget::NONE);

        let breakpoint = init.9;
        breakpoint.add_setter(&widgets.main_box, "center-widget", Some(&Widget::NONE.to_value()));
        breakpoint.add_setter(&widgets.progress_bar_overlay, "visible", Some(&true.to_value()));
        breakpoint.add_setter(&widgets.end_layout, "layout-name", Some(&"small".to_value()));
        widgets.end_layout.set_layout_name("default");

        AsyncComponentParts { model, widgets }
    }

    async fn update_with_view(
        &mut self,
        widgets: &mut Self::Widgets,
        message: Self::Input,
        sender: AsyncComponentSender<Self>,
        _root: &Self::Root,
    ) {
        let player = self.mpris_player.imp();
        match message {
            CurrentSongMsg::SongUpdate(info) => {
                if info.is_some() {
                    self.playback_position = Duration::from_micros(player.position().await.unwrap().as_micros() as u64).as_secs_f64();
                } else {
                    self.playback_position = 0.0;
                }
                widgets.cover_image.set_cover_id(
                    info.as_ref().and_then(|t| t.song.cover_art.clone())
                );
                self.song_info = match info {
                    None => None,
                    Some(i) => {
                        Some(i.song)
                    },
                };
            }
            CurrentSongMsg::ProgressUpdateSync(pos) => {
                self.playback_position = pos;
            }
            CurrentSongMsg::Seek(pos) => player.send_res(player.set_position(Duration::from_secs_f64(pos))),
            CurrentSongMsg::ToggleStarred => {
                if self.song_info.is_some() {
                    let song = self.song_info.as_ref().unwrap();
                    player.send_res(self.song_cache.toggle_starred(song).await);
                }
            },
            _ => {},
        }
        match player.player_ref.playback_status() {
            PlayState::Stopped => {}
            PlayState::Buffering => {}
            PlayState::Paused => {}
            PlayState::Playing => {}
            _ => {}
        }
        self.update_view(widgets, sender);
    }
}
