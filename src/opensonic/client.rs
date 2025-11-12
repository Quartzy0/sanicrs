use std::collections::HashSet;
use crate::opensonic::types::{Album, AlbumListType, Albums, Artist, Extensions, GenericResponse, InnerResponse, InvalidResponseError, License, LyricsLine, LyricsLines, LyricsList, OpenSubsonicResponse, OpenSubsonicResponseEmpty, Search3Results, Song, Starred, SubsonicError, SupportedExtensions};
use format_url::FormatUrl;
use rand::distr::{Alphanumeric, SampleString};
use reqwest;
use reqwest::{Client, ClientBuilder, Response};
use serde_json::Value;
use std::env;
use std::error::Error;
use std::fmt::{Debug};
use std::path::Path;
use std::rc::Rc;
use tokio::sync::RwLock;

#[derive(Debug, Clone)]
pub enum Credentials {
    UsernamePassword {
        username: String,
        password: String
    },
    ApiKey {
        key: String
    }
}

#[derive(Debug)]
pub struct OpenSubsonicClient {
    host: String,
    credentials: Credentials,
    client_name: String,
    client: Client,
    version: String,
    cover_cache: Option<String>,

    extensions: RwLock<HashSet<SupportedExtensions>>,
}

pub fn get_default_cache_dir() -> Option<String> {
    match env::var("XDG_CACHE_HOME") {
        Ok(p) => Path::new(p.as_str()).join("sanicrs").to_str().and_then(|s| Some(s.to_string())),
        Err(_) => {
            match env::var("HOME") {
                Ok(p) => Path::new(p.as_str()).join(".cache/sanicrs").to_str().and_then(|s| Some(s.to_string())),
                Err(_) => None,
            }
        },
    }
}

impl OpenSubsonicClient {

    pub fn new(
        host: &str,
        credentials: Credentials,
        client_name: &str,
        cover_cache: Option<String>,
    ) -> Self {
        // Validate cache dir
        let cover_cache_real = if let Some(cover_cache) = &cover_cache {
            let path = Path::new(cover_cache);
            let result = std::fs::create_dir_all(path);
            if result.is_ok() {
                let result = std::fs::exists(path);
                if result.is_err() {
                    eprintln!(
                        "Can't read cache dir/ ({}): {}",
                        cover_cache,
                        result.err().unwrap()
                    );
                    None
                } else if !result.ok().unwrap() {
                    println!("Cache dir not found: {}", cover_cache);
                    None
                } else {
                    Some(cover_cache)
                }
            } else {
                eprintln!("Error creating cache directory '{}': {:?}", cover_cache, result.err().unwrap());
                None
            }
        } else {
            println!("No cache dir set.");
            None
        };

        OpenSubsonicClient {
            host: String::from(host),
            credentials,
            client_name: String::from(client_name),
            client: ClientBuilder::new().build().unwrap(),
            version: String::from("1.15"),
            cover_cache: cover_cache_real.cloned(),
            extensions: RwLock::new(HashSet::new()),
        }
    }

    pub async fn init(&self) -> Result<(), Box<dyn Error>> {
        let response = self.make_action_request("getOpenSubsonicExtensions", vec![]).await?;
        if let InnerResponse::OpenSubsonicExtensions(extensions) = response {
            let mut guard = self.extensions.write().await;
            for ext in extensions.0 {
                match SupportedExtensions::try_from(&ext.name) {
                    Ok(e) => {
                        guard.insert(e);
                    },
                    Err(_) => println!("Unused extension '{}' supported by server", ext.name)
                };
            }
        } else {
            return Err(InvalidResponseError::new_invalid_response("OpenSubsonicExtensions", response));
        }
        let guard = self.extensions.read().await;
        println!("Supported extensions present: {:?}", guard);
        if let Credentials::ApiKey {..} = &self.credentials && !guard.contains(&SupportedExtensions::ApiKeyAuthentication) {
            return Err("API Key authentication not supported by server".into());
        }
        // Getting extensions doesn't check for valid authentication (at least on LMS).
        // This kind of makes sense since it isn't known if API key auth is supported before
        // making this request.
        // self.make_action_request_empty("ping", vec![]).await?;

        Ok(())
    }

