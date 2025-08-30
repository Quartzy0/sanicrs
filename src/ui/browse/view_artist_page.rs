use crate::dbus::player::MprisPlayer;
use crate::opensonic::cache::{AlbumCache, ArtistCache, CoverCache};
use crate::ui::artist_object::ArtistObject;
use crate::ui::cover_picture::{CoverPicture, CoverSize, CoverType};
use mpris_server::LocalServer;
use relm4::adw::gtk::{Align, Orientation, SignalListItemFactory};
use relm4::adw::prelude::*;
use relm4::gtk::gio::ListStore;
use relm4::prelude::*;
use std::rc::Rc;
use relm4::adw::glib::clone;
use relm4::gtk::{ListItem, Widget};
use relm4::adw::glib as glib;
use crate::icon_names;
use crate::ui::album_object::AlbumObject;

#[derive(Debug)]
pub struct ViewArtistWidget {
    mpris_player: Rc<LocalServer<MprisPlayer>>,
    album_factory: SignalListItemFactory,
    artist: ArtistObject,
    album_cache: AlbumCache,
    artist_cache: ArtistCache,
    cover_cache: CoverCache,
}

#[derive(Debug)]
pub enum ViewArtistMsg {
    PlayAlbum(u32),
    SetArtist(ArtistObject),
    ViewAlbum(u32),
}

#[derive(Debug)]
pub enum ViewArtistOut {
    ViewAlbum(AlbumObject)
}

type ViewArtistInit = (
    ArtistObject,
    Rc<LocalServer<MprisPlayer>>,
    CoverCache,
    AlbumCache,
    ArtistCache,
);

#[relm4::component(pub async)]
impl AsyncComponent for ViewArtistWidget {
    type CommandOutput = ();
    type Input = ViewArtistMsg;
    type Output = ViewArtistOut;
    type Init = ViewArtistInit;

    view! {
        adw::NavigationPage {
            set_tag: Some("view-artist"),
            set_title: "View artist",

            gtk::ScrolledWindow {
                set_hscrollbar_policy: gtk::PolicyType::Never,
                set_vexpand: true,
                set_vexpand_set: true,
                set_valign: Align::Fill,

                adw::ToolbarView{
                    add_top_bar = &adw::HeaderBar {
                        set_show_title: false,
                        set_show_end_title_buttons: false,
                    },

                    gtk::Box {
                        set_orientation: Orientation::Vertical,
                        add_css_class: "padded",
                        set_spacing: 10,

                        gtk::Box {
                            set_orientation: Orientation::Horizontal,
                            set_spacing: 10,

                            CoverPicture{
                                set_cover_size: CoverSize::Large,
                                set_cover_type: CoverType::Round,
                                set_cache: model.cover_cache.clone(),
                                #[watch]
                                set_cover_id: model.artist.cover_art_id(),
                            },

                            gtk::Box {
                                set_orientation: Orientation::Vertical,
                                set_spacing: 10,

                                append = &gtk::Label {
                                    #[watch]
                                    set_label: model.artist.name().as_str(),
                                    add_css_class: "bold",
                                    add_css_class: "t0",
                                    set_halign: Align::Start,
                                },
                                append = if let Some(count) = model.artist.album_count() {
                                    &gtk::Label {
                                        #[watch]
                                        set_label: format!("{} albums", count).as_str(),
                                        add_css_class: "t1",
                                        set_halign: Align::Start,
                                    }
                                } else {
                                    &adw::Bin{}
                                }
                            }
                        },
                        #[name = "list"]
                        gtk::ListView {
                            set_factory: Some(&model.album_factory),
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
            mpris_player: init.1,
            artist: init.0,
            album_factory: SignalListItemFactory::new(),
            album_cache: init.3,
            artist_cache: init.4,
            cover_cache: init.2,
        };

        model.album_factory.connect_setup(clone!(
            #[strong(rename_to = cover_cache)]
            model.cover_cache,
            #[strong]
            sender,
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
                let play_btn = gtk::Button::builder()
                    .icon_name(icon_names::PLAY)
                    .valign(Align::Center)
                    .halign(Align::Center)
                    .build();

                let picture = CoverPicture::new(cover_cache.clone(), CoverSize::Small);
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
                        sender.input(ViewArtistMsg::PlayAlbum(item));
                    }
                ));

                let gesture = gtk::GestureClick::new();
                gesture.connect_released(clone!(
                    #[strong]
                    sender,
                    #[weak]
                    list_item,
                    move |_this, _n: i32, _x: f64, _y: f64| {
                        let item = list_item.position();
                        sender.input(ViewArtistMsg::ViewAlbum(item));
                    }
                ));
                hbox.add_controller(gesture);

                list_item
                    .property_expression("item")
                    .chain_property::<AlbumObject>("name")
                    .bind(&title, "label", Widget::NONE);
                list_item
                    .property_expression("item")
                    .chain_property::<AlbumObject>("duration")
                    .bind(&duration, "label", Widget::NONE);
                list_item
                    .property_expression("item")
                    .chain_property::<AlbumObject>("cover-art-id")
                    .bind(&picture, "cover-id", Widget::NONE);
            }
        ));

        let widgets: Self::Widgets = view_output!();

        if let Some(albums) = model.artist.get_albums() {
            let albums = model.album_cache.add_albums(albums).await;
            let list_store = ListStore::from_iter(albums);
            widgets.list.set_model(Some(&gtk::NoSelection::new(Some(list_store))));
        }

        AsyncComponentParts { model, widgets }
    }

    async fn update_with_view(
        &mut self,
        widgets: &mut Self::Widgets,
        message: Self::Input,
        sender: AsyncComponentSender<Self>,
        _root: &Self::Root,
    ) {
        let player = self.mpris_player.imp();
        match message {
            ViewArtistMsg::PlayAlbum(index) => {
                if let Some(model) = widgets.list.model()
                    && let Some(item) = model.item(index) {
                    let item = item.downcast::<AlbumObject>().expect("Item should be AlbumObject");
                    player.send_res(player.queue_album(item.id(), None, true).await);
                }
            },
            ViewArtistMsg::SetArtist(artist) => {
                match self.artist_cache.ensure_albums(artist).await {
                    Ok(art) => {
                        self.artist = art;
                        if let Some(albums) = self.artist.get_albums() {
                            let albums = self.album_cache.add_albums(albums).await;
                            let list_store = ListStore::from_iter(albums);
                            widgets.list.set_model(Some(&gtk::NoSelection::new(Some(list_store))));
                        }
                    },
                    Err(e) => player.send_error(e),
                }
            },
            ViewArtistMsg::ViewAlbum(index) => {
                if let Some(model) = widgets.list.model()
                    && let Some(item) = model.item(index) {
                    let item = item.downcast::<AlbumObject>().expect("Item should be AlbumObject");
                    sender.output(ViewArtistOut::ViewAlbum(item)).expect("Error sending message out of ViewArtist");
                }
            },
        }

        self.update_view(widgets, sender);
    }
}
