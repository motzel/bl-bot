use futures::future::BoxFuture;
use log::info;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;

use shuttle_persist::PersistInstance;

use crate::beatleader::oauth::OAuthToken;
use crate::beatleader::player::PlayerId;
use crate::storage::persist::{CachedStorage, ShuttleStorage};
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

pub(crate) struct PlayerOAuthTokenRepository {
    storage: CachedStorage<PlayerId, PlayerOAuthToken>,
}

impl<'a> PlayerOAuthTokenRepository {
    pub(crate) async fn new(persist: Arc<PersistInstance>) -> Result<PlayerOAuthTokenRepository> {
        Ok(Self {
            storage: CachedStorage::new(ShuttleStorage::new("player-oauth-token", persist)).await?,
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
        info!("PlayerOauthToken::set()");
        let mut write_lock = self.storage.write_lock().await;

        if let Some(token_mutex) = write_lock.get(player_id) {
            info!("PlayerOauthToken::set() WRITE_LOCK GET!");
            let token_mutex_guard = &mut token_mutex.lock().await;

            info!("PlayerOauthToken, right before modify");
            modify_func(token_mutex_guard).await;
            info!(
                "PlayerOauthToken::set() MODIFIED {}!",
                &token_mutex_guard.clone().oauth_token.expiration_date
            );

            return self
                .storage
                .save(player_id.clone(), token_mutex_guard.clone())
                .await;
        }

        info!("PlayerOauthToken::set() DEFAULT VALUE");
        let value = PlayerOAuthToken::default();
        let value_clone;

        info!("PlayerOauthToken, before mutex");
        let value_mutex = Mutex::new(value);
        {
            let mut value_mutex_guard = value_mutex.lock().await;

            info!("PlayerOauthToken, right before modify");
            modify_func(&mut value_mutex_guard).await;
            info!("PlayerOauthToken, right after modify");

            value_clone = value_mutex_guard.clone();

            info!(
                "PlayerOauthToken, after modify: {}",
                &value_mutex_guard.oauth_token.expiration_date
            );
        }
        info!("PlayerOauthToken, after mutex");

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