    fn get_auth_params(&self) -> Vec<(&str, String)> {
        let mut params = vec![
            ("c", self.client_name.clone()),
            ("v", self.version.clone()),
            ("f", "json".to_string()),
        ];
        match &self.credentials {
            Credentials::UsernamePassword { username, password } => {
                let salt = Alphanumeric.sample_string(&mut rand::rng(), 16);
                let token_str = String::from(password) + salt.as_str();
                let hash: String = format!("{:x}", md5::compute(token_str));
                params.push(("u", username.clone()));
                params.push(("s", salt));
                params.push(("t", hash));
            }
            Credentials::ApiKey { key } => {
                params.push(("apiKey", key.clone()));
            }
        }
        params
    }

    async fn get_cache_resource(&self, id: &str) -> Option<Vec<u8>> {
        if let Some(cover_cache) = &self.cover_cache {
            let buf = Path::new(cover_cache).join(Path::new(id));
            return match tokio::fs::read(buf).await {
                Ok(b) => Some(b),
                Err(_) => None,
            };
        }
        None
    }

    async fn write_cached_resource(&self, id: &str, data: &Vec<u8>) {
        if let Some(cover_cache) = &self.cover_cache {
            let buf = Path::new(cover_cache).join(Path::new(id));
            let r = tokio::fs::write(buf, data).await;
            if let Err(err) = r {
                println!("Error when trying to write cache: {:?}", err);
            }
        }
    }

    fn get_action_request_get_url(
        &self,
        action: &str,
        extra_params: Vec<(&str, &str)>,
    ) -> String {
        let params = self.get_auth_params();
        let mut params: Vec<(&str, &str)> = params.iter().map(|(k, v)| (*k, v.as_str())).collect();
        params.extend(extra_params);
        let url = FormatUrl::new(&self.host)
            .with_path_template("/rest/:action")
            .with_substitutes(vec![("action", action)]);
        url.with_query_params(params).format_url()
    }

    async fn get_action_request(
        &self,
        action: &str,
        extra_params: Vec<(&str, &str)>,
    ) -> Result<Response, Box<dyn Error>> {
        let params = self.get_auth_params();
        let mut params: Vec<(&str, &str)> = params.iter().map(|(k, v)| (*k, v.as_str())).collect();
        params.extend(extra_params);
        let url = FormatUrl::new(&self.host)
            .with_path_template("/rest/:action")
            .with_substitutes(vec![("action", action)]);
        println!("Making request to '{}' with params: {:?}", action, params);
        if self.extensions.read().await.contains(&SupportedExtensions::FormPost) {
            let builder = self.client.post(url.format_url()).form(&params);
            builder.send().await.map_err(|e| e.into())
        } else {
            let builder = self.client
                .get(url.with_query_params(params).format_url());
            builder.send().await.map_err(|e| e.into())
        }
    }

    async fn make_action_request(&self, action: &str, extra_params: Vec<(&str, &str)>) -> Result<InnerResponse, Box<dyn Error>> {
        let response = self
            .get_action_request(action, extra_params)
            .await?
            .error_for_status()?;
        let x = response.text().await?;
        let response: OpenSubsonicResponse = serde_json::from_str::<GenericResponse<OpenSubsonicResponse>>(x.as_str())?.inner;
        if response.status != "ok" || response.error.is_some() {
            return if let Some(e) = response.error {
                Err(e.into())
            } else {
                Err("Unknown error".into())
            }
        }
        if !response.open_subsonic {
            return Err(InvalidResponseError::new_boxed("Response not of OpenSubsonic type (probably using an incompatible server)"));
        }
        Ok(response.inner)
    }

