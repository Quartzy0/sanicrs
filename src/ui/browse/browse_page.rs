use std::cmp::{max, min};
use crate::dbus::player::MprisPlayer;
use crate::opensonic::cache::{AlbumCache, CoverCache, SuperCache};
use crate::opensonic::types::AlbumListType;
use crate::ui::album_object::AlbumObject;
use crate::ui::app::Init;
use crate::ui::browse::album_list::AlbumList;
use crate::ui::cover_picture::{CoverPicture, CoverSize};
use mpris_server::LocalServer;
use relm4::adw::gio::ListStore;
use relm4::adw::glib::{clone, closure, Object};
use relm4::adw::gtk::Orientation;
use relm4::adw::prelude::*;
use relm4::gtk::pango::EllipsizeMode;
use relm4::gtk::{glib, Align, Justification, ListItem, SignalListItemFactory, Widget};
use relm4::prelude::*;
use relm4::AsyncComponentSender;
use std::rc::Rc;
use color_thief::Color;
use relm4::adw::gdk;
use uuid::Uuid;
use crate::icon_names;
use crate::ui::artist_object::ArtistObject;
use crate::ui::info_dialog;
use crate::ui::item_list::{ItemListInit, ItemListWidget, ItemType};
use crate::ui::song_object::{PositionState, SongObject};

pub struct BrowsePageWidget {
    album_cache: AlbumCache,
    mpris_player: Rc<LocalServer<MprisPlayer>>,
    cover_cache: CoverCache,
    randoms_ids: Vec<Option<String>>,
    super_cache: SuperCache,
    carousel_pos: u32,

    album_factory: SignalListItemFactory,
}

#[derive(Debug)]
pub enum BrowsePageMsg {
    ScrollNewest(i32),
    ScrollHighest(i32),
    ScrollExplore(i32),
    ScrollCarousel(i32),
    ScrolledCarousel(u32)
}

#[derive(Debug)]
pub enum BrowsePageOut {
    SetColors(Option<Vec<Color>>)
}

#[relm4::component(pub async)]
impl AsyncComponent for BrowsePageWidget {
    type CommandOutput = ();
    type Input = BrowsePageMsg;
    type Output = BrowsePageOut;
    type Init = Init;

