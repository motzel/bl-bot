#![allow(clippy::blocks_in_conditions)]

use std::fmt::{Display, Formatter};
use std::sync::Arc;

use poise::serenity_prelude::{GuildId, UserId};
use serde::{Deserialize, Serialize};
use tokio_util::sync::CancellationToken;
use tracing::{debug, trace};

use crate::beatleader::player::{Player as BlPlayer, PlayerId};
use crate::discord::bot::beatleader::player::Player as BotPlayer;
use crate::discord::bot::beatleader::player::{fetch_player_from_bl, Player};
use crate::discord::bot::beatleader::score::fetch_ranked_scores_stats;
use crate::storage::persist::PersistInstance;
use crate::storage::player_scores::PlayerScoresRepository;
use crate::storage::{CachedStorage, Storage, StorageError, StorageKey, StorageValue};

use super::Result;

#[derive(Debug)]
pub(crate) struct PlayerRepository {
    storage: CachedStorage<UserId, BotPlayer>,
    user_player_idx_repository: PlayerUserIdxRepository,
}

impl<'a> PlayerRepository {
    pub(crate) async fn new(persist: Arc<PersistInstance>) -> Result<PlayerRepository> {
        let storage = CachedStorage::new(Storage::new("players", persist.clone())).await?;
        let user_player_idx_repository = PlayerUserIdxRepository::new(persist).await?;

        // refresh index at start if needed
        if storage.len().await != user_player_idx_repository.len().await {
            let idx_values = user_player_idx_repository.all().await;
            let users_to_refresh = storage
                .keys()
                .await
                .into_iter()
                .filter(|user_id| !idx_values.iter().any(|idx| idx.user_id == *user_id))
                .collect::<Vec<_>>();

            tracing::debug!(
                "Refreshing player-user index ({} items)...",
                users_to_refresh.len()
            );

            for user_id in users_to_refresh {
                if let Some(Player { id, .. }) = storage.get(&user_id).await {
                    let _ = user_player_idx_repository.set(id, user_id).await?;
                }
            }

            tracing::debug!("player-user index refreshed.");
        }

        Ok(Self {
            storage,
            user_player_idx_repository,
        })
    }

    pub(crate) async fn all(&self) -> Vec<BotPlayer> {
        self.storage.values().await
    }

    pub(crate) async fn len(&self) -> usize {
        self.storage.len().await
    }

    pub(crate) async fn get(&self, user_id: &UserId) -> Option<BotPlayer> {
        match self.storage.get(user_id).await {
            None => None,
            Some(player) => {
                let _ = self
                    .user_player_idx_repository
                    .set(player.id.clone(), player.user_id)
                    .await;

                Some(player)
            }
        }
    }

    pub(crate) async fn get_by_player_id(&self, player_id: &PlayerId) -> Option<BotPlayer> {
        match self.user_player_idx_repository.get(player_id).await {
            None => None,
            Some(user_id) => self.storage.get(&user_id).await,
        }
    }

    pub(crate) async fn link(
        &self,
        guild_id: GuildId,
        user_id: UserId,
        player_id: PlayerId,
        requires_verification: bool,
    ) -> Result<BotPlayer> {
        trace!("Linking user {} with BL player {}...", user_id, player_id);

        let bl_player = fetch_player_from_bl(&player_id).await?;

        self.link_player(guild_id, user_id, bl_player, requires_verification)
            .await
    }

    pub(crate) async fn link_guild(&self, user_id: &UserId, guild_id: GuildId) -> Result<()> {
        trace!("Linking user {} to guild {}...", user_id, &guild_id);

        let mut existed = false;
        let existed_ref = &mut existed;

        match self
            .storage
            .get_and_modify_or_insert(
                user_id,
                move |player| {
                    player.linked_guilds.retain(|g| *g != u64::from(guild_id));
                    player.linked_guilds.push(guild_id);

                    *existed_ref = true;
                },
                || None,
            )
            .await?
        {
            Some(_) => {
                if existed {
                    debug!("User {} linked to the guild {}.", user_id, &guild_id);

                    Ok(())
                } else {
                    debug!("User {} does not exists", user_id);

                    Err(StorageError::NotFound("user does not exists".to_owned()))
                }
            }
            None => {
                debug!("User {} does not exists.", user_id);

                Err(StorageError::NotFound("user does not exists".to_owned()))
            }
        }
    }

    pub(crate) async fn unlink(&self, guild_id: &GuildId, user_id: &UserId) -> Result<()> {
        trace!("Unlinking user {} from guild {}...", user_id, guild_id);

        let mut existed = false;
        let existed_ref = &mut existed;

        match self
            .storage
            .get_and_modify_or_insert(
                user_id,
                move |player| {
                    let prev_len = player.linked_guilds.len();

                    player.linked_guilds.retain(|g| *g != u64::from(*guild_id));

                    *existed_ref = player.linked_guilds.len() < prev_len;
                },
                || None,
            )
            .await?
        {
            Some(_) => {
                if existed {
                    debug!("User {} unlinked from guild {}.", user_id, guild_id);

                    Ok(())
                } else {
                    debug!("User {} is not linked to guild {}.", user_id, guild_id);

                    Err(StorageError::NotFound("user is not linked".to_owned()))
                }
            }
            None => {
                debug!("User {} is not linked to guild {}.", user_id, guild_id);

                Err(StorageError::NotFound("user is not linked".to_owned()))
            }
        }
    }

    pub(crate) async fn unlink_guilds(
        &self,
        user_id: &UserId,
        guilds_to_unlink: Vec<u64>,
    ) -> Result<()> {
        if guilds_to_unlink.is_empty() {
            return Ok(());
        }

        trace!(
            "Unlinking user {} from guilds {:?}...",
            user_id,
            &guilds_to_unlink
        );

        let mut existed = false;
        let existed_ref = &mut existed;
        let guilds_to_unlink_clone = guilds_to_unlink.clone();

        match self
            .storage
            .get_and_modify_or_insert(
                user_id,
                move |player| {
                    let prev_len = player.linked_guilds.len();

                    player
                        .linked_guilds
                        .retain(|g| !guilds_to_unlink_clone.contains(&u64::from(*g)));

                    *existed_ref = player.linked_guilds.len() < prev_len;
                },
                || None,
            )
            .await?
        {
            Some(_) => {
                if existed {
                    debug!(
                        "User {} unlinked from guilds {:?}.",
                        user_id, &guilds_to_unlink
                    );

                    Ok(())
                } else {
                    debug!(
                        "User {} is not linked to any of the passed guilds {:?}.",
                        user_id, &guilds_to_unlink
                    );

                    Err(StorageError::NotFound(
                        "user is not linked to any of the passed guilds".to_owned(),
                    ))
                }
            }
            None => {
                debug!(
                    "User {} is not linked to any of the passed guilds {:?}.",
                    user_id, &guilds_to_unlink
                );

                Err(StorageError::NotFound(
                    "user is not linked to any of the passed guilds".to_owned(),
                ))
            }
        }
    }

    pub(crate) async fn update_all_players_stats(
        &self,
        player_scores_repository: &Arc<PlayerScoresRepository>,
        force_scores_download: bool,
        token: Option<CancellationToken>,
    ) -> Result<Vec<BotPlayer>> {
        trace!("Updating all users stats...");

        let mut ret = Vec::with_capacity(self.storage.len().await);

        for player in self.storage.values().await {
            if !player.is_linked_to_any_guild() {
                trace!(
                    "User {} / BL player {} is not linked to any guild, skipped.",
                    player.user_id,
                    player.id
                );

                continue;
            }

            if let Ok(player) = self
                .update_player_stats(player_scores_repository, &player, force_scores_download)
                .await
            {
                ret.push(player);
            }

            if token.is_some() && token.as_ref().unwrap().is_cancelled() {
                return Err(StorageError::Cancelled);
            }
        }

        debug!("All users stats updated.");

        Ok(ret)
    }

    pub(crate) async fn update_player_stats(
        &self,
        player_scores_repository: &Arc<PlayerScoresRepository>,
        player: &BotPlayer,
        force_scores_download: bool,
    ) -> Result<BotPlayer> {
        trace!(
            "Updating user {} / BL player {} stats...",
            player.user_id,
            player.name
        );

        // do not update if not linked in any guild
        if !player.is_linked_to_any_guild() {
            return Ok(player.clone());
        }

        let bl_player = fetch_player_from_bl(&player.id).await?;

        trace!(
            "BL player {} fetched. Player name: {}",
            bl_player.id,
            bl_player.name
        );

        let scores_stats =
            fetch_ranked_scores_stats(player_scores_repository, player, force_scores_download)
                .await?;

        match self
            .storage
            .get_and_modify_or_insert(
                &player.user_id,
                move |player| {
                    if let Some(score_stats) = scores_stats {
                        player.last_scores_fetch = Some(score_stats.last_scores_fetch);
                        player.plus_1pp = score_stats.plus_1pp;
                        player.last_ranked_paused_at = score_stats.last_ranked_paused_at;
                        player.top_stars = score_stats.top_stars;
                    }

                    **player = BotPlayer::from_user_id_and_bl_player(
                        player.user_id,
                        player.linked_guilds.clone(),
                        bl_player,
                        Some(player),
                    );
                },
                || None,
            )
            .await?
        {
            None => {
                debug!("User {} not found.", player.user_id);

                Err(StorageError::NotFound("player not found".to_owned()))
            }
            Some(player) => {
                debug!(
                    "User {} / BL player {} stats updated.",
                    player.user_id, player.name
                );

                Ok(player)
            }
        }
    }

    pub(crate) async fn restore(&self, values: Vec<BotPlayer>) -> Result<()> {
        self.storage.restore(values).await
    }

    pub(crate) async fn link_player(
        &self,
        guild_id: GuildId,
        user_id: UserId,
        bl_player: BlPlayer,
        requires_verification: bool,
    ) -> Result<BotPlayer> {
        if requires_verification
            && !bl_player
                .socials
                .iter()
                .any(|social| social.service == "Discord" && social.user_id == user_id.to_string())
        {
            return Err(StorageError::ProfileNotVerified);
        }

        let bl_player_clone = bl_player.clone();

        trace!(
            "BL player {} fetched. Player name: {}",
            bl_player.id,
            bl_player.name
        );

        let player_id = bl_player.id.clone();
        let player_id_clone = bl_player.id.clone();

        match self
            .storage
            .get_and_modify_or_insert(
                &user_id,
                move |player| {
                    player.linked_guilds.retain(|g| *g != u64::from(guild_id));
                    player.linked_guilds.push(guild_id);

                    **player = BotPlayer::from_user_id_and_bl_player(
                        user_id,
                        player.linked_guilds.clone(),
                        bl_player,
                        if player.id == player_id_clone {
                            Some(player)
                        } else {
                            None
                        },
                    );
                },
                || {
                    Some(BotPlayer::from_user_id_and_bl_player(
                        user_id,
                        vec![guild_id],
                        bl_player_clone,
                        None,
                    ))
                },
            )
            .await?
        {
            Some(player) => {
                debug!("User {} linked with BL player {}.", user_id, player_id);

                self.user_player_idx_repository
                    .set(player.id.clone(), user_id)
                    .await?;

                Ok(player)
            }
            None => Err(StorageError::Unknown),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Hash, Eq, PartialEq)]
