use crate::dbus::player::MprisPlayer;
use crate::opensonic::cache::{AlbumCache, ArtistCache, CoverCache, LyricsCache, SongCache, SuperCache};
use crate::ui::browse::search::SearchType;
use crate::ui::browse::{BrowseMsg, BrowseMsgOut, BrowseWidget};
use crate::ui::current_song::{CurrentSong, CurrentSongOut};
use crate::ui::preferences_view::{PreferencesOut, PreferencesWidget};
use crate::ui::random_songs_dialog::RandomSongsDialog;
use crate::ui::track_list::TrackListWidget;
use async_channel::Receiver;
use color_thief::Color;
use gtk::prelude::GtkWindowExt;
use libsecret::Schema;
use mpris_server::{LocalPlayerInterface, LocalServer};
use relm4::abstractions::Toaster;
use relm4::actions::{AccelsPlus, RelmAction, RelmActionGroup};
use relm4::adw::{glib as glib, Breakpoint, BreakpointConditionLengthType, LengthUnit};
use relm4::adw::glib::{clone};
use relm4::adw::gtk::CssProvider;
use relm4::adw::prelude::*;
use relm4::adw::{gdk};
use relm4::component::AsyncConnector;
use relm4::gtk::gio::{self, Settings};
use relm4::gtk::{License, Orientation};
use relm4::prelude::*;
use relm4::{adw, component::{AsyncComponent, AsyncComponentParts, AsyncComponentSender}};
use std::cell::LazyCell;
use std::rc::Rc;
use crate::{APP_ID, VERSION_STR};
use crate::ui::bottom_bar::{BottomBar, BottomBarOut};
use crate::ui::header_bar::HeaderBar;

const BG_COLORS: usize = 4;

pub struct Model {
    current_song: AsyncController<CurrentSong>,
    track_list_connector: AsyncConnector<TrackListWidget>,
    bottom_bar_connector: AsyncController<BottomBar>,
    browse_connector: AsyncController<BrowseWidget>,
    provider: CssProvider,
    settings: Settings,
    preferences_view: LazyCell<AsyncController<PreferencesWidget>, Box<dyn FnOnce() -> AsyncController<PreferencesWidget>>>,
    toaster: Toaster,
    mpris_player: Rc<LocalServer<MprisPlayer>>,
    random_songs_dialog: AsyncConnector<RandomSongsDialog>,
    artist_cache: ArtistCache,
    album_cache: AlbumCache,
    cover_cache: CoverCache,
    song_cache: SongCache,

    current_song_colors: Option<[Color; BG_COLORS]>,
    current_view_colors: Vec<Option<[Color; BG_COLORS]>>
}

#[derive(Debug)]
pub enum AppMsg {
    CurrentColorschemeChange(Option<Vec<Color>>),
    PushViewColors(Option<Vec<Color>>),
    PopViewColors,
    ClearViewColors,
    SetViewColors(Option<Vec<Color>>),
    ToggleSidebar,
    Quit,
    ShowPreferences,
    RestartRequest(String),
    Restart,
    ReloadPlayer,
    ShowError(String, String),
    PlayPause,
    Next,
    Previous,
    CloseRequest,
    ShowSong,
    Search,
    ViewAlbum(String, Option<u32>),
    ViewSong(String),
    ShowRandomSongsDialog,
    ViewArtist(String),
}

pub type Init = (
    CoverCache,
    SongCache,
    AlbumCache,
    Settings,
    Schema,
    LyricsCache,
    Rc<LocalServer<MprisPlayer>>,
    ArtistCache,
    SuperCache,
    Breakpoint
);

pub type StartInit = (
    CoverCache,
    SongCache,
    AlbumCache,
    Settings,
    Schema,
    LyricsCache,
    Receiver<Rc<LocalServer<MprisPlayer>>>,
    ArtistCache,
    SuperCache,
);

fn into_init(value: &StartInit, server: Rc<LocalServer<MprisPlayer>>, breakpoint: Breakpoint) -> Init {
    let value = value.clone();
    (value.0, value.1, value.2, value.3, value.4, value.5, server, value.7, value.8, breakpoint).into()
}