    view! {
        adw::NavigationPage {
            set_tag: Some("browse"),
            set_title: "Browse",

            #[name = "scroll"]
            gtk::ScrolledWindow {
                set_hscrollbar_policy: gtk::PolicyType::Never,
                set_vexpand: true,
                set_vexpand_set: true,
                set_valign: Align::Fill,

                gtk::Box {
                    set_orientation: Orientation::Vertical,
                    add_css_class: "padded",

                    #[name = "carousel"]
                    adw::Carousel {
                        set_allow_scroll_wheel: false,
                        connect_page_changed[sender] => move |_, index: u32| {
                            sender.input(BrowsePageMsg::ScrolledCarousel(index));
                        }
                    },

                    #[template]
                    #[name = "newest_list"]
                    AlbumList {
                        #[template_child]
                        top_label {
                            set_label: "Newest"
                        },
                        #[template_child]
                        back_btn {
                            connect_clicked => BrowsePageMsg::ScrollNewest(-100)
                        },
                        #[template_child]
                        forward_btn {
                            connect_clicked => BrowsePageMsg::ScrollNewest(100)
                        },
                        #[template_child]
                        list {
                            set_factory: Some(&model.album_factory),
                            connect_activate => move |view, index| {
                                if let Some(model) = view.model() {
                                    let album: AlbumObject = model.item(index)
                                        .expect("Item at index clicked expected to exist")
                                        .downcast::<AlbumObject>()
                                        .expect("Item expected to be AlbumObject");
                                    view.activate_action("win.album", Some(&album.id().to_variant())).expect("Error executing action");
                                }
                            }
                        }
                    },
                    #[template]
                    #[name = "highest_list"]
                    AlbumList {
                        #[template_child]
                        top_label {
                            set_label: "Most played"
                        },
                        #[template_child]
                        back_btn {
                            connect_clicked => BrowsePageMsg::ScrollHighest(-100)
                        },
                        #[template_child]
                        forward_btn {
                            connect_clicked => BrowsePageMsg::ScrollHighest(100)
                        },
                        #[template_child]
                        list {
                            set_factory: Some(&model.album_factory),
                            connect_activate => move |view, index| {
                                if let Some(model) = view.model() {
                                    let album: AlbumObject = model.item(index)
                                        .expect("Item at index clicked expected to exist")
                                        .downcast::<AlbumObject>()
                                        .expect("Item expected to be AlbumObject");
                                    view.activate_action("win.album", Some(&album.id().to_variant())).expect("Error executing action");
                                }
                            }
                        }
                    },
                    #[template]
                    #[name = "explore_list"]
                    AlbumList {
                        #[template_child]
                        top_label {
                            set_label: "Explore"
                        },
                        #[template_child]
                        back_btn {
                            connect_clicked => BrowsePageMsg::ScrollExplore(-100)
                        },
                        #[template_child]
                        forward_btn {
                            connect_clicked => BrowsePageMsg::ScrollExplore(100)
                        },
                        #[template_child]
                        list {
                            set_factory: Some(&model.album_factory),
                            connect_activate => move |view, index| {
                                if let Some(model) = view.model() {
                                    let album: AlbumObject = model.item(index)
                                        .expect("Item at index clicked expected to exist")
                                        .downcast::<AlbumObject>()
                                        .expect("Item expected to be AlbumObject");
                                    view.activate_action("win.album", Some(&album.id().to_variant())).expect("Error executing action");
                                }
                            }
                        }
                    },
                    #[name = "starred_songs_box"]
                    gtk::Box {
                        set_orientation: Orientation::Vertical,
                        set_halign: Align::Fill,
                        set_spacing: 10,

                        gtk::Label {
                            set_label: "Starred songs",
                            add_css_class: "t0",
                            add_css_class: "bold",
                            set_halign: Align::Start,
                            set_justify: Justification::Left,
                        },
                        #[name = "starred_songs_bin"]
                        gtk::ScrolledWindow {
                            add_css_class: "padded",
                            set_vscrollbar_policy: gtk::PolicyType::Automatic,
                            set_hscrollbar_policy: gtk::PolicyType::Never,
                            set_min_content_height: 450,
                        }
                    },
                    #[name = "starred_albums_box"]
                    gtk::Box {
                        set_orientation: Orientation::Vertical,
                        set_halign: Align::Fill,
                        set_spacing: 10,

                        gtk::Label {
                            set_label: "Starred albums",
                            add_css_class: "t0",
                            add_css_class: "bold",
                            set_halign: Align::Start,
                            set_justify: Justification::Left,
                        },
                        #[name = "starred_albums_bin"]
                        gtk::ScrolledWindow {
                            add_css_class: "padded",
                            set_vscrollbar_policy: gtk::PolicyType::Automatic,
                            set_hscrollbar_policy: gtk::PolicyType::Never,
                            set_min_content_height: 450,
                        }
                    },
                    #[name = "starred_artists_box"]
                    gtk::Box {
                        set_orientation: Orientation::Vertical,
                        set_halign: Align::Fill,
                        set_spacing: 10,

                        gtk::Label {
                            set_label: "Starred artists",
                            add_css_class: "t0",
                            add_css_class: "bold",
                            set_halign: Align::Start,
                            set_justify: Justification::Left,
                        },
                        #[name = "starred_artists_bin"]
                        gtk::ScrolledWindow {
                            add_css_class: "padded",
                            set_vscrollbar_policy: gtk::PolicyType::Automatic,
                            set_hscrollbar_policy: gtk::PolicyType::Never,
                            set_min_content_height: 450,
                        }
                    }
                }
            }
        }
    }

