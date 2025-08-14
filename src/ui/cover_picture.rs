// Code taken from: https://gitlab.gnome.org/World/amberol/-/blob/main/src/cover_picture.rs
// Modified to have rounded corners rendered directly + Huge size added +
// Load cover image directly in this widget + some other changes

// SPDX-FileCopyrightText: 2022  Emmanuele Bassi
// SPDX-License-Identifier: GPL-3.0-or-later

use relm4::adw::glib::clone;
use relm4::adw::gtk;
use relm4::adw::gtk::{gdk, gio, glib, gsk, prelude::*, subclass::prelude::*};
use std::cell::{Cell, RefCell};
use color_thief::{Color, ColorFormat};
use relm4::adw::gdk::{MemoryFormat, TextureDownloader};
use crate::opensonic::cache::CoverCache;

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
    use std::sync::OnceLock;
    use super::*;
    use glib::{ParamSpec, ParamSpecEnum, ParamSpecObject, Value};
    use relm4::adw::glib::{JoinHandle, ParamSpecString};
    use relm4::adw::glib::subclass::Signal;
    use relm4::adw::gtk;
    use relm4::adw::gtk::graphene::Size;
    use relm4::gtk::graphene::{Point, Rect};
    use relm4::once_cell::sync::Lazy;
    use zbus::export::futures_core::FusedFuture;
    use zvariant::NoneValue;
    use crate::opensonic::cache::CoverCache;

    const HUGE_SIZE: i32 = 512;
    const LARGE_SIZE: i32 = 192;
    const SMALL_SIZE: i32 = 64;

    pub struct CoverPicture {
        pub cover: RefCell<Option<gdk::Texture>>,
        pub cover_id: RefCell<Option<String>>,
        pub handle: Cell<Option<JoinHandle<()>>>,
        pub cover_size: Cell<CoverSize>,
        pub cache: RefCell<CoverCache>,
    }

    impl Default for CoverPicture {
        fn default() -> Self {
            Self {
                cover: RefCell::new(None),
                cover_id: RefCell::new(None),
                handle: Cell::new(None),
                cover_size: Cell::new(CoverSize::default()),
                cache: RefCell::null_value(),
            }
        }
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
                    ParamSpecString::builder("cover-id").build(),
                ]
            });
            PROPERTIES.as_ref()
        }

        fn property(&self, _id: usize, pspec: &ParamSpec) -> Value {
            match pspec.name() {
                "cover" => self.cover.borrow().to_value(),
                "cover-size" => self.cover_size.get().to_value(),
                "cover-id" => self.cover_id.borrow().to_value(),
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
                "cover-id" => self.obj().set_cover_id(
                    value
                        .get::<Option<String>>()
                        .expect("Requited Option<String>")
                ),
                _ => unimplemented!(),
            };
        }

        fn signals() -> &'static [Signal] {
            static SIGNALS: OnceLock<Vec<Signal>> = OnceLock::new();
            SIGNALS.get_or_init(|| {
                vec![Signal::builder("cover-loaded").build()]
            })
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
            let widget = self.obj();
            let scale_factor = widget.scale_factor() as f64;
            let width = widget.width() as f64 * scale_factor;
            let height = widget.height() as f64 * scale_factor;
            if let Some(ref cover) = *self.cover.borrow() {
                let ratio = cover.intrinsic_aspect_ratio();
                let w;
                let h;
                if ratio < 1.0 {
                    w = width;
                    h = width / ratio;
                } else {
                    w = height * ratio;
                    h = height;
                }
                let xoffset;
                let yoffset;
                let wclip;
                let hclip;
                if width > height {
                    wclip = h;
                    hclip = h;
                    xoffset = (w-h)/2.0;
                    yoffset = 0.0;
                } else {
                    wclip = w;
                    hclip = w;
                    xoffset = 0.0;
                    yoffset = (h-w)/2.0;
                }

                let x = (width - w.ceil()) / 2.0;
                let y = (height - h).floor() / 2.0;
                let bounds = Rect::new(0.0, 0.0, w as f32, h as f32);

                let border_radius: f32 = match self.cover_size.get() {
                    CoverSize::Huge => 10.0,
                    CoverSize::Large => 5.0,
                    CoverSize::Small => 3.0,
                };

                snapshot.save();
                snapshot.scale(1.0 / scale_factor as f32, 1.0 / scale_factor as f32);
                snapshot.translate(&Point::new(x as f32, y as f32));
                snapshot.push_rounded_clip(&gsk::RoundedRect::new(
                    Rect::new(xoffset as f32, yoffset as f32, wclip as f32, hclip as f32),
                    Size::new(border_radius, border_radius),
                    Size::new(border_radius, border_radius),
                    Size::new(border_radius, border_radius),
                    Size::new(border_radius, border_radius),
                ));
                snapshot.append_scaled_texture(cover, gsk::ScalingFilter::Trilinear, &bounds);
                snapshot.pop();
                snapshot.restore();
            } else {
                snapshot.save();
                snapshot.scale(1.0 / scale_factor as f32, 1.0 / scale_factor as f32);
                let center_x: f32 = width as f32 / 2.0;
                let center_y: f32 = height as f32 / 2.0;
                let length: f32 = match self.cover_size.get() {
                    CoverSize::Huge => 150.0,
                    CoverSize::Large => 50.0,
                    CoverSize::Small => 15.0,
                };
                let thickness: f32 = match self.cover_size.get() {
                    CoverSize::Huge => 20.0,
                    CoverSize::Large => 10.0,
                    CoverSize::Small => 5.0,
                };
                let handle = self.handle.take();
                if handle.is_none() || handle.unwrap().is_terminated() {   // If loading has finished and image is still None,
                    let p1 = gsk::PathBuilder::new();           // draw X to indicate loading failed or there is no
                    p1.move_to(center_x - length, center_y - length);// cover.
                    p1.line_to(center_x + length, center_y + length);
                    let p2 = gsk::PathBuilder::new();
                    p2.move_to(center_x + length, center_y - length);
                    p2.line_to(center_x - length, center_y + length);

                    snapshot.append_stroke(
                        &p1.to_path(),
                        &gsk::Stroke::new(thickness),
                        &gdk::RGBA::new(0.2, 0.2, 0.2, 1.0),
                    );
                    snapshot.append_stroke(
                        &p2.to_path(),
                        &gsk::Stroke::new(thickness),
                        &gdk::RGBA::new(0.2, 0.2, 0.2, 1.0),
                    );
                } else {
                    // Draw three dots (...) to indicate loading
                    let p1 = gsk::PathBuilder::new();
                    p1.add_rounded_rect(&gsk::RoundedRect::new(
                        Rect::new(
                            center_x - thickness / 2.0,
                            center_y - thickness / 2.0,
                            thickness,
                            thickness,
                        ),
                        Size::new(thickness, thickness),
                        Size::new(thickness, thickness),
                        Size::new(thickness, thickness),
                        Size::new(thickness, thickness),
                    ));
                    p1.add_rounded_rect(&gsk::RoundedRect::new(
                        Rect::new(
                            center_x - thickness / 2.0 - length,
                            center_y - thickness / 2.0,
                            thickness,
                            thickness,
                        ),
                        Size::new(thickness, thickness),
                        Size::new(thickness, thickness),
                        Size::new(thickness, thickness),
                        Size::new(thickness, thickness),
                    ));
                    p1.add_rounded_rect(&gsk::RoundedRect::new(
                        Rect::new(
                            center_x - thickness / 2.0 + length,
                            center_y - thickness / 2.0,
                            thickness,
                            thickness,
                        ),
                        Size::new(thickness, thickness),
                        Size::new(thickness, thickness),
                        Size::new(thickness, thickness),
                        Size::new(thickness, thickness),
                    ));

                    snapshot.append_stroke(
                        &p1.to_path(),
                        &gsk::Stroke::new(thickness),
                        &gdk::RGBA::new(0.2, 0.2, 0.2, 1.0),
                    );
                }
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
    pub fn new(cache: CoverCache, cover_size: CoverSize) -> Self {
        let obj = Self::default();
        obj.set_cache(cache);
        obj.set_cover_size(cover_size);
        obj
    }

    pub fn new_uninit() -> Self {
        Self::default()
    }
    
    pub fn set_cache(&self, cache: CoverCache) {
        self.imp().cache.replace(cache);
    }

    pub fn cover(&self) -> Option<gdk::Texture> {
        (*self.imp().cover.borrow()).as_ref().cloned()
    }

    pub fn cover_id(&self) -> Option<String> {
        (*self.imp().cover_id.borrow()).as_ref().cloned()
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

    pub fn set_cover_id(&self, cover_id: Option<String>) {
        let old = self.imp().cover_id.replace(cover_id.clone());
        if old == cover_id {
            return;
        }
        if let Some(handle) = self.imp().handle.take() {
            handle.abort();
        }
        self.set_cover(None);
        if let Some(cover_id) = cover_id {
            self.imp()
                .handle
                .replace(Some(glib::spawn_future_local(clone!(
                    #[strong]
                    cover_id,
                    #[weak(rename_to = cover_widget)]
                    self,
                    #[strong(rename_to = cache)]
                    self.imp().cache.borrow(),
                    async move {
                        match cache.get_cover_texture(cover_id.as_str()).await {
                            Ok(resp) => {
                                cover_widget.set_cover(Some(&resp));
                            }
                            Err(e) => {
                                println!("Error getting cover image: {}", e);
                            }
                        };
                        cover_widget.emit_by_name::<()>("cover-loaded", &[]);
                    }
                ))));
        } else {
            self.emit_by_name::<()>("cover-loaded", &[]);
        }
    }

    pub fn set_cover_size(&self, cover_size: CoverSize) {
        self.imp().cover_size.replace(cover_size);
        self.queue_resize();
        self.notify("cover-size");
    }

    pub fn get_palette(&self) -> Option<Vec<Color>> {
        if let Some(ref cover) = *self.imp().cover.borrow() {
            let mut downloader = TextureDownloader::new(cover);
            downloader.set_format(MemoryFormat::A8r8g8b8);
            let (pixels, _size) = downloader.download_bytes();
            return color_thief::get_palette(&pixels, ColorFormat::Argb, 10, 4).ok();
        }
        None
    }
}
