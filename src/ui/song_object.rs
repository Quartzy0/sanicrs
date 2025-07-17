use std::time::Duration;
use relm4::adw::glib;
use relm4::adw::glib::Object;
use crate::opensonic::types::Song;
use crate::ui::current_song::SongInfo;

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
    pub fn new(id: String, title: String, artist: String, album: String, cover_art_id: Option<String>, duration: f64, position_state: PositionState) -> Self {
        Object::builder()
            .property("id", id)
            .property("title", title)
            .property("artist", artist)
            .property("album", album)
            .property("cover_art_id", cover_art_id)
            .property("duration", duration)
            .property("position_state", position_state)
            .build()
    }
}

impl From<SongInfo> for SongObject {
    fn from(value: SongInfo) -> Self {
        SongObject::new(value.id, value.title, value.artist, value.album, value.cover_art_id, value.duration.as_secs_f64(), PositionState::default())
    }
}

impl From<&Song> for SongObject {
    fn from(value: &Song) -> Self {
        SongObject::from(SongInfo::from(value))
    }
}

impl Into<SongInfo> for SongObject {
    fn into(self) -> SongInfo {
        SongInfo {
            id: self.id(),
            title: self.title(),
            artist: self.artist(),
            album: self.album(),
            cover_art_id: self.cover_art_id(),
            duration: Duration::from_secs_f64(self.duration())
        }
    }
}

mod imp {
    use std::cell::{Cell, RefCell};
    use relm4::adw::glib::{Properties};
    use relm4::adw::gtk::glib;
    use relm4::adw::gtk::prelude::*;
    use relm4::adw::gtk::subclass::prelude::*;
    use crate::ui::song_object::PositionState;

    // Object holding the state
    #[derive(Properties, Default)]
    #[properties(wrapper_type = super::SongObject)]
    pub struct SongObject {
        #[property(get, set)]
        id: RefCell<String>,
        #[property(get, set)]
        title: RefCell<String>,
        #[property(get, set)]
        artist: RefCell<String>,
        #[property(get, set)]
        album: RefCell<String>,
        #[property(get, set)]
        cover_art_id: RefCell<Option<String>>,
        #[property(get, set)]
        duration: Cell<f64>, // Duration in seconds
        #[property(get, set, builder(PositionState::default()))]
        position_state: Cell<PositionState>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for SongObject {
        const NAME: &'static str = "SanicSongObject";
        type Type = super::SongObject;
    }

    // Trait shared by all GObjects
    #[glib::derived_properties]
    impl ObjectImpl for SongObject {}
}