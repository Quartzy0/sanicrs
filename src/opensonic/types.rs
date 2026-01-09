use serde::{Deserialize, Serialize};
use serde_json::Value;
use serde_with::DurationSeconds;
use serde_with::serde_as;
use std::cell::RefCell;
use std::error::Error;
use std::fmt;
use std::fmt::Debug;
use std::time::Duration;
use relm4::adw::glib;

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenSubsonicResponseEmpty {
    pub status: String,
    pub version: String,
    pub r#type: String,
    pub server_version: String,
    pub open_subsonic: bool,
    pub error: Option<SubsonicError>,
}

#[derive(Debug, Deserialize)]
pub struct GenericResponse<T> {
    #[serde(rename = "subsonic-response")]
    pub inner: T,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenSubsonicResponse<T> {
    pub status: String,
    pub version: String,
    pub r#type: String,
    pub server_version: String,
    pub open_subsonic: bool,
    pub error: Option<SubsonicError>,
    #[serde(flatten)]
    pub inner: T
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Songs {
    pub song: Vec<Song>
}

#[derive(Debug, Eq, PartialEq)]
pub enum AlbumListType {
    Random,
    Newest,
    Highest,
    Frequent,
    Recent,
    AlphabeticalByName,
    AlphabeticalByArtist,
    Starred,
    ByYear,
    ByGenre
}

impl Into<&str> for AlbumListType {
    fn into(self) -> &'static str {
        match self {
            AlbumListType::Random => "random",
            AlbumListType::Newest => "newest",
            AlbumListType::Highest => "highest",
            AlbumListType::Frequent => "frequent",
            AlbumListType::Recent => "recent",
            AlbumListType::AlphabeticalByName => "alphabeticalByName",
            AlbumListType::AlphabeticalByArtist => "alphabeticalByArtist",
            AlbumListType::Starred => "starred",
            AlbumListType::ByYear => "byYear",
            AlbumListType::ByGenre => "byGenre",
        }
    }
}

#[derive(Debug, PartialEq, Eq, Hash)]
pub enum SupportedExtensions { // Only extensions used by this client are included here
    FormPost,
    SongLyrics,
    ApiKeyAuthentication
}

impl TryFrom<&String> for SupportedExtensions {
    type Error = InvalidResponseError;

