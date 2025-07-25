use std::sync::Arc;

use async_channel::Sender;
use relm4::gtk::Orientation;
use relm4::prelude::*;
use relm4::adw::prelude::*;
use relm4::adw::gtk;
use relm4::adw;
use crate::PlayerCommand;


pub struct RandomSongsDialog {

}

#[derive(Debug)]
pub enum RandomSongsMsg {
}

pub type RandomSongsInit = (
    Arc<Sender<PlayerCommand>>
);

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
                    connect_clicked[init, songs_n] => move |_| {
                        let size = songs_n.text().parse::<u32>();
                        if size.is_ok() {
                            init
                                .send_blocking(PlayerCommand::QueueRandom { size: size.unwrap(), genre: None, from_year: None, to_year: None })
                                .expect("Error sending message to player");
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
        let model = Self {
        };

        let widgets: RandomSongsDialogWidgets = view_output!();

        AsyncComponentParts { model, widgets }
    }


}
