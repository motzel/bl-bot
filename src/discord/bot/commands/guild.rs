use futures::Stream;
use poise::serenity_prelude;
use poise::serenity_prelude::{ChannelId, GuildId};

use crate::discord::bot::commands::player::say_without_ping;
use crate::discord::bot::{Condition, GuildSettings, Metric, RequirementMetricValue};
use crate::discord::Context;
use crate::Error;

/// Display current bot settings
#[tracing::instrument(skip(ctx), level=tracing::Level::INFO, name="bot_command:bl-show-settings")]
#[poise::command(
    slash_command,
    rename = "bl-show-settings",
    ephemeral,
    required_permissions = "MANAGE_ROLES",
    default_member_permissions = "MANAGE_ROLES",
    required_bot_permissions = "MANAGE_ROLES",
    guild_only,
    hide_in_help
)]
pub(crate) async fn cmd_show_settings(ctx: Context<'_>) -> Result<(), Error> {
    let guild_settings = get_guild_settings(ctx, true).await?;

    ctx.say(format!("{}", guild_settings)).await?;

    Ok(())
}

/// Set or unset bot log channel
#[tracing::instrument(skip(ctx), level=tracing::Level::INFO, name="bot_command:bl-set-log-channel")]
#[poise::command(
    slash_command,
    rename = "bl-set-log-channel",
    ephemeral,
    required_permissions = "MANAGE_ROLES",
    default_member_permissions = "MANAGE_ROLES",
    required_bot_permissions = "MANAGE_ROLES",
    guild_only,
    hide_in_help
)]
pub(crate) async fn cmd_set_log_channel(
    ctx: Context<'_>,
    #[description = "The channel where the bot logs will be posted. Leave empty to disable logging."]
    #[channel_types("Text")]
    channel_id: Option<ChannelId>,
) -> Result<(), Error> {
    let guild_id = get_guild_id(ctx, true).await?;

    match ctx
        .data()
        .guild_settings_repository
        .set_bot_channel(&guild_id, channel_id)
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

/// Set or unset clan wars maps channel
#[tracing::instrument(skip(ctx), level=tracing::Level::INFO, name="bot_command:bl-set-clan-wars-maps-channel")]
#[poise::command(
    slash_command,
    rename = "bl-set-clan-wars-maps-channel",
    ephemeral,
    required_permissions = "MANAGE_ROLES",
    default_member_permissions = "MANAGE_ROLES",
    required_bot_permissions = "MANAGE_ROLES",
    guild_only,
    hide_in_help
)]
pub(crate) async fn cmd_set_clan_wars_maps_channel(
    ctx: Context<'_>,
    #[description = "The channel where the bot will post maps to play within clan wars. Leave empty to disable."]
    #[channel_types("Text")]
    channel_id: Option<ChannelId>,
) -> Result<(), Error> {
    let guild_id = get_guild_id(ctx, true).await?;

    match ctx
        .data()
        .guild_settings_repository
        .set_clan_wars_maps_channel(&guild_id, channel_id)
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

/// Set or unset clan wars contribution channel
#[tracing::instrument(skip(ctx), level=tracing::Level::INFO, name="bot_command:bl-set-clan-wars-contribution-channel")]
#[poise::command(
    slash_command,
    rename = "bl-set-clan-wars-contrib-channel",
    ephemeral,
    required_permissions = "MANAGE_ROLES",
    default_member_permissions = "MANAGE_ROLES",
    required_bot_permissions = "MANAGE_ROLES",
    guild_only,
    hide_in_help
)]
pub(crate) async fn cmd_set_clan_wars_contribution_channel(
    ctx: Context<'_>,
    #[description = "The channel where the bot will post clan wars contributions. Leave empty to disable."]
    #[channel_types("Text")]
    channel_id: Option<ChannelId>,
) -> Result<(), Error> {
    let guild_id = get_guild_id(ctx, true).await?;

    match ctx
        .data()
        .guild_settings_repository
        .set_clan_wars_maps_contribution_channel(&guild_id, channel_id)
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

/// Set profile verification requirement
#[tracing::instrument(skip(ctx), level=tracing::Level::INFO, name="bot_command:bl-set-profile-verification")]
#[poise::command(
    slash_command,
    rename = "bl-set-profile-verification",
    ephemeral,
    required_permissions = "MANAGE_ROLES",
    default_member_permissions = "MANAGE_ROLES",
    required_bot_permissions = "MANAGE_ROLES",
    guild_only,
    hide_in_help
)]
pub(crate) async fn cmd_set_profile_verification(
    ctx: Context<'_>,
    #[description = "Does the bl-link command require a verified profile or not."]
    requires_verified_profile: bool,
) -> Result<(), Error> {
    let guild_id = get_guild_id(ctx, true).await?;

    match ctx
        .data()
        .guild_settings_repository
        .set_verified_profile_requirement(&guild_id, requires_verified_profile)
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
#[tracing::instrument(skip(ctx), level=tracing::Level::INFO, name="bot_command:bl-add-auto-role")]
#[poise::command(
    slash_command,
    rename = "bl-add-auto-role",
    ephemeral,
    required_permissions = "MANAGE_ROLES",
    default_member_permissions = "MANAGE_ROLES",
    required_bot_permissions = "MANAGE_ROLES",
    guild_only,
    hide_in_help
)]
pub(crate) async fn cmd_add_auto_role(
    ctx: Context<'_>,
    #[description = "Group name, e.g. `top-pp`. Only one role from a given group will be assigned."]
    #[min_length = 1]
    #[autocomplete = "autocomplete_role_group"]
    group: String,
    #[description = "Role to assign. Only the role with the highest weight in the group will be assigned."]
    role: serenity_prelude::Role,
    #[description = "Metric to check"] metric: Metric,
    #[description = "Condition to check"] condition: Condition,
    #[description = "Metric value"] value: String,
    #[description = "Weight of auto role in the group (100, 200, etc.; the better role, the higher value)"]
    #[min = 1]
    weight: u32,
) -> Result<(), Error> {
    let guild_id = get_guild_id(ctx, true).await?;

    let metric_and_value = match RequirementMetricValue::new(metric, value.as_str()) {
        Ok(v) => v,
        Err(e) => {
            ctx.say(format!("Invalid metric value: {}", e)).await?;
            return Ok(());
        }
    };

    match ctx
        .data()
        .guild_settings_repository
        .add_auto_role(
            guild_id,
            group,
            role.id,
            metric_and_value,
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
#[tracing::instrument(skip(ctx), level=tracing::Level::INFO, name="bot_command:bl-remove-auto-role")]
#[poise::command(
    slash_command,
    rename = "bl-remove-auto-role",
    ephemeral,
    required_permissions = "MANAGE_ROLES",
    default_member_permissions = "MANAGE_ROLES",
    required_bot_permissions = "MANAGE_ROLES",
    guild_only,
    hide_in_help
)]
pub(crate) async fn cmd_remove_auto_role(
    ctx: Context<'_>,
    #[description = "The name of the group from which you want to remove the auto role, e.g. `top-pp`. "]
    #[min_length = 1]
    #[autocomplete = "autocomplete_role_group"]
    group: String,
    #[description = "Role to remove."] role: serenity_prelude::Role,
) -> Result<(), Error> {
    let guild_id = get_guild_id(ctx, true).await?;

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

pub(crate) async fn get_guild_id(ctx: Context<'_>, ephemeral: bool) -> Result<GuildId, Error> {
    let Some(guild_id) = ctx.guild_id() else {
        say_without_ping(ctx, "Error: can not get guild data", ephemeral).await?;
        return Err(Box::<dyn std::error::Error + Send + Sync>::from(
            "Error: can not get guild data",
        ));
    };

    Ok(guild_id)
}

pub(crate) async fn get_guild_settings(
    ctx: Context<'_>,
    ephemeral: bool,
) -> Result<GuildSettings, Error> {
    let guild_id = get_guild_id(ctx, ephemeral).await?;

    let Ok(guild) = ctx.data().guild_settings_repository.get(&guild_id).await else {
        say_without_ping(ctx, "Error: can not get guild settings", ephemeral).await?;

        return Err(Box::<dyn std::error::Error + Send + Sync>::from(
            "Error: can not get guild settings",
        ));
    };

    Ok(guild)
}
