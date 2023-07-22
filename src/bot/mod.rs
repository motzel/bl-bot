#![allow(dead_code)]
#![allow(unused_imports)]

mod beatleader;
pub(crate) mod commands;
pub(crate) mod db;

use log::info;
use poise::serenity_prelude as serenity;
use poise::SlashArgument;
use serenity::model::gateway::Activity;
use std::cmp::Ordering;
use std::collections::HashMap;

use crate::beatleader::player::PlayerId;
use crate::bot::beatleader::{fetch_scores, Player};
use crate::bot::db::{get_player_id, link_player};
use crate::{Context, Error};
use serde::{Deserialize, Serialize};
use serenity::model::id::GuildId;
use shuttle_poise::ShuttlePoise;
use shuttle_secrets::SecretStore;

use serenity::model::prelude::RoleId;

#[derive(Serialize, Deserialize, Clone, Debug, poise::ChoiceParameter)]
#[serde(rename_all = "camelCase")]
pub(crate) enum PlayerMetric {
    #[name = "Top PP"]
    TopPp,
    #[name = "Top Acc"]
    TopAcc,
    #[name = "Total PP"]
    TotalPp,
    #[name = "Rank"]
    Rank,
    #[name = "Country Rank"]
    CountryRank,
}

impl From<PlayerMetricWithValue> for PlayerMetric {
    fn from(value: PlayerMetricWithValue) -> Self {
        match value {
            PlayerMetricWithValue::TopPp(_) => PlayerMetric::TopPp,
            PlayerMetricWithValue::TopAcc(_) => PlayerMetric::TopAcc,
            PlayerMetricWithValue::TotalPp(_) => PlayerMetric::TotalPp,
            PlayerMetricWithValue::Rank(_) => PlayerMetric::Rank,
            PlayerMetricWithValue::CountryRank(_) => PlayerMetric::CountryRank,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, poise::ChoiceParameter)]
#[serde(rename_all = "camelCase")]
pub(crate) enum MetricCondition {
    #[name = "Less than"]
    LessThan,
    #[name = "Less than or equal to"]
    LessThanOrEqualTo,
    #[name = "Equal to"]
    EqualTo,
    #[name = "Greater than"]
    GreaterThan,
    #[name = "Greater than or equal to"]
    GreaterThanOrEqualTo,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub(crate) enum PlayerMetricWithValue {
    TopPp(f64),
    TopAcc(f64),
    TotalPp(f64),
    Rank(u32),
    CountryRank(u32),
}

impl PlayerMetricWithValue {
    pub fn is_fulfilled_for(
        &self,
        condition: MetricCondition,
        value: &PlayerMetricWithValue,
    ) -> bool {
        if std::mem::discriminant(&PlayerMetric::from(self.clone()))
            != std::mem::discriminant(&PlayerMetric::from(value.clone()))
        {
            return false;
        }

        match condition {
            MetricCondition::LessThan => self.lt(value),
            MetricCondition::LessThanOrEqualTo => self.le(value),
            MetricCondition::EqualTo => self.eq(value),
            MetricCondition::GreaterThan => self.gt(value),
            MetricCondition::GreaterThanOrEqualTo => self.ge(value),
        }
    }
}

impl PartialOrd for PlayerMetricWithValue {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        match self {
            PlayerMetricWithValue::TopPp(v) => {
                if let PlayerMetricWithValue::TopPp(o) = other {
                    v.partial_cmp(o)
                } else {
                    None
                }
            }
            PlayerMetricWithValue::TopAcc(v) => {
                if let PlayerMetricWithValue::TopAcc(o) = other {
                    v.partial_cmp(o)
                } else {
                    None
                }
            }
            PlayerMetricWithValue::TotalPp(v) => {
                if let PlayerMetricWithValue::TotalPp(o) = other {
                    v.partial_cmp(o)
                } else {
                    None
                }
            }
            PlayerMetricWithValue::Rank(v) => {
                if let PlayerMetricWithValue::Rank(o) = other {
                    v.partial_cmp(o)
                } else {
                    None
                }
            }
            PlayerMetricWithValue::CountryRank(v) => {
                if let PlayerMetricWithValue::CountryRank(o) = other {
                    v.partial_cmp(o)
                } else {
                    None
                }
            }
        }
    }
}

type RoleGroup = String;

type RoleConditionId = u32;

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct RoleCondition {
    condition_id: RoleConditionId,
    condition: MetricCondition,
    value: PlayerMetricWithValue,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct RoleSettings {
    role_id: RoleId,
    role_name: String,
    conditions: Vec<RoleCondition>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct GuildSettings {
    guild_id: GuildId,
    role_groups: HashMap<RoleGroup, Vec<RoleSettings>>,
}

impl GuildSettings {
    pub fn new(guild_id: GuildId) -> Self {
        Self {
            guild_id,
            role_groups: HashMap::new(),
        }
    }
}

/*
async fn autocomplete_name<'a>(
    ctx: Context<'_>,
    partial: &'a str,
) -> impl Iterator<Item = poise::AutocompleteChoice<String>> {
    let interaction = match ctx {
        Context::Application(poise::ApplicationContext {
            interaction: poise::ApplicationCommandOrAutocompleteInteraction::Autocomplete(x),
            ..
        }) => x,
        _ => unreachable!("non-autocomplete interaction in autocomplete callback"),
    };

    // Find user param
    let user_param = interaction
        .data
        .options
        .iter()
        .find(|o| o.name == "dsc_user");

    let user_id = match user_param {
        Some(co) => match co.value.as_ref() {
            Some(value) => value.to_string(),
            None => ctx.author().id.to_string(),
        },
        None => ctx.author().id.to_string(),
    };

    // Extract user data
    // let user_data = match user.resolved.as_ref().unwrap() {
    //     serenity::CommandDataOptionValue::User(x, _) => x,
    //     _ => unreachable!("non-user value in user parameter"),
    // };

    info!(
        "In autocomplete: {}, {:?}, {}",
        partial,
        ctx.invocation_string(),
        user_id,
    );

    let maps = vec!["Map 1", "Map 2", "Map 3", "Map 4", "Map 5"];

    [1_u32, 2, 3, 4, 5]
        .iter()
        .map(move |&n| poise::AutocompleteChoice {
            name: format!("Label: {}", maps.get((n - 1) as usize).unwrap()),
            value: n.to_string(),
        })
}

/// Post link to a replay, yours or another server user who has linked they BL account.
///
/// Enter any user of this server as a parameter. If you omit it then your replay will be searched for.
#[poise::command(slash_command, rename = "bl-replay", guild_only)]
pub(crate) async fn bl_replay_autocomplete(
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
*/
