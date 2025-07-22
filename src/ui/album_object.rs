use std::sync::Arc;
use relm4::adw::glib;
use relm4::adw::glib::Object;
use relm4::adw::prelude::*;
use relm4::adw::subclass::prelude::*;
use crate::opensonic::types::{Album, Song};

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

    pub fn set_songs(&self, songs: Vec<Arc<Song>>) {
        self.imp().songs.replace(Some(songs));
    }

    pub fn get_songs(&self) -> Option<Vec<Arc<Song>>> {
        (*self.imp().songs.borrow()).as_ref().cloned()
    }

    pub fn has_songs(&self) -> bool {
        self.imp().songs.borrow().is_some()
    }

    pub fn id(&self) -> Option<String> {
        self.property("id")
    }
}

mod imp {
    use relm4::adw::glib::{ParamSpec, ParamSpecString, ParamSpecValueArray, Value};
    use relm4::adw::gtk::glib;
    use relm4::adw::gtk::prelude::*;
    use relm4::adw::gtk::subclass::prelude::*;
    use relm4::once_cell::sync::Lazy;
    use std::cell::{RefCell};
    use std::ops::Deref;
    use std::sync::Arc;
    use crate::opensonic::types::{Album, Song};

    // Object holding the state
    #[derive(Default)]
    pub struct AlbumObject {
        pub album: RefCell<Option<Album>>,
        pub songs: RefCell<Option<Vec<Arc<Song>>>>
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