use futures::Stream;
use poise::serenity_prelude;

use crate::bot::db::{add_auto_role, remove_auto_role};
use crate::bot::{MetricCondition, PlayerMetric, PlayerMetricWithValue};
use crate::{Context, Error};

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
    role: serenity_prelude::Role,
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

    ctx.say(format!("{}", &ctx.data().guild_settings.lock().await))
        .await?;

    Ok(())
}

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
    #[autocomplete = "autocomplete_role_group"]
    group: String,
    #[description = "Role to remove."] role: serenity_prelude::Role,
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

    ctx.say(format!("{}", &ctx.data().guild_settings.lock().await))
        .await?;

    Ok(())
}

async fn autocomplete_role_group<'a>(
    ctx: Context<'_>,
    partial: &'a str,
) -> impl Stream<Item = String> + 'a {
    if let Some(_guild_id) = ctx.guild_id() {
        let group_names: Vec<String> = ctx
            .data()
            .guild_settings
            .lock()
            .await
            .get_groups()
            .iter()
            .filter(|rs| rs.contains(partial))
            .map(|s| s.to_string())
            .collect();

        futures::stream::iter(group_names)
    } else {
        futures::stream::iter(Vec::<String>::new())
    }
}
