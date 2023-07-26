use crate::bot::db::get_guild_settings;
use crate::{Context, Error};
use log::info;
use poise::serenity_prelude as serenity;

/// Display current auto roles settings
#[poise::command(slash_command, rename = "bl-show-auto-roles", ephemeral, guild_only)]
pub(crate) async fn cmd_show_auto_roles(ctx: Context<'_>) -> Result<(), Error> {
    ctx.say(format!("{}", &ctx.data().guild_settings.lock().await))
        .await?;

    Ok(())
}
