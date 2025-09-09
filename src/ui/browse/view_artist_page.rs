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
pub struct ViewArtistWidget;

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
    type Input = ();
    type Output = ();
    type Init = ViewArtistInit;

    view! {
        adw::NavigationPage {
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
                                set_cache: cover_cache.clone(),
                                set_cover_id: artist.cover_art_id(),
                            },

                            gtk::Box {
                                set_orientation: Orientation::Vertical,
                                set_spacing: 10,

                                gtk::Label {
                                    set_label: artist.name().as_str(),
                                    add_css_class: "bold",
                                    add_css_class: "t0",
                                    set_halign: Align::Start,
                                },
                                gtk::Label {
                                    set_label: artist.album_count().and_then(|c| Some(format!("{} albums", c))).unwrap_or("".to_string()).as_str(),
                                    add_css_class: "t1",
                                    set_halign: Align::Start,
                                }
                            }
                        },
                        #[name = "list"]
                        gtk::ListView {
                            set_factory: Some(&album_factory),
                        }
                    }
                }
            }
        }
    }

    async fn init(
        init: Self::Init,
        root: Self::Root,
        _sender: AsyncComponentSender<Self>,
    ) -> AsyncComponentParts<Self> {
        let model = Self {};

        let mpris_player = init.1;
        let artist = init.0;
        let album_factory = SignalListItemFactory::new();
        let album_cache = init.3;
        let cover_cache = init.2;

        let widgets: Self::Widgets = view_output!();

        album_factory.connect_setup(clone!(
            #[strong(rename_to = cover_cache)]
            cover_cache,
            #[strong(rename_to = mpris_player)]
            mpris_player,
            #[weak(rename_to = list)]
            widgets.list,
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
                    mpris_player,
                    #[weak]
                    list_item,
                    #[weak]
                    list,
                    move |_| {
                        let item = list_item.position();
                        let value = mpris_player.clone();
                        if let Some(model) = list.model()
                            && let Some(item) = model.item(item) {
                            let item = item.downcast::<AlbumObject>().expect("Item should be AlbumObject");
                            relm4::spawn_local(async move {
                                let player = value.imp();
                                player.send_res(player.queue_album(item.id(), None, true).await);
                            });
                        }
                    }
                ));

                let gesture = gtk::GestureClick::new();
                gesture.connect_released(clone!(
                    #[weak]
                    list_item,
                    #[weak]
                    list,
                    move |_this, _n: i32, _x: f64, _y: f64| {
                        if let Some(model) = list.model()
                            && let Some(item) = model.item(list_item.position()) {
                            let item = item.downcast::<AlbumObject>().expect("Item should be AlbumObject");
                            list.activate_action("win.album", Some(&item.id().to_variant())).expect("Error executing action");
                        }
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

        if let Some(albums) = artist.get_albums() {
            let albums = album_cache.add_albums(albums).await;
            let list_store = ListStore::from_iter(albums);
            widgets.list.set_model(Some(&gtk::NoSelection::new(Some(list_store))));
        }

        AsyncComponentParts { model, widgets }
    }
}
