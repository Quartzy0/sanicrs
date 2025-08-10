use std::rc::Rc;

use crate::dbus::player::MprisPlayer;
use mpris_server::LocalServer;
use relm4::adw;
use relm4::adw::gtk;
use relm4::adw::prelude::*;
use relm4::gtk::Orientation;
use relm4::prelude::*;

pub struct RandomSongsDialog {
    mpris_player: Rc<LocalServer<MprisPlayer>>,
}

#[derive(Debug)]
pub enum RandomSongsMsg {
    AddRandom(u32),
}

pub type RandomSongsInit = Rc<LocalServer<MprisPlayer>>;

#[relm4::component(pub async)]
impl AsyncComponent for RandomSongsDialog {
    type CommandOutput = ();
    type Input = RandomSongsMsg;
    type Output = ();
    type Init = RandomSongsInit;

    view! {
        adw::Dialog {
            #[wrap(Some)]
            set_child = &gtk::Box {
                set_orientation: Orientation::Vertical,
                set_spacing: 5,
                add_css_class: "padded",

                gtk::Label {
                    set_label: "Number of songs"
                },
                #[name = "songs_n"]
                gtk::Entry {},
                gtk::Button {
                    set_label: "Add",
                    connect_clicked[sender, songs_n] => move |_| {
                        let size = songs_n.text().parse::<u32>();
                        if size.is_ok() {
                            sender.input(RandomSongsMsg::AddRandom(size.unwrap()));
                        }
                        root.close();
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
        let model = Self { mpris_player: init };

        let widgets: RandomSongsDialogWidgets = view_output!();

        AsyncComponentParts { model, widgets }
    }

    async fn update(
        &mut self,
        message: Self::Input,
        _sender: AsyncComponentSender<Self>,
        _root: &Self::Root,
    ) {
        match message {
            RandomSongsMsg::AddRandom(count) => {
                self.mpris_player
                    .imp()
                    .queue_random(count, None, None, None)
                    .await
                    .expect("Error sending message to player");
            }
        }
    }
}
