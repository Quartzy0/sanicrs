use std::rc::Rc;

use mpris_server::LocalServer;
use relm4::{adw::{self, prelude::NavigationPageExt}, gtk::{self, gio::{prelude::ListModelExt, ListStore}, glib::{clone, closure, object::Cast, Object}, prelude::{BoxExt, ButtonExt, GObjectPropertyExpressionExt, ListItemExt, WidgetExt}, Align, ListItem, Orientation, SignalListItemFactory, Widget}, prelude::{AsyncComponent, AsyncComponentParts}, AsyncComponentSender};
use relm4::adw::glib as glib;
use uuid::Uuid;

use crate::{dbus::player::MprisPlayer, icon_names, opensonic::cache::{AlbumCache, ArtistCache, CoverCache, SongCache}, ui::{album_object::AlbumObject, app::Init, artist_object::ArtistObject, cover_picture::{CoverPicture, CoverSize, CoverType}, song_object::{PositionState, SongObject}}};

pub struct SearchWidget {
    song_cache: SongCache,
    cover_cache: CoverCache,
    album_cache: AlbumCache,
    artist_cache: ArtistCache,
    mpris_player: Rc<LocalServer<MprisPlayer>>,
    current_type: SearchType,

    song_factory: SignalListItemFactory,
    album_factory: SignalListItemFactory,
    artist_factory: SignalListItemFactory,
}

#[derive(Debug)]
pub enum SearchMsg {
    ClickOption(u32),
    ClickRow(u32),
    Search(String, SearchType)
}

#[derive(Debug, Clone, Copy)]
pub enum SearchType {
    Song,
    Album,
    Artist
}

#[derive(Debug)]
pub enum SearchOut {
    ViewAlbum(AlbumObject),
    ViewArtist(ArtistObject)
}

#[relm4::component(pub async)]
impl AsyncComponent for SearchWidget {
    type CommandOutput = ();
    type Input = SearchMsg;
    type Output = SearchOut;
    type Init = Init;

    view! {
        adw::NavigationPage {
            set_tag: Some("search"),
            set_title: "Search",

            gtk::ScrolledWindow {
                set_hscrollbar_policy: gtk::PolicyType::Never,
                set_vexpand: true,
                set_vexpand_set: true,
                set_valign: Align::Fill,

                #[name = "list"]
                gtk::ListView {
                    #[watch]
                    set_factory: match model.current_type {
                        SearchType::Song => Some(&model.song_factory),
                        SearchType::Album => Some(&model.album_factory),
                        SearchType::Artist => Some(&model.artist_factory),
                    },
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
            mpris_player: init.6,
            song_cache: init.1,
            cover_cache: init.0,
            album_cache: init.2,
            artist_cache: init.7,
            song_factory: SignalListItemFactory::new(),
            album_factory: SignalListItemFactory::new(),
            artist_factory: SignalListItemFactory::new(),
            current_type: SearchType::Album,
        };

        let widgets: Self::Widgets = view_output!();

        model.song_factory.connect_setup(clone!(
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
                        sender.input(SearchMsg::ClickOption(item));
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
                        sender.input(SearchMsg::ClickOption(item));
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
                        sender.input(SearchMsg::ClickRow(item));
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

        model.artist_factory.connect_setup(clone!(
            #[strong(rename_to = cover_cache)]
            model.cover_cache,
            #[strong]
            sender,
            move |_, list_item| {
                let hbox = gtk::Box::builder()
                    .orientation(Orientation::Horizontal)
                    .build();
                hbox.add_css_class("album-song-item");

                let picture = CoverPicture::new(cover_cache.clone(), CoverSize::Small);
                picture.set_cover_type(CoverType::Round);
                let title = gtk::Label::new(None);
                hbox.append(&picture);
                hbox.append(&title);

                let list_item = list_item
                    .downcast_ref::<ListItem>()
                    .expect("Needs to be ListItem");
                list_item.set_child(Some(&hbox));

                let gesture = gtk::GestureClick::new();
                gesture.connect_released(clone!(
                    #[strong]
                    sender,
                    #[weak]
                    list_item,
                    move |_this, _n: i32, _x: f64, _y: f64| {
                        let item = list_item.position();
                        sender.input(SearchMsg::ClickRow(item));
                    }
                ));
                hbox.add_controller(gesture);

                list_item
                    .property_expression("item")
                    .chain_property::<ArtistObject>("name")
                    .bind(&title, "label", Widget::NONE);
                list_item
                    .property_expression("item")
                    .chain_property::<ArtistObject>("cover-art-id")
                    .bind(&picture, "cover-id", Widget::NONE);
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
            SearchMsg::ClickOption(index) => {
                if let Some(model) = widgets.list.model()
                    && let Some(item) = model.item(index) {
                    match self.current_type {
                        SearchType::Song => {
                            let item = item.downcast::<SongObject>().expect("Item should be SongObject").get_entry();
                            if let Some(item) = item {
                                player.send_res(player.set_song(item).await);
                            }
                        },
                        SearchType::Album => {
                            let item = item.downcast::<AlbumObject>().expect("Item should be AlbumObject");
                            player.send_res(player.queue_album(item.id(), None, true).await);
                        },
                        SearchType::Artist => todo!(),
                    }
                }
            },
            SearchMsg::ClickRow(index) => {
                if let Some(model) = widgets.list.model()
                    && let Some(item) = model.item(index) {
                    match self.current_type {
                        SearchType::Song => {},
                        SearchType::Album => {
                            let item = item.downcast::<AlbumObject>().expect("Item should be AlbumObject");
                            sender.output(SearchOut::ViewAlbum(item)).expect("Error sending out Album");
                        },
                        SearchType::Artist => {
                            let item = item.downcast::<ArtistObject>().expect("Item should be ArtistObject");
                            sender.output(SearchOut::ViewArtist(item)).expect("Error sending out Artist");
                        },
                    }
                }
            }
            SearchMsg::Search(query, search_type) => {
                self.current_type = search_type;
                match search_type {
                    SearchType::Song => {
                        let results = self.song_cache.search(&query, 20, None).await;
                        if let Err(err) = results {
                            player.send_error(err);
                        } else {
                            let results = results.unwrap();
                            let list_store = ListStore::from_iter(
                                results
                                    .iter()
                                    .map(|x| SongObject::new((Uuid::from_u128(0), x.clone()).into(), PositionState::Passed)),
                            );
                            widgets.list.set_model(Some(&gtk::NoSelection::new(Some(list_store))));
                        }
                    },
                    SearchType::Album => {
                        let results = self.album_cache.search(&query, 20, None).await;
                        if let Err(err) = results {
                            player.send_error(err);
                        } else {
                            let results = results.unwrap();
                            let list_store = ListStore::from_iter(results);
                            widgets.list.set_model(Some(&gtk::NoSelection::new(Some(list_store))));
                        }
                    },
                    SearchType::Artist => {
                        let results = self.artist_cache.search(&query, 20, None).await;
                        if let Err(err) = results {
                            player.send_error(err);
                        } else {
                            let results = results.unwrap();
                            let list_store = ListStore::from_iter(results);
                            widgets.list.set_model(Some(&gtk::NoSelection::new(Some(list_store))));
                        }
                    },
                }
            }
        }
        self.update_view(widgets, sender);
    }
}
