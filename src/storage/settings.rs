use super::persist::PersistError;
use crate::bot::GuildSettings;
use crate::storage::persist::{CachedStorage, ShuttleStorage};
use poise::serenity_prelude::GuildId;
use shuttle_persist::PersistInstance;

struct SettingsRepository {
    storage: CachedStorage<GuildId, GuildSettings>,
}

impl SettingsRepository {
    pub async fn new(persist: PersistInstance) -> Result<Self, PersistError> {
        Ok(Self {
            storage: CachedStorage::new(ShuttleStorage::new("guild-settings", persist)).await?,
        })
    }
}
