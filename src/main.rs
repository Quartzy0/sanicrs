use crate::dbus::base::MprisBase;
use crate::dbus::player::{MprisPlayer, MprisPlayerSignals};
use crate::dbus::track_list::MprisTrackList;
use crate::opensonic::client::OpenSubsonicClient;
use crate::player::{PlayerInfo, TrackList};
use crate::ui::app::start_app;
use crate::ui::current_song::{CurrentSong, CurrentSongMsg};
use crate::ui::track_list::{TrackListMsg, TrackListWidget};
use readlock_tokio::Shared;
use relm4::AsyncComponentSender;
use rodio::OutputStreamBuilder;
use std::error::Error;
use std::sync::Arc;
use std::time::Duration;
use tokio::signal;
use tokio::sync::{RwLock, mpsc};
use zbus::connection;
use zbus::object_server::InterfaceRef;

mod dbus;
mod opensonic;
mod player;
mod ui;

mod icon_names {
    include!(concat!(env!("OUT_DIR"), "/icon_names.rs"));
}

pub enum PlayerCommand {
    Quit,
    Next,
    Previous,
    Play,
    Pause,
    PlayPause,
    SetVolume(f64),
    SetRate(f64),
    Stop,
    SetPosition(Duration),
    GoTo(usize),
    Remove(usize),
    TrackListSendSender(AsyncComponentSender<TrackListWidget>),
    CurrentSongSendSender(AsyncComponentSender<CurrentSong>),
    AddFromUri(String, Option<usize>, bool),
}

fn send_cs_msg(sender: &Option<AsyncComponentSender<CurrentSong>>, msg: CurrentSongMsg) {
    if let Some(sender) = sender {
        sender.input(msg);
    }
}

