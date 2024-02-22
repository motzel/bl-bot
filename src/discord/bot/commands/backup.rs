use poise::serenity_prelude::{Attachment, AttachmentType};
use serde::{Deserialize, Serialize};
use std::borrow::Cow;

use crate::discord::bot::beatleader::Player as BotPlayer;
use crate::discord::bot::GuildSettings;
use crate::discord::Context;
use crate::storage::player_oauth_token::PlayerOAuthToken;
use crate::Error;

#[derive(Serialize, Deserialize, Default, Debug, Clone)]
#[serde(rename_all = "camelCase")]
struct BotData {
    version: String,
    guilds: Vec<GuildSettings>,
    players: Vec<BotPlayer>,
    player_oauth_tokens: Vec<PlayerOAuthToken>,
}

/// Export bot data
#[poise::command(
    slash_command,
    rename = "bl-export",
    ephemeral,
    required_permissions = "MANAGE_ROLES",
    default_member_permissions = "MANAGE_ROLES",
    required_bot_permissions = "MANAGE_ROLES",
    guild_only
)]
pub(crate) async fn cmd_export(ctx: Context<'_>) -> Result<(), Error> {
    let is_bot_owner = ctx.framework().options().owners.contains(&ctx.author().id);
    if !is_bot_owner {
        ctx.say("Can only be used by bot owner").await?;
        return Ok(());
    }

    ctx.defer_ephemeral().await?;

    let data = BotData {
        version: env!("CARGO_PKG_VERSION").to_owned(),
        guilds: ctx.data().guild_settings_repository.all().await,
        players: ctx.data().players_repository.all().await,
        player_oauth_tokens: ctx.data().player_oauth_token_repository.all().await,
    };

    match serde_json::to_string::<BotData>(&data) {
        Ok(data_json) => {
            ctx.send(|f| {
                f.content("Requested backup:")
                    .attachment(AttachmentType::Bytes {
                        data: Cow::from(data_json.into_bytes()),
                        filename: "bl-bot-backup.json".to_owned(),
                    })
                    .ephemeral(true)
            })
            .await?;
        }
        Err(err) => {
            ctx.say(format!("An error occurred: {}", err)).await?;
        }
    };

    Ok(())
}

/// Import bot data
#[poise::command(
    slash_command,
    rename = "bl-import",
    ephemeral,
    required_permissions = "MANAGE_ROLES",
    default_member_permissions = "MANAGE_ROLES",
    required_bot_permissions = "MANAGE_ROLES",
    guild_only
)]
pub(crate) async fn cmd_import(
    ctx: Context<'_>,
    #[description = "bl-bot-backup.json"] backup_json: Attachment,
) -> Result<(), Error> {
    let is_bot_owner = ctx.framework().options().owners.contains(&ctx.author().id);
    if !is_bot_owner {
        ctx.say("Can only be used by bot owner").await?;
        return Ok(());
    }

    ctx.defer_ephemeral().await?;

    match backup_json.download().await {
        Ok(bytes) => match String::from_utf8(bytes) {
            Ok(json) => match serde_json::from_str::<BotData>(json.as_str()) {
                Ok(data) => {
                    if let Err(err) = ctx
                        .data()
                        .guild_settings_repository
                        .restore(data.guilds)
                        .await
                    {
                        ctx.say(format!(
                            "An error occurred during restoring guild settings: {}",
                            err
                        ))
                        .await?;

                        return Ok(());
                    }

                    if let Err(err) = ctx.data().players_repository.restore(data.players).await {
                        ctx.say(format!(
                            "An error occurred during restoring linked players: {}",
                            err
                        ))
                        .await?;

                        return Ok(());
                    }

                    if let Err(err) = ctx
                        .data()
                        .player_oauth_token_repository
                        .restore(data.player_oauth_tokens)
                        .await
                    {
                        ctx.say(format!(
                            "An error occurred during restoring oauth tokens: {}",
                            err
                        ))
                        .await?;

                        return Ok(());
                    }
                }
                Err(err) => {
                    ctx.say(format!(
                        "An error occurred during deserializing attachment: {}",
                        err
                    ))
                    .await?;

                    return Ok(());
                }
            },
            Err(err) => {
                ctx.say(format!(
                    "An error occurred during converting attachment to utf8: {}",
                    err
                ))
                .await?;

                return Ok(());
            }
        },
        Err(err) => {
            ctx.say(format!(
                "An error occurred during downloading attachment: {}",
                err
            ))
            .await?;

            return Ok(());
        }
    };

    ctx.say("Data successfully restored.").await?;

    Ok(())
}
