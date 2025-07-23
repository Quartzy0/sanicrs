use crate::opensonic::cache::{AlbumCache, SongCache};
use crate::opensonic::client::OpenSubsonicClient;
use crate::opensonic::types::AlbumListType;
use crate::player::TrackList;
use crate::ui::album_object::AlbumObject;
use crate::ui::app::Init;
use crate::ui::cover_picture::{CoverPicture, CoverSize};
use crate::ui::song_object::{PositionState, SongObject};
use crate::{PlayerCommand, icon_names};
use async_channel::Sender;
use relm4::AsyncComponentSender;
use relm4::adw::gio::ListStore;
use relm4::adw::glib;
use relm4::adw::glib::{Object, clone, closure};
use relm4::adw::gtk;
use relm4::adw::gtk::{Align, Orientation};
use relm4::adw::prelude::*;
use relm4::component::AsyncComponentParts;
use relm4::gtk::{ListItem, SignalListItemFactory, Widget};
use relm4::prelude::AsyncComponent;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

pub struct BrowseWidget {
    track_list: Arc<RwLock<TrackList>>,
    client: Arc<OpenSubsonicClient>,
    cmd_sender: Arc<Sender<PlayerCommand>>,
    song_cache: SongCache,
    album_cache: AlbumCache,

    album_factory: SignalListItemFactory,
    song_list_factory: SignalListItemFactory,

    view: CurrentView,
}

#[derive(Default, Clone)]
enum CurrentView {
    #[default]
    Browse,
    ViewAlbum(AlbumObject),
}

#[derive(Debug)]
pub enum BrowseMsg {
    ScrollNewest(i32),
    PlayAlbum(Option<String>, Option<usize>),
    ViewAlbum(AlbumObject),
    Back,
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

