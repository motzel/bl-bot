use std::sync::Arc;

use log::{debug, trace, warn};
use poise::serenity_prelude::UserId;
use serde::{Deserialize, Serialize};

use crate::beatleader::player::PlayerId;
use crate::beatleader::BlContext;
use crate::bot::beatleader::{fetch_all_player_scores, Player, Score};
use crate::file_storage::PersistInstance;
use crate::storage::persist::ShuttleStorage;

use super::{Result, StorageValue};

#[derive(Serialize, Deserialize, Default, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct PlayerScores {
    pub user_id: UserId,
    pub player_id: PlayerId,
    pub bl_context: BlContext,
    pub scores: Vec<Score>,
}

impl StorageValue<PlayerId> for PlayerScores {
    fn get_key(&self) -> PlayerId {
        self.player_id.clone()
    }
}

pub(crate) struct PlayerScoresRepository {
    storage: ShuttleStorage<PlayerId, PlayerScores>,
    pub bl_context: BlContext,
}

impl<'a> PlayerScoresRepository {
    pub(crate) async fn new(
        persist: Arc<PersistInstance>,
        bl_context: BlContext,
    ) -> Result<PlayerScoresRepository> {
        Ok(Self {
            storage: ShuttleStorage::new(
                format!("player-scores-{}", bl_context.to_string()).as_str(),
                persist,
            ),
            bl_context,
        })
    }

    pub(crate) async fn get(&self, player_id: &PlayerId) -> Option<PlayerScores> {
        self.storage.load(player_id).await.ok()
    }

    pub(crate) async fn update_player_scores(
        &self,
        player: &Player,
        force_scores_download: bool,
    ) -> Result<Option<PlayerScores>> {
        trace!(
            "Updating user {} / BL player {} scores...",
            player.user_id,
            player.name
        );

        // do not update if not linked in any guild
        if !player.is_linked_to_any_guild() {
            return Ok(None);
        }

        let mut force_scores_download = force_scores_download;
        if !force_scores_download {
            let current_player_scores = self.get(&player.id).await;
            if current_player_scores.is_none() {
                force_scores_download = true;
            }
        }

        // do not update if fetching is skipped
        let player_scores =
            fetch_all_player_scores(player, self.bl_context.clone(), force_scores_download).await?;
        if player_scores.is_none() {
            return Ok(None);
        }

        let player_scores = player_scores.unwrap();

        match self
            .storage
            .save(
                player.id.clone(),
                PlayerScores {
                    user_id: player.user_id,
                    player_id: player.id.clone(),
                    bl_context: self.bl_context.clone(),
                    scores: player_scores,
                },
            )
            .await
        {
            Ok(scores) => {
                debug!(
                    "User {} / BL player {} scores updated.",
                    player.user_id, player.name
                );

                Ok(Some(scores))
            }
            Err(err) => {
                warn!("Error occurred: {}", err);

                Err(err)
            }
        }
    }
}
