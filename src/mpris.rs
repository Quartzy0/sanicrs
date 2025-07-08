use crate::opensonic::client::OpensonicClient;
use crate::player::TrackList;
use rodio::{OutputStreamHandle, Sink};
use std::error::Error;
use std::io::Cursor;
use std::sync::Arc;
use tokio::sync::mpsc::UnboundedSender;
use tokio::sync::RwLock;
use zbus::interface;

pub enum PlayerCommand {
    Quit,
}

pub struct MprisPlayer {
    client: OpensonicClient,
    sink: Sink,
    stream_handle: OutputStreamHandle,
    track_list: Arc<RwLock<TrackList>>,
}

impl MprisPlayer {
    pub fn new(
        client: OpensonicClient,
        stream_handle: OutputStreamHandle,
        track_list: Arc<RwLock<TrackList>>,
    ) -> Self {
        let (sink, queue_rx) = Sink::new_idle();
        stream_handle.play_raw(queue_rx).expect("Error playing queue");
        MprisPlayer {
            client,
            sink,
            stream_handle,
            track_list,
        }
    }
}

impl MprisPlayer {
    pub async fn start_current(&self) -> Result<(), Box<dyn Error + Send + Sync>> {
        let track_list = self.track_list.read().await;
        let song = track_list.current();
        println!("Playing: {}", song.title);
        let stream = self.client.stream(&*song.id, None, None, None, None, None, None);

        let x1 = stream.await.expect("Error when reading bytes").bytes().await?; // TODO: Figure this out without downloading entire file first
        let reader = Cursor::new(x1);
        /*let x = stream.await.expect("Error reading stream").bytes_stream();
        let reader =
            StreamReader::new(x.map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e)));


        let reader = StreamDownload::new_async_read(
            AsyncReadStreamParams::new(reader),
            MemoryStorageProvider::default(),
            Settings::default(),
        ).await?;*/


        let decoder = match &song.suffix {
            None => rodio::Decoder::new(reader)?,
            Some(suffix) => match suffix.as_str() {
                "wav" => rodio::Decoder::new_wav(reader)?,
                "ogg" => rodio::Decoder::new_vorbis(reader)?,
                "flac" => rodio::Decoder::new_flac(reader)?,
                _ => rodio::Decoder::new(reader)?
            }
        };
        
        self.sink.clear();
        self.sink.append(decoder);
        self.sink.play();

        Ok(())
    }
}

#[interface(name = "org.mpris.MediaPlayer2.Player")]
impl MprisPlayer {
    async fn play(&self) {
        if !self.sink.empty() {
            self.sink.play();
        } else {
            self.start_current().await.expect("Error playing");
        }
    }

    async fn pause(&self) {
        if !self.sink.empty() {
            self.sink.pause();
        }
    }

    async fn play_pause(&self) {
        if !self.sink.empty() {
            if self.sink.is_paused() {
                self.play().await;
            } else {
                self.pause().await;
            }
        }
    }

    async fn next(&mut self) {
        {
            let mut track_list = self.track_list.write().await;
            track_list.next();
        }
        self.start_current().await.expect("Error starting next track");
    }

    async fn previous(&mut self) {
        {
            let mut track_list = self.track_list.write().await;
            track_list.previous();
        }
        self.start_current().await.expect("Error starting next track");
    }

    #[zbus(property)]
    fn can_go_next(&self) -> bool {
        true
    }

    #[zbus(property)]
    fn can_go_previous(&self) -> bool {
        true
    }

    #[zbus(property)]
    fn can_play(&self) -> bool {
        true
    }

    #[zbus(property)]
    fn can_pause(&self) -> bool {
        true
    }

    #[zbus(property)]
    fn can_seek(&self) -> bool {
        false // TODO: Implement
    }

    #[zbus(property)]
    fn can_control(&self) -> bool {
        true
    }
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
        false // TODO: Implement this
    }

    #[zbus(property)]
    fn identity(&self) -> &str {
        "Sanic-rs"
    }
}
