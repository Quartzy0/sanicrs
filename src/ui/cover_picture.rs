// Code taken from: https://gitlab.gnome.org/World/amberol/-/blob/main/src/cover_picture.rs
// Modified to have rounded corners rendered directly + Huge size added

// SPDX-FileCopyrightText: 2022  Emmanuele Bassi
// SPDX-License-Identifier: GPL-3.0-or-later

use crate::opensonic::client::OpenSubsonicClient;
use relm4::adw::glib::clone;
use relm4::adw::gtk;
use relm4::adw::gtk::{gdk, gio, glib, graphene, gsk, prelude::*, subclass::prelude::*};
use std::cell::{Cell, RefCell};
use std::sync::Arc;

#[derive(Clone, Copy, Debug, glib::Enum, PartialEq, Default)]
#[enum_type(name = "SanicCoverSize")]
pub enum CoverSize {
    #[default]
    Huge = 0,
    Large = 1,
    Small = 2,
}

impl AsRef<str> for CoverSize {
    fn as_ref(&self) -> &str {
        match self {
            CoverSize::Huge => "huge",
            CoverSize::Large => "large",
            CoverSize::Small => "small",
        }
    }
}

mod imp {
    use super::*;
    use glib::{ParamSpec, ParamSpecEnum, ParamSpecObject, Value};
    use relm4::adw::gtk;
    use relm4::adw::gtk::graphene::Size;
    use relm4::once_cell::sync::Lazy;

    const HUGE_SIZE: i32 = 512;
    const LARGE_SIZE: i32 = 192;
    const SMALL_SIZE: i32 = 48;

    #[derive(Debug, Default)]
    pub struct CoverPicture {
        pub cover: RefCell<Option<gdk::Texture>>,
        pub cover_size: Cell<CoverSize>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for CoverPicture {
        const NAME: &'static str = "AmberolCoverPicture";
        type Type = super::CoverPicture;
        type ParentType = gtk::Widget;

        fn class_init(klass: &mut Self::Class) {
            klass.set_css_name("picture");
            klass.set_accessible_role(gtk::AccessibleRole::Img);
        }
    }

    impl ObjectImpl for CoverPicture {
        fn constructed(&self) {
            self.parent_constructed();

            self.obj().add_css_class("cover");
            self.obj().set_overflow(gtk::Overflow::Hidden);

            self.obj().connect_notify_local(
                Some("scale-factor"),
                clone!(
                    #[weak(rename_to = _obj)]
                    self,
                    move |picture, _| {
                        picture.queue_draw();
                    }
                ),
            );

            self.obj()
                .upcast_ref::<gtk::Accessible>()
                .update_property(&[gtk::accessible::Property::Label("Cover image")]);
        }

        fn properties() -> &'static [ParamSpec] {
            static PROPERTIES: Lazy<Vec<ParamSpec>> = Lazy::new(|| {
                vec![
                    ParamSpecObject::builder::<gdk::Texture>("cover").build(),
                    ParamSpecEnum::builder::<CoverSize>("cover-size").build(),
                ]
            });
            PROPERTIES.as_ref()
        }

        fn property(&self, _id: usize, pspec: &ParamSpec) -> Value {
            match pspec.name() {
                "cover" => self.cover.borrow().to_value(),
                "cover-size" => self.cover_size.get().to_value(),
                _ => unimplemented!(),
            }
        }

        fn set_property(&self, _id: usize, value: &Value, pspec: &ParamSpec) {
            match pspec.name() {
                "cover" => self
                    .obj()
                    .set_cover(value.get::<gdk::Texture>().ok().as_ref()),
                "cover-size" => self
                    .obj()
                    .set_cover_size(value.get::<CoverSize>().expect("Required CoverSize")),
                _ => unimplemented!(),
            };
        }
    }

    impl WidgetImpl for CoverPicture {
        fn request_mode(&self) -> gtk::SizeRequestMode {
            gtk::SizeRequestMode::ConstantSize
        }

        fn measure(&self, _orientation: gtk::Orientation, _for_size: i32) -> (i32, i32, i32, i32) {
            match self.cover_size.get() {
                CoverSize::Huge => (HUGE_SIZE, HUGE_SIZE, -1, -1),
                CoverSize::Large => (LARGE_SIZE, LARGE_SIZE, -1, -1),
                CoverSize::Small => (SMALL_SIZE, SMALL_SIZE, -1, -1),
            }
        }

        fn snapshot(&self, snapshot: &gtk::Snapshot) {
            if let Some(ref cover) = *self.cover.borrow() {
                let widget = self.obj();
                let scale_factor = widget.scale_factor() as f64;
                let width = widget.width() as f64 * scale_factor;
                let height = widget.height() as f64 * scale_factor;
                let ratio = cover.intrinsic_aspect_ratio();
                let w;
                let h;
                if ratio > 1.0 {
                    w = width;
                    h = width / ratio;
                } else {
                    w = height * ratio;
                    h = height;
                }

                let x = (width - w.ceil()) / 2.0;
                let y = (height - h).floor() / 2.0;
                let bounds = graphene::Rect::new(0.0, 0.0, w as f32, h as f32);

                let border_radius: f32 = match self.cover_size.get() {
                    CoverSize::Huge => 10.0,
                    CoverSize::Large => 5.0,
                    CoverSize::Small => 3.0,
                };


                snapshot.save();
                snapshot.scale(1.0 / scale_factor as f32, 1.0 / scale_factor as f32);
                snapshot.translate(&graphene::Point::new(x as f32, y as f32));
                snapshot.push_rounded_clip(&gsk::RoundedRect::new(
                    bounds,
                    Size::new(border_radius, border_radius),
                    Size::new(border_radius, border_radius),
                    Size::new(border_radius, border_radius),
                    Size::new(border_radius, border_radius),
                ));
                snapshot.append_scaled_texture(
                    cover,
                    gsk::ScalingFilter::Trilinear,
                    &bounds,
                );
                snapshot.pop();
                snapshot.restore();
            }
        }
    }
}

glib::wrapper! {
    pub struct CoverPicture(ObjectSubclass<imp::CoverPicture>)
        @extends gtk::Widget,
        @implements gio::ActionGroup, gio::ActionMap, gtk::Accessible;
}

impl Default for CoverPicture {
    fn default() -> Self {
        glib::Object::new()
    }
}

impl CoverPicture {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn cover(&self) -> Option<gdk::Texture> {
        (*self.imp().cover.borrow()).as_ref().cloned()
    }

    pub fn set_cover(&self, cover: Option<&gdk::Texture>) {
        if let Some(cover) = cover {
            self.imp().cover.replace(Some(cover.clone()));
        } else {
            self.imp().cover.replace(None);
        }

        self.queue_draw();
        self.notify("cover");
    }
    
    pub async fn set_cover_from_id(&self, cover_id: Option<&String>, client: Arc<OpenSubsonicClient>) {
        let texture = match cover_id {
            None => None,
            Some(cover_id) => {
                let img_resp = client
                    .get_cover_image(cover_id.as_str(), Some("512"))
                    .await
                    .expect("Error getting cover image");
                let bytes = glib::Bytes::from(&img_resp);
                Some(gdk::Texture::from_bytes(&bytes).expect("Error loading textre"))
            }
        };
        self.set_cover(texture.as_ref());
    }

    pub fn set_cover_size(&self, cover_size: CoverSize) {
        self.imp().cover_size.replace(cover_size);
        self.queue_resize();
        self.notify("cover-size");
    }
}
