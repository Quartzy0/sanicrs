use crate::PlayerCommand;
use crate::opensonic::client::OpenSubsonicClient;
use crate::opensonic::types::AlbumListType;
use crate::player::TrackList;
use crate::ui::album_object::AlbumObject;
use crate::ui::app::Init;
use crate::ui::cover_picture::{CoverPicture, CoverSize};
use async_channel::Sender;
use relm4::AsyncComponentSender;
use relm4::adw::gio::ListStore;
use relm4::adw::glib::clone;
use relm4::adw::gtk;
use relm4::adw::gtk::{Align, Orientation};
use relm4::adw::prelude::*;
use relm4::component::AsyncComponentParts;
use relm4::gtk::{ListItem, SignalListItemFactory, Widget};
use relm4::prelude::AsyncComponent;
use std::sync::Arc;
use tokio::sync::RwLock;

pub struct BrowseWidget {
    track_list: Arc<RwLock<TrackList>>,
    client: Arc<OpenSubsonicClient>,
    cmd_sender: Arc<Sender<PlayerCommand>>,

    newest_factory: SignalListItemFactory,
}

#[derive(Debug)]
pub enum BrowseMsg {}

#[relm4::component(pub async)]
impl AsyncComponent for BrowseWidget {
    type CommandOutput = ();
    type Input = BrowseMsg;
    type Output = ();
    type Init = Init;

    view! {
        gtk::ScrolledWindow {
            set_hscrollbar_policy: gtk::PolicyType::Never,
            set_vexpand: true,
            set_vexpand_set: true,
            set_valign: Align::Fill,

            gtk::Box {
                set_orientation: Orientation::Vertical,

                gtk::ScrolledWindow {
                    set_vscrollbar_policy: gtk::PolicyType::Never,
                    set_hexpand: true,
                    set_hexpand_set: true,
                    set_halign: Align::Fill,

                    #[name = "newest_list"]
                    gtk::ListView {
                        set_orientation: Orientation::Horizontal,
                        set_factory: Some(&model.newest_factory),
                        set_single_click_activate: true,
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
        let model = Self {
            track_list: init.1,
            client: init.2,
            cmd_sender: init.3,
            newest_factory: SignalListItemFactory::new(),
        };

        let widgets: Self::Widgets = view_output!();

        model.newest_factory.connect_setup(clone!(
            #[strong(rename_to = client)]
            model.client,
            move |_, list_item| {
                let vbox = gtk::Box::builder()
                    .orientation(Orientation::Vertical)
                    .spacing(3)
                    .build();

                let cover_picture = CoverPicture::new(client.clone());
                cover_picture.set_cover_size(CoverSize::Large);
                vbox.append(&cover_picture);

                let name = gtk::Label::builder().css_classes(["bold"]).build();
                let artist = gtk::Label::new(None);
                vbox.append(&name);
                vbox.append(&artist);

                let list_item = list_item
                    .downcast_ref::<ListItem>()
                    .expect("Needs to be ListItem");
                list_item.set_child(Some(&vbox));

                list_item
                    .property_expression("item")
                    .chain_property::<AlbumObject>("name")
                    .bind(&name, "label", Widget::NONE);
                list_item
                    .property_expression("item")
                    .chain_property::<AlbumObject>("artist")
                    .bind(&artist, "label", Widget::NONE);
                list_item
                    .property_expression("item")
                    .chain_property::<AlbumObject>("cover-art-id")
                    .bind(&cover_picture, "cover-id", Widget::NONE);
            }
        ));

        let map: Vec<AlbumObject> = model
            .client
            .get_album_list(AlbumListType::Newest, None, None, None, None, None, None)
            .await
            .expect("Error fetching albums")
            .0
            .into_iter()
            .map(AlbumObject::new)
            .collect();
        let list_store = ListStore::from_iter(
            map
        );

        widgets
            .newest_list
            .set_model(Some(&gtk::NoSelection::new(Some(list_store))));

        AsyncComponentParts { model, widgets }
    }

    async fn update_cmd_with_view(
        &mut self,
        widgets: &mut Self::Widgets,
        message: Self::CommandOutput,
        sender: AsyncComponentSender<Self>,
        root: &Self::Root,
    ) {
        self.update_cmd(message, sender, root).await;
    }
}
