use futures::future::BoxFuture;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;

use crate::storage::persist::PersistInstance;

use crate::beatleader::oauth::OAuthToken;
use crate::beatleader::player::PlayerId;
use crate::storage::{CachedStorage, Storage};
use crate::storage::{StorageKey, StorageValue};

use super::Result;

#[derive(Serialize, Default, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
#[serde(default)]
pub(crate) struct PlayerOAuthToken {
    pub player_id: PlayerId,
    #[serde(flatten)]
    pub oauth_token: OAuthToken,
}

impl PlayerOAuthToken {
    pub fn new(player_id: PlayerId, oauth_token: OAuthToken) -> Self {
        Self {
            player_id,
            oauth_token,
        }
    }
}

impl From<PlayerOAuthToken> for OAuthToken {
    fn from(value: PlayerOAuthToken) -> Self {
        value.oauth_token.clone()
    }
}

impl StorageKey for PlayerId {}
impl StorageValue<PlayerId> for PlayerOAuthToken {
    fn get_key(&self) -> PlayerId {
        self.player_id.clone()
    }
}

#[derive(Debug)]
pub(crate) struct PlayerOAuthTokenRepository {
    storage: CachedStorage<PlayerId, PlayerOAuthToken>,
}

impl PlayerOAuthTokenRepository {
    pub(crate) async fn new(persist: Arc<PersistInstance>) -> Result<PlayerOAuthTokenRepository> {
        Ok(Self {
            storage: CachedStorage::new(Storage::new("player-oauth-token", persist)).await?,
        })
    }

    pub(crate) async fn all(&self) -> Vec<PlayerOAuthToken> {
        self.storage.values().await
    }

    pub(crate) async fn len(&self) -> usize {
        self.storage.len().await
    }

    pub(crate) async fn get(&self, player_id: &PlayerId) -> Option<PlayerOAuthToken> {
        self.storage.get(player_id).await
    }

    pub(crate) async fn set<ModifyFunc>(
        &self,
        player_id: &PlayerId,
        modify_func: ModifyFunc,
    ) -> Result<PlayerOAuthToken>
    where
        ModifyFunc: for<'b> FnOnce(&'b mut PlayerOAuthToken) -> BoxFuture<'b, ()>,
    {
        let mut write_lock = self.storage.write_lock().await;

        if let Some(token_mutex) = write_lock.get(player_id) {
            let token_mutex_guard = &mut token_mutex.lock().await;

            modify_func(token_mutex_guard).await;

            token_mutex_guard.player_id.clone_from(player_id);

            return self
                .storage
                .save(player_id.clone(), token_mutex_guard.clone())
                .await;
        }

        let value = PlayerOAuthToken::default();
        let value_clone;

        let value_mutex = Mutex::new(value);
        {
            let mut value_mutex_guard = value_mutex.lock().await;

            modify_func(&mut value_mutex_guard).await;

            value_mutex_guard.player_id.clone_from(player_id);

            value_clone = value_mutex_guard.clone();
        }

        write_lock.insert(player_id.clone(), value_mutex);

        drop(write_lock);

        let value = self.storage.save(player_id.clone(), value_clone).await?;

        self.storage.update_index().await?;

        Ok(value)
    }

    pub(crate) async fn restore(&self, values: Vec<PlayerOAuthToken>) -> Result<()> {
        self.storage.restore(values).await
    }
}
