use crate::{icon_names, PlayerCommand};
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
use crate::opensonic::cache::{AlbumCache, SongCache};

pub struct BrowseWidget {
    track_list: Arc<RwLock<TrackList>>,
    client: Arc<OpenSubsonicClient>,
    cmd_sender: Arc<Sender<PlayerCommand>>,
    song_cache: SongCache,
    album_cache: AlbumCache,

    album_factory: SignalListItemFactory,
}

enum CurrentView {
    Browse
}

#[derive(Debug)]
pub enum BrowseMsg {
    ScrollNewest(i32),
    PlayAlbum(String),
}

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
                add_css_class: "padded",

                gtk::Box {
                    set_orientation: Orientation::Vertical,

                    gtk::CenterBox {
                        set_orientation: Orientation::Horizontal,
                        set_halign: Align::Fill,
                        set_hexpand: true,
                        set_hexpand_set: true,

                        #[wrap(Some)]
                        set_start_widget = &gtk::Label {
                            add_css_class: "t0",
                            add_css_class: "bold",
                            set_label: "Newest"
                        },

                        #[wrap(Some)]
                        set_end_widget = &gtk::Box {
                            set_orientation: Orientation::Horizontal,
                            set_spacing: 5,
                            gtk::Button {
                                set_label: "<",
                                add_css_class: "no-bg",
                                add_css_class: "bold",
                                connect_clicked => BrowseMsg::ScrollNewest(-100)
                            },
                            gtk::Button {
                                set_label: ">",
                                add_css_class: "no-bg",
                                add_css_class: "bold",
                                connect_clicked => BrowseMsg::ScrollNewest(100)
                            }
                        }
                    },
                    #[name = "newest_scroll"]
                    gtk::ScrolledWindow {
                        set_vscrollbar_policy: gtk::PolicyType::Never,
                        set_hscrollbar_policy: gtk::PolicyType::Always,
                        set_hexpand: true,
                        set_hexpand_set: true,
                        set_halign: Align::Fill,
                        #[name = "newest_list"]
                        gtk::ListView {
                            set_orientation: Orientation::Horizontal,
                            set_factory: Some(&model.album_factory),
                            set_single_click_activate: true,
                            connect_activate[sender] => move |view, index| {
                                let model = view.model();
                                if let Some(model) = model {
                                    let album: AlbumObject = model.item(index)
                                        .expect("Item at index clicked expected to exist")
                                        .downcast::<AlbumObject>()
                                        .expect("Item expected to be AlbumObject");
                                    sender.input(BrowseMsg::PlayAlbum(album.id().expect("Album should have ID set")));
                                }
                            }
                        }
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
            cmd_sender: init.3,
            client: init.2,
            song_cache: init.4,
            album_cache: init.5,
            album_factory: SignalListItemFactory::new(),
        };

        let widgets: Self::Widgets = view_output!();

        model.album_factory.connect_setup(clone!(
            #[strong(rename_to = client)]
            model.client,
            move |_, list_item| {
                let vbox = gtk::Box::builder()
                    .orientation(Orientation::Vertical)
                    .spacing(3)
                    .build();
                vbox.add_css_class("album-entry");

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

        let list_store = ListStore::from_iter(
            model
                .album_cache
                .get_album_list(AlbumListType::Newest, None, None, None, None, None, None)
                .await
                .expect("Error fetching albums")
        );

        widgets
            .newest_list
            .set_model(Some(&gtk::NoSelection::new(Some(list_store))));

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
            BrowseMsg::ScrollNewest(s) => {
                widgets.newest_scroll.hadjustment().set_value(widgets.newest_scroll.hadjustment().value() + s as f64);
            },
            BrowseMsg::PlayAlbum(id) => {
                self.cmd_sender.send(PlayerCommand::PlayAlbum(id)).await.expect("Error sending command to Player");
            }
        }
        self.update_view(widgets, sender);
    }
}
