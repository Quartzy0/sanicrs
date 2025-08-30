use relm4::adw::glib;
use relm4::adw::glib::Object;
use relm4::adw::prelude::*;
use relm4::adw::subclass::prelude::*;
use crate::opensonic::types::{Album, Artist};

glib::wrapper! {
    pub struct ArtistObject(ObjectSubclass<imp::ArtistObject>);
}

impl ArtistObject {
    pub fn new(artist: Artist) -> Self {
        let obj= Object::builder::<ArtistObject>()
            .build();
        obj.set_artist(artist);
        obj
    }

    pub fn set_artist(&self, artist: Artist) {
        self.imp().artist.replace(Some(artist));
    }

    pub fn cover_art_id(&self) -> Option<String> {
        self.property("cover-art-id")
    }

    pub fn get_albums(&self) -> Option<Vec<Album>> {
        let x = self.imp().artist.borrow();
        let option = x.as_ref();
        option.and_then(|a| a.albums.clone())
    }

    pub fn has_albums(&self) -> bool {
        self.imp().artist.borrow().as_ref().and_then(|a| Some(a.albums.is_some())).unwrap_or(false)
    }

    pub fn name(&self) -> String {
        self.property("name")
    }

    pub fn album_count(&self) -> Option<String> {
        self.property("album-count")
    }

    pub fn id(&self) -> String {
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
    use crate::opensonic::types::Artist;

    // Object holding the state
    #[derive(Default)]
    pub struct ArtistObject {
        pub artist: RefCell<Option<Artist>>
    }

    #[glib::object_subclass]
    impl ObjectSubclass for ArtistObject {
        const NAME: &'static str = "SanicArtistObject";
        type Type = super::ArtistObject;
    }

    // Trait shared by all GObjects
    impl ObjectImpl for ArtistObject {
        fn properties() -> &'static [ParamSpec] {
            static PROPERTIES: Lazy<Vec<ParamSpec>> = Lazy::new(|| {
                vec![
                    ParamSpecString::builder("id").build(),
                    ParamSpecString::builder("name").build(),
                    ParamSpecString::builder("cover-art-id").build(),
                    ParamSpecString::builder("album-count").build(),
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
            let artist = &self.artist.borrow();
            if let Some(artist) = artist.deref() {
                match pspec.name() {
                    "id" => artist.id.to_value(),
                    "name" => artist.name.to_value(),
                    "cover-art-id" => artist.cover_art.to_value(),
                    "album-count" => artist.album_count.and_then(|c| Some(c.to_string())).to_value(),
                    _ => unimplemented!(),
                }
            } else {
                match pspec.name() {
                    "id" => None::<String>.to_value(),
                    "name" => None::<String>.to_value(),
                    "cover-art-id" => None::<String>.to_value(),
                    "album-count" => None::<String>.to_value(),
                    _ => unimplemented!(),
                }
            }
        }
    }
}
