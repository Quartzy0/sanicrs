use std::rc::Rc;

use crate::opensonic::types::Song;
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

    pub fn duration(&self) -> String {
        self.property("duration")
    }

    pub fn id(&self) -> String {
        self.property("id")
    }

    pub fn get_entry(&self) -> Option<Rc<Song>> {
        self.imp().song.borrow().as_ref().and_then(|s| Some(s.1.clone()))
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
                    ParamSpecString::builder("name").build(),
                    ParamSpecString::builder("artist").build(),
                    ParamSpecString::builder("album").build(),
                    ParamSpecString::builder("cover-art-id").build(),
                    ParamSpecString::builder("duration").build(),
                    ParamSpecEnum::builder::<PositionState>("position-state").build(),
                    ParamSpecString::builder("filetype").build(),
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
                    "name" => song.1.title.to_value(),
                    "artist" => song.1.artists().to_value(),
                    "album" => song.1.album.to_value(),
                    "cover-art-id" => song.1.cover_art.to_value(),
                    "position-state" => self.position_state.get().to_value(),
                    "duration" => {
                        if let Some(duration) = song.1.duration {
                            let mut secs = duration.as_secs();
                            let mut mins = secs / 60;
                            let hrs = mins / 60;
                            mins = mins % 60;
                            secs = secs % 60;
                            let mut str = String::new();
                            if hrs != 0 {
                                str.push_str(&hrs.to_string());
                                str.push_str("h ");
                                str.push_str(&mins.to_string());
                                str.push_str("m ");
                            } else if mins != 0 {
                                str.push_str(&mins.to_string());
                                str.push_str("m ");
                            }
                            str.push_str(&secs.to_string());
                            str.push_str("s");

                            str.to_value()
                        } else {
                            None::<String>.to_value()
                        }
                    },
                    "filetype" => self.song.borrow().as_ref().and_then(|s| {
                        if let Some(suf) = &s.1.suffix && let Some(bitrate) = s.1.bit_rate {
                            Some(format!("{}/{}", suf, bitrate).to_uppercase())
                        } else {
                            None
                        }
                    }).to_value(),
                    _ => unimplemented!(),
                }
            } else {
                match pspec.name() {
                    "id" => None::<String>.to_value(),
                    "title" => None::<String>.to_value(),
                    "name" => None::<String>.to_value(),
                    "artist" => None::<String>.to_value(),
                    "album" => None::<String>.to_value(),
                    "cover-art-id" => None::<String>.to_value(),
                    "position-state" => self.position_state.get().to_value(),
                    "filetype" => None::<String>.to_value(),
                    _ => unimplemented!(),
                }
            }
        }
    }
}
