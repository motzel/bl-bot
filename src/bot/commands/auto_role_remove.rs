use crate::bot::db::{add_auto_role, remove_auto_role};
use crate::bot::{GuildSettings, MetricCondition};
use crate::{Context, Error};
use log::info;
use poise::serenity_prelude as serenity;

/// Remove auto role.
#[poise::command(
    slash_command,
    rename = "bl-remove-auto-role",
    ephemeral,
    required_permissions = "MANAGE_ROLES",
    default_member_permissions = "MANAGE_ROLES",
    required_bot_permissions = "MANAGE_ROLES",
    guild_only
)]
pub(crate) async fn cmd_remove_auto_role(
    ctx: Context<'_>,
    #[description = "The name of the group from which you want to remove the auto role, e.g. `top-pp`. "]
    #[min_length = 1]
    group: String,
    #[description = "Role to remove."] role: serenity::Role,
) -> Result<(), Error> {
    if let Err(e) = remove_auto_role(
        &ctx.data().persist,
        &ctx.data().guild_settings,
        group,
        role.id,
    )
    .await
    {
        ctx.say(format!("Error removing auto role: {}", e)).await?;
        return Ok(());
    }

    ctx.say(format!(
        "Settings changed:\n{}",
        &ctx.data().guild_settings.lock().await
    ))
    .await?;

    Ok(())
}
