use super::persist::PersistError;
use crate::beatleader::player::PlayerId;
use crate::bot::beatleader::Player as BotPlayer;
use crate::bot::db::PlayerLink;
use crate::storage::persist::{CachedStorage, ShuttleStorage};
use shuttle_persist::PersistInstance;

struct PlayerRepository<'a> {
    linked_players: CachedStorage<'a, PlayerLink, ()>,
    bl_players: CachedStorage<'a, PlayerId, BotPlayer>,
}

impl<'a> PlayerRepository<'a> {
    pub async fn new(persist: &'a PersistInstance) -> Result<PlayerRepository<'a>, PersistError> {
        Ok(Self {
            linked_players: CachedStorage::new(ShuttleStorage::new("linked-players", persist))
                .await?,
            bl_players: CachedStorage::new(ShuttleStorage::new("guild-settings", persist)).await?,
        })
    }
}
