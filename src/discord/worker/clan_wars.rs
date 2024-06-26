use std::cmp::Ordering;
use std::collections::HashMap;
use std::sync::Arc;

use chrono::Utc;
use poise::serenity_prelude::{
    AutoArchiveDuration, ChannelId, ChannelType, CreateAllowedMentions, CreateEmbed, CreateMessage,
    CreateThread, UserId,
};
use tokio_util::sync::CancellationToken;

use crate::beatleader::oauth::OAuthAppCredentials;
use crate::beatleader::player::PlayerId;
use crate::discord::bot::beatleader::clan::{ClanWars, ClanWarsSort};
use crate::discord::bot::ClanSettings;
use crate::discord::{serenity, BotData};
use crate::storage::bsmaps::BsMapsRepository;
use crate::storage::guild::GuildSettingsRepository;
use crate::storage::player::PlayerRepository;
use crate::storage::player_oauth_token::PlayerOAuthTokenRepository;
use crate::storage::player_scores::PlayerScoresRepository;

pub struct BlClanWarsMapsWorker {
    context: serenity::Context,
    guild_settings_repository: Arc<GuildSettingsRepository>,
    player_repository: Arc<PlayerRepository>,
    player_scores_repository: Arc<PlayerScoresRepository>,
    player_oauth_token_repository: Arc<PlayerOAuthTokenRepository>,
    maps_repository: Arc<BsMapsRepository>,
    oauth_credentials: Option<OAuthAppCredentials>,
    refresh_interval: chrono::Duration,
    token: CancellationToken,
    count: u16,
}

impl BlClanWarsMapsWorker {
    pub fn new(
        context: serenity::Context,
        data: BotData,
        refresh_interval: chrono::Duration,
        token: CancellationToken,
    ) -> Self {
        let oauth_credentials = data.oauth_credentials();

        Self {
            context,
            guild_settings_repository: data.guild_settings_repository,
            player_repository: data.players_repository,
            player_scores_repository: data.player_scores_repository,
            player_oauth_token_repository: data.player_oauth_token_repository,
            maps_repository: data.maps_repository,
            oauth_credentials,
            refresh_interval,
            token,
            count: data.settings.clan_wars_maps_count,
        }
    }

