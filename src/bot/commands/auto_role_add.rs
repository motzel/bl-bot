use crate::bot::commands::autocomplete_role_group;
use crate::bot::db::add_auto_role;
use crate::bot::{GuildSettings, MetricCondition, PlayerMetric, PlayerMetricWithValue};
use crate::{Context, Error};
use log::info;
use poise::serenity_prelude as serenity;

/// Set conditions for automatic role assignment.
#[poise::command(
    slash_command,
    rename = "bl-add-auto-role",
    ephemeral,
    required_permissions = "MANAGE_ROLES",
    default_member_permissions = "MANAGE_ROLES",
    required_bot_permissions = "MANAGE_ROLES",
    guild_only
)]
pub(crate) async fn cmd_add_auto_role(
    ctx: Context<'_>,
    #[description = "Group name, e.g. `top-pp`. Only one role from a given group will be assigned."]
    #[min_length = 1]
    #[autocomplete = "autocomplete_role_group"]
    group: String,
    #[description = "Role to assign. Only the role with the highest weight in the group will be assigned."]
    role: serenity::Role,
    #[description = "Metric to check"] metric: PlayerMetric,
    #[description = "Condition to check"] condition: MetricCondition,
    #[description = "Metric value"]
    #[min = 1]
    value: f64,
    #[description = "Weight of auto role in the group (100, 200, etc.; the better role, the higher value)"]
    #[min = 1]
    weight: u32,
) -> Result<(), Error> {
    if let Err(e) = add_auto_role(
        &ctx.data().persist,
        &ctx.data().guild_settings,
        group,
        role.id,
        PlayerMetricWithValue::new(metric, value),
        condition,
        weight,
    )
    .await
    {
        ctx.say(format!("Error adding auto role: {}", e)).await?;
        return Ok(());
    }

    ctx.say(format!(
        "Settings changed:\n{}",
        &ctx.data().guild_settings.lock().await
    ))
    .await?;

    Ok(())
}
