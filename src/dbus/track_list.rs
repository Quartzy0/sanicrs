use std::error::Error;
use std::rc::Rc;
use crate::player::{SongEntry};
use mpris_server::{zbus::fdo, LocalTrackListInterface, Metadata, Property, TrackId, TrackListSignal};
use crate::dbus::player::{get_song_metadata, MprisPlayer};
use crate::opensonic::types::Song;
use crate::ui::current_song::CurrentSongMsg;
use crate::ui::track_list::{MoveDirection, TrackListMsg};

impl MprisPlayer {
    pub async fn add_track_to_index(&self, uri: String, index: Option<usize>, set_as_current: bool) -> Result<(), Box<dyn Error>> {
        let mut track_list_guard = self.track_list().borrow_mut();
        match track_list_guard
            .add_song_from_uri(&*uri, &self.song_cache, index)
            .await
        {
            None => {
                let songs = track_list_guard.get_songs();
                let new_i = index.unwrap_or(songs.len() - 1);
                self.track_list_emit(TrackListSignal::TrackAdded {
                    metadata: get_song_metadata(Some(&songs[new_i]), self.client.clone()).await,
                    after_track: if new_i == 0 {
                        TrackId::NO_TRACK
                    } else {
                        songs[new_i-1].dbus_obj()
                    },
                });
                if set_as_current {
                    track_list_guard.set_current(new_i);
                    drop(track_list_guard);
                    let song = self.player_ref.start_current().await?;
                    self.send_cs_msg(CurrentSongMsg::SongUpdate(song));
                    self.properties_changed([
                        Property::Metadata(self.current_song_metadata().await),
                        Property::PlaybackStatus(self.player_ref.playback_status())
                    ]);
                }
                self.send_tl_msg(TrackListMsg::ReloadList);
            }
            Some(err) => return Err(err),
        };
        Ok(())
    }

    pub async fn queue_songs(&self, songs: Vec<Rc<Song>>, set_index: Option<usize>, clear_previous: bool) -> Result<(), Box<dyn Error>> {
        let song_changed;
        {
            let mut guard = self.track_list().borrow_mut();
            let len = if clear_previous {
                guard.clear();
                0
            } else {
                guard.get_songs().len()
            };
            guard.add_songs(songs);
            if let Some(index) = set_index {
                guard.set_current(len+index);
                song_changed = true;
            } else {
                song_changed = len==0;
            }
        }
        if song_changed {
            let song = self.player_ref.start_current().await?;
            self.send_cs_msg(CurrentSongMsg::SongUpdate(song));
            self.properties_changed([
                Property::Metadata(self.current_song_metadata().await),
                Property::PlaybackStatus(self.player_ref.playback_status())
            ]);
        }
        let guard = self.track_list().borrow();
        self.track_list_replaced(guard.get_songs(), guard.current_index()).await?;
        self.send_tl_msg(TrackListMsg::ReloadList);
        Ok(())
    }

    pub async fn queue_random(&self, size: u32, genre: Option<String>, from_year: Option<u32>, to_year: Option<u32>, clear_previous: bool) -> Result<(), Box<dyn Error>> {
        let songs = self.song_cache.get_random_songs(Some(size), genre.as_deref(), from_year, to_year, None).await?;
        println!("Added {} random songs", songs.len());
        self.queue_songs(songs, None, clear_previous).await
    }

    pub async fn queue_album(&self, id: String, index: Option<usize>, clear_previous: bool) -> Result<(), Box<dyn Error>> {
        let album = self.album_cache.get_album(id.as_str()).await?;
        if let Some(songs) = album.get_songs() {
            self.queue_songs(songs, index, clear_previous).await?;
            if index.is_some() {
                self.properties_changed([
                    Property::Metadata(self.current_song_metadata().await),
                    Property::PlaybackStatus(self.player_ref.playback_status())
                ]);
            }
        }
        Ok(())
    }
    
    pub async fn goto(&self, i: usize) -> Result<(), Box<dyn Error>>{
        let song = self.player_ref.goto(i).await?;
        self.send_tl_msg(TrackListMsg::TrackChanged(Some(i)));
        self.send_cs_msg(CurrentSongMsg::SongUpdate(song));
        self.properties_changed([
            Property::Metadata(self.current_song_metadata().await),
            Property::PlaybackStatus(self.player_ref.playback_status())
        ]);
        Ok(())
    }
    
