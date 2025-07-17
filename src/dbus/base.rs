use std::sync::Arc;
use tokio::sync::mpsc::UnboundedSender;
use zbus::interface;

pub enum PlayerCommand {
    Quit,
}

pub struct MprisBase {
    pub quit_channel: Arc<UnboundedSender<PlayerCommand>>,
}

#[interface(name = "org.mpris.MediaPlayer2")]
impl MprisBase {
    fn quit(&mut self) {
        self.quit_channel
            .send(PlayerCommand::Quit)
            .expect("Error when sending quit signal");
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
        true
    }

    #[zbus(property)]
    fn identity(&self) -> &str {
        "Sanic-rs"
    }

    #[zbus(property)]
    fn supported_uri_schemes(&self) -> Vec<&str> {
        vec!["sanic"]
    }

    #[zbus(property)]
    fn supported_mime_types(&self) -> Vec<&str> {
        vec![]
    }
}