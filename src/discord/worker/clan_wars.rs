use crate::beatleader::oauth::OAuthAppCredentials;
use crate::discord::bot::beatleader::clan::{ClanWars, ClanWarsSort};
use crate::discord::{serenity, BotData};
use crate::storage::guild::GuildSettingsRepository;
use crate::storage::player_oauth_token::PlayerOAuthTokenRepository;
use chrono::Utc;
use poise::serenity_prelude::ChannelId;
use std::cmp::Ordering;
use std::sync::Arc;
use tokio_util::sync::CancellationToken;
use tracing::{error, info};

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

                    info!(
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
                        match self
                            .guild_settings_repository
                            .set_clan_wars_posted_at(&guild.get_key(), Utc::now())
                            .await
                        {
                            Ok(_) => {
                                info!(
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

                                        info!(
                                            "{} clan wars maps found. Posting maps to channel #{}",
                                            clan_wars.maps.len(),
                                            clan_wars_channel_id
                                        );

                                        async fn post_msg(
                                            global_ctx: &serenity::Context,
                                            channel_id: ChannelId,
                                            description: &str,
                                            content: &str,
                                        ) {
                                            match channel_id
                                                .send_message(global_ctx.clone(), |m| {
                                                    m.embed(|e| e.description(description))
                                                        .allowed_mentions(|am| am.empty_parse());

                                                    if !content.is_empty() {
                                                        m.content(content);
                                                    }

                                                    m
                                                })
                                                .await
                                            {
                                                Ok(_) => {}
                                                Err(err) => {
                                                    info!("Can not post clan wars map to channel #{}: {}", channel_id, err);
                                                }
                                            };
                                        }

                                        const MAX_DISCORD_MSG_LENGTH: usize = 2000;
                                        let mut msg_count = 0;
                                        let header = format!(
                                            "### **{} clan wars maps** (<t:{}:R>)",
                                            clan_settings.get_clan(),
                                            Utc::now().timestamp()
                                        );
                                        let mut description = String::new();
                                        for map in clan_wars.maps.iter() {
                                            let map_description = map.to_string();

                                            if description.len()
                                                + "\n\n".len()
                                                + map_description.len()
                                                + (if msg_count > 0 { 0 } else { header.len() })
                                                < MAX_DISCORD_MSG_LENGTH
                                            {
                                                description.push_str(&map_description);
                                            } else {
                                                post_msg(
                                                    &self.context,
                                                    clan_wars_channel_id,
                                                    description.as_str(),
                                                    if msg_count == 0 {
                                                        header.as_str()
                                                    } else {
                                                        ""
                                                    },
                                                )
                                                .await;

                                                description = String::new();
                                                msg_count += 1;
                                            }
                                        }

                                        if !description.is_empty() {
                                            post_msg(
                                                &self.context,
                                                clan_wars_channel_id,
                                                description.as_str(),
                                                if msg_count == 0 { header.as_str() } else { "" },
                                            )
                                            .await;
                                        }
                                    }
                                    Err(err) => {
                                        error!("Can not fetch clan wars map list: {:?}", err);
                                    }
                                }

                                info!(
                                    "Clan wars maps for a clan {} refreshed and posted.",
                                    clan_settings.get_clan()
                                );
                            }
                            Err(err) => {
                                error!(
                                    "Can not set clan wars posted time for clan {}: {:?}",
                                    clan_settings.get_clan(),
                                    err
                                );
                            }
                        }
                    } else {
                        info!(
                            "Clan {} wars maps do not require posting yet.",
                            clan_settings.get_clan()
                        );
                    }
                }
            }
        }
    }
}
