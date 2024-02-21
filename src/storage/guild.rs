use chrono::{DateTime, Utc};
use std::sync::Arc;

use crate::storage::persist::PersistInstance;
use log::{debug, trace};
use poise::serenity_prelude::{ChannelId, GuildId, RoleId};
use tokio::sync::MutexGuard;

use crate::discord::bot::{
    ClanSettings, Condition, GuildSettings, RequirementMetricValue, RoleGroup, RoleSettings,
};
use crate::storage::{CachedStorage, Storage, StorageError};

use super::Result;

pub(crate) struct GuildSettingsRepository {
    storage: CachedStorage<GuildId, GuildSettings>,
}

impl<'a> GuildSettingsRepository {
    pub(crate) async fn new(persist: Arc<PersistInstance>) -> Result<GuildSettingsRepository> {
        Ok(Self {
            storage: CachedStorage::new(Storage::new("guild-settings", persist)).await?,
        })
    }

    pub(crate) async fn all(&self) -> Vec<GuildSettings> {
        self.storage.values().await
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
                    None => Err(StorageError::Unknown),
                }
            }
        }
    }

    pub(crate) async fn set_bot_channel(
        &self,
        guild_id: &GuildId,
        channel_id: Option<ChannelId>,
    ) -> Result<GuildSettings> {
        trace!("Setting bot channel for guild {}...", guild_id);

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
            Err(StorageError::NotFound(
                "guild is not registered".to_string(),
            ))
        }
    }

    pub(crate) async fn set_clan_wars_maps_channel(
        &self,
        guild_id: &GuildId,
        channel_id: Option<ChannelId>,
    ) -> Result<GuildSettings> {
        trace!("Setting clan wars maps channel for guild {}...", guild_id);

        if let Some(guild_settings) = self
            .storage
            .get_and_modify_or_insert(
                guild_id,
                |guild_settings| guild_settings.set_clan_wars_maps_channel(channel_id),
                || {
                    let mut guild_settings = GuildSettings::new(*guild_id);
                    guild_settings.set_clan_wars_maps_channel(channel_id);

                    Some(guild_settings)
                },
            )
            .await?
        {
            debug!("Clan wars maps channel for guild {} set.", guild_id);

            Ok(guild_settings)
        } else {
            Err(StorageError::NotFound(
                "guild is not registered".to_string(),
            ))
        }
    }

    pub async fn set_clan_wars_posted_at(
        &self,
        guild_id: &GuildId,
        posted_at: DateTime<Utc>,
    ) -> Result<GuildSettings> {
        trace!(
            "Setting clan wars maps posted time for guild {}...",
            guild_id
        );

        if let Some(guild_settings) = self
            .storage
            .get_and_modify_or_insert(
                guild_id,
                |guild_settings| guild_settings.set_clan_wars_posted_at(posted_at),
                || {
                    let mut guild_settings = GuildSettings::new(*guild_id);
                    guild_settings.set_clan_wars_posted_at(posted_at);

                    Some(guild_settings)
                },
            )
            .await?
        {
            debug!("Clan wars maps posted time for guild {} set.", guild_id);

            Ok(guild_settings)
        } else {
            Err(StorageError::NotFound(
                "guild is not registered".to_string(),
            ))
        }
    }

    pub(crate) async fn set_verified_profile_requirement(
        &self,
        guild_id: &GuildId,
        requires_verified_profile: bool,
    ) -> Result<GuildSettings> {
        trace!(
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
            Err(StorageError::NotFound(
                "guild is not registered".to_string(),
            ))
        }
    }

    pub(crate) async fn set_clan_settings(
        &self,
        guild_id: &GuildId,
        clan_settings: Option<ClanSettings>,
    ) -> Result<GuildSettings> {
        trace!("Setting clan settings for guild {}...", guild_id);

        let clan_settings_clone = clan_settings.clone();
        if let Some(guild_settings) = self
            .storage
            .get_and_modify_or_insert(
                guild_id,
                |guild_settings| guild_settings.set_clan_settings(clan_settings),
                || {
                    let mut guild_settings = GuildSettings::new(*guild_id);
                    guild_settings.set_clan_settings(clan_settings_clone);

                    Some(guild_settings)
                },
            )
            .await?
        {
            debug!("Clan settings for guild {} set.", guild_id);

            Ok(guild_settings)
        } else {
            Err(StorageError::NotFound(
                "guild is not registered".to_string(),
            ))
        }
    }

    pub(crate) async fn set_oauth_token<ModifyFunc>(
        &self,
        guild_id: &GuildId,
        modify_func: ModifyFunc,
        oauth_token: bool,
    ) -> Result<GuildSettings>
    where
        ModifyFunc: FnOnce(&mut MutexGuard<GuildSettings>),
    {
        trace!("Setting OAuth token for guild {}...", guild_id);

        if let Some(guild_settings) = self
            .storage
            .get_and_modify_or_insert(guild_id, modify_func, || {
                let mut guild_settings = GuildSettings::new(*guild_id);

                guild_settings.set_oauth_token(oauth_token);

                Some(guild_settings)
            })
            .await?
        {
            debug!("OAuth token for guild {} set.", guild_id);

            Ok(guild_settings)
        } else {
            Err(StorageError::NotFound(
                "guild is not registered".to_string(),
            ))
        }
    }

    pub(crate) async fn add_auto_role(
        &self,
        guild_id: GuildId,
        role_group: RoleGroup,
        role_id: RoleId,
        metric_and_value: RequirementMetricValue,
        condition: Condition,
        weight: u32,
    ) -> Result<GuildSettings> {
        trace!("Adding auto role for guild {}...", guild_id);

        let mut rs = RoleSettings::new(role_id, weight);
        rs.add_requirement(condition, metric_and_value);

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
            Err(StorageError::NotFound(
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
        trace!("Removing auto role for guild {}...", guild_id);

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
            Err(StorageError::NotFound(
                "guild is not registered".to_string(),
            ))
        }
    }

    pub(crate) async fn restore(&self, values: Vec<GuildSettings>) -> Result<()> {
        self.storage.restore(values).await
    }
}