    pub async fn remove(&self, i: usize) -> Result<(), Box<dyn Error>> {
        let e = self.player_ref.remove_song(i).await?;
        self.send_tl_msg(TrackListMsg::ReloadList);
        self.track_list_emit(TrackListSignal::TrackRemoved {
            track_id: e.dbus_obj()
        });
        self.send_cs_msg(CurrentSongMsg::SongUpdate(Some(e)));
        self.properties_changed([
            Property::Metadata(self.current_song_metadata().await),
        ]);
        Ok(())
    }
    
    pub async fn move_item(&self, index: usize, direction: MoveDirection) -> Result<(), Box<dyn Error>> {
        let mut guard = self.track_list().borrow_mut();
        let new_i = guard.move_song(index, direction);
        if let Some(new_i) = new_i {
            let moved = guard.song_at_index(new_i).ok_or("No song found at moved index")?;
            self.track_list_emit(TrackListSignal::TrackRemoved {
                track_id: moved.dbus_obj(),
            });
            self.track_list_emit(TrackListSignal::TrackAdded {
                metadata: get_song_metadata(Some(moved), self.client.clone()).await,
                after_track: if index != 0 && let Some(prev) = guard.song_at_index(index-1) {
                    prev.dbus_obj()
                } else {
                    TrackId::NO_TRACK
                },
            });
            self.send_tl_msg(TrackListMsg::TrackChanged(None));
        }
        Ok(())
    }
}

impl LocalTrackListInterface for MprisPlayer{
    async fn get_tracks_metadata(
        &self,
        tracks_in: Vec<TrackId>,
    ) -> fdo::Result<Vec<Metadata>> {
        let track_list = self.track_list().borrow();
        let mut songs_refs: Vec<&SongEntry> = Vec::new();
        let loaded_songs = track_list.get_songs();
        for x in tracks_in {
            let song = loaded_songs.iter().find(|x1| x1.dbus_path() == x.as_str());
            match song {
                None => {}
                Some(s) => songs_refs.push(s),
            }
        }

        let mut map: Vec<Metadata> = Vec::new();
        for x in songs_refs {
            map.push(get_song_metadata(Some(&x), self.client.clone()).await);
        }

        Ok(map)
    }

    async fn add_track(&self, uri: String, after_track: TrackId, set_as_current: bool) -> fdo::Result<()> {
        let index: Option<usize> =
            if after_track.as_str() == "/org/mpris/MediaPlayer2/TrackList/NoTrack" {
                Some(0)
            } else {
                let track_list = self.track_list().borrow();
                track_list
                    .get_songs()
                    .iter()
                    .position(|x| x.dbus_path() == after_track.as_str())
                    .and_then(|t| Some(t + 1))
            };
        self.add_track_to_index(uri, index, set_as_current).await.map_err(|e| fdo::Error::Failed(format!("{}", e)))?;
        Ok(())
    }

    async fn remove_track(&self, track_id: TrackId) -> fdo::Result<()> {
        let index: usize = {
            let track_list = self.track_list().borrow();
            track_list
                .get_songs()
                .iter()
                .position(|x| x.dbus_path() == track_id.as_str())
                .ok_or(fdo::Error::Failed("Track not found".to_string()))?
        };
        self.remove(index).await.map_err(|e| fdo::Error::Failed(e.to_string()))?;
        Ok(())
    }

    async fn go_to(&self, track_id: TrackId) -> fdo::Result<()> {
        let index: usize = {
            let track_list = self.track_list().borrow();
            track_list
                .get_songs()
                .iter()
                .position(|x| x.dbus_path() == track_id.as_str())
                .ok_or(fdo::Error::Failed("Track not found".to_string()))?
        };
        self.goto(index).await.map_err(|e| fdo::Error::Failed(e.to_string()))?;
        Ok(())
    }

    async fn tracks(&self) -> fdo::Result<Vec<TrackId>> {
        let track_list = self.track_list().borrow();
        Ok(track_list
            .get_songs()
            .iter()
            .map(|song| song.dbus_obj())
            .collect())
    }

    async fn can_edit_tracks(&self) -> fdo::Result<bool> {
        Ok(true)
    }
}
