use super::persist::PersistError;
use crate::bot::GuildSettings;
use crate::storage::persist::{CachedStorage, ShuttleStorage};
use poise::serenity_prelude::GuildId;
use shuttle_persist::PersistInstance;

struct SettingsRepository<'a> {
    storage: CachedStorage<'a, GuildId, GuildSettings>,
}

impl<'a> SettingsRepository<'a> {
    pub async fn new(persist: &'a PersistInstance) -> Result<SettingsRepository<'a>, PersistError> {
        Ok(Self {
            storage: CachedStorage::new(ShuttleStorage::new("guild-settings", persist)).await?,
        })
    }
}
