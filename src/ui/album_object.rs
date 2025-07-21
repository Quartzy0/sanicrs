use relm4::adw::glib;
use relm4::adw::glib::Object;
use relm4::adw::prelude::*;
use relm4::adw::subclass::prelude::*;
use crate::opensonic::types::Album;

glib::wrapper! {
    pub struct AlbumObject(ObjectSubclass<imp::AlbumObject>);
}

impl AlbumObject {
    pub fn new(album: Album) -> Self {
        let obj= Object::builder::<AlbumObject>()
            .build();
        obj.set_album(album);
        obj
    }

    pub fn set_album(&self, song: Album) {
        self.imp().album.replace(Some(song));
    }

    pub fn cover_art_id(&self) -> Option<String> {
        self.property("cover-art-id")
    }

    pub fn id(&self) -> Option<String> {
        self.property("id")
    }
}

mod imp {
    use relm4::adw::glib::{ParamSpec, ParamSpecString, Value};
    use relm4::adw::gtk::glib;
    use relm4::adw::gtk::prelude::*;
    use relm4::adw::gtk::subclass::prelude::*;
    use relm4::once_cell::sync::Lazy;
    use std::cell::{RefCell};
    use std::ops::Deref;
    use crate::opensonic::types::Album;

    // Object holding the state
    #[derive(Default)]
    pub struct AlbumObject {
        pub album: RefCell<Option<Album>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for AlbumObject {
        const NAME: &'static str = "SanicAlbumObject";
        type Type = super::AlbumObject;
    }

    // Trait shared by all GObjects
    impl ObjectImpl for AlbumObject {
        fn properties() -> &'static [ParamSpec] {
            static PROPERTIES: Lazy<Vec<ParamSpec>> = Lazy::new(|| {
                vec![
                    ParamSpecString::builder("id").build(),
                    ParamSpecString::builder("name").build(),
                    ParamSpecString::builder("artist").build(),
                    ParamSpecString::builder("cover-art-id").build(),
                ]
            });
            PROPERTIES.as_ref()
        }


        fn set_property(&self, _id: usize, _value: &Value, pspec: &ParamSpec) {
            match pspec.name() {
                p => unimplemented!("{}", p),
            };
        }

        fn property(&self, _id: usize, pspec: &ParamSpec) -> Value {
            let album = &self.album.borrow();
            if let Some(album) = album.deref() {
                match pspec.name() {
                    "id" => album.id.to_value(),
                    "name" => album.name.to_value(),
                    "artist" => album.artists().to_value(),
                    "cover-art-id" => album.cover_art.to_value(),
                    _ => unimplemented!(),
                }
            } else {
                match pspec.name() {
                    "id" => None::<String>.to_value(),
                    "name" => None::<String>.to_value(),
                    "artist" => None::<String>.to_value(),
                    "cover-art-id" => None::<String>.to_value(),
                    _ => unimplemented!(),
                }
            }
        }
    }
}