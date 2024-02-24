use std::borrow::Cow;
use std::collections::HashMap;
use std::sync::Arc;

use poise::serenity_prelude::{http, AttachmentType, GuildId, SerenityError};
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, info, warn};

use crate::discord::bot::beatleader::player::Player;
use crate::discord::bot::commands::player::get_player_embed;
use crate::discord::bot::{GuildSettings, UserRoleChanges};
use crate::discord::{serenity, BotData};
use crate::storage::guild::GuildSettingsRepository;
use crate::storage::player::PlayerRepository;

pub struct UserRolesWorker {
    context: serenity::Context,
    guild_settings_repository: Arc<GuildSettingsRepository>,
    players_repository: Arc<PlayerRepository>,
    token: CancellationToken,
}

impl UserRolesWorker {
    pub fn new(context: serenity::Context, data: BotData, token: CancellationToken) -> Self {
        Self {
            context,
            guild_settings_repository: data.guild_settings_repository,
            players_repository: data.players_repository,
            token,
        }
    }

    pub async fn run(&self, bot_players: Vec<Player>) {
        info!("Updating players roles ({})...", bot_players.len());

        let mut current_players_roles = Vec::new();
        for bot_player in bot_players {
            debug!(
                "Fetching user {} ({}) roles...",
                &bot_player.user_id, &bot_player.name
            );

            let mut guilds_to_unlink = vec![];
            for guild_id in &bot_player.linked_guilds {
                // TODO: do not get user roles if guild does not have automatic roles enabled
                let member = match self
                    .context
                    .http
                    .get_member(u64::from(*guild_id), bot_player.user_id.into())
                    .await
                {
                    Ok(member) => member,
                    Err(err) => {
                        error!(
                            "Can not fetch user {} membership in {} guild due to an error: {:?}.",
                            bot_player.user_id, &guild_id, err
                        );

                        match err {
                            SerenityError::Http(http_err) => {
                                match *http_err {
                                    http::HttpError::UnsuccessfulRequest(
                                        http::error::ErrorResponse {
                                            error: discord_err, ..
                                        },
                                    ) => {
                                        // see: https://discord.com/developers/docs/topics/opcodes-and-status-codes#json
                                        if discord_err.code == 10007 {
                                            debug!("User {} ({}) is not a member of the guild {} anymore.", &bot_player.user_id, &bot_player.name, &guild_id);
                                            guilds_to_unlink.push(u64::from(*guild_id));
                                        }

                                        continue;
                                    }
                                    _ => continue,
                                }
                            }
                            _ => continue,
                        }
                    }
                };

                current_players_roles.push((*guild_id, bot_player.clone(), member.roles));

                if self.token.is_cancelled() {
                    warn!("User roles task is shutting down...");
                    return;
                }
            }

            if !guilds_to_unlink.is_empty() {
                info!(
                    "Unlinking user {} ({}) from guilds {:?}...",
                    &bot_player.user_id, &bot_player.name, &guilds_to_unlink
                );

                let _ = self
                    .players_repository
                    .unlink_guilds(&bot_player.user_id, guilds_to_unlink)
                    .await;
            }
        }

        let mut guild_ids = current_players_roles
            .iter()
            .map(|(guild_id, _player, _roles)| *guild_id)
            .collect::<Vec<GuildId>>();
        guild_ids.sort_unstable();
        guild_ids.dedup();

        let mut guilds: HashMap<GuildId, GuildSettings> = HashMap::new();

        for guild_id in &guild_ids {
            if let Ok(guild_settings) = self.guild_settings_repository.get(guild_id).await {
                guilds.insert(*guild_id, guild_settings);
            }
        }

        let role_changes = current_players_roles
            .iter()
            .filter_map(|(guild_id, player, roles)| {
                guilds
                    .get(guild_id)
                    .map(|guild_settings| guild_settings.get_role_updates(*guild_id, player, roles))
            })
            .collect::<Vec<UserRoleChanges>>();

        for rc in role_changes {
            match rc.apply(&self.context.http).await {
                Ok(rc) => {
                    if rc.is_changed() {
                        if let Some(bot_channel_id) = guilds
                            .get(&rc.guild_id)
                            .map_or_else(|| None, |guild_settings| guild_settings.get_channel())
                        {
                            info!("Logging changes to channel #{}", bot_channel_id);

                            match self.players_repository.get(&rc.user_id).await {
                                Some(player) => {
                                    let embed_image = get_player_embed(&player).await;

                                    match bot_channel_id
                                        .send_message(self.context.clone(), |m| {
                                            if let Some(embed_buffer) = embed_image {
                                                m.add_file(AttachmentType::Bytes {
                                                    data: Cow::<[u8]>::from(embed_buffer),
                                                    filename: "embed.png".to_string(),
                                                });
                                            }

                                            m.content(format!("{}", rc))
                                                .allowed_mentions(|am| am.empty_parse())
                                        })
                                        .await
                                    {
                                        Ok(_) => {}
                                        Err(err) => {
                                            info!(
                                                "Can not post log update to channel #{}: {}",
                                                bot_channel_id, err
                                            );
                                        }
                                    };
                                }
                                None => {
                                    match bot_channel_id
                                        .send_message(self.context.clone(), |m| {
                                            m.content(format!("{}", rc))
                                                .allowed_mentions(|am| am.empty_parse())
                                        })
                                        .await
                                    {
                                        Ok(_) => {}
                                        Err(err) => {
                                            info!(
                                                "Can not post log update to channel #{}: {}",
                                                bot_channel_id, err
                                            );
                                        }
                                    };
                                }
                            }
                        }
                    }
                }
                Err(e) => {
                    error!("Failed to update roles for user {}: {}", rc.user_id, e);
                }
            }

            if self.token.is_cancelled() {
                warn!("User roles task is shutting down...");
                return;
            }
        }

        info!("Players roles updated.");
    }
}