    pub async fn make_action_request_empty(&self, action: &str, extra_params: Vec<(&str, &str)>) -> Result<(), Box<dyn Error>> {
        let response = self
            .get_action_request(action, extra_params)
            .await?
            .error_for_status()?;
        let x = response.text().await?;
        let response: OpenSubsonicResponseEmpty = serde_json::from_str::<GenericResponse<OpenSubsonicResponseEmpty>>(x.as_str())?.inner;
        if response.status != "ok" || response.error.is_some() {
            return if let Some(e) = response.error {
                Err(e.into())
            } else {
                Err("Unknown error".into())
            }
        }
        if !response.open_subsonic {
            return Err(InvalidResponseError::new_boxed("Response not of OpenSubsonic type (probably using an incompatible server)"));
        }
        Ok(())
    }

    pub async fn get_license(&self) -> Result<License, Box<dyn Error>> {
        let response = self
            .make_action_request("getLicense", vec![])
            .await?;
        if let InnerResponse::License(license) = response {
            Ok(license)
        } else {
            Err(InvalidResponseError::new_invalid_response("License", response))
        }
    }

    pub async fn get_extensions(&self) -> Result<Extensions, Box<dyn Error>> {
        let response = self
            .make_action_request("getOpenSubsonicExtensions", vec![])
            .await?;
        if let InnerResponse::OpenSubsonicExtensions(ext) = response {
            Ok(ext)
        } else {
            Err(InvalidResponseError::new_invalid_response("OpenSubsonicExtensions", response))
        }
    }

    pub async fn search3(
        &self,
        query: &str,
        artist_count: Option<u32>,
        artist_offset: Option<u32>,
        album_count: Option<u32>,
        album_offset: Option<u32>,
        song_count: Option<u32>,
        song_offset: Option<u32>,
        music_folder_id: Option<&str>,
    ) -> Result<Search3Results, Box<dyn Error>> {
        let artist_count = artist_count.unwrap_or(20).to_string();
        let artist_offset = artist_offset.unwrap_or(0).to_string();
        let album_count = album_count.unwrap_or(20).to_string();
        let album_offset = album_offset.unwrap_or(0).to_string();
        let song_count = song_count.unwrap_or(20).to_string();
        let song_offset = song_offset.unwrap_or(0).to_string();

        let mut params = vec![
            ("query", query),
            ("artistCount", artist_count.as_str()),
            ("artistOffset", artist_offset.as_str()),
            ("albumCount", album_count.as_str()),
            ("albumOffset", album_offset.as_str()),
            ("songCount", song_count.as_str()),
            ("songOffset", song_offset.as_str()),
        ];

        if music_folder_id.is_some() {
            params.push(("musicFolderId", music_folder_id.unwrap()));
        }

        let response = self
            .make_action_request("search3", params)
            .await?;
        if let InnerResponse::SearchResult3(res) = response {
            Ok(res)
        } else {
            Err(InvalidResponseError::new_invalid_response("SearchResult3", response))
        }
    }

    pub fn stream_get_url(
        &self,
        id: &str,
        max_bit_rate: Option<u32>,
        format: Option<String>,
        time_offset: Option<u32>,
        size: Option<String>,
        estimate_content_length: Option<bool>,
        converted: Option<bool>,
    ) -> String{
        let max_bit_rate = max_bit_rate.and_then(|t| Some(t.to_string()));
        let time_offset = time_offset.and_then(|t| Some(t.to_string()));
        let estimate_content_length = estimate_content_length.unwrap_or(false).to_string();
        let converted = converted.unwrap_or(false).to_string();

        let mut params = vec![
            ("id", id),
            ("estimateContentLength", estimate_content_length.as_str()),
            ("converted", converted.as_str()),
        ];
        if let Some(mbr) = max_bit_rate.as_ref() {
            params.push(("maxBitRate", mbr))
        }
        if let Some(format) = format.as_ref() {
            params.push(("format", format))
        }
        if let Some(time_offset) = time_offset.as_ref() {
            params.push(("timeOffset", time_offset))
        }
        if let Some(size) = size.as_ref() {
            params.push(("size", size))
        }

        self
            .get_action_request_get_url("stream", params)
    }

