use crate::icon_names;
use crate::mpris::MprisPlayer;
use crate::opensonic::client::OpenSubsonicClient;
use crate::player::TrackList;
use crate::ui::current_song::CurrentSong;
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
use tokio::sync::RwLock;
use zbus::object_server::InterfaceRef;
use crate::ui::track_list::TrackListWidget;

pub struct Model {
    track_list: Arc<RwLock<TrackList>>,
    sender: AsyncComponentSender<Self>,
    current_song: AsyncConnector<CurrentSong>,
    track_list_connector: AsyncConnector<TrackListWidget>,
}

#[derive(Debug)]
pub enum AppMsg {

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
        relm4::set_global_css(include_str!("../css/style.css"));
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
        root: Self::Root,
        sender: AsyncComponentSender<Self>,
    ) -> AsyncComponentParts<Self> {
        let current_song = CurrentSong::builder()
            .launch(init.clone());
        let track_list_connector = TrackListWidget::builder()
            .launch(init.clone());
        let model = Model {
            track_list: init.1,
            sender: sender.clone(),
            current_song,
            track_list_connector
        };
        let widgets = view_output!();

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
            
        };
        self.update_view(widgets, sender);
    }
}