pub(crate) struct PlayerUserIdx {
    pub player_id: PlayerId,
    pub user_id: UserId,
}

impl StorageKey for PlayerUserIdx {}
impl StorageValue<PlayerId> for PlayerUserIdx {
    fn get_key(&self) -> PlayerId {
        self.player_id.clone()
    }
}

impl Display for PlayerUserIdx {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}-{}", self.player_id.as_str(), self.user_id)
    }
}

#[derive(Debug)]
pub(crate) struct PlayerUserIdxRepository {
    storage: CachedStorage<PlayerId, PlayerUserIdx>,
}

impl<'a> PlayerUserIdxRepository {
    pub(crate) async fn new(persist: Arc<PersistInstance>) -> Result<PlayerUserIdxRepository> {
        Ok(Self {
            storage: CachedStorage::new(Storage::new("player-user-idx", persist)).await?,
        })
    }

    pub(crate) async fn get(&self, player_id: &PlayerId) -> Option<UserId> {
        match self.storage.get(player_id).await {
            None => None,
            Some(player_user_idx) => Some(player_user_idx.user_id),
        }
    }

    pub(crate) async fn set(&self, player_id: PlayerId, user_id: UserId) -> Result<PlayerUserIdx> {
        self.storage
            .set(
                &player_id,
                PlayerUserIdx {
                    player_id: player_id.clone(),
                    user_id,
                },
            )
            .await
    }

    pub(crate) async fn all(&self) -> Vec<PlayerUserIdx> {
        self.storage.values().await
    }

    pub(crate) async fn len(&self) -> usize {
        self.storage.len().await
    }
}
