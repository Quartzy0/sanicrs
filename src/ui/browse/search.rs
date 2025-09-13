use std::rc::Rc;

use mpris_server::LocalServer;
use relm4::adw::glib;
use relm4::adw::prelude::ToVariant;
use relm4::component::AsyncComponentController;
use relm4::{
    AsyncComponentSender,
    adw::{self, prelude::NavigationPageExt},
    gtk::{
        self, Align,
        glib::{clone, object::Cast},
        prelude::WidgetExt,
    },
    prelude::{AsyncComponent, AsyncComponentParts},
};
use uuid::Uuid;

use crate::ui::item_list::{ItemListInit, ItemListWidget};
use crate::{
    dbus::player::MprisPlayer,
    opensonic::cache::{AlbumCache, ArtistCache, CoverCache, SongCache},
    ui::{
        album_object::AlbumObject,
        app::Init,
        artist_object::ArtistObject,
        song_object::{PositionState, SongObject},
    },
};
use crate::ui::cover_picture::CoverType;

pub struct SearchWidget {
    song_cache: SongCache,
    cover_cache: CoverCache,
    album_cache: AlbumCache,
    artist_cache: ArtistCache,
    mpris_player: Rc<LocalServer<MprisPlayer>>,
    current_type: SearchType,
}

#[derive(Debug)]
pub enum SearchMsg {
    Search(String, SearchType),
}

#[derive(Debug, Clone, Copy)]
pub enum SearchType {
    Song,
    Album,
    Artist,
}

#[relm4::component(pub async)]
impl AsyncComponent for SearchWidget {
    type CommandOutput = ();
    type Input = SearchMsg;
    type Output = ();
    type Init = Init;

    view! {
        adw::NavigationPage {
            set_tag: Some("search"),
            set_title: "Search",

            #[name = "scrolled"]
            gtk::ScrolledWindow {
                set_hscrollbar_policy: gtk::PolicyType::Never,
                set_vexpand: true,
                set_vexpand_set: true,
                set_valign: Align::Fill,
            }
        }
    }

    async fn init(
        init: Self::Init,
        root: Self::Root,
        _sender: AsyncComponentSender<Self>,
    ) -> AsyncComponentParts<Self> {
        let model = Self {
            mpris_player: init.6,
            song_cache: init.1,
            cover_cache: init.0,
            album_cache: init.2,
            artist_cache: init.7,
            current_type: SearchType::Album,
        };

        let widgets: Self::Widgets = view_output!();

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
            SearchMsg::Search(query, search_type) => {
                self.current_type = search_type;
                let mpris_player = self.mpris_player.clone();
                match search_type {
                    SearchType::Song => {
                        let song_cache = self.song_cache.clone();
                        let item_list_widget = ItemListWidget::builder().launch(ItemListInit {
                            mpris_player: self.mpris_player.clone(),
                            cover_type: CoverType::Square,
                            cover_cache: self.cover_cache.clone(),
                            play_fn: Some(Box::new(|item: SongObject, _i, mpris_player| {
                                if let Some(item) = item.get_entry() {
                                    let mpris_player = mpris_player.clone();
                                    relm4::spawn_local(async move {
                                        let mpris_player = mpris_player.clone();
                                        mpris_player
                                            .imp()
                                            .send_res(mpris_player.imp().set_song(item).await);
                                    });
                                }
                            })),
                            click_fn: None,
                            load_items: async move {
                                let results = song_cache.search(&query, 20, None).await;
                                if let Err(err) = results {
                                    mpris_player.imp().send_error(err);
                                    Vec::new()
                                } else {
                                    results
                                        .unwrap()
                                        .iter()
                                        .map(|x| {
                                            SongObject::new(
                                                (Uuid::from_u128(0), x.clone()).into(),
                                                PositionState::Passed,
                                            )
                                        })
                                        .collect()
                                }
                            },
                            highlight: None,
                        });
                        widgets.scrolled.set_child(Some(item_list_widget.widget()));
                    }
                    SearchType::Album => {
                        let album_cache = self.album_cache.clone();
                        let item_list_widget = ItemListWidget::builder().launch(ItemListInit {
                            mpris_player: self.mpris_player.clone(),
                            cover_type: CoverType::Square,
                            cover_cache: self.cover_cache.clone(),
                            play_fn: Some(Box::new(|item: AlbumObject, _i, mpris_player| {
                                let mpris_player = mpris_player.clone();
                                relm4::spawn_local(async move {
                                    let mpris_player = mpris_player.clone();
                                    mpris_player.imp().send_res(
                                        mpris_player.imp().queue_album(item.id(), None, true).await,
                                    );
                                });
                            })),
                            click_fn: Some(Box::new(clone!(
                                #[weak]
                                root,
                                move |item, _i, _mpris_player| {
                                    let album = item
                                        .downcast::<AlbumObject>()
                                        .expect("Item should be AlbumObject");
                                    root.activate_action(
                                        "win.album",
                                        Some(&album.id().to_variant()),
                                    )
                                    .expect("Error executing action");
                                }
                            ))),
                            load_items: async move {
                                let results = album_cache.search(&query, 20, None).await;
                                if let Err(err) = results {
                                    mpris_player.imp().send_error(err);
                                    Vec::new()
                                } else {
                                    results.unwrap().into_iter().collect()
                                }
                            },
                            highlight: None,
                        });
                        widgets.scrolled.set_child(Some(item_list_widget.widget()));
                    }
                    SearchType::Artist => {
                        let artist_cache = self.artist_cache.clone();
                        let item_list_widget = ItemListWidget::builder().launch(ItemListInit {
                            mpris_player: self.mpris_player.clone(),
                            cover_type: CoverType::Round,
                            cover_cache: self.cover_cache.clone(),
                            play_fn: None,
                            click_fn: Some(Box::new(clone!(
                                #[weak]
                                root,
                                move |artist: ArtistObject, _i, _mpris_player| {
                                    root.activate_action(
                                        "win.artist",
                                        Some(&artist.id().to_variant()),
                                    )
                                    .expect("Error executing action");
                                }
                            ))),
                            load_items: async move {
                                let results = artist_cache.search(&query, 20, None).await;
                                if let Err(err) = results {
                                    mpris_player.imp().send_error(err);
                                    Vec::new()
                                } else {
                                    results.unwrap().into_iter().collect()
                                }
                            },
                            highlight: None,
                        });
                        widgets.scrolled.set_child(Some(item_list_widget.widget()));
                    }
                };
            }
        }
        self.update_view(widgets, sender);
    }
}
