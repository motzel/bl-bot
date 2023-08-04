use log::debug;
use poise::serenity_prelude::UserId;
use shuttle_persist::PersistInstance;

use crate::beatleader::player::PlayerId;
use crate::bot::beatleader::Player as BotPlayer;
use crate::storage::persist::{CachedStorage, PersistError, ShuttleStorage};
use crate::BL_CLIENT;

use super::Result;

struct PlayerRepository<'a> {
    storage: CachedStorage<'a, UserId, BotPlayer>,
}

impl<'a> PlayerRepository<'a> {
    pub(crate) async fn new(persist: &'a PersistInstance) -> Result<PlayerRepository<'a>> {
        Ok(Self {
            storage: CachedStorage::new(ShuttleStorage::new("players", persist)).await?,
        })
    }

    pub(crate) async fn get(&self, user_id: &UserId) -> Option<BotPlayer> {
        self.storage.get(user_id).await
    }

    pub(crate) async fn link(&self, user_id: UserId, player_id: PlayerId) -> Result<BotPlayer> {
        debug!("Linking user {} with BL player {}...", user_id, player_id);

        let bl_player = match BL_CLIENT.player().get_by_id(&player_id).await {
            Ok(player) => BotPlayer::from_user_id_and_bl_player(user_id, player),
            Err(e) => return Err(PersistError::BlApi(e)),
        };

        debug!(
            "BL player {} fetched. Player name: {}",
            bl_player.id, bl_player.name
        );

        let result = self.storage.set(&user_id, bl_player).await;

        debug!("User {} linked with BL player {}.", user_id, player_id);

        result
    }

    pub(crate) async fn unlink(&self, user_id: UserId) -> Result<()> {
        debug!("Unlinking user {}...", user_id);

        match self.storage.remove(&user_id).await {
            Ok(existed) => {
                if existed {
                    debug!("User {} unlinked.", user_id);

                    Ok(())
                } else {
                    debug!("User {} is not linked.", user_id);

                    Err(PersistError::NotFound("user is not linked".to_owned()))
                }
            }
            Err(e) => Err(e),
        }
    }
}
