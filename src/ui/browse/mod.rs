use relm4::adw::prelude::NavigationPageExt;
use relm4::component::{AsyncComponentController, AsyncConnector};
use std::cell::OnceCell;
use std::rc::Rc;
mod album_list;
mod browse_page;
mod view_album_page;
pub(super) mod search;

use crate::dbus::player::MprisPlayer;
use crate::opensonic::cache::{AlbumCache, CoverCache};
use crate::ui::album_object::AlbumObject;
use crate::ui::app::Init;
use crate::ui::browse::browse_page::{BrowsePageOut, BrowsePageWidget};
use crate::ui::browse::search::{SearchMsg, SearchOut, SearchType, SearchWidget};
use crate::ui::browse::view_album_page::{ViewAlbumMsg, ViewAlbumWidget};
use mpris_server::LocalServer;
use relm4::component::AsyncComponentParts;
use relm4::prelude::{AsyncComponent, AsyncController};
use relm4::{adw, AsyncComponentSender};

pub struct BrowseWidget {
    cover_cache: CoverCache,
    mpris_player: Rc<LocalServer<MprisPlayer>>,
    album_cache: AlbumCache,

    browse_page: AsyncController<BrowsePageWidget>,
    search_controller: AsyncController<SearchWidget>,
    view_album_page: OnceCell<AsyncConnector<ViewAlbumWidget>>
}

#[derive(Debug)]
pub enum BrowseMsg {
    ViewAlbum(AlbumObject),
    Search(String, SearchType)
}

#[relm4::component(pub async)]
impl AsyncComponent for BrowseWidget {
    type CommandOutput = ();
    type Input = BrowseMsg;
    type Output = ();
    type Init = Init;

    view! {
        #[name = "navigation_view"]
        adw::NavigationView {
            add = model.browse_page.widget(),
            add = model.search_controller.widget(),
        }
    }

    async fn init(
        init: Self::Init,
        root: Self::Root,
        sender: AsyncComponentSender<Self>,
    ) -> AsyncComponentParts<Self> {
        let browse_page = BrowsePageWidget::builder()
            .launch(init.clone())
            .forward(sender.input_sender(), |msg| match msg {
                BrowsePageOut::ViewAlbum(a) => BrowseMsg::ViewAlbum(a)
            });
        let search_controller = SearchWidget::builder()
            .launch(init.clone())
            .forward(sender.input_sender(), |msg| match msg {
                SearchOut::ViewAlbum(a) => BrowseMsg::ViewAlbum(a)
            });
        let model = Self {
            mpris_player: init.6,
            cover_cache: init.0,
            album_cache: init.2,
            browse_page,
            view_album_page: OnceCell::new(),
            search_controller
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
            BrowseMsg::ViewAlbum(album) => {
                match self.view_album_page.get() {
                    Some(view_album_page) => {
                        view_album_page.sender().send(ViewAlbumMsg::SetAlbum(album)).expect("Error sending message to album view");
                        if widgets.navigation_view.visible_page()
                            .and_then(|p| p.tag().and_then(|t| Some(t!="view-album")))
                            .unwrap_or(true) {
                            widgets.navigation_view.push_by_tag("view-album");
                        }
                    },
                    None => {
                        let view_album_page = ViewAlbumWidget::builder()
                            .launch((album, self.mpris_player.clone(), self.cover_cache.clone(), self.album_cache.clone()));
                        widgets.navigation_view.add(view_album_page.widget());
                        widgets.navigation_view.push(view_album_page.widget());
                        self.view_album_page.set(view_album_page).expect("Error setting OnceCell for album page");
                    },
                }
            },
            BrowseMsg::Search(query, search_type) => {
                if widgets.navigation_view.visible_page()
                    .and_then(|t| t.tag())
                    .and_then(|t| Some(t!="search"))
                    .unwrap_or(true) {
                    widgets.navigation_view.replace_with_tags(&["browse", "search"]);
                }
                self.search_controller.emit(SearchMsg::Search(query, search_type));
            },
        }
        self.update_view(widgets, sender);
    }
}
