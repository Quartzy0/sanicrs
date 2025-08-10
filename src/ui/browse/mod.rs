use relm4::component::{AsyncComponentController, AsyncConnector};
use std::rc::Rc;
mod album_list;
mod browse_page;
mod view_album_page;

use crate::dbus::player::MprisPlayer;
use crate::opensonic::cache::{AlbumCache, CoverCache};
use crate::ui::album_object::AlbumObject;
use crate::ui::app::Init;
use crate::ui::browse::browse_page::{BrowsePageOut, BrowsePageWidget};
use crate::ui::browse::view_album_page::{ViewAlbumMsg, ViewAlbumWidget};
use mpris_server::LocalServer;
use relm4::adw::gtk;
use relm4::adw::gtk::Align;
use relm4::adw::prelude::*;
use relm4::component::AsyncComponentParts;
use relm4::prelude::{AsyncComponent, AsyncController};
use relm4::{adw, AsyncComponentSender};

pub struct BrowseWidget {
    cover_cache: CoverCache,
    mpris_player: Rc<LocalServer<MprisPlayer>>,
    album_cache: AlbumCache,

    browse_page: AsyncController<BrowsePageWidget>,
    view_album_page: Option<AsyncConnector<ViewAlbumWidget>>
}

#[derive(Debug)]
pub enum BrowseMsg {
    ViewAlbum(AlbumObject),
}

#[relm4::component(pub async)]
impl AsyncComponent for BrowseWidget {
    type CommandOutput = ();
    type Input = BrowseMsg;
    type Output = ();
    type Init = Init;

    view! {
        gtk::ScrolledWindow {
            set_hscrollbar_policy: gtk::PolicyType::Never,
            set_vexpand: true,
            set_vexpand_set: true,
            set_valign: Align::Fill,

            #[name = "navigation_view"]
            adw::NavigationView {
                add = model.browse_page.widget(),
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
            .forward(sender.input_sender(), |msg| match msg {
                BrowsePageOut::ViewAlbum(a) => BrowseMsg::ViewAlbum(a)
            });
        let model = Self {
            mpris_player: init.7,
            cover_cache: init.1,
            album_cache: init.3,
            browse_page,
            view_album_page: None
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
                if let Some(view_album_page) = &self.view_album_page {
                    view_album_page.sender().send(ViewAlbumMsg::SetAlbum(album)).expect("Error sending message to album view");
                    widgets.navigation_view.push_by_tag("view-album");
                } else {
                    let view_album_page = ViewAlbumWidget::builder()
                        .launch((album, self.mpris_player.clone(), self.cover_cache.clone(), self.album_cache.clone()));
                    widgets.navigation_view.add(view_album_page.widget());
                    widgets.navigation_view.push(view_album_page.widget());
                    self.view_album_page = Some(view_album_page);
                }
            }
        }
        self.update_view(widgets, sender);
    }
}
