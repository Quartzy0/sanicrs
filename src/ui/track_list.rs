use std::sync::Arc;
use relm4::adw::gio::ListStore;
use relm4::prelude::*;
use tokio::sync::RwLock;
use zbus::object_server::InterfaceRef;
use relm4::adw::{glib, gtk};
use relm4::adw::glib::{clone, closure, Object};
use relm4::adw::glib::gobject_ffi::GObject;
use relm4::adw::prelude::*;
use relm4::adw::gtk::{Align, ListItem, Orientation, SignalListItemFactory, Widget};
use crate::dbus::player::MprisPlayer;
use crate::dbus::track_list::MprisTrackList;
use crate::opensonic::client::OpenSubsonicClient;
use crate::opensonic::types::Song;
use crate::player::TrackList;
use crate::ui::app::Init;
use crate::ui::cover_picture::{CoverPicture, CoverSize};
use crate::ui::song_object::{PositionState, SongObject};

pub struct TrackListWidget {
    player_reference: InterfaceRef<MprisPlayer>,
    track_list: Arc<RwLock<TrackList>>,
    track_list_mpris: InterfaceRef<MprisTrackList>,
    client: Arc<OpenSubsonicClient>,

    factory: SignalListItemFactory,
}

#[derive(Debug)]
pub enum TrackListMsg {
    TrackActivated(usize),
    TrackChanged(usize),
    ReloadList,
}

#[relm4::component(pub async)]
impl AsyncComponent for TrackListWidget {
    type CommandOutput = ();
    type Input = TrackListMsg;
    type Output = ();
    type Init = Init;

    view! {
        gtk::ScrolledWindow {
            set_hscrollbar_policy: gtk::PolicyType::Never,
            set_min_content_width: 360,

            #[name = "list"]
            gtk::ListView {
                set_factory: Some(&model.factory),
                set_single_click_activate: true,

                connect_activate[sender] => move |_, index| {
                    sender.input(TrackListMsg::TrackActivated(index as usize));
                }
            }
        }
    }

    async fn init(
        init: Self::Init,
        root: Self::Root,
        sender: AsyncComponentSender<Self>,
    ) -> AsyncComponentParts<Self> {
        let model = TrackListWidget {
            player_reference: init.0,
            track_list: init.1,
            client: init.2,
            factory: SignalListItemFactory::new(),
            track_list_mpris: init.3,
        };
        let widgets: Self::Widgets = view_output!();
        {
            let mut track_list_mpris = model.track_list_mpris.get_mut().await;
            track_list_mpris.track_list_sender = Some(sender.clone());
        }

        model.factory.connect_setup(move |_, list_item| {
            let hbox = gtk::Box::builder()
                .orientation(Orientation::Horizontal)
                .valign(Align::Start)
                .halign(Align::Start)
                .spacing(10)
                .hexpand(true)
                .hexpand_set(true)
                .halign(Align::Fill)
                .build();
            let vbox = gtk::Box::builder()
                .orientation(Orientation::Vertical)
                .valign(Align::Start)
                .halign(Align::Start)
                .build();

            let title = gtk::Label::new(None);
            title.set_halign(Align::Start);
            title.add_css_class("bold");
            vbox.append(&title);
            let artist = gtk::Label::new(None);
            artist.set_halign(Align::Start);
            vbox.append(&artist);

            let picture = CoverPicture::new();
            picture.set_cover_size(CoverSize::Small);
            hbox.append(&picture);
            hbox.append(&vbox);

            let list_item = list_item
                .downcast_ref::<ListItem>()
                .expect("Needs to be ListItem");
            list_item
                .set_child(Some(&hbox));

            list_item
                .property_expression("item")
                .chain_property::<SongObject>("title")
                .bind(&title, "label", Widget::NONE);
            list_item
                .property_expression("item")
                .chain_property::<SongObject>("artist")
                .bind(&artist, "label", Widget::NONE);
            list_item
                .property_expression("item")
                .chain_property::<SongObject>("position-state")
                .chain_closure::<Vec<String>>(closure!(
                    move |_: Option<Object>, position_state: PositionState| {
                        match position_state {
                            PositionState::Passed => vec!["track-list-item".to_string(), "passed".to_string()],
                            PositionState::Current => vec!["track-list-item".to_string(), "current".to_string()],
                            PositionState::Upcoming => vec!["track-list-item".to_string(), "upcoming".to_string()]
                        }
                }))
                .bind(&hbox, "css-classes", Widget::NONE);
        });
        let client = model.client.clone();
        model.factory.connect_bind(move |_, list_item| {
            let song_object = list_item
                .downcast_ref::<ListItem>()
                .expect("Needs to be ListItem")
                .item()
                .and_downcast::<SongObject>()
                .expect("The item has to be an `SongObject`.");

            let hbox = list_item
                .downcast_ref::<ListItem>()
                .expect("Needs to be ListItem")
                .child()
                .and_downcast::<gtk::Box>()
                .expect("The child has to be a `Box`.");

            let cover_picture = hbox
                .first_child()
                .expect("No child in HBox")
                .downcast::<CoverPicture>()
                .expect("First child needs to be cover picture");
            glib::spawn_future_local(clone!(
                #[strong]
                cover_picture,
                #[strong]
                client,
                #[strong]
                song_object,
                async move {
                    cover_picture.set_cover_from_id(song_object.cover_art_id().as_ref(), client).await;
                }
            ));
        });

        sender.input(TrackListMsg::ReloadList);

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
            TrackListMsg::TrackActivated(i) => {
                let tracklist_ref = self.track_list_mpris.get().await;
                tracklist_ref.go_to_index(i).await.expect("Error performing goto index");
            },
            TrackListMsg::TrackChanged(pos) => {
                let model = widgets.list.model();
                if let Some(model) = model {
                    model.iter::<Object>().enumerate().for_each(|x| {
                        if let (i, Ok(song)) = x {
                            song
                                .downcast::<SongObject>()
                                .expect("Must be SongObject.")
                                .set_position_state(
                                    if i < pos {
                                        PositionState::Passed
                                    } else if i > pos {
                                        PositionState::Upcoming
                                    } else {
                                        PositionState::Current
                                    }
                                );
                        }
                    });
                }
            },
            TrackListMsg::ReloadList => {
                let guard = self.track_list.read().await;
                let pos = guard.current_index().unwrap_or(0);
                let list_store = ListStore::from_iter(guard.get_songs().iter().enumerate().map(|x1| {
                    SongObject::new(x1.1.clone(), if x1.0 < pos {
                        PositionState::Passed
                    } else if x1.0 > pos {
                        PositionState::Upcoming
                    } else {
                        PositionState::Current
                    })
                }));

                widgets.list.set_model(Some(&gtk::NoSelection::new(Some(list_store))));
            }
        };
        self.update_view(widgets, sender);
    }
}