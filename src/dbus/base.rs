use std::sync::Arc;
use async_channel::Sender;
use zbus::interface;
use crate::PlayerCommand;

pub struct MprisBase {
    pub cmd_channel: Arc<Sender<PlayerCommand>>,
}

#[interface(name = "org.mpris.MediaPlayer2")]
impl MprisBase {
    async fn quit(&self) {
        self.cmd_channel
            .send(PlayerCommand::Quit)
            .await
            .expect("Error when sending quit signal");
    }

    async fn raise(&self) {
        self.cmd_channel
            .send(PlayerCommand::Raise)
            .await
            .expect("Error when sending raise signal");
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
        true
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