    pub async fn get_cover_image(
        &self,
        id: &str,
        size: Option<&str>,
    ) -> Result<Vec<u8>, Box<dyn Error>> {
        if let Some(cached) = self.get_cache_resource(id).await {
            return Ok(cached);
        }

        let mut params = vec![("id", id)];

        if let Some(size) = size {
            params.push(("size", size));
        }

        let response = self
            .get_action_request("getCoverArt", params)
            .await?;
        if !response.status().is_success() {
            return Err(InvalidResponseError::new_boxed(format!("Response status code: {}", response.status()).as_str()));
        }
        if !response.headers().contains_key("Content-Type") {
            return Err(InvalidResponseError::new_boxed("No 'Content-Type' header in response."));
        }
        if response.headers()["Content-Type"] == "text/xml" {
            return Err(InvalidResponseError::new_boxed(
                response.text().await?.as_str(),
            ));
        } else if response.headers()["Content-Type"] == "application/json" {
            let s1 = response.text().await?;
            let response: Value = serde_json::from_str(&s1)?;
            if response["subsonic-response"]["status"] != "ok" {
                return Err(SubsonicError::from_response(response));
            }
            return Err(InvalidResponseError::new_boxed(&s1));
        }

        let bytes = response.bytes().await.unwrap().to_vec();
        self.write_cached_resource(id, &bytes).await;
        Ok(bytes)
    }

    pub async fn get_cover_image_url(&self, id: &str) -> Option<String> {
        if let Some(cover_cache) = &self.cover_cache {
            let buf = Path::new(cover_cache).join(Path::new(id));
            let path = std::path::absolute(buf.as_path());
            if let Ok(buf1) = path {
                let path = buf1.as_path();
                let path_str = path.to_str();
                if let Some(path_str) = path_str {
                    match std::fs::exists(path) {
                        Ok(exist) => {
                            if exist {
                                return Some(format!("file://{}", path_str));
                            }
                        },
                        Err(_) => {}
                    };
                    let _ = self.get_cover_image(id, None).await;
                    match std::fs::exists(path) { // Check if file exists now
                        Ok(exist) => {
                            if exist {
                                return Some(format!("file://{}", path_str));
                            }
                        },
                        Err(_) => {}
                    };
                }
            }
        } // Caching either didn't work or is not enabled
        Some(self.get_action_request_get_url("getCoverArt", vec![("id", id)]))
    }

    pub async fn get_song(&self, id: &str) -> Result<Rc<Song>, Box<dyn Error>> {
        let response = self
            .make_action_request("getSong", vec![("id", id)])
            .await?;
        if let InnerResponse::Song(res) = response {
            Ok(Rc::new(res))
        } else {
            Err(InvalidResponseError::new_invalid_response("Song", response))
        }
    }

    pub(super) async fn get_album_list(
        &self,
        list_type: AlbumListType,
        size: Option<u32>,
        offset: Option<u32>,
        from_year: Option<u32>,
        to_year: Option<u32>,
        genre: Option<String>,
        music_folder_id: Option<String>
    ) -> Result<Albums, Box<dyn Error>> {
        let size = size.unwrap_or(10).to_string();
        let offset = offset.unwrap_or(10).to_string();
        let from_year = from_year.and_then(|x| Some(x.to_string()));
        let to_year = to_year.and_then(|x| Some(x.to_string()));
        let mut params: Vec<(&str, &str)> = vec![
            ("type", list_type.into()),
            ("size", size.as_str()),
            ("offset", offset.as_str()),
        ];

        if from_year.is_some() {
            params.push(("fromYear", from_year.as_ref().unwrap()));
        }
        if to_year.is_some() {
            params.push(("toYear", to_year.as_ref().unwrap()));
        }
        if genre.is_some() {
            params.push(("genre", genre.as_ref().unwrap()));
        }
        if music_folder_id.is_some() {
            params.push(("musicFolderId", music_folder_id.as_ref().unwrap()));
        }
        let response = self
            .make_action_request("getAlbumList2", params)
            .await?;
        if let InnerResponse::AlbumList2(res) = response {
            Ok(res)
        } else {
            Err(InvalidResponseError::new_invalid_response("AlbumList2", response))
        }
    }

