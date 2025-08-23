use crate::dbus::player::MprisPlayer;
use crate::opensonic::cache::{AlbumCache, CoverCache};
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

pub struct ViewAlbumWidget {
    mpris_player: Rc<LocalServer<MprisPlayer>>,
    song_list_factory: SignalListItemFactory,
    album: AlbumObject,
    album_cache: AlbumCache,
}

#[derive(Debug)]
pub enum ViewAlbumMsg {
    PlayAlbum(Option<usize>),
    SetAlbum(AlbumObject),
    QueueAlbum,
}

type ViewAlbumInit = (
    AlbumObject,
    Rc<LocalServer<MprisPlayer>>,
    CoverCache,
    AlbumCache
);

#[relm4::component(pub async)]
impl AsyncComponent for ViewAlbumWidget {
    type CommandOutput = ();
    type Input = ViewAlbumMsg;
    type Output = ();
    type Init = ViewAlbumInit;

    view! {
        adw::NavigationPage {
            set_tag: Some("view-album"),
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
                                CoverPicture{
                                    set_cover_size: CoverSize::Large,
                                    set_cache: init.2.clone(),
                                    #[watch]
                                    set_cover_id: model.album.cover_art_id(),
                                },
                                gtk::Box {
                                    set_orientation: Orientation::Vertical,
                                    set_spacing: 5,
                                    set_valign: Align::End,

                                    gtk::Label {
                                        #[watch]
                                        set_label: model.album.name().as_str(),
                                        add_css_class: "bold",
                                        add_css_class: "t0",
                                        set_halign: Align::Start,
                                    },
                                    gtk::Label {
                                        #[watch]
                                        set_label: model.album.artist().as_str(),
                                        add_css_class: "t1",
                                        set_halign: Align::Start,
                                    },
                                    gtk::Label {
                                        #[watch]
                                        set_label: format!("{} songs", model.album.song_count()).as_str(),
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
                                    connect_clicked => ViewAlbumMsg::PlayAlbum(None),
                                    add_css_class: "album-play-btn"
                                },
                                gtk::Button {
                                    set_valign: Align::Center,
                                    set_halign: Align::Center,
                                    set_icon_name: icon_names::ADD_REGULAR,
                                    connect_clicked => ViewAlbumMsg::QueueAlbum,
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
            mpris_player: init.1,
            album: init.0,
            song_list_factory: SignalListItemFactory::new(),
            album_cache: init.3,
        };

        let widgets: Self::Widgets = view_output!();

        model.song_list_factory.connect_setup(clone!(
            #[strong(rename_to = cover_cache)]
            init.2,
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
                    sender,
                    #[weak]
                    list_item,
                    move |_| {
                        let item = list_item.position();
                        sender.input(ViewAlbumMsg::PlayAlbum(Some(item as usize)));
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

        // let album_cache = init.3;
        glib::spawn_future_local(clone!(
            #[strong(rename_to = album)]
            model.album,
            #[weak(rename_to = song_list)]
            widgets.song_list,
            #[strong(rename_to = album_cache)]
            model.album_cache,
            async move {
                let album = if album.has_songs() {
                    album
                }else {
                    album_cache.get_album(album.id().as_str()).await.expect("Error getting album")
                };
                let list_store = ListStore::from_iter(
                    album
                        .get_songs()
                        .unwrap()
                        .iter()
                        .map(|x| SongObject::new((Uuid::from_u128(0), x.clone()).into(), PositionState::Passed)),
                );
                song_list.set_model(Some(&gtk::NoSelection::new(Some(list_store))));
            }
        ));

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
            ViewAlbumMsg::PlayAlbum(index) => {
                player.send_res(player.queue_album(self.album.id(), index, true).await);
            },
            ViewAlbumMsg::SetAlbum(album) => {
                if self.album.id() == album.id(){
                    return;
                }
                self.album = album;
                let album = if self.album.has_songs() {
                    &self.album
                }else {
                    &self.album_cache.get_album(self.album.id().as_str()).await.expect("Error getting album")
                };

                let songs: Vec<SongObject> = album
                    .get_songs()
                    .unwrap()
                    .iter()
                    .map(|x| SongObject::new((Uuid::from_u128(0), x.clone()).into(), PositionState::Passed))
                    .collect();
                let selection = widgets.song_list
                    .model()
                    .expect("Song list should have model set")
                    .downcast::<gtk::NoSelection>()
                    .expect("Song list model should be NoSelection");
                let store = selection.model().unwrap().downcast::<ListStore>().expect("Should be ListStore");
                store.splice(0, store.n_items(), &songs);
            },
            ViewAlbumMsg::QueueAlbum => {
                player.send_res(player.queue_album(self.album.id(), None, false).await);
            }
        };
        self.update_view(widgets, sender);
    }
}
