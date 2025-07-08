use crate::opensonic::client::OpensonicClient;
use crate::opensonic::types::Song;
use futures_util::TryStreamExt;
use rodio::{OutputStream, OutputStreamHandle, Sink};
use std::error::Error;
use stream_download::async_read::AsyncReadStreamParams;
use stream_download::storage::temp::TempStorageProvider;
use stream_download::{Settings, StreamDownload};
use tokio_util::io::StreamReader;

pub enum PlayerCommand {
    Play,
    Pause,
    PlayPause,
    Quit,
    Next,
    Previous
}

pub struct Player {
    client: OpensonicClient,
    sink: Option<Sink>,
    stream_handle: (OutputStream, OutputStreamHandle),
    track_list: TrackList
}

pub struct TrackList {
    songs: Vec<Song>,
    current: usize,
    shuffled: bool,
    looping: bool,
}

impl Player {
    pub fn new(client: OpensonicClient) -> Self{
        Player {
            client,
            sink: None,
            stream_handle: OutputStream::try_default().expect("Failed to create a stream handle"),
            track_list: TrackList::new()
        }
    }

    pub fn tracks(&mut self) -> &mut TrackList {
        &mut self.track_list
    }

    pub async fn start_current(&mut self) -> Result<&Song, Box<dyn Error>> {
        self.sink = None;

        let song = self.track_list.current();
        println!("Playing: {:?}", song);
        let stream = self.client.steam(&*song.id, None, None, None, None, None, None);

        let x = stream.await?.bytes_stream();
        let reader =
            StreamReader::new(x.map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e)));

        let reader = StreamDownload::new_async_read(
            AsyncReadStreamParams::new(reader),
            TempStorageProvider::new(),
            Settings::default(),
        ).await?;

        let sink = Sink::try_new(&self.stream_handle.1)?;
        sink.append(rodio::Decoder::new(reader)?);
        self.sink = Some(sink);

        Ok(song)
    }

    pub fn pause(&mut self) {
        if self.sink.is_some() {
            self.sink.as_ref().unwrap().pause();
        }
    }

    pub async fn play(&mut self) {
        if self.sink.is_some() {
            self.sink.as_ref().unwrap().play();
        } else {
            self.start_current().await.expect("Error trying to play");
        }
    }

    pub async fn toggle(&mut self) {
        if self.sink.is_some() {
            if self.sink.as_ref().unwrap().is_paused() {
                self.play().await;
            } else {
                self.pause();
            }
        }
    }

    pub async fn next(&mut self) -> Result<&Song, Box<dyn Error>> {
        self.track_list.next();
        self.start_current().await
    }

    pub async fn previous(&mut self) -> Result<&Song, Box<dyn Error>> {
        self.track_list.previous();
        self.start_current().await
    }
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