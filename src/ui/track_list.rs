use crate::player::TrackList;
use crate::ui::app::Init;
use crate::ui::cover_picture::{CoverPicture, CoverSize};
use crate::ui::song_object::{PositionState, SongObject};
use relm4::adw::gio::ListStore;
use relm4::adw::glib::{clone, closure, Object};
use relm4::adw::gtk::{Align, ListItem, Orientation, SignalListItemFactory, Widget};
use relm4::adw::prelude::*;
use relm4::adw::{glib, gtk};
use relm4::prelude::*;
use std::sync::Arc;
use async_channel::Sender;
use relm4::gtk::pango::EllipsizeMode;
use tokio::sync::RwLock;
use crate::PlayerCommand;

pub struct TrackListWidget {
    track_list: Arc<RwLock<TrackList>>,
    cmd_sender: Arc<Sender<PlayerCommand>>,

    factory: SignalListItemFactory,
}

#[derive(Debug)]
pub enum TrackListMsg {
    TrackActivated(usize),
    TrackChanged(Option<usize>),
    ReloadList,
}

#[relm4::component(pub async)]
impl AsyncComponent for TrackListWidget {
    type CommandOutput = ();
    type Input = TrackListMsg;
    type Output = ();
    type Init = Init;

    view! {
        gtk::Box {
            set_orientation: Orientation::Vertical,
            add_css_class: "track-list",
            add_css_class: "t2",

            gtk::Box {
                set_orientation: Orientation::Vertical,
                add_css_class: "no-bg",
                add_css_class: "padded",

                gtk::Label {
                    set_label: "Song Queue",
                    add_css_class: "bold"
                },
            },
            gtk::Separator {
                set_orientation: Orientation::Horizontal,
            },
            gtk::ScrolledWindow {
                set_hscrollbar_policy: gtk::PolicyType::Never,
                set_min_content_width: 360,
                set_vexpand: true,
                set_vexpand_set: true,
                set_valign: Align::Fill,
                add_css_class: "no-bg",

                #[name = "list"]
                gtk::ListView {
                    set_factory: Some(&model.factory),
                    set_single_click_activate: true,
                    add_css_class: "no-bg",

                    connect_activate[sender] => move |_, index| {
                        sender.input(TrackListMsg::TrackActivated(index as usize));
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
        let model = TrackListWidget {
            track_list: init.1,
            factory: SignalListItemFactory::new(),
            cmd_sender: init.3,
        };
        model.cmd_sender.send(PlayerCommand::TrackListSendSender(sender.clone())).await.expect("Error sending sender to player");
        let widgets: Self::Widgets = view_output!();

        model.factory.connect_setup(clone!(
            #[strong(rename_to = cover_cache)]
            init.2,
            move |_, list_item| {
            let hbox = gtk::Box::builder()
                .orientation(Orientation::Horizontal)
                .valign(Align::Start)
                .halign(Align::Start)
                .spacing(10)
                .hexpand(true)
                .hexpand_set(true)
                .halign(Align::Fill)
                .build();
            let vbox = gtk::Box::builder()
                .orientation(Orientation::Vertical)
                .valign(Align::Start)
                .halign(Align::Start)
                .build();

            let title = gtk::Label::new(None);
            title.set_halign(Align::Start);
            title.add_css_class("bold");
            title.set_max_width_chars(30);
            title.set_ellipsize(EllipsizeMode::End);
            vbox.append(&title);
            let artist = gtk::Label::new(None);
            artist.set_halign(Align::Start);
            artist.set_max_width_chars(30);
            artist.set_ellipsize(EllipsizeMode::End);
            vbox.append(&artist);

            let picture = CoverPicture::new(cover_cache.clone(), CoverSize::Small);
            hbox.append(&picture);
            hbox.append(&vbox);

            let list_item = list_item
                .downcast_ref::<ListItem>()
                .expect("Needs to be ListItem");
            list_item
                .set_child(Some(&hbox));

            list_item
                .property_expression("item")
                .chain_property::<SongObject>("title")
                .bind(&title, "label", Widget::NONE);
            list_item
                .property_expression("item")
                .chain_property::<SongObject>("artist")
                .bind(&artist, "label", Widget::NONE);
            list_item
                .property_expression("item")
                .chain_property::<SongObject>("cover-art-id")
                .bind(&picture, "cover-id", Widget::NONE);
            list_item
                .property_expression("item")
                .chain_property::<SongObject>("position-state")
                .chain_closure::<Vec<String>>(closure!(
                    move |_: Option<Object>, position_state: PositionState| {
                        match position_state {
                            PositionState::Passed => vec!["track-list-item".to_string(), "passed".to_string()],
                            PositionState::Current => vec!["track-list-item".to_string(), "current".to_string()],
                            PositionState::Upcoming => vec!["track-list-item".to_string(), "upcoming".to_string()]
                        }
                }))
                .bind(&hbox, "css-classes", Widget::NONE);
        }));

        sender.input(TrackListMsg::ReloadList);

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
            TrackListMsg::TrackActivated(i) => self.cmd_sender.send(PlayerCommand::GoTo(i)).await.expect("Error sending message to player"),
            TrackListMsg::TrackChanged(pos) => {
                let pos = match pos {
                    Some(p) => p,
                    None => self.track_list.read().await.current_index().unwrap_or(0),
                };
                
                let model = widgets.list.model();
                if let Some(model) = model {
                    model.iter::<Object>().enumerate().for_each(|x| {
                        if let (i, Ok(song)) = x {
                            song
                                .downcast::<SongObject>()
                                .expect("Must be SongObject.")
                                .set_position_state(
                                    if i < pos {
                                        PositionState::Passed
                                    } else if i > pos {
                                        PositionState::Upcoming
                                    } else {
                                        PositionState::Current
                                    }
                                );
                        }
                    });
                }
            },
            TrackListMsg::ReloadList => {
                let guard = self.track_list.read().await;
                let pos = guard.current_index().unwrap_or(0);
                let list_store = ListStore::from_iter(guard.get_songs().iter().enumerate().map(|x1| {
                    SongObject::new(x1.1.clone(), if x1.0 < pos {
                        PositionState::Passed
                    } else if x1.0 > pos {
                        PositionState::Upcoming
                    } else {
                        PositionState::Current
                    })
                }));

                widgets.list.set_model(Some(&gtk::NoSelection::new(Some(list_store))));
            }
        };
        self.update_view(widgets, sender);
    }
}