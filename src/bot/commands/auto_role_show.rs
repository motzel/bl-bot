use crate::bot::db::get_guild_settings;
use crate::{Context, Error};
use log::info;
use poise::serenity_prelude as serenity;

/// Display current auto roles settings
#[poise::command(slash_command, rename = "bl-show-auto-roles", ephemeral, guild_only)]
pub(crate) async fn bl_show_auto_roles(ctx: Context<'_>) -> Result<(), Error> {
    let guild_settings = match get_guild_settings(&ctx.data().persist, ctx.data().guild_id).await {
        Ok(gs) => gs,
        Err(e) => {
            ctx.say(format!("Error fetching auto role: {}", e)).await?;
            return Ok(());
        }
    };

    ctx.say(format!("{}", guild_settings)).await?;

    Ok(())
}