relm4::new_action_group!(pub WindowActionGroup, "win");
relm4::new_stateless_action!(AboutAction, WindowActionGroup, "about");
relm4::new_stateless_action!(PreferencesAction, WindowActionGroup, "preferences");
relm4::new_stateless_action!(QuitAction, WindowActionGroup, "quit");
relm4::new_stateless_action!(pub ShowTracklistAction, WindowActionGroup, "tracklist");
relm4::new_stateless_action!(pub ShowRandomSongsAction, WindowActionGroup, "randoms");
relm4::new_stateless_action!(pub PlayPauseAction, WindowActionGroup, "playpause");
relm4::new_stateless_action!(pub NextAction, WindowActionGroup, "next");
relm4::new_stateless_action!(pub PreviousAction, WindowActionGroup, "previous");
relm4::new_stateful_action!(pub ViewArtistAction, WindowActionGroup, "artist", String, u8);
relm4::new_stateful_action!(pub ViewAlbumAction, WindowActionGroup, "album", String, u8);
relm4::new_stateful_action!(pub ViewSongAction, WindowActionGroup, "song", String, u8);

#[relm4::component(pub async)]
impl AsyncComponent for Model {
    type CommandOutput = ();
    type Input = AppMsg;
    type Output = ();
    type Init = StartInit;

    view! {
        #[name = "window"]
        adw::ApplicationWindow {
            set_title: Some("Sanic-RS"),
            set_default_width: 400,
            set_default_height: 400,
            connect_close_request[sender] => move |_| {
                sender.input(AppMsg::CloseRequest);
                glib::Propagation::Proceed
            },

            #[name = "split_view"]
            adw::OverlaySplitView{
                add_css_class: "no-bg",
                set_collapsed: true,

                #[local_ref]
                #[wrap(Some)]
                set_content = toast_overlay -> adw::ToastOverlay {
                    set_vexpand: true,

                    #[name = "nav_view"]
                    adw::NavigationView {
                        adw::NavigationPage {
                            set_title: "Browse",
                            set_tag: Some("base"),
                            #[wrap(Some)]
                            set_child = &adw::ToolbarView {
                                #[wrap(Some)]
                                set_content = &gtk::Box {
                                    set_orientation: gtk::Orientation::Vertical,
                                    set_hexpand: true,
                                    add_css_class: "current-view",

                                    append = model.browse_connector.widget(),
                                    append = &gtk::Separator{
                                        set_orientation: gtk::Orientation::Vertical,
                                    },
                                    append = model.bottom_bar_connector.widget(),
                                },

                                #[template]
                                add_top_bar = &HeaderBar{
                                    #[template_child]
                                    header_bar {
                                        #[name = "search_bar"]
                                        #[wrap(Some)]
                                        set_title_widget = &gtk::SearchBar {
                                            #[wrap(Some)]
                                            set_child = &gtk::Box {
                                                set_orientation: Orientation::Horizontal,
                                                set_spacing: 5,

                                                #[name = "search_type"]
                                                gtk::DropDown {
                                                    set_enable_search: false,
                                                    set_selected: 0,
                                                    set_model: Some(&gtk::StringList::new(&["Song", "Album", "Artist"])),
                                                    connect_selected_notify => AppMsg::Search,
                                                },
                                                #[name = "search_entry"]
                                                gtk::SearchEntry {
                                                    connect_activate => AppMsg::Search,
                                                }
                                            }
                                        }
                                    }
                                }
                            },
                        },
                        adw::NavigationPage {
                            set_title: "Current song",
                            set_tag: Some("current"),
                            #[wrap(Some)]
                            set_child = &adw::ToolbarView {
                                #[wrap(Some)]
                                set_content = model.current_song.widget(),

                                #[template]
                                add_top_bar = &HeaderBar{}
                            },
                        }
                    },
                },

                #[wrap(Some)]
                set_sidebar = model.track_list_connector.widget(),
            },
        }
    }

