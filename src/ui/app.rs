use crate::dbus::player::MprisPlayer;
use crate::opensonic::cache::{AlbumCache, CoverCache, LyricsCache, SongCache};
use crate::ui::browse::BrowseWidget;
use crate::ui::current_song::{CurrentSong, CurrentSongOut};
use crate::ui::preferences_view::{PreferencesOut, PreferencesWidget};
use crate::ui::track_list::TrackListWidget;
use crate::icon_names;
use async_channel::Receiver;
use color_thief::Color;
use gtk::prelude::GtkWindowExt;
use libsecret::Schema;
use mpris_server::{LocalPlayerInterface, LocalServer};
use relm4::abstractions::Toaster;
use relm4::actions::{AccelsPlus, RelmAction, RelmActionGroup};
use relm4::adw::glib as glib;
use relm4::adw::glib::{clone, closure};
use relm4::adw::gtk::CssProvider;
use relm4::adw::prelude::*;
use relm4::adw::{gdk, ViewSwitcherPolicy};
use relm4::component::AsyncConnector;
use relm4::gtk::gio::{self, Settings};
use relm4::gtk::Widget;
use relm4::prelude::*;
use relm4::{adw, component::{AsyncComponent, AsyncComponentParts, AsyncComponentSender}};
use std::rc::Rc;

pub struct Model {
    current_song: AsyncController<CurrentSong>,
    track_list_connector: AsyncConnector<TrackListWidget>,
    browse_connector: AsyncConnector<BrowseWidget>,
    provider: CssProvider,
    settings: Settings,
    schema: Schema,
    preferences_view: Option<AsyncController<PreferencesWidget>>,
    toaster: Toaster,
    mpris_player: Rc<LocalServer<MprisPlayer>>
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

                #[wrap(Some)]
                set_content = &adw::ToolbarView {
                    #[local_ref]
                    #[wrap(Some)]
                    set_content = toast_overlay -> adw::ToastOverlay {
                        set_vexpand: true,

                        #[name = "view_stack"]
                        adw::ViewStack {
                            add = model.current_song.widget(),
                            add = model.browse_connector.widget(),
                        }

                        // gtk::Box {
                        //     set_orientation: gtk::Orientation::Vertical,
                        //     set_hexpand: true,
                        //
                        //     append = model.current_song.widget(),
                        //     append = &gtk::Separator{
                        //         set_orientation: gtk::Orientation::Vertical,
                        //     },
                        //     append = &gtk::Box {
                        //         set_orientation: gtk::Orientation::Horizontal,
                        //         gtk::Label {
                        //             set_label: "Hello"
                        //         }
                        //     }
                        // }
                    },

                    #[name = "header_bar"]
                    add_top_bar = &adw::HeaderBar {
                        #[name = "view_switcher"]
                        #[wrap(Some)]
                        set_title_widget = &adw::ViewSwitcher {
                            set_policy: ViewSwitcherPolicy::Wide,
                        },
                        set_show_end_title_buttons: true,
                        pack_end = &gtk::MenuButton {
                            set_icon_name: icon_names::MENU,

                            #[wrap(Some)]
                            set_menu_model = &gio::Menu {
                                append_item = &gio::MenuItem::new(Some("Preferences"), Some("win.preferences")),
                            }
                        }
                    },

                    #[name = "view_switcher_bar"]
                    add_bottom_bar = &adw::ViewSwitcherBar {

                    }
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
                });
        let track_list_connector = TrackListWidget::builder().launch(into_init(&init, server.clone()));
        let browse_connector = BrowseWidget::builder().launch(into_init(&init, server.clone()));
        let model = Model {
            current_song,
            track_list_connector,
            browse_connector,
            provider: CssProvider::new(),
            settings: init.3,
            schema: init.4,
            preferences_view: None,
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

        let condition = adw::BreakpointCondition::parse("max-width: 1000px").unwrap();
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
            .bind(&widgets.split_view, "show-sidebar", Some(&widgets.split_view));

        let sndr = sender.clone();
        let action: RelmAction<PreferencesAction> = RelmAction::new_stateless(move |_| {
            sender.input(AppMsg::ShowPreferences);
        });
        let playpause_action: RelmAction<PlayPauseAction> = RelmAction::new_stateless(move |_| {
            sndr.input(AppMsg::PlayPause);
        });
        relm4::main_application().set_accelerators_for_action::<PlayPauseAction>(&["space"]);

        let mut group = RelmActionGroup::<WindowActionGroup>::new();
        group.add_action(action);
        group.add_action(playpause_action);
        group.register_for_widget(&root);

        AsyncComponentParts { model, widgets }
    }

    async fn update_with_view(
        &mut self,
        widgets: &mut Self::Widgets,
        message: Self::Input,
        sender: AsyncComponentSender<Self>,
        root: &adw::ApplicationWindow,
    ) {
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
                self.mpris_player.imp().quit_no_app().await;
                if let Some(app) = root.application() {
                    app.quit();
                }
            },
            AppMsg::ShowPreferences => {
                if self.preferences_view.is_none() {
                    let view = PreferencesWidget::builder()
                        .launch((self.settings.clone(), self.schema.clone()))
                        .forward(sender.input_sender(), move |msg| {
                            match msg {
                                PreferencesOut::Restart => AppMsg::RestartRequest(
                                    "In order for the changes to take effect, the app needs to be restarted.".to_string()
                                ),
                                PreferencesOut::ReloadPlayer => AppMsg::ReloadPlayer
                            }
                        });
                    self.preferences_view = Some(view);
                }
                self.preferences_view.as_ref().unwrap().widget().present(Some(root));
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
                self.mpris_player.imp().restart().await;
            }
            AppMsg::ReloadPlayer => {
                let err = self.mpris_player.imp().reload_settings().await;
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
                self.mpris_player.imp().play_pause().await.unwrap();
            },
            AppMsg::CloseRequest => {
                if !self.settings.boolean("stay-in-background") {
                    sender.input(AppMsg::Quit);
                } {
                    self.mpris_player.imp().close().await;

                }
            }
        };
        self.update_view(widgets, sender);
    }
}
