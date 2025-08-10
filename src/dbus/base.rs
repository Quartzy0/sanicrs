use mpris_server::{LocalRootInterface};
use mpris_server::zbus::fdo::Result;
use crate::dbus::player::MprisPlayer;
use crate::PlayerCommand;
use crate::ui::app::AppMsg;

impl MprisPlayer {
    pub async fn restart(&self) {
        self.cmd_channel
            .send(PlayerCommand::Quit(true))
            .await
            .expect("Error when sending restart signal");
    }

    pub async fn close(&self) {
        self.cmd_channel
            .send(PlayerCommand::Close)
            .await
            .expect("Error when sending close signal");
    }

    pub async fn quit_no_app(&self) {
        self.cmd_channel
            .send(PlayerCommand::Quit(false))
            .await
            .expect("Error when sending quit signal");
    }
}

impl LocalRootInterface for MprisPlayer {
    async fn raise(&self) -> Result<()> {
        self.cmd_channel
            .send(PlayerCommand::Raise)
            .await
            .expect("Error when sending raise signal");
        Ok(())
    }

    async fn quit(&self) -> Result<()> {
        self.send_app_msg(AppMsg::Quit);
        Ok(())
    }

    async fn can_quit(&self) -> Result<bool> {
        Ok(true)
    }

    async fn fullscreen(&self) -> Result<bool> {
        Ok(false)
    }

    async fn set_fullscreen(&self, _fullscreen: bool) -> std::result::Result<(), zbus::Error> {
        Ok(())
    }

    async fn can_set_fullscreen(&self) -> Result<bool> {
        Ok(false)
    }

    async fn can_raise(&self) -> Result<bool> {
        Ok(true)
    }

    async fn has_track_list(&self) -> Result<bool> {
        Ok(true)
    }

    async fn identity(&self) -> Result<String> {
        Ok("Sanic-rs".into())
    }

    async fn desktop_entry(&self) -> Result<String> {
        Ok("".into())
    }

    async fn supported_uri_schemes(&self) -> Result<Vec<String>> {
        Ok(vec!["sanic".into()])
    }

    async fn supported_mime_types(&self) -> Result<Vec<String>> {
        Ok(vec![])
    }
}