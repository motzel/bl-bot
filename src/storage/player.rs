use super::persist::PersistError;
use crate::beatleader::player::PlayerId;
use crate::bot::beatleader::Player as BotPlayer;
use crate::bot::db::PlayerLink;
use crate::storage::persist::{CachedStorage, ShuttleStorage};
use shuttle_persist::PersistInstance;

struct PlayerRepository {
    linked_players: CachedStorage<PlayerLink, ()>,
    bl_players: CachedStorage<PlayerId, BotPlayer>,
}

impl PlayerRepository {
    pub async fn new(persist: PersistInstance) -> Result<Self, PersistError> {
        Ok(Self {
            linked_players: CachedStorage::new(ShuttleStorage::new(
                "linked-players",
                persist.clone(),
            ))
            .await?,
            bl_players: CachedStorage::new(ShuttleStorage::new("guild-settings", persist)).await?,
        })
    }
}
