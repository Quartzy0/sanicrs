use std::error::Error;
use std::sync::Arc;
use uuid::Uuid;
use crate::opensonic::client::OpenSubsonicClient;
use crate::opensonic::types::{InvalidResponseError, Song};

#[derive(Clone)]
pub struct SongEntry(
    pub Uuid,
    pub Arc<Song>
);

impl SongEntry {
    pub fn dbus_path(&self) -> String {
        format!("/me/quartzy/sanicrs/song/{}", self.0.as_simple().to_string())
    }
}

impl From<(Uuid, Arc<Song>)> for SongEntry {
    fn from(value: (Uuid, Arc<Song>)) -> Self {
        Self {
            0: value.0,
            1: value.1
        }
    }
}

pub struct TrackList {
    songs: Vec<SongEntry>,
    current: usize,

    pub shuffled: bool,
    pub looping: bool,
}

impl TrackList {
    pub fn new() -> Self {
        TrackList{
            songs: Vec::new(),
            current: 0,
            shuffled: false,
            looping: false
        }
    }

    pub async fn add_song_from_uri(&mut self, uri: &str, client: Arc<OpenSubsonicClient>, index: Option<usize>) -> Option<Box<dyn Error + Send + Sync>> {
        if !uri.starts_with("sanic://song/"){
            return Some(InvalidResponseError::new_boxed("Invalid URI, should be sanic://song/<song-id>"));
        }
        let id = &uri[13..]; // 13 is length of "sanic://song/"
        self.add_song_from_id(id, client, index).await
    }
    
    pub async fn add_song_from_id(&mut self, id: &str, client: Arc<OpenSubsonicClient>, index: Option<usize>) -> Option<Box<dyn Error + Send + Sync>> {
        let result = client.get_song(id).await;
        if result.is_err(){
            return result.err();
        }
        let song = result.unwrap();
        self.add_song(song, index);
        None
    }
    
    pub fn set_current(&mut self, index: usize) {
        self.current = index;
    }

    pub fn remove_song(&mut self, index: usize) -> SongEntry {
        if self.current < index && self.current != 0 {
            self.current -= 1;
        }
        self.songs.remove(index)
    }

    pub fn clear(&mut self) {
        self.songs.clear();
        self.current = 0;
        self.shuffled = false;
        self.looping = false;
    }

    pub fn empty(&self) -> bool {
        self.songs.is_empty()
    }

    pub fn next(&mut self) {
        if self.current != self.songs.len()-1 {
            self.current += 1;
        } else {
            self.current = 0;
        }
    }

    pub fn previous(&mut self) {
        if self.current != 0 {
            self.current -= 1;
        }
    }

    pub fn current(&self) -> Option<&SongEntry> {
        self.songs.get(self.current)
    }

    pub fn current_index(&self) -> Option<usize> {
        if self.songs.len() > 0{
            Some(self.current)
        } else {
            None
        }
    }

    pub fn add_song(&mut self, song: Arc<Song>, index: Option<usize>) {
        match index {
            None => self.songs.push((Uuid::new_v4(), song).into()),
            Some(i) => {
                if i <= self.current {
                    self.current += 1;
                }
                self.songs.insert(i, (Uuid::new_v4(), song).into());
            }
        }
    }

    pub fn add_songs(&mut self, songs: &Vec<Arc<Song>>) {
        let mut x: Vec<SongEntry> = songs.iter().map(|song| {
            (Uuid::new_v4(), song.clone()).into()
        }).collect();
        self.songs.append(&mut x);
    }

    pub fn get_songs(&self) -> &Vec<SongEntry> {
        &self.songs
    }
}