use std::sync::Arc;

use log::{debug, trace};
use poise::serenity_prelude::{GuildId, UserId};
use shuttle_persist::PersistInstance;

use crate::beatleader::player::{Player as BlPlayer, PlayerId};
use crate::bot::beatleader::Player as BotPlayer;
use crate::storage::persist::{CachedStorage, PersistError, ShuttleStorage};
use crate::BL_CLIENT;

use super::Result;

pub(crate) struct PlayerRepository {
    storage: CachedStorage<UserId, BotPlayer>,
}

impl<'a> PlayerRepository {
    pub(crate) async fn new(persist: Arc<PersistInstance>) -> Result<PlayerRepository> {
        Ok(Self {
            storage: CachedStorage::new(ShuttleStorage::new("players", persist)).await?,
        })
    }

    pub(crate) async fn len(&self) -> usize {
        self.storage.len().await
    }

    pub(crate) async fn get(&self, user_id: &UserId) -> Option<BotPlayer> {
        self.storage.get(user_id).await
    }

    pub(crate) async fn link(
        &self,
        guild_id: GuildId,
        user_id: UserId,
        player_id: PlayerId,
        requires_verification: bool,
    ) -> Result<BotPlayer> {
        debug!("Linking user {} with BL player {}...", user_id, player_id);

        let bl_player = PlayerRepository::fetch_player_from_bl(&player_id).await?;

        if requires_verification
            && !bl_player
                .socials
                .iter()
                .any(|social| social.service == "Discord" && social.user_id == user_id.to_string())
        {
            return Err(PersistError::ProfileNotVerified);
        }

        let bl_player_clone = bl_player.clone();

        trace!(
            "BL player {} fetched. Player name: {}",
            bl_player.id,
            bl_player.name
        );

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
                    );
                },
                || {
                    Some(BotPlayer::from_user_id_and_bl_player(
                        user_id,
                        vec![guild_id],
                        bl_player_clone,
                    ))
                },
            )
            .await?
        {
            Some(player) => {
                debug!("User {} linked with BL player {}.", user_id, player_id);
                Ok(player)
            }
            None => Err(PersistError::Unknown),
        }
    }

    pub(crate) async fn unlink(&self, guild_id: &GuildId, user_id: &UserId) -> Result<()> {
        debug!("Unlinking user {} from guild {}...", user_id, guild_id);

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

                    Err(PersistError::NotFound("user is not linked".to_owned()))
                }
            }
            None => {
                debug!("User {} is not linked to guild {}.", user_id, guild_id);

                Err(PersistError::NotFound("user is not linked".to_owned()))
            }
        }
    }

    pub(crate) async fn update_all_players_stats(&self) -> Result<Vec<BotPlayer>> {
        debug!("Updating all users stats...");

        let mut ret = Vec::with_capacity(self.storage.len().await);

        for player in self.storage.values().await {
            if !player.is_linked_to_any_guild() {
                debug!(
                    "User {} / BL player {} is not linked to any guild, skipped.",
                    player.user_id, player.id
                );

                continue;
            }

            if let Ok(player) = self.update_player_stats(&player).await {
                ret.push(player);
            }
        }

        debug!("All users stats updated.");

        Ok(ret)
    }

    pub(crate) async fn update_player_stats(&self, player: &BotPlayer) -> Result<BotPlayer> {
        debug!(
            "Updating user {} / BL player {} stats...",
            player.user_id, player.name
        );

        // do not update if not linked in any guild
        if !player.is_linked_to_any_guild() {
            return Ok(player.clone());
        }

        let bl_player = PlayerRepository::fetch_player_from_bl(&player.id).await?;

        trace!(
            "BL player {} fetched. Player name: {}",
            bl_player.id,
            bl_player.name
        );

        match self
            .storage
            .get_and_modify_or_insert(
                &player.user_id,
                move |player| {
                    **player = BotPlayer::from_user_id_and_bl_player(
                        player.user_id,
                        player.linked_guilds.clone(),
                        bl_player,
                    );
                },
                || None,
            )
            .await?
        {
            None => {
                debug!("User {} not found.", player.user_id);

                Err(PersistError::NotFound("player not found".to_owned()))
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

    pub(crate) async fn fetch_player_from_bl(player_id: &PlayerId) -> Result<BlPlayer> {
        match BL_CLIENT.player().get_by_id(player_id).await {
            Ok(player) => Ok(player),
            Err(e) => Err(PersistError::BlApi(e)),
        }
    }
}
