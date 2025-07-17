use std::error::Error;
use std::sync::Arc;
use crate::opensonic::client::OpenSubsonicClient;
use crate::opensonic::types::{InvalidResponseError, Song};

pub struct TrackList {
    songs: Vec<Song>,
    current: usize,

    pub shuffled: bool,
    pub looping: bool,
}

impl TrackList {
    pub fn new() -> Self {
        TrackList{
            songs: vec![],
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

    pub fn remove_song(&mut self, index: usize) -> Song {
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

    pub fn current(&self) -> Option<&Song> {
        self.songs.get(self.current)
    }

    pub fn current_index(&self) -> Option<usize> {
        if self.songs.len() > 0{
            Some(self.current)
        } else {
            None
        }
    }

    pub fn add_song(&mut self, song: Song, index: Option<usize>) {
        match index {
            None => self.songs.push(song),
            Some(i) => {
                if i <= self.current {
                    self.current += 1;
                }
                self.songs.insert(i, song);
            }
        }
    }

    pub fn add_songs(&mut self, songs: &mut Vec<Song>) {
        self.songs.append(songs);
    }

    pub fn get_songs(&self) -> &Vec<Song> {
        &self.songs
    }
}