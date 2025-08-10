use std::collections::HashMap;
use std::u32;

use libsecret::{password_store_future, Schema};
use relm4::adw::prelude::PreferencesPageExt;
use relm4::adw;
use relm4::adw::prelude::*;
use relm4::gtk::gio::Settings;
use relm4::gtk::glib::Variant;
use relm4::gtk::{Editable};
use relm4::adw::gtk;
use relm4::prelude::*;

use crate::icon_names;

pub struct PreferencesWidget {
    settings: Settings,
    schema: Schema,
    requires_restart: bool,
}

#[derive(Debug)]
pub enum PreferencesMsg {
    AuthChanged{pass: bool},
    Closed,
}

#[derive(Debug)]
pub enum PreferencesOut {
    Restart,
    ReloadPlayer
}

#[relm4::component(pub async)]
impl AsyncComponent for PreferencesWidget {
    type CommandOutput = ();
    type Input = PreferencesMsg;
    type Output = PreferencesOut;
    type Init = (Settings, Schema);

    view! {
        adw::PreferencesDialog {
            connect_closed => PreferencesMsg::Closed,
            set_content_height: 512,

            add = &adw::PreferencesPage {
                set_title: "Player",
                set_icon_name: Some(icon_names::MUSIC_NOTE_SINGLE),

                adw::PreferencesGroup {

                    #[name = "replay_gain"]
                    adw::ComboRow {
                        #[wrap(Some)]
                        set_model = &gtk::StringList::new(&["None", "Track", "Album"]),
                        set_title: "Replay gain mode"
                    },
                    #[name = "open_in_bg"]
                    adw::SwitchRow {
                        set_title: "Remain open in background"
                    },
                }
            },
            add = &adw::PreferencesPage {
                set_title: "Server",
                set_icon_name: Some(icon_names::NETWORK_SERVER),

                adw::PreferencesGroup {
                    set_title: "Authenticaion",
                    set_description: Some("(requires restart)"),

                    #[name = "server_url"]
                    adw::EntryRow {
                        set_show_apply_button: true,
                        set_title: "Server URL",
                        connect_apply => PreferencesMsg::AuthChanged{pass: false},
                    },
                    #[name = "username"]
                    adw::EntryRow {
                        set_show_apply_button: true,
                        set_title: "Username",
                        connect_apply => PreferencesMsg::AuthChanged{pass: false},
                    },
                    #[name = "password"]
                    adw::PasswordEntryRow {
                        set_show_apply_button: true,
                        set_title: "Password",
                        connect_apply => PreferencesMsg::AuthChanged{pass: true},
                    }
                },
                adw::PreferencesGroup {
                    #[name = "cache_albums"]
                    adw::SwitchRow {
                        set_title: "Cache albums"
                    }
                }
            },
        }
    }

    async fn init(
        init: Self::Init,
        root: adw::PreferencesDialog,
        sender: AsyncComponentSender<Self>,
    ) -> AsyncComponentParts<Self> {
        let model = Self {
            settings: init.0,
            requires_restart: false,
            schema: init.1
        };

        let widgets: PreferencesWidgetWidgets = view_output!();

        set_text_from_setting(&widgets.server_url, "server-url", &model.settings);
        set_text_from_setting(&widgets.username, "username", &model.settings);

        model.settings.bind("should-cache-covers", &widgets.cache_albums, "active").build();
        widgets.replay_gain.set_selected(model.settings.value("replay-gain-mode").get::<u8>().unwrap() as u32);
        model.settings.bind("stay-in-background", &widgets.open_in_bg, "active").build();

        AsyncComponentParts { model, widgets }
    }

    async fn update_with_view(
        &mut self,
        widgets: &mut Self::Widgets,
        message: Self::Input,
        sender: AsyncComponentSender<Self>,
        _root: &adw::PreferencesDialog,
    ) {
        match message {
            PreferencesMsg::AuthChanged{pass} => {
                let host = widgets.server_url.text();
                let username = widgets.username.text();
                self.settings
                    .set_value("server-url",
                        &Variant::from_some(
                            &Variant::from(host.as_str())
                        ))
                    .expect("Error setting server url setting");
                self.settings
                    .set_value("username",
                        &Variant::from_some(
                            &Variant::from(username.as_str())
                        ))
                    .expect("Error setting server url setting");

                if pass {
                    let password = widgets.password.text();

                    password_store_future(
                        Some(&self.schema),
                       HashMap::new(),
                       Some(&libsecret::COLLECTION_DEFAULT),
                       "OpenSubsoncic password",
                       password.as_str())
                    .await
                    .expect("Error storing password in secret store");
                }
                self.requires_restart = true;
            },
            PreferencesMsg::Closed => {
                self.settings.set("replay-gain-mode", Variant::from(widgets.replay_gain.selected() as u8)).expect("Error setting replay gain");

                sender.output(PreferencesOut::ReloadPlayer).expect("Error sending message out");
                if self.requires_restart {
                    sender.output(PreferencesOut::Restart).expect("Error sending message out");
                }
            }
        }
        self.update_view(widgets, sender);
    }
}

fn set_text_from_setting<T: IsA<Editable>>(widget: &T, setting: &str, settings: &Settings) {
    match settings.value(setting).as_maybe() {
        Some(val) => {
            let val = val.get::<String>().unwrap_or("".to_string());
            widget.set_text(val.as_str());
        },
        None => {},
    }
}