    async fn init(
        init: Self::Init,
        root: adw::ApplicationWindow,
        sender: AsyncComponentSender<Self>,
    ) -> AsyncComponentParts<Self> {
        let rec = init.6.clone();
        let server = rec.recv().await.expect("Error receiving MPRIS server");
        server.imp().app_sender.replace(Some(sender.clone()));


        let breakpoint = adw::Breakpoint::new(
            adw::BreakpointCondition::new_length(BreakpointConditionLengthType::MaxWidth, 650.0, LengthUnit::Px)
        );
        let current_song =
            CurrentSong::builder()
                .launch(into_init(&init, server.clone(), breakpoint.clone()))
                .forward(sender.input_sender(), |msg| match msg {
                    CurrentSongOut::ColorSchemeChange(colors) => AppMsg::CurrentColorschemeChange(colors),
                });
        let track_list_connector = TrackListWidget::builder().launch(into_init(&init, server.clone(), breakpoint.clone()));
        let browse_connector = BrowseWidget::builder()
            .launch(into_init(&init, server.clone(), breakpoint.clone()))
            .forward(sender.input_sender(), |msg| match msg {
                BrowseMsgOut::PopView => AppMsg::PopViewColors,
                BrowseMsgOut::ClearView => AppMsg::ClearViewColors,
                BrowseMsgOut::SetColors(c) => AppMsg::SetViewColors(c),
            });
        let bottom_bar_connector= BottomBar::builder()
            .launch(into_init(&init, server.clone(), breakpoint.clone()))
            .forward(sender.input_sender(), |msg| match msg {
                BottomBarOut::ShowSong => AppMsg::ShowSong,
            });
        let random_songs_dialog = RandomSongsDialog::builder().launch((server.clone(), init.3.clone()));
        let preferences_view: LazyCell<AsyncController<PreferencesWidget>, Box<dyn FnOnce() -> AsyncController<PreferencesWidget>>>
            = LazyCell::new(Box::new(clone!(
                #[strong(rename_to=settings)]
                init.3,
                #[strong(rename_to=schema)]
                init.4,
                #[strong]
                sender,
                move || {
            PreferencesWidget::builder()
                .launch((settings, schema))
                .forward(sender.input_sender(), move |msg| {
                    match msg {
                        PreferencesOut::Restart => AppMsg::RestartRequest(
                            "In order for the changes to take effect, the app needs to be restarted.".to_string()
                        ),
                        PreferencesOut::ReloadPlayer => AppMsg::ReloadPlayer
                    }
                })
        })));
        let model = Model {
            current_song,
            track_list_connector,
            browse_connector,
            bottom_bar_connector,
            random_songs_dialog,
            provider: CssProvider::new(),
            settings: init.3,
            preferences_view,
            toaster: Toaster::default(),
            mpris_player: server,
            artist_cache: init.7,
            album_cache: init.2,
            cover_cache: init.0,
            current_song_colors: None,
            current_view_colors: vec![],
            song_cache: init.1,
        };
        let base_provider = CssProvider::new();
        let display = gdk::Display::default().expect("Unable to create Display object");
        base_provider.load_from_string(include_str!("../css/style.css"));
        gtk::style_context_add_provider_for_display(
            &display,
            &base_provider,
            gtk::STYLE_PROVIDER_PRIORITY_SETTINGS,
        );
        gtk::style_context_add_provider_for_display(
            &display,
            &model.provider,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );

        let toast_overlay = model.toaster.overlay_widget();

        let widgets: ModelWidgets = view_output!();

        let about_action: RelmAction<AboutAction> = RelmAction::new_stateless(clone!(
            move |_| {
                adw::AboutDialog::builder()
                    .application_name("Sanic-RS")
                    .application_icon(APP_ID)
                    .version(VERSION_STR)
                    .issue_url("https://github.com/Quartzy0/sanicrs/issues")
                    .license_type(License::Gpl30)
                    .developer_name("Quartzy")
                    .build()
                    .present(relm4::main_adw_application().active_window().as_ref())
            }
        ));
        let action: RelmAction<PreferencesAction> = RelmAction::new_stateless(clone!(
            #[strong]
            sender,
            move |_| {
                sender.input(AppMsg::ShowPreferences);
            }
        ));
        let quit_action: RelmAction<QuitAction> = RelmAction::new_stateless(clone!(
            #[strong]
            sender,
            move |_| {
                sender.input(AppMsg::Quit);
            }
        ));
        let show_tracklist_action: RelmAction<ShowTracklistAction> = RelmAction::new_stateless(clone!(
            #[strong]
            sender,
            move |_| {
                sender.input(AppMsg::ToggleSidebar);
            }
        ));
        let show_randoms_action: RelmAction<ShowRandomSongsAction> = RelmAction::new_stateless(clone!(
            #[strong]
            sender,
            move |_| {
                sender.input(AppMsg::ShowRandomSongsDialog);
            }
        ));
        let playpause_action: RelmAction<PlayPauseAction> = RelmAction::new_stateless(clone!(
            #[strong]
            sender,
            move |_| {
                sender.input(AppMsg::PlayPause);
            }
        ));
        relm4::main_application().set_accelerators_for_action::<PlayPauseAction>(&["space"]);
        let view_artist_action: RelmAction<ViewArtistAction> = RelmAction::new_stateful_with_target_value(&0, clone!(
            #[strong]
            sender,
            move |_, _state, value| {
                sender.input(AppMsg::ViewArtist(value));
            }
        ));
        let view_album_action: RelmAction<ViewAlbumAction> = RelmAction::new_stateful_with_target_value(&0, clone!(
            #[strong]
            sender,
            move |_, _state, value| {
                sender.input(AppMsg::ViewAlbum(value, None));
            }
        ));
        let view_song_action: RelmAction<ViewSongAction> = RelmAction::new_stateful_with_target_value(&0, clone!(
            #[strong]
            sender,
            move |_, _state, value| {
                sender.input(AppMsg::ViewSong(value));
            }
        ));
        let next_action: RelmAction<NextAction> = RelmAction::new_stateless(clone!(
            #[strong]
            sender,
            move |_| {
                sender.input(AppMsg::Next);
            }
        ));
        let previous_action: RelmAction<PreviousAction> = RelmAction::new_stateless(clone!(
            #[strong]
            sender,
            move |_| {
                sender.input(AppMsg::Previous);
            }
        ));

        let mut group = RelmActionGroup::<WindowActionGroup>::new();
        group.add_action(about_action);
        group.add_action(action);
        group.add_action(quit_action);
        group.add_action(show_tracklist_action);
        group.add_action(show_randoms_action);
        group.add_action(playpause_action);
        group.add_action(view_artist_action);
        group.add_action(view_album_action);
        group.add_action(view_song_action);
        group.add_action(next_action);
        group.add_action(previous_action);
        group.register_for_widget(&root);

        widgets.search_bar.connect_entry(&widgets.search_entry);
        widgets.search_bar.set_key_capture_widget(Some(&root));
        root.add_breakpoint(breakpoint);

        let focus_controller = gtk::EventControllerFocus::new();
        focus_controller.connect_enter(move |_| {
            relm4::main_application().set_accelerators_for_action::<PlayPauseAction>(&[]);
        });
        focus_controller.connect_leave(move |_| {
            relm4::main_application().set_accelerators_for_action::<PlayPauseAction>(&["space"]);
        });
        widgets.search_entry.add_controller(focus_controller);

        AsyncComponentParts { model, widgets }
    }

