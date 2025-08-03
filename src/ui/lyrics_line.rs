
use relm4::{adw::subclass::prelude::{ObjectSubclassIsExt}, gtk::glib::{self, object::ObjectExt, Object}};

use crate::{opensonic::types::{self, LyricsList}, ui::song_object::PositionState};



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

impl LyricsLine {
    pub fn set_position_state(&self, position_state: PositionState) {
        self.set_property("position-state", position_state);
    }

    pub fn value(&self) -> String{
        self.property("value")
    }

    pub fn start(&self) -> u32{
        self.property("start")
    }
}

mod imp {
    use crate::opensonic::types;
    use crate::ui::song_object::PositionState;
    use relm4::adw::glib::{ParamSpec, ParamSpecEnum, ParamSpecString, Value};
    use relm4::adw::gtk::glib;
    use relm4::adw::gtk::prelude::*;
    use relm4::adw::gtk::subclass::prelude::*;
    use relm4::gtk::glib::ParamSpecUInt;
    use relm4::once_cell::sync::Lazy;
    use std::cell::{Cell, RefCell};

    // Object holding the state
    #[derive(Default)]
    pub struct LyricsLine {
        pub lyrics_line: RefCell<types::LyricsLine>,
        pub position_state: Cell<PositionState>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for LyricsLine {
        const NAME: &'static str = "SanicLyricsLine";
        type Type = super::LyricsLine;
    }

    // Trait shared by all GObjects
    impl ObjectImpl for LyricsLine {
        fn properties() -> &'static [ParamSpec] {
            static PROPERTIES: Lazy<Vec<ParamSpec>> = Lazy::new(|| {
                vec![
                    ParamSpecUInt::builder("start").build(),
                    ParamSpecString::builder("value").build(),
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
            match pspec.name() {
                "start" => self.lyrics_line.borrow().start.to_value(),
                "value" => self.lyrics_line.borrow().value.to_value(),
                "position-state" => self.position_state.get().to_value(),
                _ => unimplemented!(),
            }
        }
    }
}