    fn try_from(value: &String) -> Result<Self, Self::Error> {
        match value.as_str() {
            "formPost" => Ok(SupportedExtensions::FormPost),
            "songLyrics" => Ok(SupportedExtensions::SongLyrics),
            "apiKeyAuthentication" => Ok(SupportedExtensions::ApiKeyAuthentication),
            _ => Err(InvalidResponseError::new("Unsupported extension type (non fatal)"))
        }
    }
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Starred {
    #[serde(rename(serialize = "artist", deserialize = "artist"))]
    pub artists: Option<Vec<Artist>>,
    #[serde(rename(serialize = "album", deserialize = "album"))]
    pub albums: Option<Vec<Album>>,
    #[serde(rename(serialize = "song", deserialize = "song"))]
    pub songs: Option<Vec<Song>>,
}

#[derive(Serialize, Deserialize, Debug, Default, Clone)]
#[serde(rename_all = "camelCase")]
pub struct LyricsLine {
    pub start: u32,
    pub value: String,
}

#[derive(Debug, Default)]
pub enum LyricsLines {
    Synced(Vec<LyricsLine>),
    NotSynced(Vec<String>),
    #[default]
    None
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct LyricsList {
    pub display_artist: Option<String>,
    pub display_title: Option<String>,
    pub lang: String,
    pub offset: Option<i64>,
    pub synced: bool,
    #[serde(skip)]
    pub lines: LyricsLines
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct License {
    pub valid: bool,
    pub email: Option<String>,
    pub license_expires: Option<String>,
    pub trial_expires: Option<String>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Extension {
    pub name: String,
    pub versions: Vec<i32>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Genre {
    pub name: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ItemDate {
    pub year: Option<u32>,
    pub month: Option<u32>,
    pub day: Option<u32>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Artist {
    pub id: String,
    pub name: String,
    pub cover_art: Option<String>,
    pub artist_image_url: Option<String>,
    pub album_count: Option<u32>,
    pub starred: RefCell<Option<String>>,
    pub music_brainz_id: Option<String>,
    pub sort_name: Option<String>,
    pub roles: Option<Vec<String>>,
    #[serde(rename(serialize = "album", deserialize = "album"))]
    pub albums: Option<Vec<Album>>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Contributor {
    pub role: String,
    pub sub_role: Option<String>,
    pub artist: Artist,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ReplayGain {
    pub track_gain: Option<f32>,
    pub album_gain: Option<f32>,
    pub track_peak: Option<f32>,
    pub album_peak: Option<f32>,
    pub base_gain: Option<f32>,
    pub fallback_gain: Option<f32>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct DiscTitle {
    pub disc: u32,
    pub title: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct RecordLabel {
    pub name: String,
}

#[serde_as]
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Album {
    pub id: String,
    pub name: String,
    pub version: Option<String>,
    pub artist: Option<String>,
    pub artist_id: Option<String>,
    pub cover_art: Option<String>,
    pub song_count: u32,
    #[serde_as(as = "DurationSeconds<u64>")]
    pub duration: Duration,
    pub play_count: Option<u64>,
    pub created: String,
    pub starred: RefCell<Option<String>>,
    pub year: Option<u32>,
    pub genre: Option<String>,
    pub played: Option<String>,
    pub user_rating: Option<u8>,
    pub record_labels: Option<Vec<RecordLabel>>,
    pub music_brainz_id: Option<String>,
    pub genres: Option<Vec<Genre>>,
    pub artists: Option<Vec<Artist>>,
    pub display_artists: Option<String>,
    pub release_type: Option<Vec<String>>,
    pub moods: Option<Vec<String>>,
    pub sort_name: Option<String>,
    pub original_release_date: Option<ItemDate>,
    pub release_date: Option<ItemDate>,
    pub is_compilation: Option<bool>,
    pub explicit_status: Option<String>,
    pub disc_titles: Option<Vec<DiscTitle>>,
    #[serde(rename = "song")]
    pub songs: Option<Vec<Song>>
}

#[serde_as]
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
#[derive(Default)]
pub struct Song {
    pub id: String,
    pub parent: Option<String>,
    pub is_dir: Option<bool>, // Not optional in OpenSubsonic spec, but not provided by LMS
    pub title: String,
    pub album: Option<String>,
    pub artist: Option<String>,
    pub track: Option<i32>,
    pub year: Option<i32>,
    pub genre: Option<String>,
    pub cover_art: Option<String>,
    pub size: Option<u64>,
    pub content_type: Option<String>,
    pub suffix: Option<String>,
    pub transcoded_content_type: Option<String>,
    pub transcoded_suffix: Option<String>,
    #[serde_as(as = "Option<DurationSeconds<u64>>")]
    pub duration: Option<Duration>,
    pub bit_rate: Option<u32>,
    pub bit_depth: Option<u32>,
    pub sampling_rate: Option<u32>,
    pub channel_count: Option<u32>,
    pub path: Option<String>,
    pub is_video: Option<bool>,
    pub user_rating: Option<u8>,
    pub average_rating: Option<f32>,
    pub play_count: Option<u64>,
    pub disc_number: Option<u32>,
    pub created: Option<String>,
    pub starred: RefCell<Option<String>>,
    pub album_id: Option<String>,
    pub artist_id: Option<String>,
    pub r#type: Option<String>, // 'type'
    pub media_type: Option<String>,
    pub bookmark_position: Option<u64>,
    pub original_width: Option<u32>,
    pub original_height: Option<u32>,
    pub played: Option<String>,
    pub bpm: Option<u32>,
    pub comment: Option<String>,
    pub sort_name: Option<String>,
    pub music_brainz_id: Option<String>,
    pub isrc: Option<Vec<String>>,
    pub genres: Option<Vec<Genre>>,
    pub artists: Option<Vec<Artist>>,
    pub display_artists: Option<String>,
    pub album_artists: Option<Vec<Artist>>,
    pub display_album_artists: Option<String>,
    pub contributors: Option<Vec<Contributor>>,
    pub display_composer: Option<String>,
    pub moods: Option<Vec<String>>,
    pub replay_gain: Option<ReplayGain>,
    pub explicit_status: Option<String>,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Search3Results {
    pub artist: Option<Vec<Artist>>,
    pub album: Option<Vec<Album>>,
    pub song: Option<Vec<Song>>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct SubsonicError {
    pub code: i32,
    pub message: String,
}

#[derive(Debug)]
pub struct InvalidResponseError {
    msg: String,
}

impl Song {
    pub fn artists(&self) -> String {
        match self.artists.as_ref() {
            Some(artists) => {
                artists
                    .iter()
                    .map(|a| format!("<a href=\"{}\" title=\"View artist\" class=\"normal-link\">{}</a>", a.id,
                                     glib::markup_escape_text(a.name.as_str())))
                    .collect::<Vec<String>>()
                    .join("•")
            }
            None => self.display_artists.clone().unwrap_or(self.artist.clone().unwrap_or("Unknown artist".to_string()))
        }
    }

    pub fn artists_no_markup(&self) -> String {
        match self.artists.as_ref() {
            Some(artists) => {
                artists
                    .iter()
                    .map(|a| a.name.clone())
                    .collect::<Vec<String>>()
                    .join(", ")
            }
            None => self.display_artists.clone().unwrap_or(self.artist.clone().unwrap_or("Unknown artist".to_string()))
        }
    }

    pub fn is_starred(&self) -> bool {
        self.starred.borrow().is_some()
    }
}

impl Album {
    pub fn artists(&self) -> String {
        match self.artists.as_ref() {
            Some(artists) => {
                artists
                    .iter()
                    .map(|a| format!("<a href=\"{}\" title=\"View artist\" class=\"normal-link\">{}</a>", a.id,
                                     glib::markup_escape_text(a.name.as_str())))
                    .collect::<Vec<String>>()
                    .join("•")
            }
            None => self.display_artists.clone().unwrap_or(self.artist.clone().unwrap_or("Unknown artist".to_string()))
        }
    }

    pub fn artists_no_markup(&self) -> String {
        match self.artists.as_ref() {
            Some(artists) => {
                artists
                    .iter()
                    .map(|a| a.name.clone())
                    .collect::<Vec<String>>()
                    .join(", ")
            }
            None => self.display_artists.clone().unwrap_or(self.artist.clone().unwrap_or("Unknown artist".to_string()))
        }
    }

    pub fn is_starred(&self) -> bool {
        self.starred.borrow().is_some()
    }
}

impl Artist {
    pub fn is_starred(&self) -> bool {
        self.starred.borrow().is_some()
    }
}

pub fn duration_display_str(duration: &Duration) -> String {
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

    str
}

impl SubsonicError {
    pub fn from_response(mut val: Value) -> Box<dyn Error + Send + Sync> {
        let err = serde_json::from_value::<Self>(val["subsonic-response"]["error"].take());
        if err.is_err() {
            err.unwrap_err().into()
        } else {
            err.unwrap().into()
        }
    }
}

impl fmt::Display for SubsonicError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "Error from subsonic (code {}): {}",
            self.code, self.message
        )
    }
}

impl Error for SubsonicError {
    fn description(&self) -> &str {
        "Subsonic error"
    }
}

impl InvalidResponseError {
    pub fn new(message: &str) -> InvalidResponseError {
        InvalidResponseError {
            msg: String::from(message),
        }
    }

    pub fn new_boxed(message: &str) -> Box<InvalidResponseError> {
        InvalidResponseError {
            msg: String::from(message),
        }
        .into()
    }
}

impl fmt::Display for InvalidResponseError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Invalid Subsonic response: {}", self.msg)
    }
}

impl Error for InvalidResponseError {
    fn description(&self) -> &str {
        "Invalid Subsonic response"
    }
}
