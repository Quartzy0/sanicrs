use std::fmt::{Debug};
use std::marker::PhantomData;
use std::rc::Rc;
use mpris_server::LocalServer;
use relm4::adw::gio::ListStore;
use relm4::adw::glib::{clone, closure, Object};
use relm4::adw::prelude::*;
use relm4::gtk::{Align, ListItem, Orientation, SignalListItemFactory, Widget};
use relm4::adw::glib as glib;
use relm4::{gtk, AsyncComponentSender};
use relm4::component::{AsyncComponent, AsyncComponentParts};
use crate::dbus::player::MprisPlayer;
use crate::icon_names;
use crate::opensonic::cache::CoverCache;
use crate::ui::cover_picture::{CoverPicture, CoverSize, CoverType};

#[derive(Debug)]
pub struct ItemListWidget<I, F, T>
where
    T: IsA<Object> + ObjectType,
    I: IntoIterator<Item = T>,
    F: Future<Output = I>
{
    phantom_t: PhantomData<T>,
    phantom_i: PhantomData<I>,
    phantom_f: PhantomData<F>,
}

pub struct ItemListInit<I, F, T>
where
    T: IsA<Object> + ObjectType,
    I: IntoIterator<Item = T>,
    F: Future<Output = I>
{
    pub cover_cache: CoverCache,
    pub play_fn: Option<Box<dyn Fn(T, u32, Rc<LocalServer<MprisPlayer>>)>>,
    pub click_fn: Option<Box<dyn Fn(T, u32, Rc<LocalServer<MprisPlayer>>)>>,
    pub load_items: F,
    pub mpris_player: Rc<LocalServer<MprisPlayer>>,
    pub cover_type: CoverType,
    pub highlight: Option<u32>,
}

#[relm4::component(pub async)]
impl<T: IsA<Object> + ObjectType, I: IntoIterator<Item = T> + 'static, F: 'static + Future<Output = I>> AsyncComponent for ItemListWidget<I, F, T> {

    type CommandOutput = ();
    type Input = ();
    type Output = ();
    type Init = ItemListInit<I, F, T>;

    view! {
        gtk::ListView {
            set_factory: Some(&factory),
            add_css_class: "card",
            add_css_class: "no-bg",
            set_vexpand: true,
            set_vexpand_set: true,
        }
    }

    async fn init(
        init: Self::Init,
        root: Self::Root,
        _sender: AsyncComponentSender<Self>,
    ) -> AsyncComponentParts<Self> {
        let factory = SignalListItemFactory::new();
        let Self::Init{
            cover_cache,
            play_fn,
            click_fn,
            load_items,
            mpris_player,
            cover_type,
            highlight,
        } = init;
        let play_fn = play_fn.and_then(|f| Some(Rc::new(f)));
        let click_fn = click_fn.and_then(|f| Some(Rc::new(f)));

        let widgets: Self::Widgets = view_output!();

        let mut iter = load_items.await.into_iter().peekable();
        let first = iter.peek();
        let has_duration = first.and_then(|f| Some(f.has_property("duration", Some(String::static_type())))).unwrap_or(false);
        let has_filetype = first.and_then(|f| Some(f.has_property("filetype", Some(String::static_type())))).unwrap_or(false);

        factory.connect_setup(clone!(
            #[strong(rename_to = cover_cache)]
            cover_cache,
            #[strong]
            click_fn,
            #[strong]
            play_fn,
            #[strong]
            mpris_player,
            move |_, list_item| {
                let hbox = gtk::CenterBox::builder()
                    .orientation(Orientation::Horizontal)
                    .build();
                hbox.add_css_class("card");
                hbox.add_css_class("padded");

                let start_hbox = gtk::Box::builder()
                    .orientation(Orientation::Horizontal)
                    .spacing(5)
                    .build();
                let end_hbox = gtk::Box::builder()
                    .orientation(Orientation::Horizontal)
                    .spacing(5)
                    .build();

                let picture = CoverPicture::new(cover_cache.clone(), CoverSize::Small);
                picture.set_cover_type(cover_type);
                let title = gtk::Label::new(None);
                hbox.set_start_widget(Some(&start_hbox));
                hbox.set_end_widget(Some(&end_hbox));

                let list_item = list_item
                    .downcast_ref::<ListItem>()
                    .expect("Needs to be ListItem");
                list_item.set_child(Some(&hbox));

                if let Some(play_fn) = &play_fn {
                    let play_btn = gtk::Button::builder()
                        .icon_name(icon_names::PLAY)
                        .valign(Align::Center)
                        .halign(Align::Center)
                        .build();
                    start_hbox.append(&play_btn);
                    play_btn.connect_clicked(clone!(
                        #[weak]
                        list_item,
                        #[strong]
                        play_fn,
                        #[strong]
                        mpris_player,
                        move |_| {
                            let item = list_item.item().expect("Expected ListItem to have item");
                            play_fn(item.downcast::<T>().expect("Unexpected type"), list_item.position(), mpris_player.clone());
                        }
                    ));
                }
                start_hbox.append(&picture);
                start_hbox.append(&title);

                if let Some(click_fn) = &click_fn {
                    let gesture = gtk::GestureClick::new();
                    gesture.connect_released(clone!(
                        #[weak]
                        list_item,
                        #[strong]
                        click_fn,
                        #[strong]
                        mpris_player,
                        move |_this, _n: i32, _x: f64, _y: f64| {
                            let item = list_item.item().expect("Expected ListItem to have item");
                            click_fn(item.downcast::<T>().expect("Unexpected type"), list_item.position(), mpris_player.clone());
                        }
                    ));
                    hbox.add_controller(gesture);
                }

                if has_duration {
                    let duration = gtk::Label::new(None);
                    end_hbox.append(&duration);
                    list_item
                        .property_expression("item")
                        .chain_property::<T>("duration")
                        .bind(&duration, "label", Widget::NONE);
                }
                if has_filetype {
                    let center_hbox = gtk::Box::new(Orientation::Horizontal, 10);
                    let filetype = gtk::Label::new(None);
                    filetype.set_halign(Align::Center);
                    filetype.set_valign(Align::Center);
                    filetype.add_css_class("frame");
                    filetype.add_css_class("numeric");
                    filetype.add_css_class("caption");
                    filetype.add_css_class("rounded");
                    filetype.add_css_class("paddeds");
                    center_hbox.append(&filetype);
                    hbox.set_center_widget(Some(&center_hbox));

                    list_item
                        .property_expression("item")
                        .chain_property::<T>("filetype")
                        .bind(&filetype, "label", Widget::NONE);
                }

                if let Some(highlight) = highlight {
                    list_item
                        .property_expression("position")
                        .chain_closure::<Vec<String>>(closure!(
                            move |_: Option<Object>, pos: u32| {
                                if pos == highlight {
                                    vec!["card".to_string(), "padded".to_string(), "highlighted".to_string()]
                                } else {
                                    vec!["card".to_string(), "padded".to_string()]
                                }
                            }
                        ))
                        .bind(&hbox, "css-classes", Widget::NONE);
                }
                list_item
                    .property_expression("item")
                    .chain_property::<T>("name")
                    .bind(&title, "label", Widget::NONE);
                list_item
                    .property_expression("item")
                    .chain_property::<T>("cover-art-id")
                    .bind(&picture, "cover-id", Widget::NONE);
            }
        ));

        let list_store = ListStore::from_iter(iter);
        root.set_model(Some(&gtk::NoSelection::new(Some(list_store))));

        let model = Self { phantom_t: Default::default(), phantom_i: Default::default(), phantom_f: Default::default() };
        AsyncComponentParts { model, widgets }
    }
}