            match &model.view {
                CurrentView::Browse => {
                    &gtk::Box {
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
                                            sender.input(BrowseMsg::ViewAlbum(album));
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                CurrentView::ViewAlbum(album) => {
                    &gtk::Box {
                        set_orientation: Orientation::Vertical,
                        add_css_class: "padded",
                        set_spacing: 5,

                        gtk::Box {
                            set_orientation: Orientation::Horizontal,
                            set_hexpand: false,
                            set_hexpand_set: false,
                            set_halign: Align::Start,

                            gtk::Button {
                                set_icon_name: icon_names::ARROW1_LEFT,
                                connect_clicked => BrowseMsg::Back,
                            },
                        },

                        gtk::CenterBox {
                            set_orientation: Orientation::Horizontal,
                            add_css_class: "padded",

                            #[wrap(Some)]
                            set_start_widget = &gtk::Box {
                                CoverPicture{
                                    set_cover_size: CoverSize::Large,
                                    set_client: model.client.clone(),
                                    #[watch]
                                    set_cover_id: album.cover_art_id(),
                                },
                                gtk::Box {
                                    set_orientation: Orientation::Vertical,
                                    set_spacing: 5,
                                    set_valign: Align::End,

                                    gtk::Label {
                                        #[watch]
                                        set_label: album.name().as_str(),
                                        add_css_class: "bold",
                                        add_css_class: "t0",
                                        set_halign: Align::Start,
                                    },
                                    gtk::Label {
                                        #[watch]
                                        set_label: album.artist().as_str(),
                                        add_css_class: "t1",
                                        set_halign: Align::Start,
                                    },
                                    gtk::Label {
                                        #[watch]
                                        set_label: format!("{} songs", album.song_count()).as_str(),
                                        add_css_class: "t1",
                                        set_halign: Align::Start,
                                    },
                                }
                            },

                            #[wrap(Some)]
                            set_end_widget = &gtk::Box{
                                set_orientation: Orientation::Horizontal,
                                set_valign: Align::Center,
                                set_vexpand: false,
                                set_vexpand_set: false,

                                gtk::Button{
                                    set_icon_name: icon_names::PLAY,
                                    connect_clicked => BrowseMsg::PlayAlbum(None, None),
                                    add_css_class: "album-play-btn"
                                }
                            }
                        },
                        gtk::Separator{},
                        #[name = "song_list"]
                        gtk::ListView {
                            set_factory: Some(&model.song_list_factory),
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
            song_list_factory: SignalListItemFactory::new(),
            view: CurrentView::Browse,
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

                let cover_picture = CoverPicture::new(client.clone(), CoverSize::Large);
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

        model.song_list_factory.connect_setup(clone!(
            #[strong(rename_to = client)]
            model.client,
            move |_, list_item| {
                let hbox = gtk::CenterBox::builder()
                    .orientation(Orientation::Horizontal)
                    .build();
                hbox.add_css_class("album-song-item");

                let start_hbox = gtk::Box::builder()
                    .orientation(Orientation::Horizontal)
                    .spacing(5)
                    .build();
                let end_hbox = gtk::Box::builder()
                    .orientation(Orientation::Horizontal)
                    .spacing(5)
                    .build();
                let play_btn = gtk::Button::builder().icon_name(icon_names::PLAY).build();

                let picture = CoverPicture::new(client.clone(), CoverSize::Small);
                let title = gtk::Label::new(None);
                let duration = gtk::Label::new(None);
                start_hbox.append(&play_btn);
                start_hbox.append(&picture);
                start_hbox.append(&title);
                end_hbox.append(&duration);
                hbox.set_start_widget(Some(&start_hbox));
                hbox.set_end_widget(Some(&end_hbox));

                let list_item = list_item
                    .downcast_ref::<ListItem>()
                    .expect("Needs to be ListItem");
                list_item.set_child(Some(&hbox));

                play_btn.connect_clicked(clone!(
                    #[strong]
                    sender,
                    #[weak]
                    list_item,
                    move |_| {
                        let item = list_item.position();
                        sender.input(BrowseMsg::PlayAlbum(None, Some(item as usize)));
                    }
                ));

                list_item
                    .property_expression("item")
                    .chain_property::<SongObject>("title")
                    .bind(&title, "label", Widget::NONE);
                list_item
                    .property_expression("item")
                    .chain_property::<SongObject>("duration")
                    .chain_closure::<String>(closure!(|_: Option<Object>, duration: u64| {
                        format!("{}:{:02}", duration / 60, duration % 60)
                    }))
                    .bind(&duration, "label", Widget::NONE);
                list_item
                    .property_expression("item")
                    .chain_property::<SongObject>("cover-art-id")
                    .bind(&picture, "cover-id", Widget::NONE);
            }
        ));

        let list_store = ListStore::from_iter(
            model
                .album_cache
                .get_album_list(AlbumListType::Newest, None, None, None, None, None, None)
                .await
                .expect("Error fetching albums"),
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
                widgets
                    .newest_scroll
                    .hadjustment()
                    .set_value(widgets.newest_scroll.hadjustment().value() + s as f64);
            }
            BrowseMsg::PlayAlbum(id, index) => {
                if let Some(id) = id.or_else(|| match &self.view {
                    CurrentView::ViewAlbum(a) => Some(a.id()),
                    _ => None,
                }) {
                    self.cmd_sender
                        .send(PlayerCommand::PlayAlbum(id, index))
                        .await
                        .expect("Error sending command to Player");
                }
            }
            BrowseMsg::ViewAlbum(album) => {
                self.view = CurrentView::ViewAlbum(album.clone());
                let album = if !album.has_songs() {
                    self.album_cache
                        .get_album(album.id().as_str())
                        .await
                        .expect("Error fetching album info")
                } else {
                    album
                };
                let list_store = ListStore::from_iter(
                    album
                        .get_songs()
                        .unwrap()
                        .iter()
                        .map(|x| SongObject::new((Uuid::from_u128(0), x.clone()).into(), PositionState::Passed)),
                );
                widgets.song_list.set_model(Some(&gtk::NoSelection::new(Some(list_store))));
            }
            BrowseMsg::Back => {
                self.view = CurrentView::Browse
            }
        }
        self.update_view(widgets, sender);
    }
}
