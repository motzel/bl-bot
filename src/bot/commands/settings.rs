use crate::bot::db::update_bot_log_channel;
use crate::{Context, Error};
use log::info;
use poise::serenity_prelude::ChannelId;

/// Display current bot settings
#[poise::command(slash_command, rename = "bl-show-settings", ephemeral, guild_only)]
pub(crate) async fn cmd_show_settings(ctx: Context<'_>) -> Result<(), Error> {
    ctx.say(format!("{}", &ctx.data().guild_settings.lock().await))
        .await?;

    Ok(())
}

/// Set bot log channel
#[poise::command(
    slash_command,
    rename = "bl-set-log-channel",
    ephemeral,
    required_permissions = "MANAGE_ROLES",
    default_member_permissions = "MANAGE_ROLES",
    required_bot_permissions = "MANAGE_ROLES",
    guild_only
)]
pub(crate) async fn cmd_set_log_channel(
    ctx: Context<'_>,
    #[description = "The channel where the bot logs will be posted. Leave empty to disable logging."]
    channel: Option<ChannelId>,
) -> Result<(), Error> {
    if let Err(e) =
        update_bot_log_channel(&ctx.data().persist, &ctx.data().guild_settings, channel).await
    {
        ctx.say(format!("Error updating bot log channel: {}", e))
            .await?;
        return Ok(());
    }

    ctx.say(format!("{}", &ctx.data().guild_settings.lock().await))
        .await?;

    Ok(())
}
