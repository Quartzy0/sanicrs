use crate::mpris::{MprisBase, MprisPlayer};
use crate::opensonic::client::OpensonicClient;
use crate::player::{Player, PlayerCommand};
use futures_util::TryStreamExt;
use rodio::Source;
use std::error::Error;
use std::sync::Arc;
use tokio::signal;
use tokio::sync::mpsc;
use zbus::connection;

mod mpris;
mod opensonic;
mod player;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let client = OpensonicClient::new(
        "https://music.quartzy.me",
        "quartzy",
        "xqFs@4GX0x}W-Sdx!~C\"\\T^)z",
        "Sanic-rs",
    )
    .await;

    let search = client
        .search3("Genius", Some(0), None, Some(0), None, Some(2), None, None)
        .await?;
    // println!("Results: {:?}", search.song.unwrap()[0]);

    let mut songs = search.song.unwrap();

    let (command_send, mut command_recv) = mpsc::unbounded_channel::<PlayerCommand>();
    let command_send = Arc::new(command_send);

    let mut player = Player::new(client);
    player.tracks().add_songs(&mut songs);
    let _connection = connection::Builder::session()?
        .name("org.mpris.MediaPlayer2.sanicrs")?
        .serve_at(
            "/org/mpris/MediaPlayer2",
            MprisBase {
                quit_channel: command_send.clone(),
            },
        )?
        .serve_at(
            "/org/mpris/MediaPlayer2",
            MprisPlayer::new(command_send.clone()),
        )?
        .build()
        .await?;

    loop {
        tokio::select! {
            _ = signal::ctrl_c() => break,
            cmd = command_recv.recv() => {
                if let Some(cmd) = cmd {
                    match cmd {
                        PlayerCommand::Play => player.play().await,
                        PlayerCommand::Pause => player.pause(),
                        PlayerCommand::PlayPause => player.toggle().await,
                        PlayerCommand::Quit => break,
                        PlayerCommand::Next => {player.next().await.expect("Error when trying to play next track");},
                        PlayerCommand::Previous => {player.previous().await.expect("Error when trying to play previous track");},
                    }
                }
            }
        }
    }

    Ok(())
}
