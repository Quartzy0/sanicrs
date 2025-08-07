use crate::opensonic::cache::LyricsCache;
use crate::opensonic::types::{Song};
use crate::player::{LoopStatus, PlaybackStatus, PlayerInfo, SongEntry};
use crate::ui::app::Init;
use crate::ui::cover_picture::{CoverPicture, CoverSize};
use crate::ui::lyrics_line::{self, LyricsLine};
use crate::ui::random_songs_dialog::RandomSongsDialog;
use crate::ui::song_object::PositionState;
use crate::{icon_names, send_error, PlayerCommand};
use async_channel::Sender;
use color_thief::Color;
use readlock_tokio::SharedReadLock;
use relm4::adw::glib::closure_local;
use relm4::adw::gtk::glib::Propagation;
use relm4::adw::gtk::prelude::OrientableExt;
use relm4::adw::gtk::{Align, Orientation};
use relm4::adw::prelude::*;
use relm4::component::{AsyncComponent, AsyncComponentParts, AsyncComponentSender};
use relm4::gtk::glib::{clone, closure, Object};
use relm4::gtk::{Justification, ListItem, ListScrollFlags, SignalListItemFactory, Widget};
use relm4::adw::gio::ListStore;
use relm4::adw::glib as glib;
use relm4::prelude::*;
use std::ops::Deref;
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use uuid::Uuid;

pub struct CurrentSong {
    player_ref: SharedReadLock<PlayerInfo>,
    cmd_sender: Arc<Sender<PlayerCommand>>,
    lyrics_cache: LyricsCache,
    lyrics_factory: SignalListItemFactory,
    synced_lyrics: bool,
    show_lyrics: bool,
    has_lyrics: bool,

    // UI data
    song_info: Option<Arc<Song>>,
    playback_state_icon: &'static str,
    loop_status_icon: &'static str,
    playback_position: f64,
    playback_rate: f64,
    previous_progress_check: SystemTime,
}

#[derive(Debug)]
pub enum CurrentSongMsg {
    PlayPause,
    Next,
    Previous,
    SongUpdate(Option<SongEntry>),
    PlaybackStateChange(PlaybackStatus),
    VolumeChanged(f64),
    VolumeChangedExternal(f64),
    ProgressUpdate,
    ProgressUpdateSync(Option<f64>),
    RateChange(f64),
    RateChangeUI(f64),
    Seek(f64),
    CycleLoopStatusUI,
    SetLoopStatus(LoopStatus),
    ToggleShuffleUI,
    SetShuffle(bool),
    ToggleLyrics,
}

#[derive(Debug)]
pub enum CurrentSongOut {
    ColorSchemeChange(Option<Vec<Color>>),
    ToggleSidebar,
}

#[relm4::component(pub async)]
impl AsyncComponent for CurrentSong {
    type CommandOutput = ();
    type Input = CurrentSongMsg;
    type Output = CurrentSongOut;
    type Init = Init;

