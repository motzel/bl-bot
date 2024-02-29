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
                            .le(&(Utc::now() - chrono::Duration::minutes(60)))
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
                                            thread_name: &str,
                                        ) -> Result<ChannelId, poise::serenity_prelude::Error>
                                        {
                                            let mut message = CreateMessage::new()
                                                .allowed_mentions(CreateAllowedMentions::new());

                                            if !content.is_empty() {
                                                message = message.content(content);
                                            }

                                            if !description.is_empty() {
                                                message = message.embed(
                                                    CreateEmbed::new().description(description),
                                                );
                                            }

                                            match channel_id
                                                .send_message(global_ctx.clone(), message)
                                                .await
                                            {
                                                Ok(msg) => {
                                                    //
                                                    if !thread_name.is_empty() {
                                                        return match channel_id
                                                            .create_thread_from_message(
                                                                global_ctx.clone(),
                                                                msg.id,
                                                                CreateThread::new(thread_name)
                                                                    .auto_archive_duration(AutoArchiveDuration::OneHour)
                                                                    .kind(ChannelType::PublicThread),
                                                            )
                                                            .await {
                                                            Ok(guild_channel) => Ok(guild_channel.id),
                                                            Err(_) => Ok(msg.channel_id)
                                                        };
                                                    }

                                                    Ok(msg.channel_id)
                                                }
                                                Err(err) => {
                                                    info!("Can not post clan wars map to channel #{}: {}", channel_id, err);
                                                    Err(err)
                                                }
                                            }
                                        }

                                        // create new thread if possible
                                        let header = format!(
                                            "### **{} clan wars maps** (<t:{}:R>)",
                                            clan_settings.get_clan(),
                                            Utc::now().timestamp()
                                        );
                                        let thread_name =
                                            format!("{} clan wars maps", clan_settings.get_clan(),);
                                        let channel_id = if let Ok(new_channel_id) = post_msg(
                                            &self.context,
                                            clan_wars_channel_id,
                                            "",
                                            header.as_str(),
                                            thread_name.as_str(),
                                        )
                                        .await
                                        {
                                            new_channel_id
                                        } else {
                                            clan_wars_channel_id
                                        };

                                        const MAX_DISCORD_MSG_LENGTH: usize = 2000;

                                        let mut description = String::new();
                                        let clan_wars_len = clan_wars.maps.len();
                                        for (idx, map) in clan_wars.maps.iter().enumerate() {
                                            let map_description = map.to_string();

                                            if description.len()
                                                + "\n\n".len()
                                                + map_description.len()
                                                + (if idx > 0 { 0 } else { header.len() })
                                                >= MAX_DISCORD_MSG_LENGTH
                                                || idx == clan_wars_len - 1
                                            {
                                                description.push_str(&map_description);

                                                let _ = post_msg(
                                                    &self.context,
                                                    channel_id,
                                                    description.as_str(),
                                                    if idx == 0 { header.as_str() } else { "" },
                                                    thread_name.as_str(),
                                                )
                                                .await;

                                                description = String::new();
                                            } else {
                                                description.push_str(&map_description);
                                            }
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