    pub async fn get_album(
        &self,
        id: &str
    ) ->  Result<Album, Box<dyn Error>> {
        let response = self
            .make_action_request("getAlbum", vec![("id", id)])
            .await?;
        if let InnerResponse::Album(res) = response {
            Ok(res)
        } else {
            Err(InvalidResponseError::new_invalid_response("Album", response))
        }
    }

    pub async fn get_similar_songs(
        &self,
        id: &str,
        count: Option<u32>
    ) -> Result<Vec<Song>, Box<dyn Error>> {
        let count = count.and_then(|o| Some(o.to_string()));

        let mut params = vec![("id", id)];
        if count.is_some() {
            params.push(("count", count.as_ref().unwrap()));
        }
        let response = self
            .make_action_request("getSimilarSongs2", params)
            .await?;
        if let InnerResponse::SimilarSongs2(res) = response {
            Ok(res.song)
        } else {
            Err(InvalidResponseError::new_invalid_response("SimilarSongs2", response))
        }
    }

    pub async fn get_random_songs(
        &self,
        size: Option<u32>,
        genre: Option<&str>,
        from_year: Option<u32>,
        to_year: Option<u32>,
        music_folder_id: Option<&str>
    ) -> Result<Vec<Song>, Box<dyn Error>> {
        let size = size.and_then(|o| Some(o.to_string()));
        let from_year = from_year.and_then(|o| Some(o.to_string()));
        let to_year = to_year.and_then(|o| Some(o.to_string()));

        let mut params: Vec<(&str, &str)> = Vec::with_capacity(5);
        if size.is_some() {
            params.push(("size", size.as_ref().unwrap()));
        }
        if let Some(genre) = genre {
            params.push(("genre", genre));
        }
        if from_year.is_some() {
            params.push(("fromYear", from_year.as_ref().unwrap()));
        }
        if to_year.is_some() {
            params.push(("toYear", to_year.as_ref().unwrap()));
        }
        if let Some(music_folder_id) = music_folder_id {
            params.push(("musicFolderId", music_folder_id));
        }

        let response = self
            .make_action_request("getRandomSongs", params)
            .await?;
        if let InnerResponse::RandomSongs(res) = response {
            Ok(res.song)
        } else {
            Err(InvalidResponseError::new_invalid_response("RandomSongs", response))
        }
    }