    view! {
        adw::Bin {
            add_css_class: "current-song",

            #[wrap(Some)]
            set_child = &gtk::Box {
                set_orientation: Orientation::Vertical,
                set_halign: Align::Center,
                set_valign: Align::Center,
                set_spacing: 5,
                add_css_class: "t2",

                append = if model.song_info.is_some() {
                    &gtk::Overlay {
                        add_overlay = &gtk::Button {
                            set_icon_name: icon_names::SUBTITLES2,
                            connect_clicked => CurrentSongMsg::ToggleLyrics,
                            set_halign: Align::End,
                            set_valign: Align::Start,
                            add_css_class: "spaced",
                            #[watch]
                            set_visible: model.has_lyrics,
                        },

                        #[wrap(Some)]
                        set_child = if !model.show_lyrics {
                            #[name = "cover_image"]
                            &CoverPicture {
                                set_cover_size: CoverSize::Huge,
                                set_cache: init.2,
                            }
                        } else {
                            &gtk::Box {
                                set_orientation: Orientation::Vertical,
                                set_valign: Align::Fill,
                                set_vexpand: true,
                                set_vexpand_set: true,

                                gtk::ScrolledWindow {
                                    set_vscrollbar_policy: gtk::PolicyType::Automatic,
                                    set_hscrollbar_policy: gtk::PolicyType::Never,
                                    set_valign: Align::Fill,
                                    set_vexpand: true,
                                    set_vexpand_set: true,
                                    set_halign: Align::Fill,
                                    set_hexpand: true,
                                    set_hexpand_set: true,

                                    #[name = "lyrics_list"]
                                    gtk::ListView {
                                        set_orientation: Orientation::Vertical,
                                        set_factory: Some(&model.lyrics_factory),
                                        add_css_class: "upcoming",
                                        add_css_class: "track-list-item"
                                    }
                                }
                            }
                        }
                    }

                } else {
                    &adw::Bin {}
                },
                append = &gtk::Label {
                    #[watch]
                    set_label: &model.song_info.as_ref()
                                    .and_then(|x| Some(x.title.clone()))
                                    .unwrap_or("No song".to_string()),
                    add_css_class: "bold",
                    add_css_class: "t1",
                },
                append = &gtk::Label {
                    #[watch]
                    set_label: &model.song_info.as_ref()
                                    .and_then(|x| Some(x.artists()))
                                    .unwrap_or("".to_string()),
                },
                append = &gtk::Label {
                    #[watch]
                    set_label: &model.song_info.as_ref()
                                    .and_then(|x| Some(x.album.clone().unwrap_or("Unknown artist".to_string())))
                                    .unwrap_or("".to_string()),
                    add_css_class: "italic",
                },

                append = &gtk::Box {
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

                append = &gtk::Box {
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
                },
                append = &gtk::CenterBox {
                    set_orientation: Orientation::Horizontal,
                    set_halign: Align::Fill,
                    set_hexpand: true,

                    #[wrap(Some)]
                    set_start_widget = &gtk::Box{
                        set_orientation: Orientation::Horizontal,
                        set_halign: Align::Center,
                        set_spacing: 3,

                        gtk::Button {
                            set_icon_name: icon_names::LIST,
                            set_tooltip_text: Some("Show queue"),
                            connect_clicked[sender] => move |_| {
                                sender.output(CurrentSongOut::ToggleSidebar)
                                    .expect("Error when sending message out of CurrentSong component");
                            },
                        },
                        #[name = "random_btn"]
                        gtk::Button {
                            set_icon_name: icon_names::ADD_REGULAR,
                            set_tooltip_text: Some("Add random songs"),
                        }
                    },

                    #[wrap(Some)]
                    set_center_widget = &gtk::Box{
                        set_orientation: Orientation::Horizontal,
                        set_halign: Align::Center,
                        set_spacing: 5,

                        gtk::Button {
                            #[watch]
                            set_icon_name: model.loop_status_icon,
                            set_tooltip_text: Some("Cycle loop status"),
                            connect_clicked => CurrentSongMsg::CycleLoopStatusUI,
                        },
                        #[name = "shuffle_toggle"]
                        gtk::ToggleButton {
                            set_icon_name: icon_names::PLAYLIST_SHUFFLE,
                            connect_clicked => CurrentSongMsg::ToggleShuffleUI,
                        }
                    },

                    #[wrap(Some)]
                    set_end_widget = &gtk::Box{
                        set_orientation: Orientation::Horizontal,
                        set_halign: Align::Center,
                        set_spacing: 5,

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
    }

    async fn init(
        init: Self::Init,
        root: Self::Root,
        sender: AsyncComponentSender<Self>,
    ) -> AsyncComponentParts<Self> {
        {
            let track_list = init.1.read().await;
            match track_list.current() {
                None => Default::default(),
                Some(song) => sender.input(CurrentSongMsg::SongUpdate(Some(SongEntry(
                    Uuid::new_v4(),
                    song.1.clone(),
                )))),
            };
        }
        let model = CurrentSong {
            player_ref: init.0,
            playback_state_icon: icon_names::PLAY,
            loop_status_icon: icon_names::PLAYLIST_CONSECUTIVE,
            song_info: Default::default(),
            playback_position: 0.0,
            playback_rate: 1.0,
            cmd_sender: init.3,
            previous_progress_check: SystemTime::now(),
            lyrics_cache: init.8,
            lyrics_factory: SignalListItemFactory::new(),
            synced_lyrics: false,
            show_lyrics: false,
            has_lyrics: false,
        };
        let widgets: Self::Widgets = view_output!();

        model
            .cmd_sender
            .send(PlayerCommand::CurrentSongSendSender(sender.clone()))
            .await
            .expect("Error sending sender to player");

        let s1 = sender.clone();
        sender.command(|_out, shutdown| {
            shutdown
                .register(async move {
                    let mut n = 0;
                    loop {
                        tokio::time::sleep(Duration::from_millis(500)).await;
                        s1.input(CurrentSongMsg::ProgressUpdate);
                        if n >= 20 {
                            s1.input(CurrentSongMsg::ProgressUpdateSync(None));
                            n = 0;
                        }
                        n += 1;
                    }
                })
                .drop_on_shutdown()
        });
        let v = model.player_ref.lock().await.volume().await;
        sender.input(CurrentSongMsg::VolumeChangedExternal(v));

        let s2 = sender.clone();
        widgets.cover_image.connect_closure(
            "cover-loaded",
            false,
            closure_local!(move |cover_picture: CoverPicture| {
                s2.output(CurrentSongOut::ColorSchemeChange(
                    cover_picture.get_palette(),
                ))
                .expect("Error when sending color scheme change event");
            }),
        );

        widgets.random_btn.connect_clicked(clone!(
            #[strong(rename_to = sndr)]
            model.cmd_sender,
            move |this| {
                let dialog = RandomSongsDialog::builder().launch(sndr.clone());
                dialog.widget().present(Some(this));
            }
        ));

        model.lyrics_factory.connect_setup(clone!(
            move |_, list_item| {
                let label = gtk::Label::new(None);
                label.set_wrap(true);
                label.set_justify(Justification::Center);

                let list_item = list_item
                    .downcast_ref::<ListItem>()
                    .expect("Needs to be ListItem");
                list_item.set_child(Some(&label));

                list_item
                    .property_expression("item")
                    .chain_property::<LyricsLine>("value")
                    .bind(&label, "label", Widget::NONE);
                list_item
                    .property_expression("item")
                    .chain_property::<LyricsLine>("position-state")
                    .chain_closure::<Vec<String>>(closure!(
                        move |_: Option<Object>, position_state: PositionState| {
                            match position_state {
                                PositionState::Passed => vec!["lyric-line".to_string()],
                                PositionState::Current => vec!["bold".to_string(), "lyric-line".to_string()],
                                PositionState::Upcoming => vec!["lyric-line".to_string()]
                            }
                    }))
                    .bind(&label, "css-classes", Widget::NONE);
            }
        ));

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
            CurrentSongMsg::PlayPause => self
                .cmd_sender
                .send(PlayerCommand::PlayPause)
                .await
                .expect("Error sending message to player"),
            CurrentSongMsg::Next => self
                .cmd_sender
                .send(PlayerCommand::Next)
                .await
                .expect("Error sending message to player"),
            CurrentSongMsg::Previous => self
                .cmd_sender
                .send(PlayerCommand::Previous)
                .await
                .expect("Error sending message to player"),
            CurrentSongMsg::PlaybackStateChange(new_state) => match new_state {
                PlaybackStatus::Paused => self.playback_state_icon = icon_names::PLAY,
                PlaybackStatus::Playing => self.playback_state_icon = icon_names::PAUSE,
                PlaybackStatus::Stopped => self.playback_state_icon = icon_names::STOP,
            },
            CurrentSongMsg::SongUpdate(info) => {
                self.playback_position = Duration::from_micros(PlayerInfo::position(
                    self.player_ref.lock().await.deref(),
                ) as u64)
                .as_secs_f64();
                widgets.cover_image.set_cover_id(
                    info.as_ref().and_then(|t| t.1.cover_art.clone())
                );
                self.has_lyrics = false;
                self.song_info = match info {
                    None => None,
                    Some(i) => {
                        let lyrics = self.lyrics_cache.get_lyrics(&i.1.id).await;
                        match lyrics {
                            Ok(l) => {
                                if let Some(list) = l.get(0){
                                    let lines = lyrics_line::from_list(list);
                                    let lyrics_store = ListStore::from_iter(lines);
                                    widgets.lyrics_list.set_model(Some(&gtk::NoSelection::new(Some(lyrics_store))));
                                    self.synced_lyrics = l[0].synced;
                                    self.has_lyrics = true;
                                } else {
                                    self.show_lyrics = false;
                                }
                            },
                            Err(e) => {
                                self.show_lyrics = false;
                                send_error(&self.cmd_sender, e).await;
                            },
                        }
                        Some(i.1)
                    },
                };


                self.previous_progress_check = SystemTime::now();
                sender.input(CurrentSongMsg::PlaybackStateChange(
                    self.player_ref.lock().await.playback_status(),
                ));
            }
            CurrentSongMsg::VolumeChanged(v) => self
                .cmd_sender
                .send(PlayerCommand::SetVolume(v))
                .await
                .expect("Error sending message to player"),
            CurrentSongMsg::VolumeChangedExternal(v) => widgets.volume_btn.set_value(v),
            CurrentSongMsg::ProgressUpdate => {
                if self.playback_state_icon == icon_names::PAUSE {
                    // If icon is PAUSE, then its currently playing
                    self.playback_position += SystemTime::now()
                        .duration_since(self.previous_progress_check)
                        .expect("Error calculating progress tiem")
                        .as_secs_f64()
                        * self.playback_rate;
                    self.update_lyrics(&widgets.lyrics_list);
                }
                self.previous_progress_check = SystemTime::now();
            }
            CurrentSongMsg::RateChange(rate) => {
                self.playback_rate = rate;
                sender.input(CurrentSongMsg::ProgressUpdateSync(None));
            }
            CurrentSongMsg::RateChangeUI(r) => self
                .cmd_sender
                .send(PlayerCommand::SetRate(r))
                .await
                .expect("Error sending message to player"),
            CurrentSongMsg::ProgressUpdateSync(pos) => {
                if let Some(pos) = pos {
                    self.playback_position = pos;
                    self.update_lyrics(&widgets.lyrics_list);
                } else {
                    self.playback_position = Duration::from_micros(PlayerInfo::position(
                        self.player_ref.lock().await.deref(),
                    ) as u64)
                    .as_secs_f64();
                }
            }
            CurrentSongMsg::Seek(pos) => self
                .cmd_sender
                .send(PlayerCommand::SetPosition(Duration::from_secs_f64(pos)))
                .await
                .expect("Error sending message to player"),
            CurrentSongMsg::CycleLoopStatusUI => {
                let loop_status = self.player_ref.lock().await.loop_status().await;
                let new_status = match loop_status {
                    LoopStatus::None => LoopStatus::Playlist,
                    LoopStatus::Playlist => LoopStatus::Track,
                    LoopStatus::Track => LoopStatus::None,
                };
                self.cmd_sender
                    .send(PlayerCommand::SetLoopStatus(new_status))
                    .await
                    .expect("Error sending message to player");
            }
            CurrentSongMsg::SetLoopStatus(loop_status) => {
                self.loop_status_icon = match loop_status {
                    LoopStatus::None => icon_names::PLAYLIST_CONSECUTIVE,
                    LoopStatus::Track => icon_names::PLAYLIST_REPEAT_SONG,
                    LoopStatus::Playlist => icon_names::PLAYLIST_REPEAT,
                }
            }
            CurrentSongMsg::ToggleShuffleUI => {
                let shuffle = !self.player_ref.lock().await.shuffled().await;
                self.cmd_sender
                    .send(PlayerCommand::SetShuffle(shuffle))
                    .await
                    .expect("Error sending message to player");
            }
            CurrentSongMsg::SetShuffle(shuffle) => {
                widgets.shuffle_toggle.set_active(shuffle);
            },
            CurrentSongMsg::ToggleLyrics => {
                self.show_lyrics = !self.show_lyrics;
                self.update_lyrics(&widgets.lyrics_list);
            },
        }
        self.update_view(widgets, sender);
    }
}

impl CurrentSong {
    fn update_lyrics(
        &mut self,
        list_view: &gtk::ListView
    ) {
        if let Some(model) = list_view.model() && self.synced_lyrics && self.show_lyrics {
            let selection = model.downcast::<gtk::NoSelection>()
                .expect("Song list model should be NoSelection");
            let store = selection.model().unwrap().downcast::<ListStore>().expect("Should be ListStore");
            let pos = Duration::from_secs_f64(self.playback_position);
            let mut prev_item = store.item(0)
                .expect("Expected item at 0")
                .downcast::<LyricsLine>()
                .expect("Expected LyricsLine");
            for (i, item) in store.iter::<LyricsLine>().enumerate().skip(1) {
                if let Ok(item) = item {
                    let start = Duration::from_millis(item.start() as u64);
                    let prev_start = Duration::from_millis(prev_item.start() as u64);
                    let state = if pos > prev_start {
                            if pos > start {
                                PositionState::Passed
                            } else {
                                PositionState::Current
                            }
                        } else if pos < prev_start {
                            PositionState::Upcoming
                        } else {
                            PositionState::Current
                        };
                    prev_item.set_position_state(state);
                    if state == PositionState::Current {
                        let scroll_info = gtk::ScrollInfo::new();
                        scroll_info.set_enable_vertical(true);
                        list_view.scroll_to(i as u32, ListScrollFlags::NONE, Some(scroll_info));
                    }
                    prev_item = item;
                }
            }
        }
    }
}
