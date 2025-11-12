use std::collections::HashMap;

use async_channel::Sender;
use libsecret::password_store_future;
use libsecret::Schema;
use relm4::gtk::gio::Settings;
use relm4::gtk::glib::Variant;
use relm4::gtk::{Align, IconSize, Orientation};
use relm4::prelude::*;
use relm4::adw;
use relm4::gtk;
use relm4::adw::glib::clone;
use relm4::adw::gtk::prelude::*;
use relm4::adw::LengthUnit;
use relm4::adw::glib as glib;
use crate::APP_ID;
use crate::opensonic::client;
use crate::opensonic::client::{Credentials, OpenSubsonicClient};


pub struct SetupWidget {
    sender: Sender<OpenSubsonicClient>,
    settings: Settings,
    schema: Schema,
}

#[derive(Debug)]
pub enum SetupMsg {
    Test,
    Save,
}

pub type SetupOut = OpenSubsonicClient;

#[relm4::component(pub async)]
impl AsyncComponent for SetupWidget {
    type CommandOutput = ();
    type Input = SetupMsg;
    type Output = ();
    type Init = (Settings, Sender<OpenSubsonicClient>, Schema, Option<String>);

    view! {
        adw::ApplicationWindow {
            set_title: Some("Sanic-RS - Setup"),
            set_default_width: 400,
            set_default_height: 400,

            adw::Clamp {
                set_orientation: Orientation::Horizontal,
                set_maximum_size: 600,
                set_tightening_threshold: 400,
                set_unit: LengthUnit::Px,

                gtk::Box {
                    set_orientation: Orientation::Vertical,
                    set_valign: Align::Center,
                    set_halign: Align::Fill,
                    set_spacing: 10,

                    gtk::Image {
                        set_icon_name: Some(APP_ID),
                        set_icon_size: IconSize::Large,
                    },
                    gtk::Label {
                        set_label: "Server URL",
                        add_css_class: "bold"
                    },
                    #[name = "server_url"]
                    gtk::Entry {
                        set_placeholder_text: Some("https://music.example.com")
                    },
                    #[name = "up_toggle"]
                    gtk::ToggleButton {
                        set_label: "Username & Password",
                    },
                    #[name = "api_key_toggle"]
                    gtk::ToggleButton {
                        set_label: "API Key",
                    },
                    #[name = "stack"]
                    gtk::Stack {
                        #[name = "name_pass_box"]
                        gtk::Box {
                            set_orientation: Orientation::Vertical,
                            set_spacing: 10,
                            set_halign: Align::Fill,
                            set_valign: Align::Center,

                            gtk::Label {
                                set_label: "Username",
                                add_css_class: "bold"
                            },
                            #[name = "username"]
                            gtk::Entry {
                                set_placeholder_text: Some("Username")
                            },
                            gtk::Label {
                                set_label: "Password",
                                add_css_class: "bold"
                            },
                            #[name = "password"]
                            gtk::PasswordEntry {
                                set_placeholder_text: Some("Password")
                            },
                        },
                        #[name = "key_box"]
                        gtk::Box {
                            set_orientation: Orientation::Vertical,
                            set_spacing: 10,
                            set_halign: Align::Fill,
                            set_valign: Align::Center,

                            gtk::Label {
                                set_label: "API Key",
                                add_css_class: "bold"
                            },
                            #[name = "api_key"]
                            gtk::Entry {
                                set_placeholder_text: Some("API Key")
                            },
                        },
                    },
                    gtk::Box {
                        set_orientation: Orientation::Horizontal,
                        set_valign: Align::Center,
                        set_halign: Align::Start,
                        set_spacing: 15,

                        gtk::Button {
                            set_label: "Test",
                            connect_clicked => SetupMsg::Test
                        },
                        gtk::Button {
                            set_label: "Save",
                            connect_clicked => SetupMsg::Save
                        }
                    },
                    #[name = "status"]
                    gtk::Label {
                        add_css_class: "title-4",
                        set_label: ""
                    }
                }
            }
        }
    }

