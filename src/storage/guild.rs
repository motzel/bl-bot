use std::sync::Arc;

use log::debug;
use poise::serenity_prelude::{ChannelId, GuildId, RoleId};
use shuttle_persist::PersistInstance;

use crate::bot::{GuildSettings, MetricCondition, PlayerMetricWithValue, RoleGroup, RoleSettings};
use crate::storage::persist::{CachedStorage, PersistError, ShuttleStorage};

use super::Result;

pub(crate) struct GuildSettingsRepository {
    storage: CachedStorage<GuildId, GuildSettings>,
}

impl<'a> GuildSettingsRepository {
    pub(crate) async fn new(persist: Arc<PersistInstance>) -> Result<GuildSettingsRepository> {
        Ok(Self {
            storage: CachedStorage::new(ShuttleStorage::new("guild-settings", persist)).await?,
        })
    }

    pub(crate) async fn len(&self) -> usize {
        self.storage.len().await
    }

    pub(crate) async fn get(&self, guild_id: &GuildId) -> Result<GuildSettings> {
        match self.storage.get(guild_id).await {
            Some(guild_settings) => Ok(guild_settings),
            None => {
                match self
                    .storage
                    .get_and_modify_or_insert(
                        guild_id,
                        |_| {},
                        || Some(GuildSettings::new(*guild_id)),
                    )
                    .await?
                {
                    Some(guild_settings) => Ok(guild_settings),
                    None => Err(PersistError::Unknown),
                }
            }
        }
    }

    pub(crate) async fn set_bot_channel(
        &self,
        guild_id: &GuildId,
        channel_id: Option<ChannelId>,
    ) -> Result<GuildSettings> {
        debug!("Setting bot channel for guild {}...", guild_id);

        if let Some(guild_settings) = self
            .storage
            .get_and_modify_or_insert(
                guild_id,
                |guild_settings| guild_settings.set_channel(channel_id),
                || {
                    let mut guild_settings = GuildSettings::new(*guild_id);
                    guild_settings.set_channel(channel_id);

                    Some(guild_settings)
                },
            )
            .await?
        {
            debug!("Bot channel for guild {} set.", guild_id);

            Ok(guild_settings)
        } else {
            Err(PersistError::NotFound(
                "guild is not registered".to_string(),
            ))
        }
    }

    pub(crate) async fn set_verified_profile_requirement(
        &self,
        guild_id: &GuildId,
        requires_verified_profile: bool,
    ) -> Result<GuildSettings> {
        debug!(
            "Setting verified profile requirement for guild {}...",
            guild_id
        );

        if let Some(guild_settings) = self
            .storage
            .get_and_modify_or_insert(
                guild_id,
                |guild_settings| {
                    guild_settings.set_verified_profile_requirement(requires_verified_profile)
                },
                || {
                    let mut guild_settings = GuildSettings::new(*guild_id);
                    guild_settings.set_verified_profile_requirement(requires_verified_profile);

                    Some(guild_settings)
                },
            )
            .await?
        {
            debug!("Verified profile requirement for guild {} set.", guild_id);

            Ok(guild_settings)
        } else {
            Err(PersistError::NotFound(
                "guild is not registered".to_string(),
            ))
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
        debug!("Adding auto role for guild {}...", guild_id);

        let mut rs = RoleSettings::new(role_id, weight);
        rs.add_condition(condition, metric_and_value);

        let role_group_clone = role_group.clone();
        let role_settings_clone = rs.clone();

        if let Some(guild_settings) = self
            .storage
            .get_and_modify_or_insert(
                &guild_id,
                move |guild_settings| {
                    guild_settings.merge(role_group, rs);
                },
                || {
                    let mut guild_settings = GuildSettings::new(guild_id);
                    guild_settings.merge(role_group_clone, role_settings_clone);

                    Some(guild_settings)
                },
            )
            .await?
        {
            debug!("Auto role for guild {} added.", guild_id);

            Ok(guild_settings)
        } else {
            Err(PersistError::NotFound(
                "guild is not registered".to_string(),
            ))
        }
    }

    pub(crate) async fn remove_auto_role(
        &self,
        guild_id: GuildId,
        role_group: RoleGroup,
        role_id: RoleId,
    ) -> Result<GuildSettings> {
        debug!("Removing auto role for guild {}...", guild_id);

        if let Some(guild_settings) = self
            .storage
            .get_and_modify_or_insert(
                &guild_id,
                move |guild_settings| {
                    guild_settings.remove(role_group, role_id);
                },
                || Some(GuildSettings::new(guild_id)),
            )
            .await?
        {
            debug!("Auto role for guild {} removed.", guild_id);

            Ok(guild_settings)
        } else {
            Err(PersistError::NotFound(
                "guild is not registered".to_string(),
            ))
        }
    }
}
