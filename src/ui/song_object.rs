use crate::player::SongEntry;
use relm4::adw::glib;
use relm4::adw::glib::Object;
use relm4::adw::prelude::*;
use relm4::adw::subclass::prelude::*;

#[derive(Clone, Copy, Debug, glib::Enum, PartialEq, Default)]
#[enum_type(name = "SanicPositionState")]
pub enum PositionState {
    Passed = 0,
    Current = 1,
    #[default]
    Upcoming = 2,
}

impl AsRef<str> for PositionState {
    fn as_ref(&self) -> &str {
        match self {
            PositionState::Passed => "passed",
            PositionState::Current => "current",
            PositionState::Upcoming => "upcoming"
        }
    }
}


glib::wrapper! {
    pub struct SongObject(ObjectSubclass<imp::SongObject>);
}

impl SongObject {
    pub fn new(song: SongEntry, position_state: PositionState) -> Self {
        let obj= Object::builder::<SongObject>()
            .property("position-state", position_state)
            .build();
        obj.set_song(song);
        obj
    }

    pub fn set_song(&self, song: SongEntry) {
        self.imp().song.replace(Some(song));
    }

    pub fn set_position_state(&self, position_state: PositionState) {
        self.set_property("position-state", position_state);
    }

    pub fn cover_art_id(&self) -> Option<String> {
        self.property("cover-art-id")
    }
}

mod imp {
    use crate::player::SongEntry;
    use crate::ui::song_object::PositionState;
    use relm4::adw::glib::{ParamSpec, ParamSpecEnum, ParamSpecString, Value};
    use relm4::adw::gtk::glib;
    use relm4::adw::gtk::prelude::*;
    use relm4::adw::gtk::subclass::prelude::*;
    use relm4::once_cell::sync::Lazy;
    use std::cell::{Cell, RefCell};
    use std::ops::Deref;

    // Object holding the state
    pub struct SongObject {
        pub song: RefCell<Option<SongEntry>>,
        pub position_state: Cell<PositionState>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for SongObject {
        const NAME: &'static str = "SanicSongObject";
        type Type = super::SongObject;

        fn new() -> Self {
            Self {
                song: RefCell::new(None),
                position_state: Default::default()
            }
        }
    }

    // Trait shared by all GObjects
    impl ObjectImpl for SongObject {
        fn properties() -> &'static [ParamSpec] {
            static PROPERTIES: Lazy<Vec<ParamSpec>> = Lazy::new(|| {
                vec![
                    ParamSpecString::builder("id").build(),
                    ParamSpecString::builder("title").build(),
                    ParamSpecString::builder("artist").build(),
                    ParamSpecString::builder("album").build(),
                    ParamSpecString::builder("cover-art-id").build(),
                    ParamSpecEnum::builder::<PositionState>("position-state").build(),
                ]
            });
            PROPERTIES.as_ref()
        }


        fn set_property(&self, _id: usize, value: &Value, pspec: &ParamSpec) {
            match pspec.name() {
                "position-state" => {
                    self.position_state.replace(value.get::<PositionState>().expect("Required PositionState"));
                },
                p => unimplemented!("{}", p),
            };
        }

        fn property(&self, _id: usize, pspec: &ParamSpec) -> Value {
            let song = &self.song.borrow();
            if let Some(song) = song.deref() {
                match pspec.name() {
                    "id" => song.1.id.to_value(),
                    "title" => song.1.title.to_value(),
                    "artist" => song.1.artists().to_value(),
                    "album" => song.1.album.to_value(),
                    "cover-art-id" => song.1.cover_art.to_value(),
                    "position-state" => self.position_state.get().to_value(),
                    _ => unimplemented!(),
                }
            } else {
                match pspec.name() {
                    "id" => None::<String>.to_value(),
                    "title" => None::<String>.to_value(),
                    "artist" => None::<String>.to_value(),
                    "album" => None::<String>.to_value(),
                    "cover-art-id" => None::<String>.to_value(),
                    "position-state" => self.position_state.get().to_value(),
                    _ => unimplemented!(),
                }
            }
        }
    }
}