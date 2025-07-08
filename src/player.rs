use crate::opensonic::types::Song;

pub struct TrackList {
    songs: Vec<Song>,
    current: usize,
    shuffled: bool,
    looping: bool,
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

    pub fn current(&self) -> &Song {
        &self.songs[self.current]
    }

    pub fn add_song(&mut self, song: Song) {
        self.songs.push(song);
    }

    pub fn add_songs(&mut self, songs: &mut Vec<Song>) {
        self.songs.append(songs);
    }
}