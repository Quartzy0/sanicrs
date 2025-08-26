use crate::dbus::player::MprisPlayer;
use crate::opensonic::cache::{AlbumCache, CoverCache, LyricsCache, SongCache};
use crate::ui::album_object::AlbumObject;
use crate::ui::browse::search::SearchType;
use crate::ui::browse::{BrowseMsg, BrowseWidget};
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
use relm4::adw::glib as glib;
use relm4::adw::glib::{clone};
use relm4::adw::gtk::CssProvider;
use relm4::adw::prelude::*;
use relm4::adw::{gdk};
use relm4::component::AsyncConnector;
use relm4::gtk::gio::{self, Settings};
use relm4::gtk::Orientation;
use relm4::prelude::*;
use relm4::{adw, component::{AsyncComponent, AsyncComponentParts, AsyncComponentSender}};
use std::cell::LazyCell;
use std::rc::Rc;
use crate::ui::bottom_bar::{BottomBar, BottomBarOut};
use crate::ui::header_bar::HeaderBar;

pub struct Model {
    current_song: AsyncController<CurrentSong>,
    track_list_connector: AsyncConnector<TrackListWidget>,
    bottom_bar_connector: AsyncController<BottomBar>,
    browse_connector: AsyncConnector<BrowseWidget>,
    provider: CssProvider,
    settings: Settings,
    preferences_view: LazyCell<AsyncController<PreferencesWidget>, Box<dyn FnOnce() -> AsyncController<PreferencesWidget>>>,
    toaster: Toaster,
    mpris_player: Rc<LocalServer<MprisPlayer>>,
    random_songs_dialog: AsyncConnector<RandomSongsDialog>,
}

#[derive(Debug)]
pub enum AppMsg {
    ColorschemeChange(Option<Vec<Color>>),
    ToggleSidebar,
    Quit,
    ShowPreferences,
    RestartRequest(String),
    Restart,
    ReloadPlayer,
    ShowError(String, String),
    PlayPause,
    CloseRequest,
    ShowSong,
    Search,
    ViewAlbum(AlbumObject),
    ShowRandomSongsDialog,
}

pub type Init = (
    CoverCache,
    SongCache,
    AlbumCache,
    Settings,
    Schema,
    LyricsCache,
    Rc<LocalServer<MprisPlayer>>
);

pub type StartInit = (
    CoverCache,
    SongCache,
    AlbumCache,
    Settings,
    Schema,
    LyricsCache,
    Receiver<Rc<LocalServer<MprisPlayer>>>
);

fn into_init(value: &StartInit, server: Rc<LocalServer<MprisPlayer>>) -> Init {
    let value = value.clone();
    (value.0, value.1, value.2, value.3, value.4, value.5, server).into()
}

relm4::new_action_group!(WindowActionGroup, "win");
relm4::new_stateless_action!(PreferencesAction, WindowActionGroup, "preferences");
relm4::new_stateless_action!(PlayPauseAction, WindowActionGroup, "playpause");

#[relm4::component(pub async)]
impl AsyncComponent for Model {
    type CommandOutput = ();
    type Input = AppMsg;
    type Output = ();
    type Init = StartInit;

