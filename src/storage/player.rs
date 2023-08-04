use poise::serenity_prelude::UserId;
use shuttle_persist::PersistInstance;

use crate::beatleader::player::PlayerId;
use crate::bot::beatleader::Player as BotPlayer;
use crate::storage::persist::{CachedStorage, ShuttleStorage};

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

    pub(crate) async fn link(user_id: UserId, player_id: PlayerId) -> Result<BotPlayer> {
        // 1. fetch player from bl
        // 2. store player in db & update index
        // 3. return player

        todo!()
    }

    pub(crate) async fn unlink(user_id: UserId) -> Result<()> {
        // 1. check if player exists, return error if not
        // 2. remove player and update index

        todo!()
    }
}
