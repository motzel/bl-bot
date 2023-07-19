#![allow(dead_code)]
#![allow(unused_imports)]

mod beatleader;
pub(crate) mod db;

use log::info;
use poise::serenity_prelude as serenity;
use poise::SlashArgument;
use serenity::model::gateway::Activity;

use crate::beatleader::player::PlayerId;
use crate::bot::db::link_player;
use crate::{Context, Error};
use shuttle_poise::ShuttlePoise;
use shuttle_secrets::SecretStore;

#[derive(Debug, poise::ChoiceParameter)]
pub(crate) enum PlayerMetric {
    #[name = "Top PP"]
    TopPp,
    #[name = "Top Acc"]
    TopAcc,
    #[name = "Total PP"]
    TotalPp,
}

async fn autocomplete_name<'a>(
    _ctx: Context<'_>,
    partial: &'a str,
) -> impl Iterator<Item = poise::AutocompleteChoice<String>> {
    info!("In autocomplete: {}", partial);

    let maps = vec!["Map 1", "Map 2", "Map 3", "Map 4", "Map 5"];

    [1_u32, 2, 3, 4, 5]
        .iter()
        .map(move |&n| poise::AutocompleteChoice {
            name: format!("Label: {}", maps.get((n - 1) as usize).unwrap()),
            value: n.to_string(),
        })
}

/// Command to insert a link to a replay, yours or another server user who has linked they BL account.
///
/// Enter any user of this server as a parameter. If you omit it then your replay will be searched for.
#[poise::command(slash_command, rename = "bl-replay", guild_only)]
pub(crate) async fn bl_replay(
    ctx: Context<'_>,
    #[description = "Test variable"]
    #[autocomplete = "autocomplete_name"]
    test_var: String,
    #[description = "Discord user (YOU if not specified)"] dsc_user: Option<serenity::User>,
) -> Result<(), Error> {
    let selected_user = dsc_user.as_ref().unwrap_or_else(|| ctx.author());

    ctx.say(format!("Data: {:#?}, test: {}", selected_user, test_var))
        .await?;
    Ok(())
}

/// Command to set conditions for automatic role assignment.
#[poise::command(
    slash_command,
    rename = "bl-add-auto-role",
    ephemeral,
    required_permissions = "MANAGE_ROLES",
    default_member_permissions = "MANAGE_ROLES",
    required_bot_permissions = "MANAGE_ROLES",
    guild_only
)]
pub(crate) async fn bl_add_auto_role(
    ctx: Context<'_>,
    #[description = "Group name, e.g. `pp`. Only one role from a given group will be assigned."]
    #[min_length = 1]
    group: String,
    #[description = "Role to asign"] role: serenity::Role,
    #[description = "Metric to check"] metric: PlayerMetric,
    #[description = "Metric value. A metric value equal to or higher than this will assign the member a role"]
    #[min = 1]
    value: u32,
) -> Result<(), Error> {
    let current_member = ctx.author_member().await.unwrap();

    let op = ">=";
    ctx.say(format!(
        "Group: {}, Role: {:#?}, Metric: {:#?}, Op: {:#?}, Value: {:#?}",
        group, role, metric, op, value
    ))
    .await?;

    ctx.say(format!("Current Member: {:#?}", current_member))
        .await?;

    Ok(())
}

/// Command to link your account to your Beat Leader profile.
#[poise::command(slash_command, rename = "bl-link", ephemeral, guild_only)]
pub(crate) async fn bl_link(
    ctx: Context<'_>,
    #[description = "Beat Leader PlayerID"] bl_player_id: String,
    #[description = "Discord user (YOU if not specified)"] dsc_user: Option<serenity::User>,
) -> Result<(), Error> {
    let selected_user = dsc_user.as_ref().unwrap_or_else(|| ctx.author());

    let bl_client = &ctx.data().bl_client;
    let persist = &ctx.data().persist;

    let player = link_player(
        bl_client,
        persist,
        selected_user.id.0,
        bl_player_id.to_owned(),
    )
    .await?;

    ctx.say(format!(
        "User linked to the player {} ({}).",
        player.name, bl_player_id,
    ))
    .await?;
    Ok(())
}