    pub async fn get_lyrics(
        &self,
        id: &str
    ) -> Result<Vec<LyricsList>, Box<dyn Error>> {
        if !self.extensions.read().await.contains(&SupportedExtensions::SongLyrics) {
            return Ok(Vec::new());
        }

        let params = vec![("id", id)];

        let body = self
            .get_action_request("getLyricsBySongId", params)
            .await?
            .text()
            .await?;
        let mut response: Value = serde_json::from_str(&body)?;
        if response["subsonic-response"]["status"] != "ok" {
            return Err(SubsonicError::from_response(response));
        }
        let mut response = response["subsonic-response"]["lyricsList"].take();
        let response = response.as_object_mut().ok_or(InvalidResponseError::new_boxed("'lyricsList' wasn't object"))?;
        if !response.contains_key("structuredLyrics") {
            return Ok(Vec::new());
        }
        let response = response["structuredLyrics"]
            .as_array_mut().ok_or(InvalidResponseError::new_boxed("'structuredLyrics' wasn't an array"))?;
        let resp: Vec<LyricsList> = response.iter_mut().map(|v: &mut Value| {
            let synced = v["synced"].as_bool().unwrap_or(false);
            let lines: Result<LyricsLines, Box<dyn Error>> = if synced{
                serde_json::from_value::<Vec<LyricsLine>>(v["line"].take())
                    .and_then(|v| Ok(LyricsLines::Synced(v)))
                    .map_err(|e| e.into())
            } else {
                v["line"].take().as_array().and_then(
                    |a|
                    Some(a.iter().map(|v: &Value| v["start"].as_str().unwrap().to_string()).collect())
                ).and_then(|v| Some(LyricsLines::NotSynced(v)))
                .ok_or(InvalidResponseError::new_boxed("Error parsing lyrics lines"))
            };
            lines.and_then(|l| {
                serde_json::from_value::<LyricsList>(v.take())
                    .and_then(|mut info: LyricsList| {
                        info.synced = match &l {
                            LyricsLines::Synced(_) => true,
                            LyricsLines::NotSynced(_) => false,
                            LyricsLines::None => false,
                        };
                        info.lines = l;
                        Ok(info)
                    })
                    .map_err(|e| e.into())
            })
        })
        .filter_map(|r| {
            match r {
                Ok(v) => Some(v),
                Err(e) => {
                    eprintln!("Error when parsing lyrics: {}", e);
                    None
                },
            }
        })
        .collect();

        Ok(resp)
    }

    pub async fn scrobble(
        &self,
        id: &str,
        submission: Option<bool>
    ) -> Result<(), Box<dyn Error>> {
        let submission = submission.unwrap_or(true).to_string();
        let params = vec![
            ("id", id),
            ("submission", submission.as_str())
        ];

        self.make_action_request_empty("scrobble", params).await
    }

    pub async fn star(
        &self,
        id: Vec<&str>,
        album_id: Vec<&str>,
        artist_id: Vec<&str>
    ) -> Result<(), Box<dyn Error>> {
        let mut params = vec![];
        for id in id {
            params.push(("id", id));
        }
        for album_id in album_id {
            params.push(("albumId", album_id));
        }
        for artist_id in artist_id {
            params.push(("artistId", artist_id));
        }
        if params.len() == 0 {
            return Err("No song, album or artist specified to be starred".into());
        }

        self.make_action_request_empty("star", params).await
    }

    pub async fn unstar(
        &self,
        id: Vec<&str>,
        album_id: Vec<&str>,
        artist_id: Vec<&str>
    ) -> Result<(), Box<dyn Error>> {
        let mut params = vec![];
        for id in id {
            params.push(("id", id));
        }
        for album_id in album_id {
            params.push(("albumId", album_id));
        }
        for artist_id in artist_id {
            params.push(("artistId", artist_id));
        }
        if params.len() == 0 {
            return Err("No song, album or artist specified to be unstarred".into());
        }

        self.make_action_request_empty("unstar", params).await
    }

    pub async fn get_artist(
        &self,
        id: &str,
    ) -> Result<Artist, Box<dyn Error>> {
        let response = self
            .make_action_request("getArtist", vec![("id", id)])
            .await?;
        if let InnerResponse::Artist(res) = response {
            Ok(res)
        } else {
            Err(InvalidResponseError::new_invalid_response("Artist", response))
        }
    }

    pub async fn get_starred(
        &self,
    ) -> Result<Starred, Box<dyn Error>> {
        let response = self
            .make_action_request("getStarred2", vec![])
            .await?;
        if let InnerResponse::Starred2(res) = response {
            Ok(res)
        } else {
            Err(InvalidResponseError::new_invalid_response("Starred2", response))
        }
    }
}