use crate::player::PlayerCommand::{Next, Pause, Play, PlayPause, Previous, Quit};
use crate::player::PlayerCommand;
use std::sync::Arc;
use tokio::sync::mpsc::UnboundedSender;
use zbus::interface;

pub struct MprisPlayer {
    cmd_channel: Arc<UnboundedSender<PlayerCommand>>,
}

impl MprisPlayer {
    pub fn new(cmd_channel: Arc<UnboundedSender<PlayerCommand>>) -> Self{
        MprisPlayer {
            cmd_channel
        }
    }
}

#[interface(name = "org.mpris.MediaPlayer2.Player")]
impl MprisPlayer {
    fn play(&self) {
        self.cmd_channel.send(Play).expect("Error when sending play signal");
    }

    fn pause(&self) {
        self.cmd_channel.send(Pause).expect("Error when sending pause signal");
    }

    fn play_pause(&self) {
        self.cmd_channel.send(PlayPause).expect("Error when sending playpause signal");
    }

    fn next(&self) {
        self.cmd_channel.send(Next).expect("Error when sending next signal");
    }

    fn previous(&self) {
        self.cmd_channel.send(Previous).expect("Error when sending next signal");
    }

    #[zbus(property)]
    fn can_go_next(&self) -> bool {
        true
    }

    #[zbus(property)]
    fn can_go_previous(&self) -> bool {
        true
    }

    #[zbus(property)]
    fn can_play(&self) -> bool {
        true
    }

    #[zbus(property)]
    fn can_pause(&self) -> bool {
        true
    }

    #[zbus(property)]
    fn can_seek(&self) -> bool {
        false // TODO: Implement
    }

    #[zbus(property)]
    fn can_control(&self) -> bool {
        true
    }
}

pub struct MprisBase {
    pub quit_channel: Arc<UnboundedSender<PlayerCommand>>
}

#[interface(name = "org.mpris.MediaPlayer2")]
impl MprisBase {
    fn quit(&mut self) {
        self.quit_channel.send(Quit).expect("Error when sending quit signal");
    }

    #[zbus(property)]
    fn can_quit(&self) -> bool {
        true
    }

    #[zbus(property)]
    fn can_set_fullscreen(&self) -> bool {
        false
    }

    #[zbus(property)]
    fn can_raise(&self) -> bool {
        false
    }

    #[zbus(property)]
    fn has_track_list(&self) -> bool {
        false // TODO: Implement this
    }

    #[zbus(property)]
    fn identity(&self) -> &str {
        "Sanic-rs"
    }
}