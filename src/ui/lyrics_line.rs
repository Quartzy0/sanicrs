
use relm4::{adw::subclass::prelude::{ObjectSubclassIsExt}, gtk::glib::{self, Object}};

use crate::opensonic::types::{self, LyricsList};



glib::wrapper! {
    pub struct LyricsLine(ObjectSubclass<imp::LyricsLine>);
}

impl From<types::LyricsLine> for LyricsLine {
    fn from(value: types::LyricsLine) -> Self {
        let obj = Object::builder::<Self>().build();
        obj.imp().lyrics_line.replace(value);
        obj
    }
}

impl LyricsLine {
    pub fn new(start: u32, value: String) -> Self {
        let obj = Object::builder::<Self>().build();
        obj.imp().lyrics_line.replace(types::LyricsLine {
            start,
            value
        });
        obj
    }
}

pub fn from_list(lines: &LyricsList) -> Vec<LyricsLine> {
    let offset = lines.offset.unwrap_or(0);

    match &lines.lines {
        types::LyricsLines::Synced(lyrics_lines) => {
            lyrics_lines.iter().map(|l| {
                LyricsLine::new((l.start as i64 + offset) as u32, l.value.clone())
            }).collect()
        },
        types::LyricsLines::NotSynced(items) => {
            items.iter().map(|l| {
                LyricsLine::new(0, l.clone())
            }).collect()
        },
        types::LyricsLines::None => Vec::new(),
    }
}

mod imp {
    use crate::opensonic::types;
    use crate::ui::song_object::PositionState;
    use relm4::adw::glib::derived_properties;
    use relm4::adw::gtk::prelude::*;
    use relm4::adw::gtk::subclass::prelude::*;
    use std::cell::{Cell, RefCell};
    use gstreamer::glib::Properties;
    use relm4::adw::glib as glib;

    // Object holding the state
    #[derive(Default, Properties)]
    #[properties(wrapper_type = super::LyricsLine)]
    pub struct LyricsLine {
        #[property(get, name = "value", member = value, type = String)]
        #[property(get, name = "start", member = start, type = u32)]
        pub lyrics_line: RefCell<types::LyricsLine>,
        #[property(get, set, builder(PositionState::default()))]
        pub position_state: Cell<PositionState>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for LyricsLine {
        const NAME: &'static str = "SanicLyricsLine";
        type Type = super::LyricsLine;
    }

    // Trait shared by all GObjects
    #[derived_properties]
    impl ObjectImpl for LyricsLine {
    }
}
