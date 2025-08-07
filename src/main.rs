use crate::dbus::base::MprisBase;
use crate::dbus::player::{MprisPlayer, MprisPlayerSignals};
use crate::dbus::track_list::{track_list_replaced, MprisTrackList, MprisTrackListSignals};
use crate::opensonic::client::{self, OpenSubsonicClient};
use crate::player::{LoopStatus, PlayerInfo, TrackList};
use crate::ui::app::{AppMsg, Init, Model};
use crate::ui::current_song::{CurrentSong, CurrentSongMsg};
use crate::ui::setup::{SetupMsg, SetupOut, SetupWidget};
use crate::ui::track_list::{MoveDirection, TrackListMsg, TrackListWidget};
use libsecret::{password_lookup_sync, Schema, SchemaFlags};
use readlock_tokio::{Shared};
use relm4::gtk::gio::prelude::{ApplicationExt, SettingsExt};
use relm4::gtk::gio::{ApplicationFlags, Cancellable, Settings, SettingsBackend, SettingsSchemaSource};
use relm4::{adw, AsyncComponentSender, RelmApp};
use rodio::OutputStreamBuilder;
use std::collections::HashMap;
use std::{env, io};
use std::error::Error;
use std::os::unix::process::CommandExt;
use std::path::Path;
use std::process::Command;
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
use zbus::blocking;
use zvariant::ObjectPath;
use crate::opensonic::cache::{AlbumCache, CoverCache, LyricsCache, SongCache};

mod dbus;
mod opensonic;
mod player;
mod ui;

const APP_ID: &'static str = "me.quartzy.sanicrs";
const DBUS_NAME: &'static str = "org.mpris.MediaPlayer2.sanicrs";

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
    QueueRandom{size: u32, genre: Option<String>, from_year: Option<u32>, to_year: Option<u32>},
    Restart,
    ReloadPlayerSettings,
    ReportError(String, String),
    MoveItem{index: usize, direction: MoveDirection}
}

