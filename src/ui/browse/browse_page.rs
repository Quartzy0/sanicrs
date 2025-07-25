use std::sync::Arc;
use async_channel::Sender;
use relm4::adw::gio::ListStore;
use relm4::adw::glib::clone;
use relm4::adw::prelude::*;
use relm4::AsyncComponentSender;
use relm4::adw::gtk::Orientation;
use relm4::gtk::{ListItem, SignalListItemFactory, Widget};
use relm4::prelude::*;
use crate::opensonic::cache::AlbumCache;
use crate::opensonic::types::AlbumListType;
use crate::PlayerCommand;
use crate::ui::album_object::AlbumObject;
use crate::ui::app::Init;
use crate::ui::browse::album_list::AlbumList;
use crate::ui::cover_picture::{CoverPicture, CoverSize};

pub struct BrowsePageWidget {
    album_cache: AlbumCache,
    cmd_sender: Arc<Sender<PlayerCommand>>,

    album_factory: SignalListItemFactory,
}

#[derive(Debug)]
pub enum BrowsePageMsg {
    ScrollNewest(i32),
}

#[derive(Debug)]
pub enum BrowsePageOut {
    ViewAlbum(AlbumObject),
}

#[relm4::component(pub async)]
impl AsyncComponent for BrowsePageWidget {
    type CommandOutput = ();
    type Input = BrowsePageMsg;
    type Output = BrowsePageOut;
    type Init = Init;

    view! {
        adw::NavigationPage {
            set_tag: Some("browse"),
            set_title: "Browse",

            gtk::Box {
                set_orientation: Orientation::Vertical,
                add_css_class: "padded",

                #[template]
                #[name = "newest_list"]
                AlbumList {
                    #[template_child]
                    back_btn {
                        connect_clicked => BrowsePageMsg::ScrollNewest(-100)
                    },
                    #[template_child]
                    forward_btn {
                        connect_clicked => BrowsePageMsg::ScrollNewest(100)
                    },
                    #[template_child]
                    list {
                        set_factory: Some(&model.album_factory),
                        connect_activate[sender] => move |view, index| {
                            if let Some(model) = view.model() {
                                let album: AlbumObject = model.item(index)
                                    .expect("Item at index clicked expected to exist")
                                    .downcast::<AlbumObject>()
                                    .expect("Item expected to be AlbumObject");
                                sender.output(BrowsePageOut::ViewAlbum(album)).expect("Error sending output");
                            }
                        }
                    }
                },
                #[template]
                #[name = "highest_list"]
                AlbumList {
                    #[template_child]
                    back_btn {
                        connect_clicked => BrowsePageMsg::ScrollNewest(-100)
                    },
                    #[template_child]
                    forward_btn {
                        connect_clicked => BrowsePageMsg::ScrollNewest(100)
                    },
                    #[template_child]
                    list {
                        set_factory: Some(&model.album_factory),
                        connect_activate[sender] => move |view, index| {
                            if let Some(model) = view.model() {
                                let album: AlbumObject = model.item(index)
                                    .expect("Item at index clicked expected to exist")
                                    .downcast::<AlbumObject>()
                                    .expect("Item expected to be AlbumObject");
                                sender.output(BrowsePageOut::ViewAlbum(album)).expect("Error sending output");
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
            cmd_sender: init.3,
            album_cache: init.5,
            album_factory: SignalListItemFactory::new(),
        };

        let widgets: Self::Widgets = view_output!();

        model.album_factory.connect_setup(clone!(
            #[strong(rename_to = cover_cache)]
            init.2,
            move |_, list_item| {
                let vbox = gtk::Box::builder()
                    .orientation(Orientation::Vertical)
                    .spacing(3)
                    .build();
                vbox.add_css_class("album-entry");

                let cover_picture = CoverPicture::new(cover_cache.clone(), CoverSize::Large);
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

        let newest_store = ListStore::from_iter(
            model
                .album_cache
                .get_album_list(AlbumListType::Newest, None, None, None, None, None, None)
                .await
                .expect("Error fetching albums"),
        );

        let highest_store = ListStore::from_iter(
            model
                .album_cache
                .get_album_list(AlbumListType::Frequent, None, None, None, None, None, None)
                .await
                .expect("Error fetching albums"),
        );

        widgets
            .newest_list
            .list
            .set_model(Some(&gtk::NoSelection::new(Some(newest_store))));
        widgets
            .highest_list
            .list
            .set_model(Some(&gtk::NoSelection::new(Some(highest_store))));

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
            BrowsePageMsg::ScrollNewest(s) => {
                widgets
                    .newest_list
                    .scroll
                    .hadjustment()
                    .set_value(widgets.newest_list.scroll.hadjustment().value() + s as f64);
            }
        };
        self.update_view(widgets, sender);
    }
}
