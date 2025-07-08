use crate::opensonic::types::{Extensions, InvalidResponseError, License, Search3Results, SubsonicError};
use format_url::FormatUrl;
use rand::distr::{Alphanumeric, SampleString};
use reqwest;
use reqwest::{Client, ClientBuilder, Response};
use std::error::Error;

pub struct OpensonicClient {
    host: String,
    username: String,
    password: String,
    client_name: String,
    client: Client,
    version: String,

    post_form: bool,
    lyrics: bool,
}

impl OpensonicClient {
    pub fn new_non_inited(host: &str, username: &str, password: &str, client_name: &str) -> Self {
        OpensonicClient {
            host: String::from(host),
            username: String::from(username),
            password: String::from(password),
            client_name: String::from(client_name),
            client: ClientBuilder::new().build().unwrap(),
            version: String::from("1.15"),

            post_form: false,
            lyrics: false,
        }
    }

    pub async fn new(host: &str, username: &str, password: &str, client_name: &str) -> Self {
        let mut client: Self = Self::new_non_inited(host, username, password, client_name);
        client.init().await.expect("Error when initializing");
        client
    }

    pub async fn init(&mut self) -> Result<(), Box<dyn Error>> {
        let extensions = self.get_extensions().await?;

        for ext in extensions.0 {
            if ext.name == "formPost" {
                self.post_form = true;
            } else if ext.name == "songLyrics" {
                self.lyrics = true;
            }
        }

        Ok(())
    }

    fn get_action_request(
        &self,
        action: &str,
        extra_params: Vec<(&str, &str)>,
    ) -> impl Future<Output = Result<Response, reqwest::Error>> + use<> {
        let salt = Alphanumeric.sample_string(&mut rand::rng(), 16);
        let token_str = String::from(&self.password) + salt.as_str();
        let hash: String = format!("{:x}", md5::compute(token_str));
        let mut params = vec![
            ("c", self.client_name.as_str()),
            ("v", self.version.as_str()),
            ("f", "json"),
            ("u", self.username.as_str()),
            ("s", salt.as_str()),
            ("t", hash.as_str()),
        ];
        params.extend(extra_params);
        let url = FormatUrl::new(&self.host)
            .with_path_template("/rest/:action")
            .with_substitutes(vec![("action", action)]);
        if self.post_form {
            self.client.post(url.format_url()).form(&params).send()
        } else {
            self.client
                .get(url.with_query_params(params).format_url())
                .send()
        }
    }

    pub async fn get_license(&self) -> Result<License, Box<dyn Error>> {
        let body = self
            .get_action_request("getLicense.view", vec![])
            .await?
            .text()
            .await?;
        let response: serde_json::Value = serde_json::from_str(&body)?;
        if response["subsonic-response"]["status"] != "ok" {
            return Err(SubsonicError::from_response(response));
        }

        let resp: License =
            serde_json::from_value(response["subsonic-response"]["license"].clone())?;
        Ok(resp)
    }

    pub async fn get_extensions(&self) -> Result<Extensions, Box<dyn Error>> {
        let body = self
            .get_action_request("getOpenSubsonicExtensions.view", vec![])
            .await?
            .text()
            .await?;
        let response: serde_json::Value = serde_json::from_str(&body)?;
        if response["subsonic-response"]["status"] != "ok" {
            return Err(SubsonicError::from_response(response));
        }

        let resp: Extensions = serde_json::from_value(
            response["subsonic-response"]["openSubsonicExtensions"].clone(),
        )?;
        Ok(resp)
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
        music_folder_id: Option<&str>
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
            ("songOffset", song_offset.as_str())
        ];

        if music_folder_id.is_some() {
            params.push(("musicFolderId", music_folder_id.unwrap()));
        }

        let body = self
            .get_action_request("search3.view", params)
            .await?
            .text()
            .await?;
        let response: serde_json::Value = serde_json::from_str(&body)?;
        if response["subsonic-response"]["status"] != "ok" {
            return Err(SubsonicError::from_response(response));
        }

        let resp: Search3Results =
            serde_json::from_value(response["subsonic-response"]["searchResult3"].clone())?;
        Ok(resp)
    }

    pub async fn steam(
        &self,
        id: &str,
        max_bit_rate: Option<u32>,
        format: Option<String>,
        time_offset: Option<u32>,
        size: Option<String>,
        estimate_content_length: Option<bool>,
        converted: Option<bool>
    ) -> Result<Response, Box<dyn Error>> {
        let max_bit_rate = max_bit_rate.and_then(|t| Some(t.to_string()));
        let time_offset = time_offset.and_then(|t| Some(t.to_string()));
        let estimate_content_length = estimate_content_length.unwrap_or(false).to_string();
        let converted = converted.unwrap_or(false).to_string();

        let mut params = vec![
            ("id", id),
            ("estimateContentLength", estimate_content_length.as_str()),
            ("converted", converted.as_str())
        ];
        let mbr: String;
        if max_bit_rate.is_some() {
            mbr = max_bit_rate.unwrap();
            params.push(("maxBitRate", &*mbr))
        }
        let f: String;
        if format.is_some() {
            f = format.unwrap();
            params.push(("format", &*f))
        }
        let to: String;
        if time_offset.is_some() {
            to = time_offset.unwrap();
            params.push(("timeOffset", &*to))
        }
        let s: String;
        if size.is_some() {
            s = size.unwrap();
            params.push(("size", &*s))
        }

        let response = self
            .get_action_request("stream.view", params)
            .await?;
        if response.headers()["Content-Type"] == "text/xml" {
            return Err(InvalidResponseError::new_boxed(response.text().await?.as_str()));
        } else if response.headers()["Content-Type"] == "application/json" {
            let s1 = response.text().await?;
            let response: serde_json::Value = serde_json::from_str(&*s1)?;
            if response["subsonic-response"]["status"] != "ok" {
                return Err(SubsonicError::from_response(response));
            }
            return Err(InvalidResponseError::new_boxed(&*s1));
        }

        Ok(response)
    }
}
