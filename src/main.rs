use crate::dbus::player::MprisPlayer;
use crate::opensonic::cache::{AlbumCache, ArtistCache, CoverCache, LyricsCache, SongCache, SuperCache};
use crate::opensonic::client::{self, OpenSubsonicClient};
use crate::player::{PlayerInfo, TrackList};
use crate::ui::app::{AppMsg, Model, StartInit};
use crate::ui::setup::{SetupMsg, SetupOut, SetupWidget};
use async_channel::{Receiver, Sender};
use libsecret::{password_lookup_sync, Schema, SchemaFlags};
use mpris_server::{LocalPlayerInterface, LocalServer};
use relm4::adw::{glib, Application};
use relm4::adw::glib::clone;
use relm4::adw::prelude::{ApplicationExtManual, GtkApplicationExt, WidgetExt};
use relm4::component::{AsyncComponentBuilder, AsyncComponentController};
use relm4::gtk::gio::prelude::{ApplicationExt, SettingsExt};
use relm4::gtk::gio::{ApplicationFlags, Cancellable, Settings};
use relm4::RelmApp;
use std::cell::RefCell;
use std::collections::HashMap;
use std::error::Error;
use std::os::unix::process::CommandExt;
use std::process::Command;
use std::rc::Rc;
use std::sync::{Arc, LazyLock};
use std::io;
use gstreamer_play::PlayState;
use relm4::prelude::AsyncController;
use tokio::runtime::Handle;
use zbus::blocking;
use crate::ui::current_song::CurrentSongMsg;

mod dbus;
mod opensonic;
mod player;
mod ui;

const APP_ID: &'static str = "me.quartzy.sanicrs";
const VERSION_STR: &'static str = "0.0.0";
const DBUS_NAME_PREFIX: &'static str = "org.mpris.MediaPlayer2.";

mod icon_names {
    include!(concat!(env!("OUT_DIR"), "/icon_names.rs"));
}

pub enum PlayerCommand {
    Quit(bool),
    Raise,
    TrackOver,
    Close,
    Error(String, String),
    PositionUpdate(f64),
    PlayStateUpdate(PlayState)
}

fn do_setup(settings: &Settings, secret_schema: &Schema) -> OpenSubsonicClient {
    let setup_app: RelmApp<SetupMsg> = RelmApp::new(APP_ID);
    let (setup_send, setup_recv) = async_channel::bounded::<SetupOut>(1);
    relm4_icons::initialize_icons(icon_names::GRESOURCE_BYTES, icon_names::RESOURCE_PREFIX);

    let gtk_app = relm4::main_adw_application();
    setup_app.run_async::<SetupWidget>((settings.clone(), setup_send, secret_schema.clone()));
    let client = setup_recv.try_recv().expect("Error receiving message from setup");
    gtk_app.quit();
    client
}

fn make_client_from_saved(settings: &Settings, secret_schema: &Schema) -> Result<OpenSubsonicClient, String> {
    let host: String = settings.value("server-url").as_maybe().ok_or("Server-url not set".to_string())?.get().ok_or("Should be string".to_string())?;
    let username: String = settings.value("username").as_maybe().ok_or("Username not set".to_string())?.get().ok_or("Should be string".to_string())?;
    Ok(OpenSubsonicClient::new(
            host.as_str(),
            username.as_str(),
            password_lookup_sync(Some(&secret_schema), HashMap::new(), Cancellable::NONE)
                .map_err(|e| format!("{:?}", e))?
                .ok_or("No password found in secret store")?.as_str(),
            "Sanic-rs",
            if settings.boolean("should-cache-covers") {client::get_default_cache_dir()} else {None},
        ).map_err(|e| format!("{:?}", e))?
    )
}

fn make_window(app: &Application, payload: &StartInit) -> AsyncController<Model> {
    let builder = AsyncComponentBuilder::<Model>::default();

    let connector = builder.launch(payload.clone());

    let controller = connector.detach();
    let window = controller.widget();
    window.set_visible(true);
    app.add_window(window);

    controller
}

pub fn run_async(payload: StartInit, controller_cell: Rc<RefCell<Option<AsyncController<Model>>>>){
    let app = relm4::main_adw_application();
    app.connect_startup(move |app| {
        let controller = make_window(app, &payload);
        let old = controller_cell.replace(Some(controller));
        drop(old); // Would be dropped anyway but just making it explicit
    });

    app.connect_activate(move |app| {
        if let Some(window) = app.active_window() {
            window.set_visible(true);
        }
    });

    app.run();

    // Make sure everything is shut down
    glib::MainContext::ref_thread_default().iteration(true);
}

