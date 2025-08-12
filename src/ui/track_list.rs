use crate::dbus::player::MprisPlayer;
use crate::ui::app::Init;
use crate::ui::cover_picture::{CoverPicture, CoverSize};
use crate::ui::song_object::{PositionState, SongObject};
use crate::icon_names;
use mpris_server::LocalServer;
use relm4::adw::gio::ListStore;
use relm4::adw::glib::{clone, closure, Object};
use relm4::adw::gtk::{Align, ListItem, Orientation, SignalListItemFactory, Widget};
use relm4::adw::prelude::*;
use relm4::adw::{glib, gtk};
use relm4::gtk::pango::EllipsizeMode;
use relm4::prelude::*;
use std::rc::Rc;

pub struct TrackListWidget {
    mpris_player: Rc<LocalServer<MprisPlayer>>,

    factory: SignalListItemFactory,
}

#[derive(Debug)]
pub enum MoveDirection{
    Up,
    Down
}

#[derive(Debug)]
pub enum TrackListMsg {
    TrackActivated(usize),
    TrackChanged(Option<usize>),
    ReloadList,
    MoveItem{index: u32, direction: MoveDirection}
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
            factory: SignalListItemFactory::new(),
            mpris_player: init.6,
        };
        model.mpris_player.imp().tl_sender.replace(Some(sender.clone()));
        let widgets: Self::Widgets = view_output!();

        model.factory.connect_setup(clone!(
            #[strong(rename_to = cover_cache)]
            init.0,
            #[weak(rename_to = list)]
            widgets.list,
            #[strong]
            sender,
            move |_, list_item| {
            let center_box = gtk::CenterBox::builder()
                .orientation(Orientation::Horizontal)
                .hexpand(true)
                .hexpand_set(true)
                .halign(Align::Fill)
                .build();
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

            let btn_vbox = gtk::Box::builder()
                .orientation(Orientation::Vertical)
                .valign(Align::Start)
                .halign(Align::End)
                .build();
            let up_btn = gtk::Button::from_icon_name(icon_names::UP_SMALL);
            up_btn.add_css_class("no-bg");
            let down_btn = gtk::Button::from_icon_name(icon_names::DOWN_SMALL);
            down_btn.add_css_class("no-bg");
            btn_vbox.append(&up_btn);
            btn_vbox.append(&down_btn);

            center_box.set_start_widget(Some(&hbox));
            center_box.set_end_widget(Some(&btn_vbox));

            let list_item = list_item
                .downcast_ref::<ListItem>()
                .expect("Needs to be ListItem");
            list_item
                .set_child(Some(&center_box));

            up_btn.connect_clicked(clone!(
                #[strong]
                sender,
                #[weak]
                list_item,
                move |_| {
                    sender.input(TrackListMsg::MoveItem { index: list_item.position(), direction: MoveDirection::Up });
                }
            ));
            down_btn.connect_clicked(clone!(
                #[strong]
                sender,
                #[weak]
                list_item,
                move |_| {
                    sender.input(TrackListMsg::MoveItem { index: list_item.position(), direction: MoveDirection::Down });
                }
            ));

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
                .bind(&center_box, "css-classes", Widget::NONE);
            list_item
                .property_expression("position")
                .chain_closure::<bool>(closure!(
                    move |_: Option<Object>, pos: u32| {
                        pos != 0
                    }
                ))
                .bind(&up_btn, "visible", Widget::NONE);
            list_item
                .property_expression("position")
                .chain_closure::<Align>(closure!(
                    move |_: Option<Object>, pos: u32| {
                        if pos == 0 {
                            Align::End
                        } else {
                            Align::Start
                        }
                    }
                ))
                .bind(&btn_vbox, "valign", Widget::NONE);
            list_item
                .property_expression("position")
                .chain_closure::<bool>(closure!(
                    move |list_view: Option<gtk::ListView>, pos: u32| {
                        if let Some(list_view) = list_view {
                            if let Some(model) = list_view.model() {
                                return pos != model.n_items()-1
                            }
                        }
                        false
                    }
                ))
                .bind(&down_btn, "visible", Some(&list));
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
        let player = self.mpris_player.imp();
        match message {
            TrackListMsg::TrackActivated(i) => {
                player.send_res(player.goto(i).await);
            },
            TrackListMsg::TrackChanged(pos) => {
                let pos = match pos {
                    Some(p) => p,
                    None => player.track_list().borrow().current_index().unwrap_or(0),
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
                let guard = player.track_list().borrow();
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
            },
            TrackListMsg::MoveItem { index, direction } => {
                if let Some(model) = widgets.list.model() {
                    let model = model.downcast::<gtk::NoSelection>().expect("Model should be NoSelection");
                    if let Some(model) = model.model() {
                        let model = model.downcast::<ListStore>().expect("Model should be ListStore");
                        if let Some(item) = model.item(index) {
                            model.remove(index);
                            model.insert((index as i32 + match direction {
                                MoveDirection::Up => -1,
                                MoveDirection::Down => 1i32,
                            }) as u32, &item);
                            player.send_res(player.move_item(index as usize, direction).await);
                        }
                    }
                }
            }
        };
        self.update_view(widgets, sender);
    }
}
