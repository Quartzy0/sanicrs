use crate::dbus::player::MprisPlayer;
use crate::opensonic::cache::{AlbumCache, ArtistCache, CoverCache};
use crate::ui::album_object::AlbumObject;
use crate::ui::cover_picture::{CoverPicture, CoverSize, CoverType};
use crate::ui::song_object::{PositionState, SongObject};
use crate::icon_names;
use mpris_server::LocalServer;
use relm4::adw::glib as glib;
use relm4::adw::gtk::{Align, Orientation};
use relm4::adw::prelude::*;
use relm4::prelude::*;
use std::rc::Rc;
use relm4::gtk::pango::WrapMode;
use uuid::Uuid;
use crate::ui::item_list::{ItemListInit, ItemListWidget};

#[derive(Debug)]
pub struct ViewAlbumWidget;

type ViewAlbumInit = (
    AlbumObject,
    Rc<LocalServer<MprisPlayer>>,
    CoverCache,
    AlbumCache,
    ArtistCache,
    Option<u32>
);

#[relm4::component(pub async)]
impl AsyncComponent for ViewAlbumWidget {
    type CommandOutput = ();
    type Input = ();
    type Output = ();
    type Init = ViewAlbumInit;

    view! {
        adw::NavigationPage {
            set_title: "View album",

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
                        set_spacing: 5,

                        gtk::CenterBox {
                            set_orientation: Orientation::Horizontal,
                            add_css_class: "padded",

                            #[wrap(Some)]
                            set_start_widget = &gtk::Box {
                                set_orientation: Orientation::Horizontal,
                                set_spacing: 10,

                                CoverPicture{
                                    set_cover_size: CoverSize::Large,
                                    set_cache: cover_cache.clone(),
                                    set_cover_id: album.cover_art_id(),
                                },
                                gtk::Box {
                                    set_orientation: Orientation::Vertical,
                                    set_spacing: 5,
                                    set_valign: Align::End,

                                    gtk::Label {
                                        set_label: album.name().as_str(),
                                        add_css_class: "bold",
                                        add_css_class: "t0",
                                        set_halign: Align::Start,
                                        set_wrap: true,
                                        set_wrap_mode: WrapMode::WordChar,
                                    },
                                    gtk::Label {
                                        set_markup: album.artist().as_str(),
                                        add_css_class: "t1",
                                        set_halign: Align::Start,
                                        connect_activate_link => move |this, url| {
                                            this.activate_action("win.artist", Some(&url.to_variant())).expect("Error executing action");
                                            glib::Propagation::Stop
                                        },
                                        set_wrap: true,
                                        set_wrap_mode: WrapMode::WordChar,
                                    },
                                    gtk::Label {
                                        set_label: format!("{} songs", album.song_count()).as_str(),
                                        add_css_class: "t1",
                                        set_halign: Align::Start,
                                    },
                                    gtk::Label {
                                        set_label: format!("Duration: {}", album.duration()).as_str(),
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
                                set_spacing: 10,

                                gtk::Button{
                                    set_valign: Align::Center,
                                    set_halign: Align::Center,
                                    set_icon_name: icon_names::PLAY,
                                    connect_clicked[mpris_player, album_id] => move |_| {
                                        let value = mpris_player.clone();
                                        let id = album_id.clone();
                                        relm4::spawn_local(async move {
                                            let player = value.imp();
                                            player.send_res(player.queue_album(id, None, true).await);
                                        });
                                    },
                                    add_css_class: "album-play-btn",
                                    set_tooltip: "Play",
                                },
                                gtk::Button {
                                    set_valign: Align::Center,
                                    set_halign: Align::Center,
                                    set_icon_name: icon_names::ADD_REGULAR,
                                    connect_clicked[mpris_player, album_id] => move |_| {
                                       let value = mpris_player.clone();
                                       let id = album_id.clone();
                                       relm4::spawn_local(async move {
                                           let player = value.imp();
                                           player.send_res(player.queue_album(id, None, false).await);
                                       });
                                    },
                                    add_css_class: "album-play-btn",
                                    set_tooltip: "Add to queue",
                                }
                            }
                        },
                        gtk::Separator{},
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
        let album = if init.0.has_songs() {
            init.0
        }else {
            init.3.get_album(init.0.id().as_str()).await.expect("Error getting album")
        };
        let mpris_player = init.1;
        let cover_cache = init.2;

        let album_id = album.id().clone();
        let album_id_c = album.id().clone();
        let album_c = album.clone();

        let item_list_widget = ItemListWidget::builder()
            .launch(ItemListInit {
                mpris_player: mpris_player.clone(),
                cover_type: CoverType::Square,
                cover_cache: cover_cache.clone(),
                play_fn: Some(Box::new(move |_song: SongObject, i, mpris_player| {
                    let album_id = album_id_c.clone();
                    relm4::spawn_local(async move {
                        let player = mpris_player.imp();
                        player.send_res(player.queue_album(album_id, Some(i as usize), true).await);
                    });
                })),
                click_fn: None,
                load_items: async move {
                    album_c
                        .get_songs()
                        .unwrap()
                        .into_iter()
                        .map(|x| SongObject::new((Uuid::from_u128(0), x.clone()).into(), PositionState::Passed))
                },
                highlight: init.5
            });

        let widgets: Self::Widgets = view_output!();

        AsyncComponentParts { model, widgets }
    }
}