    async fn init(
        init: Self::Init,
        root: Self::Root,
        sender: AsyncComponentSender<Self>,
    ) -> AsyncComponentParts<Self> {
        let mut model = Self {
            mpris_player: init.6,
            album_cache: init.2,
            album_factory: SignalListItemFactory::new(),
            cover_cache: init.0,
            randoms_ids: vec![],
            super_cache: init.8,
            carousel_pos: 0,
        };


        let widgets: Self::Widgets = view_output!();

        relm4::spawn_local(clone!(
            #[weak(rename_to = songs_bin)]
            widgets.starred_songs_bin,
            #[weak(rename_to = albums_bin)]
            widgets.starred_albums_bin,
            #[weak(rename_to = artists_bin)]
            widgets.starred_artists_bin,
            #[strong(rename_to = scache)]
            model.super_cache,
            #[strong(rename_to = mpris_player)]
            model.mpris_player,
            #[strong(rename_to = cover_cache)]
            model.cover_cache,
            async move {
                let starred = scache.get_starred().await;
                if let Err(err) = starred {
                    mpris_player.imp().send_error(err);
                    return;
                }
                let starred = starred.unwrap();
                if starred.0.len() == 0 {
                    songs_bin.set_visible(false);
                } else {
                    let cloned = songs_bin.clone();
                    let starred_songs_list = ItemListWidget::builder()
                        .launch(ItemListInit {
                            cover_cache: cover_cache.clone(),
                            play_fn: Some(Box::new(move |song: SongObject, _i, mpris_player| {
                                relm4::spawn_local(async move {
                                    mpris_player.imp().send_res(mpris_player.imp().set_song(song.get_entry().unwrap()).await);
                                });
                            })),
                            click_fn: Some(Box::new(move |song: SongObject, _i, _mpris_player| {
                                cloned.activate_action("win.song", Some(&song.id().to_variant())).expect("Error executing action");
                            })),
                            load_items: async move {
                                starred.0.into_iter().map(|v| SongObject::new((Uuid::max(), v).into(), PositionState::Passed))
                            },
                            mpris_player: mpris_player.clone(),
                            cover_type: Default::default(),
                            highlight: None,
                        });
                    songs_bin.set_child(Some(starred_songs_list.widget()));
                }
                if starred.1.len() == 0{
                    albums_bin.set_visible(false);
                } else {
                    let cloned = albums_bin.clone();
                    let starred_albums_list = ItemListWidget::builder()
                        .launch(ItemListInit {
                            cover_cache: cover_cache.clone(),
                            play_fn: Some(Box::new(move |album: AlbumObject, _i, mpris_player| {
                                relm4::spawn_local(async move {
                                    mpris_player.imp().send_res(mpris_player.imp().queue_album(album.id(), None, true).await);
                                });
                            })),
                            click_fn: Some(Box::new(move |album: AlbumObject, _i, _mpris_player| {
                                cloned.activate_action("win.album", Some(&album.id().to_variant())).expect("Error executing action");
                            })),
                            load_items: async move {
                                starred.1.into_iter()
                            },
                            mpris_player: mpris_player.clone(),
                            cover_type: Default::default(),
                            highlight: None,
                        });
                    albums_bin.set_child(Some(starred_albums_list.widget()));
                }
                if starred.2.len() == 0 {
                    artists_bin.set_visible(false);
                } else {
                    let cloned = artists_bin.clone();
                    let starred_artists_list = ItemListWidget::builder()
                        .launch(ItemListInit {
                            cover_cache: cover_cache.clone(),
                            play_fn: None,
                            click_fn: Some(Box::new(move |artist: ArtistObject, _i, _mpris_player| {
                                cloned.activate_action("win.artist", Some(&artist.id().to_variant())).expect("Error executing action");
                            })),
                            load_items: async move {
                                starred.2.into_iter()
                            },
                            mpris_player: mpris_player.clone(),
                            cover_type: Default::default(),
                            highlight: None,
                        });
                    artists_bin.set_child(Some(starred_artists_list.widget()));
                }
            }
        ));

        let controller = gtk::EventControllerScroll::new(gtk::EventControllerScrollFlags::VERTICAL);
        controller.set_propagation_phase(gtk::PropagationPhase::Capture);
        controller.connect_scroll(clone!(
            #[strong(rename_to = scroll)]
            widgets.scroll,
            move |_,_x,y| {
                scroll.vadjustment().set_value(scroll.vadjustment().value()+y*scroll.vadjustment().step_increment());
                glib::Propagation::Proceed
            }
        ));
        widgets.newest_list.scroll.add_controller(controller);
        let controller = gtk::EventControllerScroll::new(gtk::EventControllerScrollFlags::VERTICAL);
        controller.set_propagation_phase(gtk::PropagationPhase::Capture);
        controller.connect_scroll(clone!(
            #[strong(rename_to = scroll)]
            widgets.scroll,
            move |_,_x,y| {
                scroll.vadjustment().set_value(scroll.vadjustment().value()+y*scroll.vadjustment().step_increment());
                glib::Propagation::Proceed
            }
        ));
        widgets.highest_list.scroll.add_controller(controller);
        let controller = gtk::EventControllerScroll::new(gtk::EventControllerScrollFlags::VERTICAL);
        controller.set_propagation_phase(gtk::PropagationPhase::Capture);
        controller.connect_scroll(clone!(
            #[strong(rename_to = scroll)]
            widgets.scroll,
            move |_,_x,y| {
                scroll.vadjustment().set_value(scroll.vadjustment().value()+y*scroll.vadjustment().step_increment());
                glib::Propagation::Proceed
            }
        ));
        widgets.explore_list.scroll.add_controller(controller);

        model.album_factory.connect_setup(clone!(
            #[strong(rename_to = cover_cache)]
            model.cover_cache,
            #[strong(rename_to = album_cache)]
            model.album_cache,
            #[strong(rename_to = mpris_player)]
            model.mpris_player,
            move |_, list_item| {
                let vbox = gtk::Box::builder()
                    .orientation(Orientation::Vertical)
                    .spacing(3)
                    .build();
                vbox.add_css_class("album-entry");

                let overlay = gtk::Overlay::new();
                let cover_picture = CoverPicture::new(cover_cache.clone(), CoverSize::Large);
                overlay.set_child(Some(&cover_picture));
                overlay.set_halign(Align::Center);
                overlay.set_valign(Align::Center);
                let overlay_box = gtk::Box::new(Orientation::Horizontal, 5);
                overlay_box.set_halign(Align::End);
                overlay_box.set_valign(Align::End);
                let play_btn = gtk::Button::new();
                play_btn.set_icon_name(icon_names::PLAY);
                play_btn.add_css_class("flat");
                play_btn.set_tooltip("Play");
                let like_btn = gtk::ToggleButton::new();
                like_btn.add_css_class("flat");
                like_btn.set_tooltip("Star");
                like_btn
                    .property_expression("active")
                    .chain_closure::<String>(closure!(
                        move |_: Option<Object>, active: bool| {
                            if active {
                                icon_names::HEART_FILLED
                            } else {
                                icon_names::HEART_OUTLINE_THIN
                            }
                        }
                    ))
                    .bind(&like_btn, "icon-name", Widget::NONE);
                overlay_box.append(&like_btn);
                overlay_box.append(&play_btn);
                overlay.add_overlay(&overlay_box);
                vbox.append(&overlay);

                let name = gtk::Label::builder().css_classes(["bold"]).build();
                let artist = gtk::Label::new(None);
                name.set_width_chars(25);
                name.set_ellipsize(EllipsizeMode::End);
                artist.set_width_chars(25);
                artist.set_use_markup(true);
                artist.set_ellipsize(EllipsizeMode::End);
                vbox.append(&name);
                vbox.append(&artist);

                artist.connect_activate_link(move |this, url| {
                    this.activate_action("win.artist", Some(&url.to_variant())).expect("Error executing action");
                    glib::Propagation::Stop
                });

                let list_item = list_item
                    .downcast_ref::<ListItem>()
                    .expect("Needs to be ListItem");
                list_item.set_child(Some(&vbox));
                
                let ctrl = gtk::GestureClick::builder()
                    .button(3)
                    .build();
                ctrl.connect_pressed(clone!(
                    #[weak]
                    list_item,
                    #[weak]
                    vbox,
                    move |_controller, _btn, x, y| {
                        let item = list_item.item().expect("Expected ListItem to have item");
                        let id: String = item.property("id");
                        let menu = info_dialog::make_popup_menu(ItemType::Album, id);
                        menu.set_parent(&vbox);
                        menu.set_pointing_to(Some(&gdk::Rectangle::new(x as i32, y as i32, 1, 1)));
                        menu.popup();
                    }
                ));
                vbox.add_controller(ctrl);

                play_btn.connect_clicked(clone!(
                    #[weak]
                    list_item,
                    #[strong]
                    mpris_player,
                    move |_this| {
                        if let Some(item) = list_item.item() {
                            let album = item.downcast::<AlbumObject>().expect("Item should be AlbumObject");
                            let mpris_player = mpris_player.clone();
                            relm4::spawn_local(async move {
                                mpris_player.imp().send_res(mpris_player.imp().queue_album(album.id(), None, true).await);
                            });
                        }
                    }
                ));
                like_btn.connect_clicked(clone!(
                    #[weak]
                    list_item,
                    #[strong]
                    album_cache,
                    #[strong]
                    mpris_player,
                    move |_this| {
                        if let Some(item) = list_item.item() {
                            let album = item.downcast::<AlbumObject>().expect("Item should be AlbumObject");
                            let album_cache = album_cache.clone();
                            let mpris_player = mpris_player.clone();
                            relm4::spawn_local(async move {
                                mpris_player.imp().send_res(album_cache.toggle_starred(&album).await);
                            });
                        }
                    }
                ));

                list_item
                    .property_expression("item")
                    .chain_property::<AlbumObject>("name")
                    .bind(&name, "label", Widget::NONE);
                list_item
                    .property_expression("item")
                    .chain_property::<AlbumObject>("artist")
                    .bind(&artist, "label", Widget::NONE);
                list_item
                    .property_expression("item")
                    .chain_property::<AlbumObject>("cover-art-id")
                    .bind(&cover_picture, "cover-id", Widget::NONE);
                list_item
                    .property_expression("item")
                    .chain_property::<AlbumObject>("starred")
                    .bind(&like_btn, "active", Widget::NONE);
            }
        ));

        let random = model
            .album_cache
            .get_album_list(AlbumListType::Random, None, None, None, None, None, None)
            .await;
        match random {
            Ok(random) => {
                model.randoms_ids = random.iter().map(|a| a.cover_art_id().clone()).collect();
                sender.input(BrowsePageMsg::ScrolledCarousel(0));
                let breakpoint = init.9;
                for album in random {
                    let cbox = gtk::Box::new(Orientation::Horizontal, 10);
                    cbox.add_css_class("card");
                    cbox.add_css_class("padded");
                    cbox.set_halign(Align::Fill);
                    cbox.set_hexpand(true);
                    cbox.set_hexpand_set(true);
                    let ctrl = gtk::GestureClick::builder()
                        .button(3)
                        .build();
                    ctrl.connect_pressed(clone!(
                        #[weak]
                        cbox,
                        #[strong(rename_to=id)]
                        album.id(),
                        move |_controller, _btn, x, y| {
                            let menu = info_dialog::make_popup_menu(ItemType::Album, id.clone());
                            menu.set_parent(&cbox);
                            menu.set_pointing_to(Some(&gdk::Rectangle::new(x as i32, y as i32, 1, 1)));
                            menu.popup();
                        }
                    ));
                    cbox.add_controller(ctrl);
                    let cover_picture = CoverPicture::new(model.cover_cache.clone(), CoverSize::Large);
                    cover_picture.set_cover_id(album.cover_art_id());
                    cover_picture.add_css_class("shadowed");
                    cover_picture.set_halign(Align::Start);
                    cover_picture.set_valign(Align::Start);
                    cbox.append(&cover_picture);
                    let text_vbox = gtk::Box::new(Orientation::Vertical, 10);
                    text_vbox.set_valign(Align::Center);
                    let title = gtk::Label::new(Some(album.name().as_str()));
                    title.add_css_class("t0");
                    title.add_css_class("bold");
                    title.set_halign(Align::Start);
                    title.set_justify(Justification::Left);
                    title.set_max_width_chars(30);
                    title.set_ellipsize(EllipsizeMode::End);
                    let artists = gtk::Label::builder()
                        .use_markup(true)
                        .label(album.artist())
                        .build();
                    artists.set_halign(Align::Start);
                    artists.set_justify(Justification::Left);
                    artists.add_css_class("t1");
                    artists.set_ellipsize(EllipsizeMode::End);
                    artists.connect_activate_link(move |this, url| {
                        this.activate_action("win.artist", Some(&url.to_variant())).expect("Error executing action");
                        glib::Propagation::Stop
                    });
                    text_vbox.append(&title);
                    text_vbox.append(&artists);
                    cbox.append(&text_vbox);

                    let gesture = gtk::GestureClick::new();
                    gesture.connect_released(clone!(
                        #[weak]
                        cbox,
                        #[strong(rename_to = id)]
                        album.id(),
                        move |_, _, _, _| {
                            cbox.activate_action("win.album", Some(&id.to_variant())).expect("Error executing action");
                        }
                    ));
                    cbox.add_controller(gesture);

                    let vbox = gtk::CenterBox::builder().orientation(Orientation::Vertical).build();
                    vbox.set_valign(Align::Fill);
                    vbox.set_halign(Align::End);
                    vbox.set_hexpand(true);
                    vbox.set_hexpand_set(true);
                    let song_count_str = format!("Song count: {}", album.song_count());
                    let song_count = gtk::Label::new(Some(song_count_str.as_str()));
                    song_count.add_css_class("t2");
                    song_count.set_justify(Justification::Right);
                    song_count.set_valign(Align::End);
                    song_count.set_halign(Align::End);
                    let play_btn = gtk::Button::builder()
                        .icon_name(icon_names::PLAY)
                        .tooltip_text("Play")
                        .build();
                    play_btn.set_valign(Align::Center);
                    play_btn.set_halign(Align::End);
                    play_btn.set_size_request(64, 64);
                    play_btn.add_css_class("circular");
                    play_btn.add_css_class("raised");
                    play_btn.add_css_class("bigicon");
                    play_btn.connect_clicked(clone!(
                        #[strong(rename_to = mpris_player)]
                        model.mpris_player,
                        #[strong(rename_to = id)]
                        album.id(),
                        move |_| {
                            relm4::spawn_local(clone!(
                                #[strong]
                                mpris_player,
                                #[strong]
                                id,
                                async move {
                                    mpris_player.imp().send_res(mpris_player.imp().queue_album(id, None, true).await);
                                }
                            ));
                        }
                    ));

                    let btns_hbox = gtk::Box::new(Orientation::Horizontal, 10);
                    let next_btn = gtk::Button::from_icon_name(icon_names::RIGHT);
                    let prev_btn = gtk::Button::from_icon_name(icon_names::LEFT);
                    next_btn.add_css_class("pill");
                    next_btn.set_tooltip("Next item");
                    prev_btn.add_css_class("pill");
                    prev_btn.set_tooltip("Previous item");
                    next_btn.connect_clicked(clone!(
                        #[strong]
                        sender,
                        move |_| {
                            sender.input(BrowsePageMsg::ScrollCarousel(1));
                        }
                    ));
                    prev_btn.connect_clicked(clone!(
                        #[strong]
                        sender,
                        move |_| {
                            sender.input(BrowsePageMsg::ScrollCarousel(-1));
                        }
                    ));
                    btns_hbox.append(&prev_btn);
                    btns_hbox.append(&next_btn);

                    vbox.set_center_widget(Some(&play_btn));
                    vbox.set_start_widget(Some(&song_count));
                    vbox.set_end_widget(Some(&btns_hbox));
                    cbox.append(&vbox);

                    widgets.carousel.append(&cbox);

                    breakpoint.add_setter(&cbox, "orientation", Some(&Orientation::Vertical.to_value()));
                    breakpoint.add_setter(&cbox, "halign", Some(&Align::Center.to_value()));
                    breakpoint.add_setter(&cbox, "css-classes", Some(&["vertical", "card", "paddedx"].to_value()));
                    breakpoint.add_setter(&vbox, "halign", Some(&Align::Start.to_value()));
                    breakpoint.add_setter(&btns_hbox, "halign", Some(&Align::Start.to_value()));
                    breakpoint.add_setter(&song_count, "visible", Some(&false.to_value()));
                    breakpoint.add_setter(&play_btn, "visible", Some(&false.to_value()));
                }
            }
            Err(err) => model.mpris_player.imp().send_error(err),
        }

        let newest = model
            .album_cache
            .get_album_list(AlbumListType::Newest, None, None, None, None, None, None)
            .await;
        match newest {
            Ok(newest) => {
                let newest_store = ListStore::from_iter(newest);
                widgets
                    .newest_list
                    .list
                    .set_model(Some(&gtk::NoSelection::new(Some(newest_store))));
            },
            Err(err) => model.mpris_player.imp().send_error(err),
        };

        let highest = model
            .album_cache
            .get_album_list(AlbumListType::Frequent, None, None, None, None, None, None)
            .await;
        match highest {
            Ok(highest) => {
                let highest_store = ListStore::from_iter(highest);
                widgets
                    .highest_list
                    .list
                    .set_model(Some(&gtk::NoSelection::new(Some(highest_store))));
            },
            Err(err) => model.mpris_player.imp().send_error(err),
        }

        let explore = model
            .album_cache
            .get_album_list(AlbumListType::Random, None, None, None, None, None, None)
            .await;
        match explore {
            Ok(explore) => {
                let explore_store = ListStore::from_iter(explore);
                widgets
                    .explore_list
                    .list
                    .set_model(Some(&gtk::NoSelection::new(Some(explore_store))));
            },
            Err(err) => model.mpris_player.imp().send_error(err),
        }

        AsyncComponentParts { model, widgets }
    }