    async fn update_with_view(
        &mut self,
        widgets: &mut Self::Widgets,
        message: Self::Input,
        sender: AsyncComponentSender<Self>,
        root: &adw::ApplicationWindow,
    ) {
        let player = self.mpris_player.imp();
        match message {
            AppMsg::Next => player.next().await.unwrap(),
            AppMsg::Previous => player.previous().await.unwrap(),
            AppMsg::PlayPause => player.play_pause().await.unwrap(),
            AppMsg::PushViewColors(colors) => {
                let colors = Self::vec_to_arr(colors);
                self.current_view_colors.push(colors);
                self.update_colors(root);
            }
            AppMsg::PopViewColors => {
                self.current_view_colors.pop();
                self.update_colors(root);
            }
            AppMsg::ClearViewColors => {
                self.current_view_colors.clear();
                self.update_colors(root);
            }
            AppMsg::SetViewColors(colors) => {
                let colors = Self::vec_to_arr(colors);
                let len = self.current_view_colors.len();
                if len == 0 {
                    self.current_view_colors.push(colors);
                } else {
                    self.current_view_colors[len - 1] = colors;
                }
                self.update_colors(root);
            }
            AppMsg::CurrentColorschemeChange(colors) => {
                if let Some(color) = colors {
                    let mut it = color.into_iter().cycle();
                    let mut arr: [Color; BG_COLORS] = [Default::default(); BG_COLORS];
                    for i in 0..BG_COLORS {
                        if let Some(color) = it.next() {
                            arr[i] = color;
                        }
                    }
                    self.current_song_colors = Some(arr);
                } else {
                    self.current_song_colors = None;
                }
                self.update_colors(root);
            },
            AppMsg::ToggleSidebar => {
                widgets.split_view.set_show_sidebar(true);
            },
            AppMsg::Quit => {
                player.quit_no_app().await;
                if let Some(app) = root.application() {
                    app.quit();
                }
            },
            AppMsg::ShowPreferences => {
                self.preferences_view.widget().present(Some(root));
            },
            AppMsg::RestartRequest(reason) => {
                let dialog = adw::AlertDialog::new(Some("Restart required"), Some(reason.as_str()));
                dialog.add_responses(&[("now", "Restart"), ("later", "Later")]);
                dialog.set_default_response(Some("now"));
                dialog.set_response_appearance("now", adw::ResponseAppearance::Destructive);
                dialog.set_close_response("later");
                dialog.choose(root, None::<&gio::Cancellable>, clone!(
                    #[strong(rename_to = sndr)]
                    sender,
                    move |response| {
                        if response == "now" {
                            sndr.input(AppMsg::Restart)
                        }
                    }
                ));
            },
            AppMsg::Restart => {
                player.restart().await;
            }
            AppMsg::ReloadPlayer => {
                let err = player.reload_settings();
                if err.is_err() {
                    let err = err.err().unwrap();
                    sender.input(AppMsg::ShowError(err.to_string(), format!("{:?}", err)))
                }
            },
            AppMsg::ShowError(summary, description) => {
                let str = format!("Error occurred: {}", summary);
                let toast = adw::Toast::builder()
                    .title(glib::markup_escape_text(&str))
                    .button_label("Details")
                    .timeout(8)
                    .build();
                eprintln!("Error occurred: {}", description);
                toast.connect_button_clicked(clone!(
                    #[strong]
                    root,
                    move |_this| {
                        let dialog = adw::AlertDialog::new(Some("Error details"), Some(description.as_str()));
                        dialog.add_responses(&[("ok", "Ok")]);
                        dialog.set_default_response(Some("ok"));
                        dialog.set_close_response("ok");
                        dialog.choose(&root, None::<&gio::Cancellable>, move |_| {});
                    }
                ));
                self.toaster.add_toast(toast);
            },
            AppMsg::CloseRequest => {
                if !self.settings.boolean("stay-in-background") {
                    sender.input(AppMsg::Quit);
                } {
                    player.close().await;

                }
            },
            AppMsg::ShowSong => {
                widgets.nav_view.push_by_tag("current");
            },
            AppMsg::Search => {
                let search_type = match widgets.search_type.selected() {
                    0 => SearchType::Song,
                    1 => SearchType::Album,
                    2 => SearchType::Artist,
                    _ => {
                        player.send_error("Invalid search type".into());
                        SearchType::Song
                    },
                };
                self.browse_connector.emit(BrowseMsg::Search(widgets.search_entry.text().into(), search_type));
            },
            AppMsg::ViewAlbum(album, highlight) => {
                match self.album_cache.get_album(&album).await {
                    Ok(album) => {
                        widgets.nav_view.pop_to_tag("base");
                        relm4::spawn_local(clone!(
                            #[strong]
                            sender,
                            #[strong(rename_to = mpris_player)]
                            self.mpris_player,
                            #[strong(rename_to = id)]
                            album.cover_art_id(),
                            #[strong(rename_to = cover_cache)]
                            self.cover_cache,
                            async move {
                                if let Some(id) = id {
                                    match cover_cache.get_palette(id.as_str()).await {
                                        Ok(c) => sender.input(AppMsg::PushViewColors(c)),
                                        Err(err) => mpris_player.imp().send_error(err),
                                    }
                                }
                            }
                        ));
                        self.browse_connector.emit(BrowseMsg::ViewAlbum(album, highlight));
                    },
                    Err(err) => self.mpris_player.imp().send_error(err),
                };
            },
            AppMsg::ViewSong(song) => {
                match self.song_cache.get_song(&song).await {
                    Ok(song) => {
                        if let Some(album_id) = &song.album_id {
                            sender.input(AppMsg::ViewAlbum(album_id.clone(), song.track.and_then(|v| Some(v as u32 - 1))));
                        } else {
                            self.mpris_player.imp().send_error("Song has no album_id".into());
                        }
                    }
                    Err(err) => self.mpris_player.imp().send_error(err),
                }
            }
            AppMsg::ShowRandomSongsDialog => {
                self.random_songs_dialog.widget().present(Some(root));
            },
            AppMsg::ViewArtist(artist) => {
                widgets.nav_view.pop_to_tag("base");
                match self.artist_cache.get_artist(&artist).await {
                    Ok(artist) => {
                        relm4::spawn_local(clone!(
                            #[strong]
                            sender,
                            #[strong(rename_to = mpris_player)]
                            self.mpris_player,
                            #[strong(rename_to = id)]
                            artist.cover_art_id(),
                            #[strong(rename_to = cover_cache)]
                            self.cover_cache,
                            async move {
                                if let Some(id) = id {
                                    match cover_cache.get_palette(id.as_str()).await {
                                        Ok(c) => sender.input(AppMsg::PushViewColors(c)),
                                        Err(err) => mpris_player.imp().send_error(err),
                                    }
                                }
                            }
                        ));
                        self.browse_connector.emit(BrowseMsg::ViewArtist(artist))
                    },
                    Err(err) => self.mpris_player.imp().send_error(err),
                };
            },
        };
        self.update_view(widgets, sender);
    }
}

