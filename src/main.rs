use crate::dbus::base::MprisBase;
use crate::dbus::player::{MprisPlayer, MprisPlayerSignals};
use crate::dbus::track_list::{MprisTrackList, MprisTrackListSignals};
use crate::opensonic::client::OpenSubsonicClient;
use crate::player::{LoopStatus, PlayerInfo, TrackList};
use crate::ui::app::{AppMsg, Init, Model};
use crate::ui::current_song::{CurrentSong, CurrentSongMsg};
use crate::ui::track_list::{TrackListMsg, TrackListWidget};
use readlock_tokio::{Shared};
use relm4::{AsyncComponentSender, RelmApp};
use rodio::OutputStreamBuilder;
use std::error::Error;
use std::sync::Arc;
use async_channel::{Receiver, Sender};
use std::time::Duration;
use relm4::adw::{glib};
use relm4::adw::glib::clone;
use relm4::adw::prelude::{ApplicationExtManual, GtkApplicationExt, WidgetExt};
use relm4::component::{AsyncComponentBuilder, AsyncComponentController};
use tokio::sync::{RwLock};
use zbus::connection;
use zbus::object_server::InterfaceRef;
use zvariant::ObjectPath;
use crate::opensonic::cache::{AlbumCache, CoverCache, SongCache};

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
    AppSendSender(AsyncComponentSender<Model>),
    AddFromUri(String, Option<usize>, bool),
    Raise,
    SetLoopStatus(LoopStatus),
    SetShuffle(bool),
    PlayAlbum(String, Option<usize>),
    QueueAlbum(String),
}

fn send_app_msg(sender_opt: &mut Option<AsyncComponentSender<Model>>, msg: AppMsg) {
    if let Some(sender) = sender_opt {
        let r = sender.input_sender().send(msg);
        if r.is_err() {
            *sender_opt = None;
        }
    }
}

fn send_cs_msg(sender_opt: &mut Option<AsyncComponentSender<CurrentSong>>, msg: CurrentSongMsg) {
    if let Some(sender) = sender_opt {
        let r = sender.input_sender().send(msg);
        if r.is_err() {
            *sender_opt = None;
        }
    }
}

