use crate::dbus::player::MprisPlayer;
use crate::opensonic::cache::{AlbumCache, ArtistCache, CoverCache};
use crate::ui::album_object::AlbumObject;
use crate::ui::cover_picture::{CoverPicture, CoverSize};
use crate::ui::song_object::{PositionState, SongObject};
use crate::icon_names;
use mpris_server::LocalServer;
use relm4::adw::gio::ListStore;
use relm4::adw::glib as glib;
use relm4::adw::glib::{clone, closure, Object};
use relm4::adw::gtk::{Align, ListItem, Orientation, SignalListItemFactory, Widget};
use relm4::adw::prelude::*;
use relm4::prelude::*;
use std::rc::Rc;
use uuid::Uuid;

#[derive(Debug)]
pub struct ViewAlbumWidget;

type ViewAlbumInit = (
    AlbumObject,
    Rc<LocalServer<MprisPlayer>>,
    CoverCache,
    AlbumCache,
    ArtistCache,
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
                                    },
                                    gtk::Label {
                                        set_markup: album.artist().as_str(),
                                        add_css_class: "t1",
                                        set_halign: Align::Start,
                                        connect_activate_link => move |this, url| {
                                            this.activate_action("win.artist", Some(&url.to_variant())).expect("Error executing action");
                                            glib::Propagation::Stop
                                        },
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
                                    connect_clicked[mpris_server, album_id] => move |_| {
                                        let value = mpris_server.clone();
                                        let id = album_id.clone();
                                        relm4::spawn_local(async move {
                                            let player = value.imp();
                                            player.send_res(player.queue_album(id, None, true).await);
                                        });
                                    },
                                    add_css_class: "album-play-btn"
                                },
                                gtk::Button {
                                    set_valign: Align::Center,
                                    set_halign: Align::Center,
                                    set_icon_name: icon_names::ADD_REGULAR,
                                    connect_clicked[mpris_server, album_id] => move |_| {
                                       let value = mpris_server.clone();
                                       let id = album_id.clone();
                                       relm4::spawn_local(async move {
                                           let player = value.imp();
                                           player.send_res(player.queue_album(id, None, false).await);
                                       });
                                    },
                                    add_css_class: "album-play-btn"
                                }
                            }
                        },
                        gtk::Separator{},
                        #[name = "song_list"]
                        gtk::ListView {
                            set_factory: Some(&song_list_factory),
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
        let album = if init.0.has_songs() {
            init.0
        }else {
            init.3.get_album(init.0.id().as_str()).await.expect("Error getting album")
        };
        let mpris_player = init.1;
        let song_list_factory = SignalListItemFactory::new();
        let cover_cache = init.2;

        let album_id = album.id().clone();
        let mpris_server = mpris_player.clone();
        let widgets: Self::Widgets = view_output!();

        song_list_factory.connect_setup(clone!(
            #[strong]
            cover_cache,
            #[strong]
            mpris_player,
            #[strong]
            album,
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
                    album,
                    #[weak]
                    list_item,
                    #[strong]
                    mpris_player,
                    move |_| {
                        let item = list_item.position();
                        let value = mpris_player.clone();
                        let id = album.id().clone();
                        relm4::spawn_local(async move {
                            let player = value.imp();
                            player.send_res(player.queue_album(id, Some(item as usize), true).await);
                        });
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
            album
                .get_songs()
                .unwrap()
                .iter()
                .map(|x| SongObject::new((Uuid::from_u128(0), x.clone()).into(), PositionState::Passed)),
        );
        widgets.song_list.set_model(Some(&gtk::NoSelection::new(Some(list_store))));

        AsyncComponentParts { model, widgets }
    }
}
