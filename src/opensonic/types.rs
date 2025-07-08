use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::error::Error;
use std::fmt;
use std::fmt::Debug;

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct License {
    pub valid: bool,
    pub email: Option<String>,
    pub license_expires: Option<String>,
    pub trial_expires: Option<String>
}


#[derive(Serialize, Deserialize, Debug)]
pub struct Extensions (pub Vec<Extension>);

#[derive(Serialize, Deserialize, Debug)]
pub struct Extension {
    pub name: String,
    pub versions: Vec<i32>
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Genre {
    pub name: String
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ItemDate {
    pub year: Option<u32>,
    pub month: Option<u32>,
    pub day: Option<u32>
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Artist {
    pub id: String,
    pub name: String,
    pub cover_art: Option<String>,
    pub artist_image_url: Option<String>,
    pub album_count: Option<u32>,
    pub starred: Option<String>,
    pub music_brainz_id: Option<String>,
    pub sort_name: Option<String>,
    pub roles: Option<Vec<String>>
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Contributor {
    pub role: String,
    pub sub_role: Option<String>,
    pub artist: Artist
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ReplayGain {
    pub track_gain: Option<f32>,
    pub album_gain: Option<f32>,
    pub track_peak: Option<f32>,
    pub album_peak: Option<f32>,
    pub base_gain: Option<f32>,
    pub fallback_gain: Option<f32>
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct DiscTitle {
    pub disc: u32,
    pub title: String
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Album {
    pub id: String,
    pub name: String,
    pub version: Option<String>,
    pub artist: Option<String>,
    pub artist_id: Option<String>,
    pub cover_art: Option<String>,
    pub song_count: u32,
    pub duration: u32,
    pub play_count: Option<u64>,
    pub created: String,
    pub starred: Option<String>,
    pub year: Option<u32>,
    pub genre: Option<String>,
    pub played: Option<String>,
    pub user_rating: Option<u8>,
    pub record_labels: Option<Vec<String>>, // TODO: RecordLabel
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
    pub disc_titles: Option<Vec<DiscTitle>>
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Song {
    pub id: String,
    pub parent: Option<String>,
    pub is_dir: bool,
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
    pub duration: Option<u32>,
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
    pub starred: Option<String>,
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
    pub explicit_status: Option<String>
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Search3Results {
    pub artist: Option<Vec<Artist>>,
    pub album: Option<Vec<Album>>,
    pub song: Option<Vec<Song>>
}

#[derive(Serialize, Deserialize, Debug)]
pub struct SubsonicError {
    pub code: i32,
    pub message: String
}

#[derive(Debug)]
pub struct InvalidResponseError{
    msg: String
}

impl SubsonicError {
    pub fn from_response(val: Value) -> Box<dyn Error> {
        let err = serde_json::from_value::<Self>(val["subsonic-response"]["error"].clone());
        if err.is_err(){
            err.unwrap_err().into()
        } else {
            err.unwrap().into()
        }
    }
}

impl fmt::Display for SubsonicError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Error from subsonic (code {}): {}", self.code, self.message)
    }
}

impl Error for SubsonicError {
    fn description(&self) -> &str {
        "Subsonic error"
    }
}

impl InvalidResponseError{
    pub fn new(message: &str) -> InvalidResponseError{
        InvalidResponseError{
            msg: String::from(message)
        }
    }

    pub fn new_boxed(message: &str) -> Box<InvalidResponseError>{
        InvalidResponseError{
            msg: String::from(message)
        }.into()
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