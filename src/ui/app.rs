use crate::{icon_names, PlayerCommand};
use crate::opensonic::client::OpenSubsonicClient;
use crate::player::{PlayerInfo, TrackList};
use crate::ui::current_song::{CurrentSong, CurrentSongOut};
use gtk::prelude::GtkWindowExt;
use relm4::adw::prelude::*;
use relm4::component::AsyncConnector;
use relm4::prelude::*;
use relm4::{
    adw, component::{AsyncComponent, AsyncComponentParts, AsyncComponentSender},
    RelmApp,
};
use std::sync::Arc;
use std::thread;
use readlock_tokio::SharedReadLock;
use relm4::adw::gdk;
use relm4::gtk::CssProvider;
use tokio::sync::mpsc::UnboundedSender;
use tokio::sync::RwLock;
use zbus::object_server::InterfaceRef;
use crate::dbus::track_list::MprisTrackList;
use crate::ui::track_list::TrackListWidget;

pub struct Model {
    current_song: AsyncController<CurrentSong>,
    track_list_connector: AsyncConnector<TrackListWidget>,
    provider: CssProvider,
}

#[derive(Debug)]
pub enum AppMsg {
    ColorschemeChange,
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
            add_css_class: "main-window",

            adw::ToolbarView {
                #[wrap(Some)]
                set_content = &adw::ViewStack {
                    add = &adw::OverlaySplitView{
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
        let current_song = CurrentSong::builder()
            .launch(init.clone())
            .forward(sender.input_sender(), |msg| match msg {
            CurrentSongOut::ColorSchemeChange => AppMsg::ColorschemeChange,
        });
        let track_list_connector = TrackListWidget::builder()
            .launch(init.clone());
        let model = Model {
            current_song,
            track_list_connector,
            provider: CssProvider::new()
        };
        let base_provider = CssProvider::new();
        let display = gdk::Display::default().expect("Unable to create Display object");
        base_provider.load_from_string(include_str!("../css/style.css"));
        gtk::style_context_add_provider_for_display(&display, &base_provider, gtk::STYLE_PROVIDER_PRIORITY_SETTINGS);
        // gtk::style_context_add_provider_for_display(&display, &model.provider, gtk::STYLE_PROVIDER_PRIORITY_APPLICATION);

        let widgets = view_output!();

        /*model.provider.load_from_string(":root {--background-color-0: #ffffffff;}");
        root.action_set_enabled("win.enable-recoloring", true);*/

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
            AppMsg::ColorschemeChange => {

            },
        };
        self.update_view(widgets, sender);
    }
}
