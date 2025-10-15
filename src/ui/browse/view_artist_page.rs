use crate::dbus::player::MprisPlayer;
use crate::opensonic::cache::{AlbumCache, ArtistCache, CoverCache};
use crate::ui::artist_object::ArtistObject;
use crate::ui::cover_picture::{CoverPicture, CoverSize, CoverType};
use mpris_server::LocalServer;
use relm4::adw::gtk::{Align, Orientation};
use relm4::adw::prelude::*;
use relm4::prelude::*;
use std::rc::Rc;
use relm4::adw::glib::{clone, closure, Object};
use relm4::adw::glib as glib;
use relm4::gtk::pango::WrapMode;
use relm4::gtk::Widget;
use crate::icon_names;
use crate::ui::album_object::AlbumObject;
use crate::ui::item_list::{ItemListInit, ItemListWidget};

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

                        gtk::CenterBox {
                            #[wrap(Some)]
                            set_start_widget = &gtk::Box {
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
                                    set_valign: Align::Center,

                                    gtk::Label {
                                        set_label: artist.name().as_str(),
                                        add_css_class: "bold",
                                        add_css_class: "t0",
                                        set_halign: Align::Start,
                                        set_wrap: true,
                                        set_wrap_mode: WrapMode::WordChar,
                                    },
                                    gtk::Label {
                                        set_label: artist.album_count().and_then(|c| Some(format!("{} albums", c))).unwrap_or("".to_string()).as_str(),
                                        add_css_class: "t1",
                                        set_halign: Align::Start,
                                    }
                                }
                            },

                            #[wrap(Some)]
                            set_end_widget = &gtk::Box {
                                set_orientation: Orientation::Horizontal,
                                set_halign: Align::End,
                                set_spacing: 10,

                                gtk::Button {
                                    set_halign: Align::Center,
                                    set_valign: Align::Center,
                                    add_css_class: "circular",
                                    add_css_class: "midicon",
                                    set_icon_name: icon_names::PLAY,
                                    set_width_request: 48,
                                    set_height_request: 48,
                                    set_tooltip: "Play artist's albums",
                                    connect_clicked[artist, mpris_player] => move |_| {
                                        relm4::spawn_local(clone!(
                                            #[strong]
                                            artist,
                                            #[strong]
                                            mpris_player,
                                            async move {
                                                let mut albums = artist.get_albums().unwrap_or(Vec::new());
                                                if let Some(first) = albums.pop() {
                                                    // Clear previous play queue with first album
                                                    mpris_player.imp().send_res(mpris_player.imp().queue_album(first.id, None, true).await);
                                                    for album in albums {
                                                        mpris_player.imp().send_res(mpris_player.imp().queue_album(album.id, None, false).await);
                                                    }
                                                }
                                            }
                                        ));
                                    }
                                },
                                #[name = "like_btn"]
                                gtk::ToggleButton {
                                    set_halign: Align::Center,
                                    set_valign: Align::Center,
                                    add_css_class: "circular",
                                    add_css_class: "midicon",
                                    set_width_request: 48,
                                    set_height_request: 48,
                                    set_tooltip: "Star artist",
                                    connect_clicked[artist_cache, artist, mpris_player] => move |_| {
                                        relm4::spawn_local(clone!(
                                            #[strong]
                                            artist,
                                            #[strong]
                                            artist_cache,
                                            #[strong]
                                            mpris_player,
                                            async move {
                                                mpris_player.imp().send_res(artist_cache.toggle_starred(&artist).await);
                                            }
                                        ));
                                    }
                                }
                            }
                        },
                        item_list_widget.widget(),
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
        let artist_c = artist.clone();
        let album_cache = init.3;
        let cover_cache = init.2;
        let artist_cache = init.4;

        let item_list_widget = ItemListWidget::builder()
            .launch(ItemListInit {
                cover_type: CoverType::Square,
                mpris_player: mpris_player.clone(),
                cover_cache: cover_cache.clone(),
                play_fn: Some(Box::new(move |album: AlbumObject, _i, mpris_player| {
                    relm4::spawn_local(async move {
                        let player = mpris_player.imp();
                        player.send_res(player.queue_album(album.id(), None, true).await);
                    });
                })),
                click_fn: Some(Box::new(clone!(
                    #[weak]
                    root,
                    move |album, _i, _mpris_player| {
                        let album = album.downcast::<AlbumObject>().expect("Item should be AlbumObject");
                        root.activate_action("win.album", Some(&album.id().to_variant())).expect("Error executing action");
                    }
                ))),
                load_items: async move {
                    if let Some(albums) = artist_c.get_albums() {
                        album_cache.add_albums(albums).await.into_iter().collect()
                    } else {
                        Vec::new()
                    }
                },
                highlight: None,
            });

        let widgets: Self::Widgets = view_output!();

        widgets.like_btn
            .property_expression("active")
            .chain_closure::<String>(closure!(
                        move |_: Option<Object>, active: bool| {
                            if active {
                                icon_names::HEART_FILLED
                            } else {
                                icon_names::HEART_OUTLINE_THIN
                            }
                        }
                    ))
            .bind(&widgets.like_btn, "icon-name", Widget::NONE);
        artist
            .property_expression("starred")
            .bind(&widgets.like_btn, "active", Widget::NONE);

        AsyncComponentParts { model, widgets }
    }
}
