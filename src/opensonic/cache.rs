use crate::opensonic::client::OpenSubsonicClient;
use crate::opensonic::types::{AlbumListType, Song};
use crate::ui::album_object::AlbumObject;
use std::collections::HashMap;
use std::error::Error;
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Clone, Debug)]
pub struct SongCache {
    cache: Arc<RwLock<HashMap<String, Arc<Song>>>>,
    client: Arc<OpenSubsonicClient>,
}

impl SongCache {
    pub fn new(client: Arc<OpenSubsonicClient>) -> Self {
        Self {
            client,
            cache: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn get_song(&self, id: &str) -> Result<Arc<Song>, Box<dyn Error + Send + Sync>> {
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

    pub async fn add_songs(&self, songs: Vec<Song>) -> Vec<Arc<Song>> {
        let mut cache_w = self.cache.write().await;

        songs
            .into_iter()
            .map(|s| {
                cache_w.get(&s.id).cloned().unwrap_or_else(|| {
                    let s1 = Arc::new(s);
                    cache_w.insert(s1.id.clone(), s1.clone());
                    s1
                })
            })
            .collect()
    }
}

#[derive(Clone, Debug)]
pub struct AlbumCache {
    cache: Arc<RwLock<HashMap<String, AlbumObject>>>,
    client: Arc<OpenSubsonicClient>,
    song_cache: SongCache,
}

impl AlbumCache {
    pub fn new(client: Arc<OpenSubsonicClient>, song_cache: SongCache) -> Self {
        Self {
            client,
            cache: Arc::new(RwLock::new(HashMap::new())),
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
    ) -> Result<Vec<AlbumObject>, Box<dyn Error + Send + Sync>> {
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

    pub async fn get_album(&self, id: &str) -> Result<AlbumObject, Box<dyn Error + Send + Sync>> {
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
}
