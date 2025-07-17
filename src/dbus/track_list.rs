use crate::dbus::player;
use crate::dbus::player::MprisPlayer;
use crate::opensonic::client::OpenSubsonicClient;
use crate::opensonic::types::Song;
use crate::player::TrackList;
use std::collections::HashMap;
use std::sync::Arc;
use relm4::AsyncComponentSender;
use tokio::sync::RwLock;
use zbus::interface;
use zbus::object_server::InterfaceRef;
use zvariant::{ObjectPath, Value};
use crate::ui::current_song::{CurrentSong, CurrentSongMsg, SongInfo};
use crate::ui::track_list::TrackListMsg::{ReloadList, TrackChanged};
use crate::ui::track_list::TrackListWidget;

pub struct MprisTrackList {
    pub track_list: Arc<RwLock<TrackList>>,
    pub client: Arc<OpenSubsonicClient>,
    pub player_reference: InterfaceRef<MprisPlayer>,
    pub track_list_sender: Option<AsyncComponentSender<TrackListWidget>>,
}

impl MprisTrackList {
    pub async fn remove_track_index(&self, index: usize) -> Result<(), zbus::fdo::Error> {
        {
            let mut track_list = self.track_list.write().await;
            track_list.remove_song(index);
        }
        self.notify_reload_list();
        self.player_reference
            .get()
            .await
            .start_current()
            .await
            .map_err(|e| {
                zbus::fdo::Error::Failed(format!("Error trying to play song: {}", e))
            })
    }

    pub async fn go_to_index(&self, index: usize) -> Result<(), zbus::fdo::Error> {
        {
            let mut track_list = self.track_list.write().await;
            track_list.set_current(index);
            if let Some(sender) = &self.track_list_sender {
                sender.input(TrackChanged(index));
            }
        }
        self.player_reference
            .get()
            .await
            .start_current()
            .await
            .map_err(|e| {
                zbus::fdo::Error::Failed(format!("Error trying to play song: {}", e))
            })
    }

    fn notify_reload_list(&self) {
        if let Some(sender) = &self.track_list_sender {
            sender.input(ReloadList);
        }
    }
}

#[interface(name = "org.mpris.MediaPlayer2.TrackList")]
impl MprisTrackList {
    async fn add_track(
        &self,
        uri: &str,
        after_track: ObjectPath<'_>,
        set_as_current: bool,
    ) -> Result<(), zbus::fdo::Error> {
        let index: Option<usize> =
            if after_track.as_str() == "/org/mpris/MediaPlayer2/TrackList/NoTrack" {
                Some(0)
            } else {
                let track_list = self.track_list.read().await;
                track_list
                    .get_songs()
                    .iter()
                    .position(|x| x.dbus_path() == after_track.as_str())
                    .and_then(|t| Some(t+1))
            };
        let mut track_list = self.track_list.write().await;
        match track_list
            .add_song_from_uri(uri, self.client.clone(), index)
            .await
        {
            None => {
                if set_as_current {
                    let new_i = track_list.get_songs().len() - 1;
                    track_list.set_current(index.unwrap_or(new_i));
                    self.notify_reload_list();
                    drop(track_list);
                    return self.player_reference
                        .get()
                        .await
                        .start_current()
                        .await
                        .map_err(|e| {
                            zbus::fdo::Error::Failed(format!("Error trying to play song: {}", e))
                        });
                }
                self.notify_reload_list();
                Ok(())
            }
            Some(err) => Err(zbus::fdo::Error::Failed(format!(
                "Error when adding song: {}",
                err
            ))),
        }
    }

    async fn remove_track(&self, track_id: ObjectPath<'_>) -> Result<(), zbus::fdo::Error> {
        let index: Option<usize> = {
            let track_list = self.track_list.read().await;
            track_list
                .get_songs()
                .iter()
                .position(|x| x.dbus_path() == track_id.as_str())
        };
        if let Some(index) = index {
            return self.remove_track_index(index).await
        }
        Ok(())
    }

    async fn go_to(&self, track_id: ObjectPath<'_>) -> Result<(), zbus::fdo::Error> {
        let index: Option<usize> = {
            let track_list = self.track_list.read().await;
            track_list
                .get_songs()
                .iter()
                .position(|x| x.dbus_path() == track_id.as_str())
        };
        if let Some(index) = index {
            return self.go_to_index(index).await;
        }
        Ok(())
    }

    async fn get_tracks_metadata(
        &self,
        tracks_in: Vec<ObjectPath<'_>>,
    ) -> Vec<HashMap<&str, Value>> {
        let track_list = self.track_list.read().await;
        let mut songs_refs: Vec<&Song> = Vec::new();
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
            let result = player::get_song_metadata(&x, self.client.clone()).await;
            if let Ok(meta) = result {
                map.push(meta);
            }
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
