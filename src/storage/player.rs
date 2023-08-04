use super::persist::PersistError;
use crate::bot::beatleader::Player as BotPlayer;
use crate::storage::persist::{CachedStorage, ShuttleStorage};
use poise::serenity_prelude::UserId;
use shuttle_persist::PersistInstance;

struct PlayerRepository<'a> {
    storage: CachedStorage<'a, UserId, BotPlayer>,
}

impl<'a> PlayerRepository<'a> {
    pub async fn new(persist: &'a PersistInstance) -> Result<PlayerRepository<'a>, PersistError> {
        Ok(Self {
            storage: CachedStorage::new(ShuttleStorage::new("players", persist)).await?,
        })
    }
}
