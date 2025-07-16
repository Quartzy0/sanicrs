use std::sync::Arc;
use relm4::adw::gio::ListStore;
use relm4::prelude::*;
use tokio::sync::RwLock;
use zbus::object_server::InterfaceRef;
use relm4::adw::{glib, gtk};
use relm4::adw::glib::clone;
use relm4::adw::prelude::*;
use relm4::gtk::{Align, ListItem, Orientation, Overflow, SignalListItemFactory};
use crate::mpris::MprisPlayer;
use crate::opensonic::client::OpenSubsonicClient;
use crate::player::TrackList;
use crate::ui::cover_picture::{CoverPicture, CoverSize};
use crate::ui::song_object::SongObject;

pub struct TrackListWidget {
    player_reference: InterfaceRef<MprisPlayer>,
    track_list: Arc<RwLock<TrackList>>,
    client: Arc<OpenSubsonicClient>,

    factory: SignalListItemFactory,
}

#[derive(Debug)]
pub enum TrackListMsg {

}

type Init = (
    InterfaceRef<MprisPlayer>,
    Arc<RwLock<TrackList>>,
    Arc<OpenSubsonicClient>,
);

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
        };
        let widgets: Self::Widgets = view_output!();

        model.factory.connect_setup(move |_, list_item| {
            let hbox = gtk::Box::builder()
                .orientation(Orientation::Horizontal)
                .valign(Align::Start)
                .halign(Align::Start)
                .css_classes(["track-list-item"])
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

            list_item
                .downcast_ref::<ListItem>()
                .expect("Needs to be ListItem")
                .set_child(Some(&hbox));
        });
        let client = model.client.clone();
        model.factory.connect_bind(move |_, list_item| {
            glib::spawn_future_local(clone!(
                #[strong]
                list_item,
                #[strong]
                client,
                async move {
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
        
                    let vbox = hbox
                        .last_child()
                        .expect("No child in HBox")
                        .downcast::<gtk::Box>()
                        .expect("Last child needs to be gtk::Box");
        
                    let title = vbox
                        .first_child()
                        .expect("No child in VBox")
                        .downcast::<gtk::Label>()
                        .expect("First child needs to be gtk::Label");
        
                    let artist = vbox
                        .last_child()
                        .expect("No child in VBox")
                        .downcast::<gtk::Label>()
                        .expect("Last child needs to be gtk::Label");
        
                    title.set_label(song_object.title().as_str());
                    artist.set_label(song_object.artist().as_str());
                    cover_picture.set_cover_from_id(song_object.cover_art_id().as_ref(), client).await;
                }
            ));
        });

        let track_list = model.track_list.clone();
        let guard = track_list.read().await;
        let list_store = ListStore::from_iter(guard.get_songs().iter().map(SongObject::from));

        widgets.list.set_model(Some(&gtk::NoSelection::new(Some(list_store))));

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