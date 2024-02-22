use std::sync::Arc;

use crate::discord::bot::commands::playlist::{Playlist, PlaylistId};
use crate::storage::persist::PersistInstance;
use crate::storage::{CachedStorage, Storage, StorageValue};

use super::Result;

impl StorageValue<PlaylistId> for Playlist {
    fn get_key(&self) -> PlaylistId {
        self.id.clone()
    }
}

pub(crate) struct PlaylistRepository {
    storage: CachedStorage<PlaylistId, Playlist>,
}

impl<'a> PlaylistRepository {
    pub(crate) async fn new(persist: Arc<PersistInstance>) -> Result<Self> {
        Ok(Self {
            storage: CachedStorage::new(Storage::new("playlists", persist)).await?,
        })
    }

    pub(crate) async fn all(&self) -> Vec<Playlist> {
        self.storage.values().await
    }

    pub(crate) async fn len(&self) -> usize {
        self.storage.len().await
    }

    pub(crate) async fn get(&self, playlist_id: &PlaylistId) -> Option<Playlist> {
        self.storage.get(playlist_id).await
    }

    pub(crate) async fn save(&self, playlist: Playlist) -> Result<Playlist> {
        let playlist = self.storage.set(&playlist.id.clone(), playlist).await?;

        self.storage.update_index().await?;

        Ok(playlist)
    }
    pub(crate) async fn restore(&self, values: Vec<Playlist>) -> Result<()> {
        self.storage.restore(values).await
    }
}
