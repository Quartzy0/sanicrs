use crate::dbus::player::MprisPlayer;
use crate::opensonic::types::{duration_display_str, Song};
use crate::ui::album_object::AlbumObject;
use crate::ui::app::{PlayAlbum, PlaySong, QueueAlbum, QueueSong, ViewAlbumInfo, ViewArtistInfo, ViewSongInfo};
use crate::ui::artist_object::ArtistObject;
use crate::ui::item_list::ItemType;
use mpris_server::LocalServer;
use relm4::actions::ActionName;
use relm4::adw;
use relm4::adw::gio;
use relm4::adw::gtk;
use relm4::adw::prelude::*;
use relm4::gtk::{Align, Orientation};
use relm4::prelude::*;
use std::rc::Rc;

pub struct InfoDialogWidget {
    server: Rc<LocalServer<MprisPlayer>>
}

#[derive(Debug)]
pub enum InfoDialogUpdate {
    Song { song: Rc<Song> },
    Album { album: AlbumObject },
    Artist { artist: ArtistObject },
}

pub type InfoDialogInit = Rc<LocalServer<MprisPlayer>>;

pub fn make_popup_menu(item_type: ItemType, id: String) -> gtk::PopoverMenu {
    let menu = gio::Menu::new();
    let (view_info_action, play_action, queue_action) = match item_type {
        ItemType::Song => (ViewSongInfo::action_name(), Some(PlaySong::action_name()), Some(QueueSong::action_name())),
        ItemType::Album => (ViewAlbumInfo::action_name(), Some(PlayAlbum::action_name()), Some(QueueAlbum::action_name())),
        ItemType::Artist => (ViewArtistInfo::action_name(), None, None),
    };
    if let Some(play_action) = play_action {
        let play_item = gio::MenuItem::new(Some("Play"), Some(play_action.as_str()));
        play_item.set_action_and_target_value(Some(play_action.as_str()), Some(&id.to_variant()));
        menu.append_item(&play_item);
    }
    if let Some(queue_action) = queue_action {
        let queue_item = gio::MenuItem::new(Some("Add to queue"), Some(queue_action.as_str()));
        queue_item.set_action_and_target_value(Some(queue_action.as_str()), Some(&id.to_variant()));
        menu.append_item(&queue_item);
    }
    let info_item = gio::MenuItem::new(Some("View info"), Some(view_info_action.as_str()));
    info_item.set_action_and_target_value(Some(view_info_action.as_str()), Some(&id.to_variant()));
    menu.append_item(&info_item);

    gtk::PopoverMenu::from_model_full(&menu, gtk::PopoverMenuFlags::NESTED)
}

#[relm4::component(pub async)]
impl AsyncComponent for InfoDialogWidget {
    type CommandOutput = ();
    type Input = InfoDialogUpdate;
    type Output = ();
    type Init = InfoDialogInit;

    view! {
        adw::Dialog {
            set_can_close: true,
            set_title: "Detailed info",
            set_follows_content_size: true,

            #[wrap(Some)]
            set_child = &gtk::Box {
                set_orientation: Orientation::Vertical,
                set_spacing: 20,
                set_margin_all: 20,

                #[name = "info_box"]
                gtk::ListBox {
                    set_selection_mode: gtk::SelectionMode::None,
                    add_css_class: "boxed-list",
                },
                gtk::Expander {
                    set_label: Some("Raw JSON"),

                    gtk::ScrolledWindow {
                        set_min_content_height: 200,
                        #[name = "raw_info_text"]
                        gtk::TextView {
                            set_editable: false,
                        }
                    }
                }
            }
        }
    }

    async fn init(
        init: Self::Init,
        root: Self::Root,
        _sender: AsyncComponentSender<Self>,
    ) -> AsyncComponentParts<Self> {
        let model = InfoDialogWidget {
            server: init
        };
        let widgets: Self::Widgets = view_output!();

        AsyncComponentParts { model, widgets }
    }

