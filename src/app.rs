use crate::icon_names;
use crate::mpris::MprisPlayer;
use crate::opensonic::types::Song;
use crate::player::TrackList;
use gtk::prelude::GtkWindowExt;
use relm4::SimpleComponent;
use relm4::adw::prelude::*;
use relm4::gtk::prelude::OrientableExt;
use relm4::gtk::{Align, Orientation};
use relm4::prelude::*;
use relm4::{
    RelmApp, adw,
    component::{AsyncComponent, AsyncComponentParts, AsyncComponentSender},
    gtk,
};
use std::sync::Arc;
use std::thread;
use tokio::sync::RwLock;
use zbus::object_server::InterfaceRef;

#[derive(Debug)]
pub struct SongInfo {
    song_title: String,
    song_artist: String,
    song_album: String,
}

impl From<&Song> for SongInfo {
    fn from(value: &Song) -> Self {
        SongInfo {
            song_title: value.title.clone(),
            song_artist: value
                .display_artists
                .clone()
                .unwrap_or(value.artist.clone().unwrap_or("Unknown Artist".to_string())),
            song_album: value.album.clone().unwrap_or("Unknown Album".to_string()),
        }
    }
}

impl Default for SongInfo{
    fn default() -> Self {
        SongInfo{
            song_album: "".to_string(),
            song_artist: "".to_string(),
            song_title: "".to_string(),
        }
    }
}

pub struct Model {
    player_reference: InterfaceRef<MprisPlayer>,
    track_list: Arc<RwLock<TrackList>>,
    sender: AsyncComponentSender<Self>,

    // UI data
    song_info: SongInfo,
    playback_state: String,
}

#[derive(Debug)]
pub enum AppMsg {
    PlayPause,
    Start,
    Next,
    Previous,
    SongUpdate(SongInfo),
}

type Init = (InterfaceRef<MprisPlayer>, Arc<RwLock<TrackList>>);

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
                    add = &gtk::Box {
                        set_orientation: Orientation::Vertical,
                        set_halign: Align::Center,
                        set_valign: Align::Center,
                        set_spacing: 5,

                        #[name = "cover_image"]
                        gtk::Image {},
                        gtk::Label {
                            #[watch]
                            set_label: &model.song_info.song_title
                        },
                        gtk::Label {
                            #[watch]
                            set_label: &model.song_info.song_artist
                        },
                        gtk::Label {
                            #[watch]
                            set_label: &model.song_info.song_album
                        },

                        gtk::Box {
                            set_orientation: Orientation::Horizontal,
                            set_halign: Align::Center,
                            set_spacing: 10,

                            gtk::Button {
                                set_icon_name: icon_names::PREVIOUS_REGULAR,
                                connect_clicked => AppMsg::Previous
                            },
                            #[name = "play_pause"]
                            gtk::Button {
                                set_icon_name: icon_names::PLAY,
                                connect_clicked => AppMsg::PlayPause
                            },
                            gtk::Button {
                                set_icon_name: icon_names::NEXT_REGULAR,
                                connect_clicked => AppMsg::Next
                            }
                        }
                    } -> {
                        set_title: Some("Song"),
                    },
                }
            }
        }
    }

    async fn init(
        init: Self::Init,
        root: Self::Root,
        sender: AsyncComponentSender<Self>,
    ) -> AsyncComponentParts<Self> {
        let x1 = init.0.clone();
        let mut x = x1.get_mut().await;
        x.set_model(sender.clone());
        
        let model = Model {
            player_reference: init.0,
            track_list: init.1,
            playback_state: "".to_string(),
            sender: sender.clone(),
            song_info: Default::default(),
        };
        let widgets = view_output!();

        AsyncComponentParts { model, widgets }
    }

    async fn update(
        &mut self,
        msg: Self::Input,
        _sender: AsyncComponentSender<Self>,
        _root: &Self::Root,
    ) {
        match msg {
            AppMsg::Start => {
                self.player_reference
                    .get()
                    .await
                    .start_current()
                    .await
                    .expect("Error starting!");
            }
            AppMsg::PlayPause => {
                self.player_reference.get().await.play_pause().await;
            }
            AppMsg::Next => {
                self.player_reference.get_mut().await.next().await;
            }
            AppMsg::Previous => self.player_reference.get_mut().await.previous().await,
            AppMsg::SongUpdate(info) => self.song_info = info,
        }
    }
}