impl Model {
    fn vec_to_arr(colors: Option<Vec<Color>>) -> Option<[Color; BG_COLORS]> {
        if let Some(color) = colors {
            let mut it = color.into_iter().cycle();
            let mut arr: [Color; BG_COLORS] = [Default::default(); BG_COLORS];
            for i in 0..BG_COLORS {
                if let Some(color) = it.next() {
                    arr[i] = color;
                }
            }
            Some(arr)
        } else {
            None
        }
    }

    fn update_colors(&self, root: &<Model as AsyncComponent>::Root) {
        let mut css = String::from(":root {");
        if let Some(colors) = self.current_song_colors {
            for (i, color) in colors.iter().enumerate() {
                css.push_str(
                    format!(
                        "--song-color-{}:rgb({},{},{});",
                        i, color.r, color.g, color.b
                    )
                        .as_str(),
                );
            }
        }
        if let Some(colors) = self.current_view_colors.last() &&  let Some(colors) = colors {
            for (i, color) in colors.iter().enumerate() {
                css.push_str(
                    format!(
                        "--view-color-{}:rgb({},{},{});",
                        i, color.r, color.g, color.b
                    )
                        .as_str(),
                );
            }
        }
        css.push_str("}");
        self.provider
            .load_from_string(css.as_str());
        root.action_set_enabled("win.enable-recoloring", true);
    }
}