    async fn update_with_view(
        &mut self,
        widgets: &mut Self::Widgets,
        message: Self::Input,
        sender: AsyncComponentSender<Self>,
        _root: &adw::Dialog,
    ) {
        let fields: Option<(serde_json::Result<String>, Vec<(String, String)>)> = match &message {
            InfoDialogUpdate::Song { song } => {
                Some((serde_json::to_string_pretty(song.as_ref()), vec![
                    ("ID".into(), song.id.clone()),
                    ("Title".into(), song.title.clone()),
                    ("Artist".into(), song.artists_no_markup()),
                    ("Path".into(), song.path.clone().unwrap_or("Unknown".into())),
                    ("Duration".into(), song.duration.and_then(|d| Some(duration_display_str(&d))).unwrap_or("Unknown".into())),
                    ("Play count".into(), song.play_count.and_then(|i| Some(i.to_string())).unwrap_or("Unknown".into())),
                    ("Created".into(), song.created.clone().unwrap_or("Unknown".into())),
                    ("Bit rate".into(), song.bit_rate.clone().and_then(|i| Some(i.to_string())).unwrap_or("Unknown".into())),
                    ("Bit depth".into(), song.bit_depth.clone().and_then(|i| Some(i.to_string())).unwrap_or("Unknown".into())),
                    ("Channel count".into(), song.channel_count.clone().and_then(|i| Some(i.to_string())).unwrap_or("Unknown".into())),
                    ("Sampling rate".into(), song.sampling_rate.clone().and_then(|i| Some(i.to_string())).unwrap_or("Unknown".into())),
                    ("MusicBrainz ID".into(), song.music_brainz_id.clone().unwrap_or("Unknown".into())),
                    ("ISRC".into(), song.isrc.clone().and_then(|v| Some(v.join(", "))).unwrap_or("Unknown".into())),
                ]))
            },
            InfoDialogUpdate::Album { album } => {
                let album = album.get_inner();
                if let Some(album) = album {
                    Some((serde_json::to_string_pretty(&album), vec![
                        ("Artist".into(), album.artists_no_markup()),
                        ("ID".into(), album.id),
                        ("Name".into(), album.name),
                        ("Song count".into(), album.song_count.to_string()),
                        ("Duration".into(), duration_display_str(&album.duration)),
                        ("Play count".into(), album.play_count.and_then(|i| Some(i.to_string())).unwrap_or("Unknown".into())),
                        ("MusicBrainz ID".into(), album.music_brainz_id.unwrap_or("Unknown".into())),
                        ("Year".into(), album.year.and_then(|i| Some(i.to_string())).unwrap_or("Unknown".into())),
                    ]))
                } else {
                    None
                }
            },
            InfoDialogUpdate::Artist { artist } => {
                let artist = artist.get_inner();
                if let Some(artist) = artist {
                    Some((serde_json::to_string_pretty(&artist), vec![
                        ("ID".into(), artist.id),
                        ("Name".into(), artist.name),
                        ("Album count".into(), artist.album_count.and_then(|i| Some(i.to_string())).unwrap_or("Unknown".into())),
                        ("MusicBrainz ID".into(), artist.music_brainz_id.unwrap_or("Unknown".into())),
                    ]))
                } else {
                    None
                }
            },
        };
        widgets.info_box.remove_all();
        if fields.is_none() {
            self.server.imp().send_error("Error getting info fields for song/album/artist in info dialog.".into());
            return;
        }
        let fields = fields.unwrap();
        let rows = fields.1.into_iter().map(|e| {
            let row = gtk::ListBoxRow::new();
            row.set_margin_horizontal(5);
            let hbox = gtk::CenterBox::builder()
                .orientation(Orientation::Horizontal)
                .halign(Align::Fill)
                .build();
            let prop_name = gtk::Label::new(Some(&e.0));
            let prop_value = gtk::Label::new(Some(&e.1));
            prop_value.set_selectable(true);
            prop_value.add_css_class("monospace");
            hbox.set_start_widget(Some(&prop_name));
            hbox.set_end_widget(Some(&prop_value));
            row.set_child(Some(&hbox));

            row
        });
        for row in rows {
            widgets.info_box.append(&row);
        }
        widgets.raw_info_text.buffer().set_text(fields.0.unwrap_or_else(|e| format!("Error serializing: {e}")).as_str());
        self.update_view(widgets, sender);
    }
}