    view! {
        #[name = "window"]
        adw::ApplicationWindow {
            set_title: Some("Sanic-rs"),
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
                                                    connect_activate => AppMsg::Search
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


        let current_song =
            CurrentSong::builder()
                .launch(into_init(&init, server.clone()))
                .forward(sender.input_sender(), |msg| match msg {
                    CurrentSongOut::ColorSchemeChange(colors) => AppMsg::ColorschemeChange(colors),
                    CurrentSongOut::ToggleSidebar => AppMsg::ToggleSidebar,
                    CurrentSongOut::ViewAlbum(album) => AppMsg::ViewAlbum(album),
                    CurrentSongOut::ShowRandomSongsDialog => AppMsg::ShowRandomSongsDialog
                });
        let track_list_connector = TrackListWidget::builder().launch(into_init(&init, server.clone()));
        let browse_connector = BrowseWidget::builder().launch(into_init(&init, server.clone()));
        let bottom_bar_connector= BottomBar::builder()
            .launch(into_init(&init, server.clone()))
            .forward(sender.input_sender(), |msg| match msg {
                BottomBarOut::ShowSong => AppMsg::ShowSong,
                BottomBarOut::ShowRandomSongsDialog => AppMsg::ShowRandomSongsDialog,
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

        /*let condition = adw::BreakpointCondition::parse("max-width: 1000px").unwrap();
        let breakpoint = adw::Breakpoint::new(condition.clone());
        breakpoint.add_setter(&widgets.view_switcher_bar, "reveal", Some(&true.to_value()));
        breakpoint.add_setter(&widgets.header_bar, "title-widget", Some(&None::<Widget>.to_value()));
        breakpoint.connect_apply(clone!(
            #[weak(rename_to = view_stack)]
            widgets.view_stack,
            #[weak(rename_to = split_view)]
            widgets.split_view,
            move |_| {
                if let Some(name) = view_stack.visible_child_name() {
                    if name == "Song" {
                        split_view.set_collapsed(true);
                    }
                }
            }
        ));
        breakpoint.connect_unapply(clone!(
            #[weak(rename_to = view_stack)]
            widgets.view_stack,
            #[weak(rename_to = split_view)]
            widgets.split_view,
            move |_| {
                if let Some(name) = view_stack.visible_child_name() {
                    if name == "Song" {
                        split_view.set_collapsed(false);
                    }
                }
            }
        ));
        root.add_breakpoint(breakpoint);

        let song_page = widgets.view_stack.page(model.current_song.widget());
        song_page.set_title(Some("Song"));
        song_page.set_name(Some("Song"));
        song_page.set_icon_name(Some(icon_names::MUSIC_NOTE));
        let browse_page = widgets.view_stack.page(model.browse_connector.widget());
        browse_page.set_title(Some("Browse"));
        browse_page.set_name(Some("Browse"));
        browse_page.set_icon_name(Some(icon_names::EXPLORE2));
        widgets.view_switcher.set_stack(Some(&widgets.view_stack));
        widgets.view_switcher_bar.set_stack(Some(&widgets.view_stack));

        widgets.view_stack
            .property_expression("visible-child-name")
            .chain_closure::<bool>(closure!(|this: Option<adw::OverlaySplitView>, name: Option<&str>| {
                name.is_some() && name.unwrap() == "Song" && !this.unwrap().is_collapsed()
            }))
            .bind(&widgets.split_view, "show-sidebar", Some(&widgets.split_view));*/

        let action: RelmAction<PreferencesAction> = RelmAction::new_stateless(clone!(
            #[strong]
            sender,
            move |_| {
                sender.input(AppMsg::ShowPreferences);
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

        let mut group = RelmActionGroup::<WindowActionGroup>::new();
        group.add_action(action);
        group.add_action(playpause_action);
        group.register_for_widget(&root);

        widgets.search_bar.connect_entry(&widgets.search_entry);
        widgets.search_bar.set_key_capture_widget(Some(&root));

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
            AppMsg::ColorschemeChange(colors) => {
                let mut css = String::from(":root {");
                if let Some(colors) = colors {
                    for (i, color) in colors.iter().enumerate() {
                        css.push_str(
                            format!(
                                "--background-color-{}:rgb({},{},{});",
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
                let toast = adw::Toast::builder()
                    .title(format!("Error occurred: {}", summary))
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
            AppMsg::PlayPause => {
                player.play_pause().await.unwrap();
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
            AppMsg::ViewAlbum(album) => {
                widgets.nav_view.pop_to_tag("base");
                self.browse_connector.emit(BrowseMsg::ViewAlbum(album));
            },
            AppMsg::ShowRandomSongsDialog => {
                self.random_songs_dialog.widget().present(Some(root));
            }
        };
        self.update_view(widgets, sender);
    }
}
