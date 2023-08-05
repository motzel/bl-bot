use futures::Stream;
use poise::serenity_prelude;
use poise::serenity_prelude::ChannelId;

use crate::bot::{MetricCondition, PlayerMetric, PlayerMetricWithValue};
use crate::{Context, Error};

/// Display current bot settings
#[poise::command(slash_command, rename = "bl-show-settings", ephemeral, guild_only)]
pub(crate) async fn cmd_show_settings(ctx: Context<'_>) -> Result<(), Error> {
    let Some(guild_id) = ctx.guild_id() else {
        ctx.say("Can not get guild data".to_string()).await?;
        return Ok(());
    };

    match ctx.data().guild_settings_repository.get(&guild_id).await {
        Ok(guild_settings) => {
            ctx.say(format!("{}", guild_settings)).await?;

            Ok(())
        }
        Err(e) => {
            ctx.send(|f| {
                f.content(format!("An error occurred: {}", e))
                    .ephemeral(true)
            })
            .await?;

            Ok(())
        }
    }
}

/// Set or unset bot log channel
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
    let Some(guild_id) = ctx.guild_id() else {
        ctx.say("Can not get guild data".to_string()).await?;
        return Ok(());
    };

    match ctx
        .data()
        .guild_settings_repository
        .set_bot_channel(&guild_id, channel)
        .await
    {
        Ok(guild_settings) => {
            ctx.say(format!("{}", guild_settings)).await?;

            Ok(())
        }
        Err(e) => {
            ctx.say(format!("An error occurred: {}", e)).await?;

            Ok(())
        }
    }
}

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
    let Some(guild_id) = ctx.guild_id() else {
        ctx.say("Can not get guild data".to_string()).await?;
        return Ok(());
    };

    match ctx
        .data()
        .guild_settings_repository
        .add_auto_role(
            guild_id,
            group,
            role.id,
            PlayerMetricWithValue::new(metric, value),
            condition,
            weight,
        )
        .await
    {
        Ok(guild_settings) => {
            ctx.say(format!("{}", guild_settings)).await?;

            Ok(())
        }
        Err(e) => {
            ctx.say(format!("An error occurred: {}", e)).await?;

            Ok(())
        }
    }
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
    let Some(guild_id) = ctx.guild_id() else {
        ctx.say("Can not get guild data".to_string()).await?;
        return Ok(());
    };

    match ctx
        .data()
        .guild_settings_repository
        .remove_auto_role(guild_id, group, role.id)
        .await
    {
        Ok(guild_settings) => {
            ctx.say(format!("{}", guild_settings)).await?;

            Ok(())
        }
        Err(e) => {
            ctx.say(format!("An error occurred: {}", e)).await?;

            Ok(())
        }
    }
}

async fn autocomplete_role_group<'a>(
    ctx: Context<'_>,
    partial: &'a str,
) -> impl Stream<Item = String> + 'a {
    match ctx.guild_id() {
        None => futures::stream::iter(Vec::<String>::new()),
        Some(guild_id) => match ctx.data().guild_settings_repository.get(&guild_id).await {
            Err(_) => futures::stream::iter(Vec::<String>::new()),
            Ok(guild_settings) => futures::stream::iter(
                guild_settings
                    .get_groups()
                    .iter()
                    .filter(|rs| rs.contains(partial))
                    .map(|s| s.to_string())
                    .collect::<Vec<String>>(),
            ),
        },
    }
}
