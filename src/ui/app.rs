use crate::player::{PlayerInfo, TrackList};
use crate::ui::current_song::{CurrentSong, CurrentSongOut};
use crate::ui::preferences_view::{PreferencesOut, PreferencesWidget};
use crate::ui::track_list::TrackListWidget;
use crate::{icon_names, PlayerCommand};
use color_thief::Color;
use gtk::prelude::GtkWindowExt;
use libsecret::Schema;
use readlock_tokio::SharedReadLock;
use relm4::actions::{RelmAction, RelmActionGroup};
use relm4::adw::{gdk, ViewSwitcherPolicy};
use relm4::adw::prelude::*;
use relm4::component::AsyncConnector;
use relm4::adw::gtk::CssProvider;
use relm4::gtk::gio::{self, Settings};
use relm4::prelude::*;
use relm4::{
    adw,
    component::{AsyncComponent, AsyncComponentParts, AsyncComponentSender},
};
use std::sync::Arc;
use async_channel::Sender;
use relm4::adw::glib::{clone, closure};
use tokio::sync::RwLock;
use crate::ui::browse::BrowseWidget;
use relm4::adw::glib as glib;
use relm4::gtk::Widget;
use crate::opensonic::cache::{AlbumCache, CoverCache, SongCache};

pub struct Model {
    current_song: AsyncController<CurrentSong>,
    track_list_connector: AsyncConnector<TrackListWidget>,
    browse_connector: AsyncConnector<BrowseWidget>,
    provider: CssProvider,
    settings: Settings,
    schema: Schema,
    cmd_sender: Arc<Sender<PlayerCommand>>,
    preferences_view: Option<AsyncController<PreferencesWidget>>,
}

#[derive(Debug)]
pub enum AppMsg {
    ColorschemeChange(Option<Vec<Color>>),
    ToggleSidebar,
    Quit,
    ShowPreferences,
    Restart(String),
}

pub type Init = (
    SharedReadLock<PlayerInfo>,
    Arc<RwLock<TrackList>>,
    CoverCache,
    Arc<Sender<PlayerCommand>>,
    SongCache,
    AlbumCache,
    Settings,
    Schema,
);

relm4::new_action_group!(WindowActionGroup, "win");
relm4::new_stateless_action!(PreferencesAction, WindowActionGroup, "preferences");

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

            #[name = "split_view"]
            adw::OverlaySplitView{
                add_css_class: "no-bg",

                #[wrap(Some)]
                set_content = &adw::ToolbarView {
                    #[name = "view_stack"]
                    #[wrap(Some)]
                    set_content = &adw::ViewStack {
                        add = model.current_song.widget(),
                        add = model.browse_connector.widget(),
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
        let current_song =
            CurrentSong::builder()
                .launch(init.clone())
                .forward(sender.input_sender(), |msg| match msg {
                    CurrentSongOut::ColorSchemeChange(colors) => AppMsg::ColorschemeChange(colors),
                    CurrentSongOut::ToggleSidebar => AppMsg::ToggleSidebar,
                });
        let track_list_connector = TrackListWidget::builder().launch(init.clone());
        let browse_connector = BrowseWidget::builder().launch(init.clone());
        let model = Model {
            current_song,
            track_list_connector,
            browse_connector,
            provider: CssProvider::new(),
            settings: init.6,
            schema: init.7,
            cmd_sender: init.3,
            preferences_view: None
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

        model.cmd_sender.send(PlayerCommand::AppSendSender(sender.clone())).await.expect("Error sending sender to app");

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

        let action: RelmAction<PreferencesAction> = RelmAction::new_stateless(move |_| {
            sender.input(AppMsg::ShowPreferences);
        });

        let mut group = RelmActionGroup::<WindowActionGroup>::new();
        group.add_action(action);
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
                                PreferencesOut::Restart => AppMsg::Restart(
                                    "In order for the changes to take effect, the app needs to be restarted.".to_string()
                                ),
                            }
                        });
                    self.preferences_view = Some(view);
                }
                self.preferences_view.as_ref().unwrap().widget().present(Some(root));
            },
            AppMsg::Restart(reason) => {
                let dialog = adw::AlertDialog::new(Some("Restart required"), Some(reason.as_str()));
                dialog.add_responses(&[("now", "Restart"), ("later", "Later")]);
                dialog.set_default_response(Some("now"));
                dialog.set_response_appearance("now", adw::ResponseAppearance::Destructive);
                dialog.set_close_response("later");
                dialog.choose(root, None::<&gio::Cancellable>, clone!(
                    #[strong(rename_to = sndr)]
                    self.cmd_sender,
                    move |response| {
                        if response == "now" {
                            sndr.send_blocking(PlayerCommand::Restart).expect("Error sending restart message to main");
                        }
                    }
                ));
            }
        };
        self.update_view(widgets, sender);
    }
}
