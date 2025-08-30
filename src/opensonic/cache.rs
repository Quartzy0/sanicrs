use crate::opensonic::client::OpenSubsonicClient;
use crate::opensonic::types::{Album, AlbumListType, Artist, LyricsList, Song};
use crate::ui::album_object::AlbumObject;
use crate::ui::artist_object::ArtistObject;
use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::error::Error;
use std::rc::Rc;
use std::sync::Arc;
use relm4::adw::gdk::Texture;
use relm4::adw::glib;
use tokio::sync::RwLock;

#[derive(Clone, Debug)]
pub struct SongCache {
    cache: Rc<RwLock<HashMap<String, Rc<Song>>>>,
    client: &'static OpenSubsonicClient,
}

impl SongCache {
    pub fn new(client: &'static OpenSubsonicClient) -> Self {
        Self {
            client,
            cache: Rc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn get_song_cached(&self, song: Song) -> Rc<Song> {
        let mut cache_w = self.cache.write().await;

        match cache_w.entry(song.id.clone()) {
            Entry::Occupied(occupied_entry) => occupied_entry.get().clone(),
            Entry::Vacant(vacant_entry) => {
                let song = Rc::new(song);
                vacant_entry.insert(song.clone());
                song
            },
        }
    }

    pub async fn get_song(&self, id: &str) -> Result<Rc<Song>, Box<dyn Error>> {
        {
            let cahce_r = self.cache.read().await;
            if let Some(song) = cahce_r.get(id) {
                return Ok(song.clone());
            }
        }
        let mut cache_w = self.cache.write().await;
        let song = self.client.get_song(id).await?;
        cache_w.insert(id.to_string(), song.clone());
        Ok(song)
    }

    pub async fn add_songs(&self, songs: Vec<Song>) -> Vec<Rc<Song>> {
        let mut cache_w = self.cache.write().await;

        songs
            .into_iter()
            .map(|s| {
                cache_w.get(&s.id).cloned().unwrap_or_else(|| {
                    let s1 = Rc::new(s);
                    cache_w.insert(s1.id.clone(), s1.clone());
                    s1
                })
            })
            .collect()
    }

    pub async fn get_similar_songs(&self, id: &str, count: Option<u32>) -> Result<Vec<Rc<Song>>, Box<dyn Error>> {
        let songs = self.client.get_similar_songs(id, count).await?;
        Ok(self.add_songs(songs).await)
    }

    pub async fn get_random_songs(
        &self,
        size: Option<u32>,
        genre: Option<&str>,
        from_year: Option<u32>,
        to_year: Option<u32>,
        music_folder_id: Option<&str>
    ) -> Result<Vec<Rc<Song>>, Box<dyn Error>> {
        let songs = self.client.get_random_songs(size, genre, from_year, to_year, music_folder_id).await?;
        Ok(self.add_songs(songs).await)
    }

    pub async fn search(&self, query: &str, count: u32, offset: Option<u32>) -> Result<Vec<Rc<Song>>, Box<dyn Error>> {
        let res = self.client.search3(query, Some(0), None, Some(0), None, Some(count), offset, None).await?;
        if let Some(songs) = res.song {
            Ok(self.add_songs(songs).await)
        } else {
            Err("No songs found".into())
        }
    }

    pub async fn toggle_starred(&self, song: &Rc<Song>) -> Result<(), Box<dyn Error>> {
        if song.is_starred() {
            self.client.unstar(vec![&song.id], Vec::new(), Vec::new()).await?;
            song.starred.replace(None);
        } else {
            self.client.star(vec![&song.id], Vec::new(), Vec::new()).await?;
            song.starred.replace(Some("yes".into()));
        }
        Ok(())
    }
}

#[derive(Clone, Debug)]
pub struct AlbumCache {
    cache: Rc<RwLock<HashMap<String, AlbumObject>>>,
    client: &'static OpenSubsonicClient,
    song_cache: SongCache,
}

impl AlbumCache {
    pub fn new(client: &'static OpenSubsonicClient, song_cache: SongCache) -> Self {
        Self {
            client,
            cache: Rc::new(RwLock::new(HashMap::new())),
            song_cache,
        }
    }

    pub async fn get_album_list(
        &self,
        list_type: AlbumListType,
        size: Option<u32>,
        offset: Option<u32>,
        from_year: Option<u32>,
        to_year: Option<u32>,
        genre: Option<String>,
        music_folder_id: Option<String>,
    ) -> Result<Vec<AlbumObject>, Box<dyn Error>> {
        let resp = self
            .client
            .get_album_list(
                list_type,
                size,
                offset,
                from_year,
                to_year,
                genre,
                music_folder_id,
            )
            .await?;

        let mut ret: Vec<AlbumObject> = Vec::with_capacity(resp.0.len());

        let mut cache_w = self.cache.write().await;
        for album in resp.0 {
            if let Some(cached) = cache_w.get(&album.id) {
                ret.push(cached.clone());
            } else {
                let object = AlbumObject::new(album);
                cache_w.insert(object.id(), object.clone());
                ret.push(object);
            }
        }

        Ok(ret)
    }

    pub async fn get_album(&self, id: &str) -> Result<AlbumObject, Box<dyn Error>> {
        {
            let cache_r = self.cache.read().await;
            if let Some(cached) = cache_r.get(id) {
                if !cached.has_songs() {
                    let (_resp, songs) = self.client.get_album(id).await?;
                    cached.set_songs(self.song_cache.add_songs(songs).await);
                }
                Ok(cached.clone())
            } else {
                drop(cache_r);
                let (resp, songs) = self.client.get_album(id).await?;
                let album = AlbumObject::new(resp);
                album.set_songs(self.song_cache.add_songs(songs).await);
                let mut cache_w = self.cache.write().await;
                cache_w.insert(album.id(), album.clone());
                Ok(album)
            }
        }
    }

    pub async fn add_albums(&self, albums: Vec<Album>) -> Vec<AlbumObject> {
        let mut cache_w = self.cache.write().await;

        albums
            .into_iter()
            .map(|s| {
                cache_w.get(&s.id).cloned().unwrap_or_else(|| {
                    let s1 = AlbumObject::new(s);
                    cache_w.insert(s1.id().clone(), s1.clone());
                    s1
                })
            })
            .collect()
    }

    pub async fn search(&self, query: &str, count: u32, offset: Option<u32>) -> Result<Vec<AlbumObject>, Box<dyn Error>> {
        let res = self.client.search3(query, Some(0), None, Some(count), offset, Some(0), None, None).await?;
        if let Some(albums) = res.album {
            Ok(self.add_albums(albums).await)
        } else {
            Err("No albums found".into())
        }
    }
}

#[derive(Clone, Debug)]
pub struct CoverCache {
    cache: Rc<RwLock<HashMap<String, Texture>>>,
    client: &'static OpenSubsonicClient,
}

impl CoverCache {
    pub fn new(client: &'static OpenSubsonicClient) -> Self {
        Self {
            client,
            cache: Rc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn get_cover_texture(&self, id: &str) -> Result<Texture, Box<dyn Error>> {
        {
            let cache_r = self.cache.read().await;
            if let Some(texture) = cache_r.get(id) {
                return Ok(texture.clone());
            }
        }
        let resp = self.client.get_cover_image(id, Some("512")).await?;
        let bytes = glib::Bytes::from(&resp);
        let texture = Texture::from_bytes(&bytes)?;
        let mut cache_w = self.cache.write().await;
        cache_w.insert(id.to_string(), texture.clone());
        Ok(texture)
    }
}

#[derive(Clone, Debug)]
pub struct LyricsCache {
    cache: Rc<RwLock<HashMap<String, Arc<Vec<LyricsList>>>>>,
    client: &'static OpenSubsonicClient,
}

impl LyricsCache {
    pub fn new(client: &'static OpenSubsonicClient) -> Self {
        Self {
            client,
            cache: Rc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn get_lyrics(&self, id: &str) -> Result<Arc<Vec<LyricsList>>, Box<dyn Error>> {
        {
            let cache_r = self.cache.read().await;
            if let Some(lyrics) = cache_r.get(id) {
                return Ok(lyrics.clone());
            }
        }
        let lyrics = Arc::new(self.client.get_lyrics(id).await?);
        let mut cache_w = self.cache.write().await;
        cache_w.insert(id.to_string(), lyrics.clone());
        Ok(lyrics)
    }
}

#[derive(Clone, Debug)]
pub struct ArtistCache {
    cache: Rc<RwLock<HashMap<String, ArtistObject>>>,
    client: &'static OpenSubsonicClient,
}

impl ArtistCache {
    pub fn new(client: &'static OpenSubsonicClient) -> Self {
        Self {
            client,
            cache: Rc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn get_artist(&self, id: &str) -> Result<ArtistObject, Box<dyn Error>> {
        {
            let cache_r = self.cache.read().await;
            if let Some(artist) = cache_r.get(id) {
                if !artist.has_albums() {
                    let artist_new = self.client.get_artist(id).await?;
                    artist.set_artist(artist_new);
                }
                return Ok(artist.clone());
            }
        }
        let artist = self.client.get_artist(id).await?;
        let artist = ArtistObject::new(artist);
        let mut cache_w = self.cache.write().await;
        cache_w.insert(id.to_string(), artist.clone());
        Ok(artist)
    }

    pub async fn ensure_albums(&self, artist: ArtistObject) -> Result<ArtistObject, Box<dyn Error>> {
        if artist.has_albums() {
            Ok(artist)
        } else {
            self.get_artist(&artist.id()).await
        }
    }

    pub async fn add_artist(&self, artists: Vec<Artist>) -> Vec<ArtistObject> {
        let mut cache_w = self.cache.write().await;

        artists
            .into_iter()
            .map(|s| {
                cache_w.get(&s.id).cloned().unwrap_or_else(|| {
                    let s1 = ArtistObject::new(s);
                    cache_w.insert(s1.id().clone(), s1.clone());
                    s1
                })
            })
            .collect()
    }

    pub async fn search(&self, query: &str, count: u32, offset: Option<u32>) -> Result<Vec<ArtistObject>, Box<dyn Error>> {
        let res = self.client.search3(query, Some(count), offset, Some(0), None, Some(0), None, None).await?;
        if let Some(artists) = res.artist {
            Ok(self.add_artist(artists).await)
        } else {
            Err("No artists found".into())
        }
    }
}