fn send_tl_msg(sender_opt: &mut Option<AsyncComponentSender<TrackListWidget>>, msg: TrackListMsg) {
    if let Some(sender) = sender_opt {
        let r = sender.input_sender().send(msg);
        if r.is_err() {
            *sender_opt = None;
        }
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    let app: RelmApp<AppMsg> = RelmApp::new("me.quarty.sanicrs");
    relm4_icons::initialize_icons(icon_names::GRESOURCE_BYTES, icon_names::RESOURCE_PREFIX);

    let client = Arc::new(
        OpenSubsonicClient::new(
            "https://music.quartzy.me",
            "quartzy",
            "xqFs@4GX0x}W-Sdx!~C\"\\T^)z",
            "Sanic-rs",
            Some("cache"),
            // None
        )
    );

    let stream = OutputStreamBuilder::from_default_device()
        .expect("Error building output stream")
        .open_stream()
        .expect("Error opening output stream");

    let track_list = TrackList::new();
    let track_list = Arc::new(RwLock::new(track_list));

    let (command_send, command_recv) = async_channel::unbounded::<PlayerCommand>();
    let command_send = Arc::new(command_send);

    let song_cache = SongCache::new(client.clone());
    let album_cache = AlbumCache::new(client.clone(), song_cache.clone());
    let cover_cache = CoverCache::new(client.clone());

    let player = Shared::new(PlayerInfo::new(
        client.clone(),
        &stream,
        track_list.clone(),
        command_send.clone(),
    ));
    let player_read = Shared::<PlayerInfo>::get_read_lock(&player);
    let payload: Init = (
        player_read,
        track_list.clone(),
        cover_cache.clone(),
        command_send.clone(),
        song_cache.clone(),
        album_cache.clone(),
    );

    glib::spawn_future_local(clone!(
        #[strong]
        track_list,
        #[strong]
        client,
        #[strong]
        command_send,
        #[strong]
        payload,
        #[strong]
        song_cache,
        #[strong]
        album_cache,
        async move {
            app_main(command_recv, command_send.clone(), client.clone(), track_list.clone(), player, song_cache, album_cache, payload).await.expect("Error");
        }
    ));

    app.run_async::<Model>(payload);

    Ok(())
}

async fn app_main(
    command_recv: Receiver<PlayerCommand>,
    command_send: Arc<Sender<PlayerCommand>>,
    client: Arc<OpenSubsonicClient>,
    track_list: Arc<RwLock<TrackList>>,
    player: Shared<PlayerInfo>,
    song_cache: SongCache,
    album_cache: AlbumCache,
    payload: Init
) -> Result<(), Box<dyn Error>> {
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
                    player_ref: Arc::new(Shared::<PlayerInfo>::get_read_lock(&player)),
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

    let mut app_sender: Option<AsyncComponentSender<Model>> = None;
    let mut tl_sender: Option<AsyncComponentSender<TrackListWidget>> = None;
    let mut cs_sender: Option<AsyncComponentSender<CurrentSong>> = None;

    let _h = relm4::main_application().hold();

    loop {
        match command_recv.recv().await.expect("Error when receiving message") {
            PlayerCommand::Quit => break,
            PlayerCommand::Next => {
                let s = player.next().await;
                send_cs_msg(&mut cs_sender, CurrentSongMsg::SongUpdate(s));
                send_tl_msg(&mut tl_sender, TrackListMsg::TrackChanged(None));
                player_ref.get().await.metadata_changed(player_ref.signal_emitter()).await.expect("Error sending DBus signal");
                player_ref.get().await.playback_status_changed(player_ref.signal_emitter()).await.expect("Error sending DBus signal");
            },
            PlayerCommand::Previous => {
                let s = player.previous().await;
                send_cs_msg(&mut cs_sender, CurrentSongMsg::SongUpdate(s));
                send_tl_msg(&mut tl_sender, TrackListMsg::TrackChanged(None));
                player_ref.get().await.metadata_changed(player_ref.signal_emitter()).await.expect("Error sending DBus signal");
                player_ref.get().await.playback_status_changed(player_ref.signal_emitter()).await.expect("Error sending DBus signal");
            },
            PlayerCommand::Play => {
                player.play().await;
                send_cs_msg(&mut cs_sender, CurrentSongMsg::PlaybackStateChange(player.playback_status()));
                player_ref.get().await.playback_status_changed(player_ref.signal_emitter()).await.expect("Error sending DBus signal");
            },
            PlayerCommand::Pause => {
                player.pause();
                send_cs_msg(&mut cs_sender, CurrentSongMsg::PlaybackStateChange(player.playback_status()));
                player_ref.get().await.playback_status_changed(player_ref.signal_emitter()).await.expect("Error sending DBus signal");
            },
            PlayerCommand::PlayPause => {
                player.playpause().await;
                send_cs_msg(&mut cs_sender, CurrentSongMsg::PlaybackStateChange(player.playback_status()));
                player_ref.get().await.playback_status_changed(player_ref.signal_emitter()).await.expect("Error sending DBus signal");
            },
            PlayerCommand::Stop => {
                player.stop().await;
                send_cs_msg(&mut cs_sender, CurrentSongMsg::PlaybackStateChange(player.playback_status()));
                send_cs_msg(&mut cs_sender, CurrentSongMsg::SongUpdate(None));
                player_ref.get().await.playback_status_changed(player_ref.signal_emitter()).await.expect("Error sending DBus signal");
                track_list_ref.track_list_replaced(Vec::new(),
                    ObjectPath::from_static_str_unchecked("/org/mpris/MediaPlayer2/TrackList/NoTrack"))
                .await.expect("Error sending DBus signal");
                send_cs_msg(&mut cs_sender, CurrentSongMsg::SetLoopStatus(LoopStatus::None));
                player_ref.get().await.loop_status_changed(player_ref.signal_emitter()).await.expect("Error sending DBus signal");
                player_ref.get().await.playback_status_changed(player_ref.signal_emitter()).await.expect("Error sending DBus signal");
            },
            PlayerCommand::SetRate(r) => {
                player.set_rate(r);
                send_cs_msg(&mut cs_sender, CurrentSongMsg::RateChange(r));
                player_ref.get().await.rate_changed(player_ref.signal_emitter()).await.expect("Error sending DBus signal");
            },
            PlayerCommand::SetVolume(v) => {
                player.set_volume(v);
                send_cs_msg(&mut cs_sender, CurrentSongMsg::VolumeChangedExternal(v));
                player_ref.get().await.volume_changed(player_ref.signal_emitter()).await.expect("Error sending DBus signal");
            },
            PlayerCommand::SetPosition(p) => {
                player.set_position(p).await.expect("Error when seeking");
                send_cs_msg(&mut cs_sender, CurrentSongMsg::ProgressUpdateSync(Some(p.as_secs_f64())));
                player_ref.seeked(p.as_secs() as i64).await.expect("Error sending DBus seeked signal");
            },
            PlayerCommand::GoTo(i) => {
                let song = player.goto(i).await.expect("Error performing goto");
                send_tl_msg(&mut tl_sender, TrackListMsg::TrackChanged(Some(i)));
                send_cs_msg(&mut cs_sender, CurrentSongMsg::SongUpdate(song));
                player_ref.get().await.metadata_changed(player_ref.signal_emitter()).await.expect("Error sending DBus signal");
                player_ref.get().await.playback_status_changed(player_ref.signal_emitter()).await.expect("Error sending DBus signal");
            },
            PlayerCommand::Remove(i) => {
                let e = player.remove_song(i).await.expect("Error removing track");
                track_list_ref.track_removed(e.dbus_obj()).await.expect("Error sending DBus signal");
                send_tl_msg(&mut tl_sender, TrackListMsg::ReloadList);
                send_cs_msg(&mut cs_sender, CurrentSongMsg::SongUpdate(Some(e)));
                player_ref.get().await.metadata_changed(player_ref.signal_emitter()).await.expect("Error sending DBus signal");
            },
            PlayerCommand::TrackListSendSender(s) => tl_sender = Some(s),
            PlayerCommand::CurrentSongSendSender(s) => cs_sender = Some(s),
            PlayerCommand::AppSendSender(s) => app_sender = Some(s),
            PlayerCommand::AddFromUri(uri, index, set_as_current) => {
                let mut track_list_guard = track_list.write().await;
                match track_list_guard
                    .add_song_from_uri(&*uri, &song_cache, index)
                    .await
                {
                    None => {
                        let songs = track_list_guard.get_songs();
                        let new_i = index.unwrap_or(songs.len() - 1);
                        track_list_ref.track_added(
                            dbus::player::get_song_metadata(Some(&songs[new_i]), client.clone()).await,
                            if new_i == 0 {
                                ObjectPath::from_static_str_unchecked("/org/mpris/MediaPlayer2/TrackList/NoTrack")
                            } else {
                                songs[new_i-1].dbus_obj()
                            }
                        ).await.expect("Error sending DBus signal");
                        if set_as_current {
                            track_list_guard.set_current(new_i);
                            drop(track_list_guard);
                            let song = player.start_current().await.expect("Error when starting current track");
                            send_cs_msg(&mut cs_sender, CurrentSongMsg::SongUpdate(song));
                            player_ref.get().await
                                .metadata_changed(player_ref.signal_emitter()).await.expect("Error sending DBus signal");
                            player_ref.get().await.playback_status_changed(player_ref.signal_emitter()).await.expect("Error sending DBus signal");
                        }
                        send_tl_msg(&mut tl_sender, TrackListMsg::ReloadList);
                    }
                    Some(err) => println!("Error when adding song from URI: {}", err),
                };
            },
            PlayerCommand::Raise => {
                if relm4::main_application().windows().len() == 0 { // Only allow 1 window
                    let builder = AsyncComponentBuilder::<Model>::default();

                    let connector = builder.launch(payload.clone());

                    let mut controller = connector.detach();
                    let window = controller.widget();
                    window.set_visible(true);
                    relm4::main_application().add_window(window);

                    controller.detach_runtime();
                }
            },
            PlayerCommand::SetLoopStatus(loop_status) => {
                {
                    let mut guard = track_list.write().await;
                    guard.loop_status = loop_status;
                }
                send_cs_msg(&mut cs_sender, CurrentSongMsg::SetLoopStatus(loop_status));
                player_ref.get().await.loop_status_changed(player_ref.signal_emitter()).await.expect("Error sending DBus signal");
            },
            PlayerCommand::SetShuffle(shuffle) => {
                {
                    let mut guard = track_list.write().await;
                    guard.set_shuffle(shuffle);
                }
                send_cs_msg(&mut cs_sender, CurrentSongMsg::SetShuffle(shuffle));
                player_ref.get().await.shuffle_changed(player_ref.signal_emitter()).await.expect("Error sending DBus signal");
            }
            PlayerCommand::PlayAlbum(id, index) => {
                let album = album_cache.get_album(id.as_str()).await.expect("Error getting album"); // TODO: this shouldn't panic
                if let Some(songs) = album.get_songs() {
                    {
                        let mut guard = track_list.write().await;
                        guard.clear();
                        guard.add_songs(songs);
                        if let Some(index) = index {
                            guard.set_current(index);
                        }
                    }
                    let song = player.start_current().await.expect("Error playing current song");
                    send_cs_msg(&mut cs_sender, CurrentSongMsg::SongUpdate(song));
                    player_ref.get().await
                        .metadata_changed(player_ref.signal_emitter()).await.expect("Error sending DBus signal");
                    send_tl_msg(&mut tl_sender, TrackListMsg::ReloadList);
                    player_ref.get().await.playback_status_changed(player_ref.signal_emitter()).await.expect("Error sending DBus signal");
                }
            },
            PlayerCommand::QueueAlbum(id) => {
                let album = album_cache.get_album(id.as_str()).await.expect("Error getting album"); // TODO: this shouldn't panic
                if let Some(songs) = album.get_songs() {
                    let was_empty;
                    {
                        let mut guard = track_list.write().await;
                        was_empty = guard.empty();
                        guard.add_songs(songs);
                    }
                    if was_empty {
                        let song = player.start_current().await.expect("Error playing current song");
                        send_cs_msg(&mut cs_sender, CurrentSongMsg::SongUpdate(song));
                        player_ref.get().await
                            .metadata_changed(player_ref.signal_emitter()).await.expect("Error sending DBus signal");
                        player_ref.get().await.playback_status_changed(player_ref.signal_emitter()).await.expect("Error sending DBus signal");
                    }
                    send_tl_msg(&mut tl_sender, TrackListMsg::ReloadList);
                }
            }
        }
    }
    send_app_msg(&mut app_sender, AppMsg::Quit);

    Ok(())
}
