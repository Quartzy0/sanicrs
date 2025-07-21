use crate::PlayerCommand;
use crate::dbus::player;
use crate::opensonic::client::OpenSubsonicClient;
use crate::player::{SongEntry, TrackList};
use std::collections::HashMap;
use std::sync::Arc;
use async_channel::Sender;
use tokio::sync::RwLock;
use zbus::interface;
use zbus::object_server::SignalEmitter;
use zvariant::{ObjectPath, Value};

pub struct MprisTrackList {
    pub track_list: Arc<RwLock<TrackList>>,
    pub client: Arc<OpenSubsonicClient>,
    pub cmd_channel: Arc<Sender<PlayerCommand>>,
}

#[interface(name = "org.mpris.MediaPlayer2.TrackList")]
impl MprisTrackList {
    #[zbus(signal)]
    pub async fn track_list_replaced(
        emitter: &SignalEmitter<'_>,
        tracks: Vec<ObjectPath<'_>>,
        current: ObjectPath<'_>,
    ) -> Result<(), zbus::Error>;

    #[zbus(signal)]
    pub async fn track_added(
        emitter: &SignalEmitter<'_>,
        metadata: HashMap<&str, Value<'_>>,
        after_track: ObjectPath<'_>,
    ) -> Result<(), zbus::Error>;

    #[zbus(signal)]
    pub async fn track_removed(
        emitter: &SignalEmitter<'_>,
        track_id: ObjectPath<'_>,
    ) -> Result<(), zbus::Error>;

    #[zbus(signal)]
    pub async fn track_metadata_changed(
        emitter: &SignalEmitter<'_>,
        track: ObjectPath<'_>,
        metadata: HashMap<&str, Value<'_>>,
    ) -> Result<(), zbus::Error>;

    async fn add_track(&self, uri: String, after_track: ObjectPath<'_>, set_as_current: bool) {
        let index: Option<usize> =
            if after_track.as_str() == "/org/mpris/MediaPlayer2/TrackList/NoTrack" {
                Some(0)
            } else {
                let track_list = self.track_list.read().await;
                track_list
                    .get_songs()
                    .iter()
                    .position(|x| x.dbus_path() == after_track.as_str())
                    .and_then(|t| Some(t + 1))
            };
        self.cmd_channel
            .send(PlayerCommand::AddFromUri(uri, index, set_as_current))
            .await
            .expect("Error sending message to player");
    }

    async fn remove_track(&self, track_id: ObjectPath<'_>) -> Result<(), zbus::fdo::Error> {
        let index: usize = {
            let track_list = self.track_list.read().await;
            track_list
                .get_songs()
                .iter()
                .position(|x| x.dbus_path() == track_id.as_str())
                .ok_or(zbus::fdo::Error::Failed("Track not found".to_string()))?
        };
        self.cmd_channel
            .send(PlayerCommand::Remove(index))
            .await
            .expect("Error sending message to player");
        Ok(())
    }

    async fn go_to(&self, track_id: ObjectPath<'_>) -> Result<(), zbus::fdo::Error> {
        let index: usize = {
            let track_list = self.track_list.read().await;
            track_list
                .get_songs()
                .iter()
                .position(|x| x.dbus_path() == track_id.as_str())
                .ok_or(zbus::fdo::Error::Failed("Track not found".to_string()))?
        };
        self.cmd_channel
            .send(PlayerCommand::GoTo(index))
            .await
            .expect("Error sending message to player");
        Ok(())
    }

    async fn get_tracks_metadata(
        &self,
        tracks_in: Vec<ObjectPath<'_>>,
    ) -> Vec<HashMap<&str, Value>> {
        let track_list = self.track_list.read().await;
        let mut songs_refs: Vec<&SongEntry> = Vec::new();
        let loaded_songs = track_list.get_songs();
        for x in tracks_in {
            let song = loaded_songs.iter().find(|x1| x1.dbus_path() == x.as_str());
            match song {
                None => {}
                Some(s) => songs_refs.push(s),
            }
        }

        let mut map: Vec<HashMap<&str, Value>> = Vec::new();
        for x in songs_refs {
            map.push(player::get_song_metadata(Some(&x), self.client.clone()).await);
        }

        map
    }

    #[zbus(property)]
    fn can_edit_tracks(&self) -> bool {
        true
    }

    #[zbus(property)]
    async fn tracks(&self) -> Vec<ObjectPath> {
        let track_list = self.track_list.read().await;
        track_list
            .get_songs()
            .iter()
            .map(|song| ObjectPath::try_from(song.dbus_path()).expect("Invalid object path"))
            .collect()
    }
}