    async fn init(
        init: Self::Init,
        root: adw::ApplicationWindow,
        sender: AsyncComponentSender<Self>,
    ) -> AsyncComponentParts<Self> {
        let model = Self {
            settings: init.0,
            sender: init.1,
            schema: init.2,
        };

        let widgets: SetupWidgetWidgets = view_output!();

        if let Some(host) = model.settings.value("server-url")
            .as_maybe().and_then(|s| s.get::<String>()) {
            widgets.server_url.set_text(&host);
        }

        if let Some(err) = init.3 {
            widgets.status.set_label(format!("Error while creating client: {:?}", err).as_str());
            widgets.status.set_css_classes(&["error"]);
        }
        
        widgets.up_toggle.set_active(true);
        widgets.up_toggle.set_group(Some(&widgets.api_key_toggle));

        widgets.up_toggle.connect_toggled(clone!(
            #[weak(rename_to = stack)]
            widgets.stack,
            #[weak(rename_to = up_box)]
            widgets.name_pass_box,
            move |this| {
                if this.is_active() {
                    stack.set_visible_child(&up_box);
                }
            }
        ));
        widgets.api_key_toggle.connect_toggled(clone!(
            #[weak(rename_to = stack)]
            widgets.stack,
            #[weak(rename_to = key_box)]
            widgets.key_box,
            move |this| {
                if this.is_active() {
                    stack.set_visible_child(&key_box);
                }
            }
        ));
        model.settings.bind("use-api-key", &widgets.api_key_toggle, "active").build();

        AsyncComponentParts { model, widgets }
    }

    async fn update_with_view(
        &mut self,
        widgets: &mut Self::Widgets,
        message: Self::Input,
        sender: AsyncComponentSender<Self>,
        root: &adw::ApplicationWindow,
    ) {
        let credentials = if widgets.up_toggle.is_active() {
            Credentials::UsernamePassword {
                username: widgets.username.text().to_string(),
                password: widgets.password.text().to_string(),
            }
        } else {
            Credentials::ApiKey {
                key: widgets.api_key.text().to_string(),
            }
        };
        match message {
            SetupMsg::Test => {
                let client = OpenSubsonicClient::new(widgets.server_url.text().as_str(),
                    credentials, "Sanic-rs", None);
                if let Err(e) = client.init().await {
                    widgets.status.set_label(format!("Error while creating client: {:?}", e).as_str());
                    widgets.status.set_css_classes(&["error"]);
                } else {
                    widgets.status.set_label("Success");
                    widgets.status.set_css_classes(&["success"]);
                }
            },
            SetupMsg::Save => {
                let host = widgets.server_url.text();
                let client = OpenSubsonicClient::new(
                    host.as_str(),
                    credentials.clone(),
                    "Sanic-rs",
                    if self.settings.boolean("should-cache-covers") {client::get_default_cache_dir()} else {None}
                );
                if let Err(e) = client.init().await {
                    widgets.status.set_label(format!("Error while creating client: {:?}", e).as_str());
                    widgets.status.set_css_classes(&["error"]);
                } else {
                    widgets.status.set_label("Success");
                    widgets.status.set_css_classes(&["success"]);

                    self.settings.set_value("server-url", &Variant::from_some(&Variant::from(host.as_str()))).expect("Error setting server url setting");
                    
                    let password = match credentials {
                        Credentials::UsernamePassword { username, password } => {
                            self.settings.set_value("username", &Variant::from_some(&Variant::from(username.as_str()))).expect("Error setting username setting");
                            password
                        }
                        Credentials::ApiKey { key } => {
                            key
                        }
                    };
                    password_store_future(
                        Some(&self.schema),
                        HashMap::new(),
                        Some(&libsecret::COLLECTION_DEFAULT),
                        "OpenSubsoncic password",
                        password.as_str())
                        .await
                        .expect("Error storing password in secret store");

                    self.sender.send(client).await.expect("Error sending created client");

                    root.close();
                }
            },
        }
        self.update_view(widgets, sender);
    }
}