fn send_tl_msg(sender: &Option<AsyncComponentSender<TrackListWidget>>, msg: TrackListMsg) {
    if let Some(sender) = sender {
        sender.input(msg);
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let client = Arc::new(
        OpenSubsonicClient::new(
            "https://music.quartzy.me",
            "quartzy",
            "xqFs@4GX0x}W-Sdx!~C\"\\T^)z",
            "Sanic-rs",
            Some("cache"),
        )
        .await,
    );

    let search = client
        .search3("Genius", Some(0), None, Some(0), None, Some(2), None, None)
        .await?;

    let mut songs = search.song.unwrap().into_iter().map(Arc::new).collect();

    let (command_send, mut command_recv) = mpsc::unbounded_channel::<PlayerCommand>();
    let command_send = Arc::new(command_send);

    let stream = OutputStreamBuilder::from_default_device()
        .expect("Error building output stream")
        .open_stream()
        .expect("Error opening output stream");
    let mut track_list = TrackList::new();
    track_list.add_songs(&mut songs);
    let track_list = Arc::new(RwLock::new(track_list));

    let player = Shared::new(PlayerInfo::new(
        client.clone(),
        &stream,
        track_list.clone(),
        command_send.clone(),
    ));

    let connection = Arc::new(
        connection::Builder::session()?
            .name("org.mpris.MediaPlayer2.sanicrs")?
            .serve_at(
                "/org/mpris/MediaPlayer2",
                MprisBase {
                    cmd_channel: command_send.clone(),
                },
            )?
            .serve_at(
                "/org/mpris/MediaPlayer2",
                MprisPlayer {
                    client: client.clone(),
                    track_list: track_list.clone(),
                    cmd_channel: command_send.clone(),
                    player_ref: Shared::<PlayerInfo>::get_read_lock(&player),
                },
            )?
            .serve_at(
                "/org/mpris/MediaPlayer2",
                MprisTrackList {
                    track_list: track_list.clone(),
                    client: client.clone(),
                    cmd_channel: command_send.clone(),
                },
            )?
            .build()
            .await?,
    );

    let player_ref: InterfaceRef<MprisPlayer> = connection
        .object_server()
        .interface("/org/mpris/MediaPlayer2")
        .await?;

    let track_list_ref: InterfaceRef<MprisTrackList> = connection
        .object_server()
        .interface("/org/mpris/MediaPlayer2")
        .await?;

    let handle = start_app((
        Shared::<PlayerInfo>::get_read_lock(&player),
        track_list.clone(),
        client.clone(),
        track_list_ref,
        command_send.clone(),
    ));

    let mut tl_sender: Option<AsyncComponentSender<TrackListWidget>> = None;
    let mut cs_sender: Option<AsyncComponentSender<CurrentSong>> = None;

    loop {
        tokio::select! {
            _ = signal::ctrl_c() => break,
            cmd = command_recv.recv() => {
                if let Some(cmd) = cmd {
                    match cmd {
                        PlayerCommand::Quit => break,
                        PlayerCommand::Next => {
                            player.next().await;
                            send_cs_msg(&cs_sender, CurrentSongMsg::SongUpdate(track_list.read().await.current().cloned()));
                            player_ref.get().await.metadata_changed(player_ref.signal_emitter()).await.expect("Error sending DBus signal");
                        },
                        PlayerCommand::Previous => {
                            player.previous().await;
                            send_cs_msg(&cs_sender, CurrentSongMsg::SongUpdate(track_list.read().await.current().cloned()));
                            player_ref.get().await.metadata_changed(player_ref.signal_emitter()).await.expect("Error sending DBus signal");
                        },
                        PlayerCommand::Play => {
                            player.play().await;
                            send_cs_msg(&cs_sender, CurrentSongMsg::PlaybackStateChange(player.playback_status()));
                            player_ref.get().await.playback_status_changed(player_ref.signal_emitter()).await.expect("Error sending DBus signal");
                        },
                        PlayerCommand::Pause => {
                            player.pause();
                            send_cs_msg(&cs_sender, CurrentSongMsg::PlaybackStateChange(player.playback_status()));
                            player_ref.get().await.playback_status_changed(player_ref.signal_emitter()).await.expect("Error sending DBus signal");
                        },
                        PlayerCommand::PlayPause => {
                            player.playpause().await;
                            send_cs_msg(&cs_sender, CurrentSongMsg::PlaybackStateChange(player.playback_status()));
                            player_ref.get().await.playback_status_changed(player_ref.signal_emitter()).await.expect("Error sending DBus signal");
                        },
                        PlayerCommand::Stop => {
                            player.stop().await;
                            send_cs_msg(&cs_sender, CurrentSongMsg::PlaybackStateChange(player.playback_status()));
                            player_ref.get().await.playback_status_changed(player_ref.signal_emitter()).await.expect("Error sending DBus signal");
                        },
                        PlayerCommand::SetRate(r) => {
                            player.set_rate(r);
                            send_cs_msg(&cs_sender, CurrentSongMsg::RateChange(r));
                            player_ref.get().await.rate_changed(player_ref.signal_emitter()).await.expect("Error sending DBus signal");
                        },
                        PlayerCommand::SetVolume(v) => {
                            player.set_volume(v);
                            send_cs_msg(&cs_sender, CurrentSongMsg::VolumeChangedExternal(v));
                            player_ref.get().await.volume_changed(player_ref.signal_emitter()).await.expect("Error sending DBus signal");
                        },
                        PlayerCommand::SetPosition(p) => {
                            player.set_position(p).await.expect("Error when seeking");
                            send_cs_msg(&cs_sender, CurrentSongMsg::ProgressUpdateSync(Some(p.as_secs_f64())));
                            player_ref.seeked(p.as_secs() as i64).await.expect("Error sending DBus seeked signal");
                        },
                        PlayerCommand::GoTo(i) => {
                            player.goto(i).await.expect("Error performing goto");
                            send_tl_msg(&tl_sender, TrackListMsg::TrackChanged(i));
                            send_cs_msg(&cs_sender, CurrentSongMsg::SongUpdate(track_list.read().await.current().cloned()));
                            player_ref.get().await.metadata_changed(player_ref.signal_emitter()).await.expect("Error sending DBus signal");
                        },
                        PlayerCommand::Remove(i) => {
                            player.remove_song(i).await.expect("Error removing track");
                            send_tl_msg(&tl_sender, TrackListMsg::ReloadList);
                            send_cs_msg(&cs_sender, CurrentSongMsg::SongUpdate(track_list.read().await.current().cloned()));
                            player_ref.get().await.metadata_changed(player_ref.signal_emitter()).await.expect("Error sending DBus signal");
                        },
                        PlayerCommand::TrackListSendSender(s) => tl_sender = Some(s),
                        PlayerCommand::CurrentSongSendSender(s) => cs_sender = Some(s),
                        PlayerCommand::AddFromUri(uri, index, set_as_current) => {
                            let mut track_list = track_list.write().await;
                            match track_list
                                .add_song_from_uri(&*uri, client.clone(), index)
                                .await
                            {
                                None => {
                                    if set_as_current {
                                        let new_i = track_list.get_songs().len() - 1;
                                        track_list.set_current(index.unwrap_or(new_i));
                                        send_tl_msg(&tl_sender, TrackListMsg::ReloadList);
                                        send_cs_msg(&cs_sender, CurrentSongMsg::SongUpdate(track_list.current().cloned()));
                                        player_ref.get().await.metadata_changed(player_ref.signal_emitter()).await.expect("Error sending DBus signal");
                                        drop(track_list);
                                        player.start_current().await.expect("Error when starting current track");
                                    } else {
                                        send_tl_msg(&tl_sender, TrackListMsg::ReloadList);
                                    }
                                }
                                Some(err) => println!("Error when adding song from URI: {}", err),
                            };
                        }
                        // _ => todo!(),
                    }
                }
            }
        }
    }
    handle.join().expect("Error when joining UI thread");

    Ok(())
}
