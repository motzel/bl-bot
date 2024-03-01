use crate::beatleader::oauth::OAuthAppCredentials;
use crate::discord::bot::beatleader::clan::{ClanWars, ClanWarsSort};
use crate::discord::{serenity, BotData};
use crate::storage::guild::GuildSettingsRepository;
use crate::storage::player_oauth_token::PlayerOAuthTokenRepository;
use chrono::Utc;
use poise::serenity_prelude::{
    AutoArchiveDuration, ChannelId, ChannelType, CreateAllowedMentions, CreateEmbed, CreateMessage,
    CreateThread,
};
use std::cmp::Ordering;
use std::sync::Arc;
use tokio_util::sync::CancellationToken;

pub struct BlClanWarsMapsWorker {
    context: serenity::Context,
    guild_settings_repository: Arc<GuildSettingsRepository>,
    player_oauth_token_repository: Arc<PlayerOAuthTokenRepository>,
    oauth_credentials: Option<OAuthAppCredentials>,
    refresh_interval: chrono::Duration,
    token: CancellationToken,
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
            player_oauth_token_repository: data.player_oauth_token_repository,
            oauth_credentials,
            refresh_interval,
            token,
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
                            .le(&(Utc::now() - chrono::Duration::minutes(60)))
                    {
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

                                match ClanWars::fetch(
                                    clan_settings.get_clan(),
                                    ClanWarsSort::ToConquer,
                                    Some(30),
                                )
                                .await
                                {
                                    Ok(mut clan_wars) => {
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

                                        const MAX_DISCORD_MSG_LENGTH: usize = 2000;

                                        for map in clan_wars.maps.iter() {
                                            let map_description = map.to_string();

                                            if let Err(err) = post_msg(
                                                &self.context,
                                                channel_id,
                                                map_description.as_str(),
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
}