    pub async fn run(&self) {
        for guild in self.guild_settings_repository.all().await {
            if let Some(clan_settings) = guild.get_clan_settings() {
                if let Some(clan_wars_channel_id) = clan_settings.get_clan_wars_maps_channel() {
                    let last_posted_at = clan_settings.get_clan_wars_posted_at();

                    tracing::info!(
                        "Refreshing clan {} wars maps, last posted at: {}...",
                        clan_settings.get_clan(),
                        if last_posted_at.is_some() {
                            format!("{}", last_posted_at.unwrap())
                        } else {
                            "never".to_owned()
                        }
                    );

                    if last_posted_at.is_none()
                        || last_posted_at
                            .unwrap()
                            .le(&(Utc::now() - self.refresh_interval))
                    {
                        tracing::debug!("Fetching maps removed from the map list...");

                        let skip_leaderboard_ids = match self
                            .maps_repository
                            .map_list_bans(&clan_settings.get_clan())
                            .await
                        {
                            Ok(maps) => Some(
                                maps.into_iter()
                                    .map(|m| m.leaderboard_id)
                                    .collect::<Vec<_>>(),
                            ),
                            Err(_) => None,
                        };

                        let skip_leaderboards_count =
                            skip_leaderboard_ids.as_ref().map(|v| v.len()).unwrap_or(0);

                        tracing::debug!(
                            "{} maps removed from the map list found.",
                            skip_leaderboards_count
                        );

                        match self
                            .guild_settings_repository
                            .set_clan_wars_posted_at(&guild.get_key(), Utc::now())
                            .await
                        {
                            Ok(_) => {
                                tracing::info!(
                                    "{} clan wars maps posted time set.",
                                    clan_settings.get_clan()
                                );

                                let soldiers = self.get_clan_soldiers(&clan_settings).await;

                                match ClanWars::fetch(
                                    clan_settings.get_clan(),
                                    ClanWarsSort::ToConquer,
                                    Some(self.count.into()),
                                    false,
                                    skip_leaderboard_ids,
                                )
                                .await
                                {
                                    Ok(mut clan_wars) if !clan_wars.maps.is_empty() => {
                                        clan_wars.maps.sort_unstable_by(|a, b| {
                                            a.pp_boundary
                                                .partial_cmp(&b.pp_boundary)
                                                .unwrap_or(Ordering::Equal)
                                        });

                                        tracing::info!(
                                            "{} clan wars maps found. Posting maps to channel #{}",
                                            clan_wars.maps.len(),
                                            clan_wars_channel_id
                                        );

                                        async fn post_msg(
                                            global_ctx: &serenity::Context,
                                            channel_id: ChannelId,
                                            description: &str,
                                        ) -> Result<ChannelId, poise::serenity_prelude::Error>
                                        {
                                            let message = CreateMessage::new()
                                                .embed(CreateEmbed::new().description(description))
                                                .allowed_mentions(CreateAllowedMentions::new());

                                            match channel_id
                                                .send_message(global_ctx.clone(), message)
                                                .await
                                            {
                                                Ok(msg) => Ok(msg.channel_id),
                                                Err(err) => Err(err),
                                            }
                                        }

                                        // create new thread if possible
                                        tracing::debug!(
                                            "Creating clan wars maps thread for the clan {}...",
                                            clan_settings.get_clan()
                                        );

                                        let channel_id = match clan_wars_channel_id
                                            .create_thread(
                                                &self.context,
                                                CreateThread::new(format!(
                                                    "{} clan wars maps",
                                                    clan_settings.get_clan(),
                                                ))
                                                .auto_archive_duration(AutoArchiveDuration::OneHour)
                                                .kind(ChannelType::PublicThread),
                                            )
                                            .await
                                        {
                                            Ok(guild_channel) => {
                                                tracing::debug!("Clan wars maps thread for the {} clan created.", clan_settings.get_clan());

                                                guild_channel.id
                                            }
                                            Err(err) => {
                                                tracing::error!("Can not create clan wars maps thread for the {} clan on channel #{}: {}", clan_settings.get_clan(),clan_wars_channel_id, err);

                                                clan_wars_channel_id
                                            }
                                        };

                                        for map in clan_wars.maps.iter() {
                                            let map_description = map.to_string();

                                            let not_played_by_soldiers = soldiers
                                                .iter()
                                                .filter_map(|(player_id, user_id)| {
                                                    if map
                                                        .scores
                                                        .iter()
                                                        .any(|s| &s.player_id == player_id)
                                                    {
                                                        None
                                                    } else {
                                                        Some(format!("<@{}>", user_id))
                                                    }
                                                })
                                                .collect::<Vec<_>>();

                                            let description = if !not_played_by_soldiers.is_empty()
                                            {
                                                format!(
                                                    "{}\nMap not played by: {}",
                                                    map_description,
                                                    not_played_by_soldiers.join(" | ")
                                                )
                                            } else {
                                                map_description
                                            };

                                            if let Err(err) = post_msg(
                                                &self.context,
                                                channel_id,
                                                description.as_str(),
                                            )
                                            .await
                                            {
                                                tracing::error!(
                                                    "Can not post clan wars map message to the channel #{}: {}",
                                                    channel_id,
                                                    err
                                                );
                                            }
                                        }
                                    }
                                    Ok(_) => {
                                        tracing::warn!("No clan wars maps found",);
                                    }
                                    Err(err) => {
                                        tracing::error!(
                                            "Can not fetch clan wars map list: {:?}",
                                            err
                                        );
                                    }
                                }

                                tracing::info!(
                                    "Clan wars maps for a clan {} refreshed and posted.",
                                    clan_settings.get_clan()
                                );
                            }
                            Err(err) => {
                                tracing::error!(
                                    "Can not set clan wars posted time for clan {}: {:?}",
                                    clan_settings.get_clan(),
                                    err
                                );
                            }
                        }
                    } else {
                        tracing::info!(
                            "Clan {} wars maps do not require posting yet.",
                            clan_settings.get_clan()
                        );
                    }
                }
            }
        }
    }

    pub(crate) async fn get_clan_soldiers(
        &self,
        clan_settings: &ClanSettings,
    ) -> HashMap<PlayerId, UserId> {
        let mut soldiers = HashMap::<PlayerId, UserId>::new();
        for user_id in clan_settings.get_clan_wars_soldiers().iter() {
            match self.player_repository.get(user_id).await {
                Some(player) => {
                    if player.is_primary_clan_member(&clan_settings.get_clan()) {
                        soldiers.insert(player.id, *user_id);
                    }
                }
                None => {
                    tracing::warn!("Can not get player for user @{}", user_id)
                }
            };
        }
        soldiers
    }
}