pub async fn send_error(sender: &Sender<PlayerCommand>, error: Box<dyn Error>) {
    sender.send(PlayerCommand::ReportError(format!("{}", error), format!("{:?}", error)))
        .await.expect("Error sending error info to main thread");
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

fn do_setup(settings: &Settings, secret_schema: &Schema) -> Arc<OpenSubsonicClient> {
    let setup_app: RelmApp<SetupMsg> = RelmApp::new(APP_ID);
    let (setup_send, setup_recv) = async_channel::bounded::<SetupOut>(1);
    relm4_icons::initialize_icons(icon_names::GRESOURCE_BYTES, icon_names::RESOURCE_PREFIX);

    let gtk_app = relm4::main_adw_application();
    setup_app.run_async::<SetupWidget>((settings.clone(), setup_send, secret_schema.clone()));
    let client = setup_recv.try_recv().expect("Error receiving message from setup");
    gtk_app.quit();
    client
}

fn make_client_from_saved(settings: &Settings, secret_schema: &Schema) -> Result<Arc<OpenSubsonicClient>, String> {
    let host: String = settings.value("server-url").as_maybe().ok_or("Server-url not set".to_string())?.get().ok_or("Should be string".to_string())?;
    let username: String = settings.value("username").as_maybe().ok_or("Username not set".to_string())?.get().ok_or("Should be string".to_string())?;
    Ok(Arc::new(
        OpenSubsonicClient::new(
            host.as_str(),
            username.as_str(),
            password_lookup_sync(Some(&secret_schema), HashMap::new(), Cancellable::NONE)
                .map_err(|e| format!("{:?}", e))?
                .ok_or("No password found in secret store")?.as_str(),
            "Sanic-rs",
            if settings.boolean("should-cache-covers") {client::get_default_cache_dir()} else {None},
        ).map_err(|e| format!("{:?}", e))?
    ))
}

fn main() -> Result<(), Box<dyn Error>> {
    // First check if app is already running
    {
        let session = blocking::Connection::session()?;

        let reply = session
            .call_method(Some(DBUS_NAME), "/org/mpris/MediaPlayer2", Some("org.mpris.MediaPlayer2"), "Raise", &());
        if reply.is_ok() {
            println!("An instance is already running. Raised.");
            return Ok(());
        }
    }

    let should_restart;
    {
        let path = env::var("XDG_DATA_HOME");
        let settings_schema = match path {
            Ok(path) => SettingsSchemaSource::from_directory(Path::new(path.as_str()).join("glib-2.0/schemas").as_path(), SettingsSchemaSource::default().as_ref(), false).expect("Error getting settings scheme source"),
            Err(_) => SettingsSchemaSource::default().expect("No default settings scheme source")
        };
        let schema = settings_schema.lookup(APP_ID, false).expect(format!("No settings schema found for '{}'", APP_ID).as_str());
        let settings = Settings::new_full(&schema, None::<&SettingsBackend>, None);

        let secret_schema = Schema::new(APP_ID, SchemaFlags::NONE, HashMap::new());

        let client: Arc<OpenSubsonicClient>;
        if settings.value("server-url").as_maybe().is_none() {
            client = do_setup(&settings, &secret_schema);
        } else {
            match make_client_from_saved(&settings, &secret_schema) {
                Ok(c) => client = c,
                Err(e) => {
                    eprintln!("Error when trying to make client: {}", e);
                    client = do_setup(&settings, &secret_schema);
                }
            }
            relm4_icons::initialize_icons(icon_names::GRESOURCE_BYTES, icon_names::RESOURCE_PREFIX);
        }
        let adw_app = adw::Application::new(Some(APP_ID), ApplicationFlags::empty());
        let app: RelmApp<AppMsg> = RelmApp::from_app(adw_app);

        let stream = OutputStreamBuilder::from_default_device()
            .expect("Error building output stream")
            .open_stream()
            .expect("Error opening output stream");

        let track_list = TrackList::new();
        let track_list = Arc::new(RwLock::new(track_list));

        let (command_send, command_recv) = async_channel::unbounded::<PlayerCommand>();
        let (restart_send, restart_recv) = async_channel::bounded::<bool>(1);
        let command_send = Arc::new(command_send);

        let song_cache = SongCache::new(client.clone());
        let album_cache = AlbumCache::new(client.clone(), song_cache.clone());
        let cover_cache = CoverCache::new(client.clone());
        let lyrics_cache = LyricsCache::new(client.clone());

        let player = Shared::new(PlayerInfo::new(
            client.clone(),
            &stream,
            track_list.clone(),
            command_send.clone(),
        ));
        player.load_settings_blocking(&settings).expect("Error loading player settings");
        let player_read = Shared::<PlayerInfo>::get_read_lock(&player);
        let payload: Init = (
            player_read,
            track_list.clone(),
            cover_cache.clone(),
            command_send.clone(),
            song_cache.clone(),
            album_cache.clone(),
            settings.clone(),
            secret_schema,
            lyrics_cache
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
            #[strong]
            settings,
            async move {
                let restart = app_main(command_recv,
                   command_send,
                   client,
                   track_list,
                   player,
                   song_cache,
                   album_cache,
                   settings,
                   payload
                ).await.expect("Error");
                restart_send.send(restart).await.expect("Error sending restart status");
            }
        ));

        app.run_async::<Model>(payload);

        should_restart = restart_recv.try_recv().unwrap_or(false);
    }
    if should_restart {
        Err::<(), io::Error>(Command::new("/proc/self/exe").exec()).expect("Failed trying to restart process");
    }

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
    settings: Settings,
    payload: Init
) -> Result<bool, Box<dyn Error>> {
    let connection = Arc::new(
        connection::Builder::session()?
            .name(DBUS_NAME)?
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
    let mut should_restart = false;

    loop {
        let res: Result<Status, Box<dyn Error>> = process_command(
            command_recv.recv().await.expect("Error when receiving message"),
            &player,
            &mut app_sender,
            &mut tl_sender,
            &mut cs_sender,
            &player_ref,
            &track_list_ref,
            &track_list,
            &song_cache,
            &album_cache,
            &settings,
            &client,
            &payload).await;

        match res {
            Ok(status) => {
                match status {
                    Status::Ok => {},
                    Status::Quit => break,
                    Status::Restart => {
                        should_restart = true;
                        break;
                    },
                }
            }
            Err(e) => {
                let summary = format!("{}", e);
                let description = format!("{:?}", e);
                send_app_msg(&mut app_sender, AppMsg::ShowError(summary, description));
            },
        }
    }
    send_app_msg(&mut app_sender, AppMsg::Quit);

    Ok(should_restart)
}

enum Status {
    Ok,
    Quit,
    Restart
}

async fn process_command(
    command: PlayerCommand,
    player: &Shared<PlayerInfo>,
    app_sender: &mut Option<AsyncComponentSender<Model>>,
    tl_sender: &mut Option<AsyncComponentSender<TrackListWidget>>,
    cs_sender: &mut Option<AsyncComponentSender<CurrentSong>>,
    player_ref: &InterfaceRef<MprisPlayer>,
    track_list_ref: &InterfaceRef<MprisTrackList>,
    track_list: &Arc<RwLock<TrackList>>,
    song_cache: &SongCache,
    album_cache: &AlbumCache,
    settings: &Settings,
    client: &Arc<OpenSubsonicClient>,
    payload: &Init,
) -> Result<Status, Box<dyn Error>> {
    match command {
        PlayerCommand::Quit => return Ok(Status::Quit),
        PlayerCommand::Restart => {
            return Ok(Status::Restart);
        },
        PlayerCommand::Next => {
            let s = player.next().await;
            send_cs_msg(cs_sender, CurrentSongMsg::SongUpdate(s));
            send_tl_msg(tl_sender, TrackListMsg::TrackChanged(None));
            player_ref.get().await.metadata_changed(player_ref.signal_emitter()).await?;
            player_ref.get().await.playback_status_changed(player_ref.signal_emitter()).await?;
        },
        PlayerCommand::Previous => {
            let s = player.previous().await;
            send_cs_msg(cs_sender, CurrentSongMsg::SongUpdate(s));
            send_tl_msg(tl_sender, TrackListMsg::TrackChanged(None));
            player_ref.get().await.metadata_changed(player_ref.signal_emitter()).await?;
            player_ref.get().await.playback_status_changed(player_ref.signal_emitter()).await?;
        },
        PlayerCommand::Play => {
            player.play().await;
            send_cs_msg(cs_sender, CurrentSongMsg::PlaybackStateChange(player.playback_status()));
            player_ref.get().await.playback_status_changed(player_ref.signal_emitter()).await?;
        },
        PlayerCommand::Pause => {
            player.pause();
            send_cs_msg(cs_sender, CurrentSongMsg::PlaybackStateChange(player.playback_status()));
            player_ref.get().await.playback_status_changed(player_ref.signal_emitter()).await?;
        },
        PlayerCommand::PlayPause => {
            player.playpause().await;
            send_cs_msg(cs_sender, CurrentSongMsg::PlaybackStateChange(player.playback_status()));
            player_ref.get().await.playback_status_changed(player_ref.signal_emitter()).await?;
        },
        PlayerCommand::Stop => {
            player.stop().await;
            send_cs_msg(cs_sender, CurrentSongMsg::PlaybackStateChange(player.playback_status()));
            send_cs_msg(cs_sender, CurrentSongMsg::SongUpdate(None));
            player_ref.get().await.playback_status_changed(player_ref.signal_emitter()).await?;
            track_list_ref.track_list_replaced(Vec::new(),
                ObjectPath::from_static_str_unchecked("/org/mpris/MediaPlayer2/TrackList/NoTrack"))
            .await?;
            send_cs_msg(cs_sender, CurrentSongMsg::SetLoopStatus(LoopStatus::None));
            player_ref.get().await.loop_status_changed(player_ref.signal_emitter()).await?;
            player_ref.get().await.playback_status_changed(player_ref.signal_emitter()).await?;
        },
        PlayerCommand::SetRate(r) => {
            player.set_rate(r);
            send_cs_msg(cs_sender, CurrentSongMsg::RateChange(r));
            player_ref.get().await.rate_changed(player_ref.signal_emitter()).await?;
        },
        PlayerCommand::SetVolume(v) => {
            player.set_volume(v).await;
            settings.set_double("volume", v)?;
            send_cs_msg(cs_sender, CurrentSongMsg::VolumeChangedExternal(v));
            player_ref.get().await.volume_changed(player_ref.signal_emitter()).await?;
        },
        PlayerCommand::SetPosition(p) => {
            player.set_position(p).await?;
            send_cs_msg(cs_sender, CurrentSongMsg::ProgressUpdateSync(Some(p.as_secs_f64())));
            player_ref.seeked(p.as_secs() as i64).await?;
        },
        PlayerCommand::GoTo(i) => {
            let song = player.goto(i).await?;
            send_tl_msg(tl_sender, TrackListMsg::TrackChanged(Some(i)));
            send_cs_msg(cs_sender, CurrentSongMsg::SongUpdate(song));
            player_ref.get().await.metadata_changed(player_ref.signal_emitter()).await?;
            player_ref.get().await.playback_status_changed(player_ref.signal_emitter()).await?;
        },
        PlayerCommand::Remove(i) => {
            let e = player.remove_song(i).await?;
            track_list_ref.track_removed(e.dbus_obj()).await?;
            send_tl_msg(tl_sender, TrackListMsg::ReloadList);
            send_cs_msg(cs_sender, CurrentSongMsg::SongUpdate(Some(e)));
            player_ref.get().await.metadata_changed(player_ref.signal_emitter()).await?;
        },
        PlayerCommand::TrackListSendSender(s) => *tl_sender = Some(s),
        PlayerCommand::CurrentSongSendSender(s) => *cs_sender = Some(s),
        PlayerCommand::AppSendSender(s) => *app_sender = Some(s),
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
                    ).await?;
                    if set_as_current {
                        track_list_guard.set_current(new_i);
                        drop(track_list_guard);
                        let song = player.start_current().await?;
                        send_cs_msg(cs_sender, CurrentSongMsg::SongUpdate(song));
                        player_ref.get().await
                            .metadata_changed(player_ref.signal_emitter()).await?;
                        player_ref.get().await.playback_status_changed(player_ref.signal_emitter()).await?;
                    }
                    send_tl_msg(tl_sender, TrackListMsg::ReloadList);
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
            send_cs_msg(cs_sender, CurrentSongMsg::SetLoopStatus(loop_status));
            player_ref.get().await.loop_status_changed(player_ref.signal_emitter()).await?;
        },
        PlayerCommand::SetShuffle(shuffle) => {
            {
                let mut guard = track_list.write().await;
                guard.set_shuffle(shuffle);
            }
            send_cs_msg(cs_sender, CurrentSongMsg::SetShuffle(shuffle));
            player_ref.get().await.shuffle_changed(player_ref.signal_emitter()).await?;
        }
        PlayerCommand::PlayAlbum(id, index) => {
            let album = album_cache.get_album(id.as_str()).await?; // TODO: this shouldn't panic
            if let Some(songs) = album.get_songs() {
                {
                    let mut guard = track_list.write().await;
                    guard.clear();
                    guard.add_songs(songs);
                    if let Some(index) = index {
                        guard.set_current(index);
                    }
                }
                let song = player.start_current().await?;
                send_cs_msg(cs_sender, CurrentSongMsg::SongUpdate(song));
                player_ref.get().await
                    .metadata_changed(player_ref.signal_emitter()).await?;
                send_tl_msg(tl_sender, TrackListMsg::ReloadList);
                player_ref.get().await.playback_status_changed(player_ref.signal_emitter()).await?;
                let guard = track_list.read().await;
                track_list_replaced(track_list_ref, guard.get_songs(), guard.current_index()).await?;
            }
        },
        PlayerCommand::QueueAlbum(id) => {
            let album = album_cache.get_album(id.as_str()).await?; // TODO: this shouldn't panic
            if let Some(songs) = album.get_songs() {
                let was_empty;
                {
                    let mut guard = track_list.write().await;
                    was_empty = guard.empty();
                    guard.add_songs(songs);
                }
                if was_empty {
                    let song = player.start_current().await?;
                    send_cs_msg(cs_sender, CurrentSongMsg::SongUpdate(song));
                    player_ref.get().await
                        .metadata_changed(player_ref.signal_emitter()).await?;
                    player_ref.get().await.playback_status_changed(player_ref.signal_emitter()).await?;
                }
                let guard = track_list.read().await;
                track_list_replaced(track_list_ref, guard.get_songs(), guard.current_index()).await?;
                send_tl_msg(tl_sender, TrackListMsg::ReloadList);
            }
        }
        PlayerCommand::QueueRandom { size, genre, from_year, to_year } => {
            let songs = song_cache.get_random_songs(Some(size), genre.as_deref(), from_year, to_year, None).await?;
            println!("Added {} random songs", songs.len());
            let was_empty;
            {
                let mut guard = track_list.write().await;
                was_empty = guard.empty();
                guard.add_songs(songs);
            }
            if was_empty {
                let song = player.start_current().await?;
                send_cs_msg(cs_sender, CurrentSongMsg::SongUpdate(song));
                player_ref.get().await
                    .metadata_changed(player_ref.signal_emitter()).await?;
                player_ref.get().await.playback_status_changed(player_ref.signal_emitter()).await?;
            }
            let guard = track_list.read().await;
            track_list_replaced(track_list_ref, guard.get_songs(), guard.current_index()).await?;
            send_tl_msg(tl_sender, TrackListMsg::ReloadList);
        },
        PlayerCommand::ReloadPlayerSettings => {
            player.load_settings(&settings).await?;
        },
        PlayerCommand::ReportError(summary, description) => send_app_msg(app_sender, AppMsg::ShowError(summary, description)),
        PlayerCommand::MoveItem { index, direction } => {
            let mut guard = track_list.write().await;
            let new_i = guard.move_song(index, direction);
            if let Some(new_i) = new_i {
                let moved = guard.song_at_index(new_i).ok_or("No song found at moved index")?;
                track_list_ref.track_removed(moved.dbus_obj()).await?;
                track_list_ref.track_added(
                    dbus::player::get_song_metadata(Some(moved), client.clone()).await,
                    if index != 0 && let Some(prev) = guard.song_at_index(index-1) {
                        prev.dbus_obj()
                    } else {
                        ObjectPath::from_static_str_unchecked("/org/mpris/MediaPlayer2/TrackList/NoTrack")
                    }
                ).await?;
                send_tl_msg(tl_sender, TrackListMsg::TrackChanged(None));
            }
        }
    };

    Ok(Status::Ok)
}
