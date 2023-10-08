use log::trace;
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use shuttle_persist::PersistInstance;

use crate::beatleader::oauth::OAuthToken;
use crate::beatleader::player::PlayerId;
use crate::storage::persist::{CachedStorage, ShuttleStorage};
use crate::storage::{PersistError, StorageKey, StorageValue};

use super::Result;

#[derive(Serialize, Default, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
#[serde(default)]
pub(crate) struct PlayerOAuthToken {
    player_id: PlayerId,
    #[serde(flatten)]
    oauth_token: OAuthToken,
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

    pub(crate) async fn set(
        &self,
        player_id: &PlayerId,
        oauth_token: OAuthToken,
    ) -> Result<PlayerOAuthToken> {
        trace!("Setting OAuth token for player {}...", &player_id);

        let player_id_clone = player_id.clone();
        let oauth_token_clone = oauth_token.clone();

        if let Some(token) = self
            .storage
            .get_and_modify_or_insert(
                player_id,
                |token| {
                    // TODO: check if oauth_token is newer than existing and store it only in that case
                    token.oauth_token = oauth_token
                },
                || Some(PlayerOAuthToken::new(player_id_clone, oauth_token_clone)),
            )
            .await?
        {
            trace!("OAuth token for player {} set.", player_id);

            Ok(token)
        } else {
            Err(PersistError::NotFound(
                "player is not registered".to_string(),
            ))
        }
    }

    pub(crate) async fn restore(&self, values: Vec<PlayerOAuthToken>) -> Result<()> {
        self.storage.restore(values).await
    }
}
