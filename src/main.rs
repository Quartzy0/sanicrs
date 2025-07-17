use crate::ui::app::start_app;
use crate::dbus::player::MprisPlayer;
use crate::opensonic::client::OpenSubsonicClient;
use crate::player::TrackList;
use rodio::{OutputStreamBuilder};
use std::error::Error;
use std::sync::Arc;
use tokio::signal;
use tokio::sync::{RwLock, mpsc};
use zbus::connection;
use zbus::object_server::InterfaceRef;
use crate::dbus::base::MprisBase;
use crate::dbus::track_list::MprisTrackList;

mod opensonic;
mod player;
mod ui;
mod dbus;

mod icon_names {
    include!(concat!(env!("OUT_DIR"), "/icon_names.rs"));
}

pub enum PlayerCommand {
    Quit,
    Next
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let client = Arc::new(
        OpenSubsonicClient::new(
            "https://music.quartzy.me",
            "quartzy",
            "xqFs@4GX0x}W-Sdx!~C\"\\T^)z",
            "Sanic-rs",
            Some("cache")
        )
        .await,
    );

    let search = client
        .search3("Genius", Some(0), None, Some(0), None, Some(2), None, None)
        .await?;

    let mut songs = search.song.unwrap();

    let (command_send, mut command_recv) = mpsc::unbounded_channel::<PlayerCommand>();
    let command_send = Arc::new(command_send);

    let stream = OutputStreamBuilder::from_default_device()
        .expect("Error building output stream")
        .open_stream()
        .expect("Error opening output stream");
    let mut track_list = TrackList::new();
    track_list.add_songs(&mut songs);
    let track_list = Arc::new(RwLock::new(track_list));
    let connection = Arc::new(connection::Builder::session()?
        .name("org.mpris.MediaPlayer2.sanicrs")?
        .serve_at(
            "/org/mpris/MediaPlayer2",
            MprisBase {
                cmd_channel: command_send.clone(),
            },
        )?
        .serve_at(
            "/org/mpris/MediaPlayer2",
            MprisPlayer::new(client.clone(), &stream, track_list.clone(), command_send.clone()),
        )?
        .build()
        .await?);

    let player_ref: InterfaceRef<MprisPlayer> = connection
        .object_server()
        .interface("/org/mpris/MediaPlayer2")
        .await?;

    connection.object_server()
        .at(
            "/org/mpris/MediaPlayer2",
            MprisTrackList {
                track_list: track_list.clone(),
                client: client.clone(),
                player_reference: player_ref.clone(),
                track_list_sender: None
            }
        ).await?;

    let track_list_ref: InterfaceRef<MprisTrackList> = connection
        .object_server()
        .interface("/org/mpris/MediaPlayer2")
        .await?;

    let handle = start_app((player_ref.clone(), track_list.clone(), client.clone(), track_list_ref));

    loop {
        tokio::select! {
            _ = signal::ctrl_c() => break,
            cmd = command_recv.recv() => {
                if let Some(cmd) = cmd {
                    match cmd {
                        PlayerCommand::Quit => break,
                        PlayerCommand::Next => player_ref.get_mut().await.next().await,
                    }
                }
            }
        }
    }
    handle.join().expect("Error when joining UI thread");

    Ok(())
}
