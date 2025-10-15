use std::rc::Rc;

use crate::dbus::player::MprisPlayer;
use mpris_server::LocalServer;
use relm4::adw;
use relm4::adw::gio::Settings;
use relm4::adw::gtk;
use relm4::adw::prelude::*;
use relm4::gtk::{Align, InputPurpose, Orientation};
use relm4::prelude::*;
use crate::icon_names;

pub struct RandomSongsDialog {
    mpris_player: Rc<LocalServer<MprisPlayer>>,
    settings: Settings,
}

#[derive(Debug)]
pub enum RandomSongsMsg {
    PlayRandom(u32),
    AddRandom(u32),
}

pub type RandomSongsInit = (Rc<LocalServer<MprisPlayer>>, Settings);

#[relm4::component(pub async)]
impl AsyncComponent for RandomSongsDialog {
    type CommandOutput = ();
    type Input = RandomSongsMsg;
    type Output = ();
    type Init = RandomSongsInit;

    view! {
        adw::Dialog {
            set_title: "Add random",

            #[wrap(Some)]
            set_child = &adw::ToolbarView{
                add_top_bar = &adw::HeaderBar {

                },

                gtk::Box {
                    set_orientation: Orientation::Vertical,
                    set_spacing: 10,
                    add_css_class: "padded",

                    gtk::Label {
                        set_label: "Number of songs"
                    },
                    #[name = "songs_n"]
                    gtk::Entry {
                        set_input_purpose: InputPurpose::Digits,
                        set_text: &model.settings.uint("random-songs-prefill").to_string(),
                    },
                    gtk::Box {
                        set_orientation: Orientation::Horizontal,
                        set_spacing: 10,
                        set_halign: Align::Fill,

                        gtk::Button {
                            #[wrap(Some)]
                            set_child = &adw::ButtonContent {
                                set_label: "Play",
                                set_icon_name: icon_names::PLAY,
                            },
                            set_hexpand: true,
                            set_tooltip: "Play random songs",
                            connect_clicked[sender, songs_n, mplayer, root] => move |_| {
                                let size = songs_n.text().parse::<u32>();
                                if size.is_ok() {
                                    sender.input(RandomSongsMsg::PlayRandom(size.unwrap()));
                                    root.close();
                                } else {
                                    mplayer.imp().send_error(size.err().unwrap().into());
                                }
                            }
                        },
                        gtk::Button {
                            #[wrap(Some)]
                            set_child = &adw::ButtonContent {
                                set_label: "Add",
                                set_icon_name: icon_names::ADD_REGULAR,
                            },
                            set_hexpand: true,
                            set_tooltip: "Add random songs",
                            connect_clicked[sender, songs_n, mplayer, root] => move |_| {
                                let size = songs_n.text().parse::<u32>();
                                if size.is_ok() {
                                    sender.input(RandomSongsMsg::AddRandom(size.unwrap()));
                                    root.close();
                                } else {
                                    mplayer.imp().send_error(size.err().unwrap().into());
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    async fn init(
        init: Self::Init,
        root: adw::Dialog,
        sender: AsyncComponentSender<Self>,
    ) -> AsyncComponentParts<Self> {
        let model = Self { mpris_player: init.0, settings: init.1 };

        let mplayer = &model.mpris_player;
        let widgets: RandomSongsDialogWidgets = view_output!();

        AsyncComponentParts { model, widgets }
    }

    async fn update(
        &mut self,
        message: Self::Input,
        _sender: AsyncComponentSender<Self>,
        _root: &Self::Root,
    ) {
        let player = self.mpris_player.imp();
        match message {
            RandomSongsMsg::PlayRandom(count) => {
                let res = self.settings.set_uint("random-songs-prefill", count);
                if let Err(err) = res {
                    player.send_error(err.into());
                }
                player.send_res(player
                    .queue_random(count, None, None, None, true)
                    .await);
            },
            RandomSongsMsg::AddRandom(count) => {
                let res = self.settings.set_uint("random-songs-prefill", count);
                if let Err(err) = res {
                    player.send_error(err.into());
                }
                player.send_res(player
                    .queue_random(count, None, None, None, false)
                    .await);
            }
        }
    }
}
