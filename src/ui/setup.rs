use std::collections::HashMap;

use async_channel::Sender;
use libsecret::password_store_future;
use libsecret::Schema;
use relm4::gtk::gio::Settings;
use relm4::gtk::glib::Variant;
use relm4::gtk::{Align, IconSize, Orientation};
use relm4::prelude::*;
use relm4::adw;
use relm4::adw::gtk::prelude::*;
use relm4::adw::LengthUnit;
use crate::APP_ID;
use crate::opensonic::client;
use crate::opensonic::client::OpenSubsonicClient;


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
    type Init = (Settings, Sender<OpenSubsonicClient>, Schema);

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

        AsyncComponentParts { model, widgets }
    }

    async fn update_with_view(
        &mut self,
        widgets: &mut Self::Widgets,
        message: Self::Input,
        sender: AsyncComponentSender<Self>,
        root: &adw::ApplicationWindow,
    ) {
        match message {
            SetupMsg::Test => {
                let client = OpenSubsonicClient::new(widgets.server_url.text().as_str(),
                    widgets.username.text().as_str(), widgets.password.text().as_str(), "Sanic-rs", None);
                if let Err(e) = client {
                    widgets.status.set_label(format!("Error while creating client: {:?}", e).as_str());
                    widgets.status.set_css_classes(&["error"]);
                } else {
                    widgets.status.set_label("Success");
                    widgets.status.set_css_classes(&["success"]);
                }
            },
            SetupMsg::Save => {
                let host = widgets.server_url.text();
                let username = widgets.username.text();
                let password = widgets.password.text();
                let client = OpenSubsonicClient::new(
                    host.as_str(),
                    username.as_str(),
                    password.as_str(),
                    "Sanic-rs",
                    if self.settings.boolean("should-cache-covers") {client::get_default_cache_dir()} else {None}
                );
                if client.is_err() {
                    widgets.status.set_label(format!("Error while creating client: {:?}", client.err().unwrap()).as_str());
                    widgets.status.set_css_classes(&["error"]);
                } else {
                    widgets.status.set_label("Success");
                    widgets.status.set_css_classes(&["success"]);

                    self.settings.set_value("server-url", &Variant::from_some(&Variant::from(host.as_str()))).expect("Error setting server url setting");
                    self.settings.set_value("username", &Variant::from_some(&Variant::from(username.as_str()))).expect("Error setting username setting");
                    password_store_future(
                        Some(&self.schema),
                       HashMap::new(),
                       Some(&libsecret::COLLECTION_DEFAULT),
                       "OpenSubsoncic password",
                       password.as_str())
                    .await
                    .expect("Error storing password in secret store");

                    self.sender.send(client.unwrap()).await.expect("Error sending created client");

                    root.close();
                }
            },
        }
        self.update_view(widgets, sender);
    }
}
