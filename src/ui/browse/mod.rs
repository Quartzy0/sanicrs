use relm4::adw::prelude::NavigationPageExt;
use relm4::component::{AsyncComponentController, AsyncConnector};
use std::rc::Rc;
use color_thief::Color;

mod album_list;
mod browse_page;
mod view_album_page;
pub(super) mod search;
mod view_artist_page;

use crate::dbus::player::MprisPlayer;
use crate::opensonic::cache::{AlbumCache, ArtistCache, CoverCache, SongCache};
use crate::ui::album_object::AlbumObject;
use crate::ui::app::Init;
use crate::ui::artist_object::ArtistObject;
use crate::ui::browse::browse_page::{BrowsePageOut, BrowsePageWidget};
use crate::ui::browse::search::{SearchMsg, SearchType, SearchWidget};
use crate::ui::browse::view_album_page::ViewAlbumWidget;
use crate::ui::browse::view_artist_page::ViewArtistWidget;
use mpris_server::LocalServer;
use relm4::component::AsyncComponentParts;
use relm4::prelude::{AsyncComponent, AsyncController};
use relm4::{adw, AsyncComponentSender};

pub struct BrowseWidget {
    cover_cache: CoverCache,
    mpris_player: Rc<LocalServer<MprisPlayer>>,
    album_cache: AlbumCache,
    artist_cache: ArtistCache,
    song_cache: SongCache,

    browse_page: AsyncController<BrowsePageWidget>,
    search_controller: AsyncConnector<SearchWidget>,
}

#[derive(Debug)]
pub enum BrowseMsg {
    ViewAlbum(AlbumObject, Option<u32>),
    ViewArtist(ArtistObject),
    Search(String, SearchType)
}

#[derive(Debug)]
pub enum BrowseMsgOut {
    PopView,
    ClearView,
    SetColors(Option<Vec<Color>>)
}

#[relm4::component(pub async)]
impl AsyncComponent for BrowseWidget {
    type CommandOutput = ();
    type Input = BrowseMsg;
    type Output = BrowseMsgOut;
    type Init = Init;

    view! {
        #[name = "navigation_view"]
        adw::NavigationView {
            add = model.browse_page.widget(),
            add = model.search_controller.widget(),

            connect_popped[sender] => move |_, _| {
                sender.output(BrowseMsgOut::PopView).expect("Error sending out popped message");
            }
        }
    }

    async fn init(
        init: Self::Init,
        root: Self::Root,
        sender: AsyncComponentSender<Self>,
    ) -> AsyncComponentParts<Self> {
        let browse_page = BrowsePageWidget::builder()
            .launch(init.clone())
            .forward(sender.output_sender(), |msg| match msg {
                BrowsePageOut::SetColors(c) => BrowseMsgOut::SetColors(c)
            });
        let search_controller = SearchWidget::builder()
            .launch(init.clone());
        let model = Self {
            mpris_player: init.6,
            cover_cache: init.0,
            album_cache: init.2,
            browse_page,
            search_controller,
            artist_cache: init.7,
            song_cache: init.1,
        };

        let widgets: Self::Widgets = view_output!();

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
            BrowseMsg::ViewAlbum(album, highlight) => {
                let view_album_page = ViewAlbumWidget::builder()
                    .launch((album, self.mpris_player.clone(), self.cover_cache.clone(), self.album_cache.clone(), self.artist_cache.clone(), highlight, self.song_cache.clone()));
                widgets.navigation_view.push(view_album_page.widget());
            },
            BrowseMsg::ViewArtist(artist) => {
                match self.artist_cache.ensure_albums(artist).await {
                    Ok(art) => {
                        let view_artist_page = ViewArtistWidget::builder()
                            .launch((art, self.mpris_player.clone(), self.cover_cache.clone(), self.album_cache.clone(), self.artist_cache.clone()));
                        widgets.navigation_view.push(view_artist_page.widget());
                    }
                    Err(e) => self.mpris_player.imp().send_error(e),
                }
            }
            BrowseMsg::Search(query, search_type) => {
                if widgets.navigation_view.visible_page()
                    .and_then(|t| t.tag())
                    .and_then(|t| Some(t!="search"))
                    .unwrap_or(true) {
                    widgets.navigation_view.replace_with_tags(&["browse", "search"]);
                    sender.output(BrowseMsgOut::ClearView).expect("Error sending out clear view message");
                }
                self.search_controller.emit(SearchMsg::Search(query, search_type));
            },
        }
        self.update_view(widgets, sender);
    }
}