/// Command to display current conditions for automatic role assignment
#[poise::command(
    slash_command,
    rename = "bl-display-auto-roles",
    ephemeral,
    required_permissions = "MANAGE_ROLES",
    default_member_permissions = "MANAGE_ROLES",
    guild_only
)]
pub(crate) async fn bl_display_auto_roles(ctx: Context<'_>) -> Result<(), Error> {
    ctx.say(format!(
        "TODO: displaying all automatically assigned roles. App config GuildId: {:#?}",
        ctx.data().guild_id
    ))
    .await?;
    Ok(())
}
/// Command to remove the condition for automatic role assignment. Use ``bl-display-auto-roles`` first.
#[poise::command(
    slash_command,
    rename = "bl-remove-auto-role",
    ephemeral,
    required_permissions = "MANAGE_ROLES",
    default_member_permissions = "MANAGE_ROLES",
    required_bot_permissions = "MANAGE_ROLES",
    guild_only
)]
pub(crate) async fn bl_remove_auto_roles(
    ctx: Context<'_>,
    #[description = "RoleID to remove"]
    #[min = 1]
    role_id: u32,
) -> Result<(), Error> {
    ctx.say(format!("TODO: delete condition {}", role_id))
        .await?;
    Ok(())
}

#[poise::command(slash_command, rename = "bl-test", guild_only)]
pub(crate) async fn bl_test(ctx: Context<'_>) -> Result<(), Error> {
    let uuid_boop = ctx.id();

    ctx.send(|m| {
        m.content("I want some boops!").components(|c| {
            c.create_action_row(|ar| {
                ar.create_button(|b| {
                    b.style(serenity::ButtonStyle::Primary)
                        .label("Boop me!")
                        .custom_id(uuid_boop)
                })
            })
        })
    })
    .await?;

    let mut boop_count = 0;
    while let Some(mci) = serenity::CollectComponentInteraction::new(ctx)
        .author_id(ctx.author().id)
        .channel_id(ctx.channel_id())
        .timeout(std::time::Duration::from_secs(120))
        .filter(move |mci| mci.data.custom_id == uuid_boop.to_string())
        .await
    {
        boop_count += 1;

        let mut msg = mci.message.clone();
        msg.edit(ctx, |m| m.content(format!("Boop count: {}", boop_count)))
            .await?;

        mci.create_interaction_response(ctx, |ir| {
            ir.kind(serenity::InteractionResponseType::DeferredUpdateMessage)
        })
        .await?;
    }

    Ok(())
}

#[poise::command(slash_command, rename = "bl-test2", guild_only, reuse_response)]
pub(crate) async fn bl_test2(ctx: Context<'_>) -> Result<(), Error> {
    let image_url = "https://www.beatleader.xyz/assets/logo.png";
    ctx.send(|b| {
        b.content("message 1")
            .embed(|b| b.description("embed 1").image(image_url))
            .components(|b| {
                b.create_action_row(|b| {
                    b.create_button(|b| {
                        b.label("button 1")
                            .style(serenity::ButtonStyle::Primary)
                            .custom_id(1)
                    })
                })
            })
    })
    .await?;

    tokio::time::sleep(std::time::Duration::from_secs(2)).await;

    let image_url = "https://cdn.assets.beatleader.xyz/76561198035381239.png";
    ctx.send(|b| {
        b.content("message 2")
            .embed(|b| b.description("embed 2").image(image_url))
            .components(|b| {
                b.create_action_row(|b| {
                    b.create_button(|b| {
                        b.label("button 2")
                            .style(serenity::ButtonStyle::Danger)
                            .custom_id(2)
                    })
                })
            })
    })
    .await?;

    Ok(())
}