    async fn update_with_view(
        &mut self,
        widgets: &mut Self::Widgets,
        message: Self::Input,
        sender: AsyncComponentSender<Self>,
        _root: &Self::Root,
    ) {
        match message {
            BrowsePageMsg::ScrollCarousel(s) => {
                self.carousel_pos = min(max(self.carousel_pos as i32 + s, 0) as u32, widgets.carousel.n_pages() - 1);
                widgets.carousel.scroll_to(&widgets.carousel.nth_page(self.carousel_pos), true);
            }
            BrowsePageMsg::ScrollNewest(s) => {
                widgets
                    .newest_list
                    .scroll
                    .hadjustment()
                    .set_value(widgets.newest_list.scroll.hadjustment().value() + s as f64);
            },
            BrowsePageMsg::ScrollHighest(s) => {
                widgets
                    .highest_list
                    .scroll
                    .hadjustment()
                    .set_value(widgets.highest_list.scroll.hadjustment().value() + s as f64);
            },
            BrowsePageMsg::ScrollExplore(s) => {
                widgets
                    .explore_list
                    .scroll
                    .hadjustment()
                    .set_value(widgets.explore_list.scroll.hadjustment().value() + s as f64);
            },
            BrowsePageMsg::ScrolledCarousel(i) => {
                self.carousel_pos = i;
                if let Some(id) = self.randoms_ids.get(i as usize) && let Some(id) = id {
                    let colors = self.cover_cache.get_palette(id).await;
                    if let Err(err) = colors {
                        self.mpris_player.imp().send_error(err);
                    } else {
                        sender.output(BrowsePageOut::SetColors(colors.ok().and_then(|c| c))).expect("Error sending out colors");
                    }
                }
            },
        };
        self.update_view(widgets, sender);
    }
}
