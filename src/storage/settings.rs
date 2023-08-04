use poise::serenity_prelude::{ChannelId, GuildId, RoleId};
use shuttle_persist::PersistInstance;

use crate::bot::{GuildSettings, MetricCondition, PlayerMetricWithValue, RoleGroup, RoleSettings};
use crate::storage::persist::{CachedStorage, PersistError, ShuttleStorage};

use super::Result;

struct SettingsRepository<'a> {
    storage: CachedStorage<'a, GuildId, GuildSettings>,
}

impl<'a> SettingsRepository<'a> {
    pub(crate) async fn new(persist: &'a PersistInstance) -> Result<SettingsRepository<'a>> {
        Ok(Self {
            storage: CachedStorage::new(ShuttleStorage::new("guild-settings", persist)).await?,
        })
    }

    pub(crate) async fn get(&self, guild_id: &GuildId) -> Option<GuildSettings> {
        self.storage.get(guild_id).await
    }

    pub(crate) async fn set_bot_channel(
        &self,
        guild_id: GuildId,
        channel_id: Option<ChannelId>,
    ) -> Result<GuildSettings> {
        if let Some(guild_settings) = self
            .storage
            .get_and_modify_or_insert(
                &guild_id,
                |guild_settings| guild_settings.set_channel(channel_id),
                || None,
            )
            .await?
        {
            Ok(guild_settings)
        } else {
            Err(PersistError::NotFound("guild not registered".to_string()))
        }
    }

    pub(crate) async fn add_auto_role(
        &self,
        guild_id: GuildId,
        role_group: RoleGroup,
        role_id: RoleId,
        metric_and_value: PlayerMetricWithValue,
        condition: MetricCondition,
        weight: u32,
    ) -> Result<GuildSettings> {
        let mut rs = RoleSettings::new(role_id, weight);
        rs.add_condition(condition, metric_and_value);

        if let Some(guild_settings) = self
            .storage
            .get_and_modify_or_insert(
                &guild_id,
                move |guild_settings| {
                    guild_settings.merge(role_group, rs);
                },
                || None,
            )
            .await?
        {
            Ok(guild_settings)
        } else {
            Err(PersistError::NotFound("guild not registered".to_string()))
        }
    }

    pub(crate) async fn remove_auto_role(
        &self,
        guild_id: GuildId,
        role_group: RoleGroup,
        role_id: RoleId,
    ) -> Result<GuildSettings> {
        if let Some(guild_settings) = self
            .storage
            .get_and_modify_or_insert(
                &guild_id,
                move |guild_settings| {
                    guild_settings.remove(role_group, role_id);
                },
                || None,
            )
            .await?
        {
            Ok(guild_settings)
        } else {
            Err(PersistError::NotFound("guild not registered".to_string()))
        }
    }
}