fn main() -> Result<(), Box<dyn Error>> {
    // First check if app is already running
    {
        let session = blocking::Connection::session()?;

        let reply = session
            .call_method(Some(DBUS_NAME_PREFIX.to_owned() + APP_ID), "/org/mpris/MediaPlayer2", Some("org.mpris.MediaPlayer2"), "Raise", &());
        if reply.is_ok() {
            println!("An instance is already running. Raised.");
            return Ok(());
        }
    }

    let should_restart;
    {
        let settings = Settings::new(APP_ID);

        let secret_schema = Schema::new(APP_ID, SchemaFlags::NONE, HashMap::new());

        static CLIENT: LazyLock<OpenSubsonicClient> = LazyLock::new(|| {
            println!("initializing");
            let settings = Settings::new(APP_ID);

            let secret_schema = Schema::new(APP_ID, SchemaFlags::NONE, HashMap::new());
            if settings.value("server-url").as_maybe().is_none() {
                do_setup(&settings, &secret_schema)
            } else {
                match make_client_from_saved(&settings, &secret_schema) {
                    Ok(c) => c,
                    Err(e) => {
                        eprintln!("Error when trying to make client: {}", e);
                        do_setup(&settings, &secret_schema)
                    }
                }
            }
        });
        let song_cache = SongCache::new(&CLIENT);
        let album_cache = AlbumCache::new(&CLIENT, song_cache.clone());
        let cover_cache = CoverCache::new(&CLIENT);
        let lyrics_cache = LyricsCache::new(&CLIENT);
        let artist_cache = ArtistCache::new(&CLIENT);
        let super_cache = SuperCache::new(&album_cache, &song_cache, &artist_cache, &CLIENT);

        relm4_icons::initialize_icons(icon_names::GRESOURCE_BYTES, icon_names::RESOURCE_PREFIX);
        let adw_app = Application::new(Some(APP_ID), ApplicationFlags::empty());
        let _app: RelmApp<AppMsg> = RelmApp::from_app(adw_app);
        let controller_cell: Rc<RefCell<Option<AsyncController<Model>>>> = Rc::new(RefCell::new(None));


        let (command_send, command_recv) = async_channel::unbounded::<PlayerCommand>();
        let (restart_send, restart_recv) = async_channel::bounded::<bool>(1);
        let command_send = Arc::new(command_send);

        let player_inner = PlayerInfo::new(
            &CLIENT,
            TrackList::new(),
            command_send.clone()
        )?;
        player_inner.load_settings(&settings).expect("Error loading player settings");
        let player = MprisPlayer {
            client: &CLIENT,
            cmd_channel: command_send.clone(),
            player_ref: player_inner,
            app_sender: RefCell::new(None),
            tl_sender: RefCell::new(None),
            cs_sender: RefCell::new(None),
            bb_sender: RefCell::new(None),
            server: RefCell::new(None),
            song_cache: song_cache.clone(),
            album_cache: album_cache.clone(),
            settings: settings.clone(),
        };


        let (mpris_send, mpris_receive) = async_channel::unbounded::<Rc<LocalServer<MprisPlayer>>>();
        let payload: StartInit = (
            cover_cache.clone(),
            song_cache.clone(),
            album_cache.clone(),
            settings.clone(),
            secret_schema,
            lyrics_cache,
            mpris_receive,
            artist_cache,
            super_cache
        );


        // Get relm4's internal runtime handle and enter it, because relm4's run function isn't being called
        // so the runtime has to be entered manually.
        let handle;
        {
            let (runtime_send, runtime_recv) = async_channel::bounded::<Handle>(1);
            relm4::spawn(async move {
                runtime_send.send(Handle::current()).await.expect("Error sending runtime");
            });
            handle = runtime_recv.recv_blocking().expect("Error receiving runtime");
        }
        let _guard = handle.enter();



        relm4::spawn_local(clone!(
            #[strong]
            payload,
            #[strong]
            controller_cell,
            async move {
                let restart = app_main(command_recv,
                   player,
                   payload,
                   mpris_send,
                    controller_cell
                ).await.expect("Error");
                restart_send.send(restart).await.expect("Error sending restart message");
            }
        ));

        run_async(payload, controller_cell);

        should_restart = restart_recv.try_recv().unwrap_or(false);
    }
    if should_restart {
        Err::<(), io::Error>(Command::new("/proc/self/exe").exec()).expect("Failed trying to restart process");
    }

    Ok(())
}

async fn app_main(
    command_recv: Receiver<PlayerCommand>,
    player: MprisPlayer,
    payload: StartInit,
    mpris_send: Sender<Rc<LocalServer<MprisPlayer>>>,
    controller_cell: Rc<RefCell<Option<AsyncController<Model>>>>
) -> Result<bool, Box<dyn Error>> {
    let server: Rc<LocalServer<MprisPlayer>> = Rc::new(LocalServer::new_with_track_list(APP_ID, player).await?);
    mpris_send.send(server.clone()).await?;
    server.imp().server.replace(Some(server.clone()));
    let _h = relm4::main_application().hold();
    let task = server.run();

    tokio::select! {
        _ = task => {
            Ok(false)
        }
        ret = handle_command(&command_recv, &server, &payload, &mpris_send, &controller_cell) => {
            Ok(ret)
        }
    }
}

async fn handle_command(
    command_recv: &Receiver<PlayerCommand>,
    server: &Rc<LocalServer<MprisPlayer>>,
    payload: &StartInit,
    mpris_send: &Sender<Rc<LocalServer<MprisPlayer>>>,
    controller_cell: &Rc<RefCell<Option<AsyncController<Model>>>>
) -> bool {
    loop {
        match command_recv.recv().await.expect("Error receiving message from command_recv") {
            PlayerCommand::Quit(should_restart) => return should_restart,
            PlayerCommand::Close => {
                let old = controller_cell.replace(None);
                drop(old);
            }
            PlayerCommand::Raise => {
                let app = relm4::main_adw_application();
                if app.windows().len() == 0 { // Only allow 1 window
                    let controller = make_window(&app, payload);
                    let old = controller_cell.replace(Some(controller));
                    drop(old);
                    mpris_send.send(server.clone()).await.expect("Error sending MPRIS server instance to app");
                }
            },
            PlayerCommand::TrackOver => server.imp().send_res_fdo(server.imp().next().await),
            PlayerCommand::Error(error, description) => server.imp().send_app_msg(AppMsg::ShowError(error, description)),
            PlayerCommand::PositionUpdate(pos) => server.imp().send_cs_msg(CurrentSongMsg::ProgressUpdateSync(pos)),
            PlayerCommand::PlayStateUpdate(state) => server.imp().update_playstate(state).await,
        }
    }
}
