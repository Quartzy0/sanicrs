use crate::dbus::track_list::MprisTrackList;
use crate::opensonic::client::OpenSubsonicClient;
use crate::player::{PlayerInfo, TrackList};
use crate::ui::current_song::{CurrentSong, CurrentSongOut};
use crate::ui::track_list::TrackListWidget;
use crate::{PlayerCommand, icon_names};
use color_thief::Color;
use gtk::prelude::GtkWindowExt;
use readlock_tokio::SharedReadLock;
use relm4::adw::gdk;
use relm4::adw::prelude::*;
use relm4::component::AsyncConnector;
use relm4::gtk::CssProvider;
use relm4::prelude::*;
use relm4::{
    RelmApp, adw,
    component::{AsyncComponent, AsyncComponentParts, AsyncComponentSender},
};
use std::sync::Arc;
use std::thread;
use tokio::sync::RwLock;
use tokio::sync::mpsc::UnboundedSender;
use zbus::object_server::InterfaceRef;

pub struct Model {
    current_song: AsyncController<CurrentSong>,
    track_list_connector: AsyncConnector<TrackListWidget>,
    provider: CssProvider,
}

#[derive(Debug)]
pub enum AppMsg {
    ColorschemeChange(Option<Vec<Color>>),
    Quit,
}

pub type Init = (
    SharedReadLock<PlayerInfo>,
    Arc<RwLock<TrackList>>,
    Arc<OpenSubsonicClient>,
    InterfaceRef<MprisTrackList>,
    Arc<UnboundedSender<PlayerCommand>>,
);

pub fn start_app(init: Init) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        let app = RelmApp::new("me.quartzy.sanicrs");
        relm4_icons::initialize_icons(icon_names::GRESOURCE_BYTES, icon_names::RESOURCE_PREFIX);
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
                    add = &adw::OverlaySplitView{
                        // set_collapsed: true,
                        // set_show_sidebar: true,

                        #[wrap(Some)]
                        set_content = model.current_song.widget(),

                        #[wrap(Some)]
                        set_sidebar = model.track_list_connector.widget(),
                    }
                }
            }
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
                });
        let track_list_connector = TrackListWidget::builder().launch(init.clone());
        let model = Model {
            current_song,
            track_list_connector,
            provider: CssProvider::new(),
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

        init.4.send(PlayerCommand::AppSendSender(sender)).expect("Error sending sender to app");

        let widgets = view_output!();

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
            AppMsg::Quit => root.application().expect("Error getting GIO application").quit(),
        };
        self.update_view(widgets, sender);
    }
}
