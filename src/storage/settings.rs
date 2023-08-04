use poise::serenity_prelude::{ChannelId, GuildId, RoleId};
use shuttle_persist::PersistInstance;

use crate::bot::{GuildSettings, MetricCondition, PlayerMetricWithValue, RoleGroup};
use crate::storage::persist::{CachedStorage, ShuttleStorage};

use super::Result;

struct SettingsRepository<'a> {
    storage: CachedStorage<'a, GuildId, GuildSettings>,
}

impl<'a> SettingsRepository<'a> {
    pub async fn new(persist: &'a PersistInstance) -> Result<SettingsRepository<'a>> {
        Ok(Self {
            storage: CachedStorage::new(ShuttleStorage::new("guild-settings", persist)).await?,
        })
    }

    pub async fn get(&self, guild_id: &GuildId) -> Result<Option<GuildSettings>> {
        self.storage.get(guild_id).await
    }

    pub async fn set_bot_channel(
        guild_id: GuildId,
        channel_id: Option<ChannelId>,
    ) -> Result<GuildSettings> {
        // Warning: 1-3 should take common write lock -> refactor to get-and-modify

        // 1. get guild settings, return error is not exists
        // 2. set new channel
        // 3. store
        // 4. return settings clone

        todo!()
    }

    pub async fn add_auto_role(
        guild_id: GuildId,
        role_group: RoleGroup,
        role_id: RoleId,
        metric_and_value: PlayerMetricWithValue,
        condition: MetricCondition,
        weight: u32,
    ) -> Result<GuildSettings> {
        // Warning: 2-4 should take common write lock -> refactor to get-and-modify

        // 1. create role settings and add condition
        // 2. get guild settings, return error is not exists
        // 3. merge role settings
        // 4. store
        // 5. return settings clone

        todo!()
    }

    pub async fn remove_auto_role(
        guild_id: GuildId,
        role_group: RoleGroup,
        role_id: RoleId,
    ) -> Result<GuildSettings> {
        // Warning: 1-3 should take common write lock -> refactor to get-and-modify

        // 1. get guild settings, return error is not exists
        // 2. remove role settings
        // 3. store
        // 4. return settings clone

        todo!()
    }
}
