#![allow(dead_code)]
#![allow(unused_imports)]

use std::cmp::Ordering;
use std::collections::HashMap;
use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration as TimeDuration;

use bytes::Bytes;
use chrono::{DateTime, Duration, Utc};
use futures::future::BoxFuture;
use log::{error, info, trace};
use poise::serenity_prelude::{ChannelId, User, UserId};
use poise::SlashArgument;
use poise::{async_trait, serenity_prelude as serenity};
use reqwest::Method;
use serde::{Deserialize, Serialize};
use serenity::model::gateway::Activity;
use serenity::model::id::GuildId;
use serenity::model::prelude::RoleId;
use shuttle_poise::ShuttlePoise;
use shuttle_secrets::SecretStore;

use crate::beatleader::clan::ClanTag;
use crate::beatleader::error::Error as BlError;
use crate::beatleader::oauth::{OAuthToken, OAuthTokenRepository};
use crate::beatleader::player::PlayerId;
use crate::beatleader::APP_USER_AGENT;
use crate::bot::beatleader::{fetch_scores, Player};
use crate::storage::player_oauth_token::PlayerOAuthTokenRepository;
use crate::storage::{StorageKey, StorageValue};
use crate::Context;
use crate::Error;

pub(crate) mod beatleader;
pub(crate) mod commands;

#[derive(Serialize, Deserialize, Clone, Debug, poise::ChoiceParameter)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub(crate) enum Metric {
    #[name = "Total PP"]
    TotalPp,
    #[name = "Top PP"]
    TopPp,
    #[name = "Rank"]
    Rank,
    #[name = "Country Rank"]
    CountryRank,
    #[name = "Top Acc"]
    TopAcc,
    #[name = "Max Streak"]
    MaxStreak,
    #[name = "#1 Count"]
    Top1Count,
    #[name = "My replays watched"]
    MyReplaysWatched,
    #[name = "Replays I watched"]
    ReplaysIWatched,
    #[name = "Clans"]
    Clan,
    #[name = "Top Stars"]
    TopStars,
    #[name = "Last pause (days)"]
    LastPause,
}

impl From<&RequirementMetricValue> for Metric {
    fn from(value: &RequirementMetricValue) -> Self {
        match value {
            RequirementMetricValue::TopPp(_) => Metric::TopPp,
            RequirementMetricValue::TopAcc(_) => Metric::TopAcc,
            RequirementMetricValue::TotalPp(_) => Metric::TotalPp,
            RequirementMetricValue::Rank(_) => Metric::Rank,
            RequirementMetricValue::CountryRank(_) => Metric::CountryRank,
            RequirementMetricValue::MaxStreak(_) => Metric::MaxStreak,
            RequirementMetricValue::Top1Count(_) => Metric::Top1Count,
            RequirementMetricValue::MyReplaysWatched(_) => Metric::MyReplaysWatched,
            RequirementMetricValue::ReplaysIWatched(_) => Metric::ReplaysIWatched,
            RequirementMetricValue::Clan(_) => Metric::Clan,
            RequirementMetricValue::TopStars(_) => Metric::TopStars,
            RequirementMetricValue::LastPause(_) => Metric::LastPause,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, poise::ChoiceParameter)]
#[serde(rename_all = "camelCase")]
pub(crate) enum Condition {
    #[name = "Better than or equal to"]
    BetterThanOrEqualTo,
    #[name = "Better than"]
    BetterThan,
    #[name = "Equal to"]
    EqualTo,
    #[name = "Worse than or equal to"]
    WorseThanOrEqualTo,
    #[name = "Worse than"]
    WorseThan,
    #[name = "Contains (clan metric only)"]
    Contains,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, PartialOrd)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub(crate) enum RequirementMetricValue {
    TopPp(f64),
    TopAcc(f64),
    TotalPp(f64),
    Rank(u32),
    CountryRank(u32),
    MaxStreak(u32),
    Top1Count(u32),
    MyReplaysWatched(u32),
    ReplaysIWatched(u32),
    Clan(Vec<String>),
    TopStars(f64),
    LastPause(u32),
}

impl RequirementMetricValue {
    pub fn new(metric: Metric, value: &str) -> Result<Self, Error> {
        match metric {
            Metric::TotalPp => Ok(RequirementMetricValue::TotalPp(value.parse::<f64>()?)),
            Metric::TopPp => Ok(RequirementMetricValue::TopPp(value.parse::<f64>()?)),
            Metric::Rank => Ok(RequirementMetricValue::Rank(value.parse::<u32>()?)),
            Metric::CountryRank => Ok(RequirementMetricValue::CountryRank(value.parse::<u32>()?)),
            Metric::TopAcc => Ok(RequirementMetricValue::TopAcc(value.parse::<f64>()?)),
            Metric::MaxStreak => Ok(RequirementMetricValue::MaxStreak(value.parse::<u32>()?)),
            Metric::Top1Count => Ok(RequirementMetricValue::Top1Count(value.parse::<u32>()?)),
            Metric::MyReplaysWatched => Ok(RequirementMetricValue::MyReplaysWatched(
                value.parse::<u32>()?,
            )),
            Metric::ReplaysIWatched => Ok(RequirementMetricValue::ReplaysIWatched(
                value.parse::<u32>()?,
            )),
            Metric::Clan => {
                if value.len() < 2 || value.len() > 4 {
                    return Err(From::from("name of the clan should have 2 to 4 characters"));
                }

                Ok(RequirementMetricValue::Clan(vec![value.to_string()]))
            }
            Metric::TopStars => Ok(RequirementMetricValue::TopStars(value.parse::<f64>()?)),
            Metric::LastPause => Ok(RequirementMetricValue::LastPause(value.parse::<u32>()?)),
        }
    }

    pub fn is_contained_by(&self, other: &PlayerMetricValue) -> bool {
        match self {
            RequirementMetricValue::TopPp(_) => false,
            RequirementMetricValue::TopAcc(_) => false,
            RequirementMetricValue::TotalPp(_) => false,
            RequirementMetricValue::Rank(_) => false,
            RequirementMetricValue::CountryRank(_) => false,
            RequirementMetricValue::MaxStreak(_) => false,
            RequirementMetricValue::Top1Count(_) => false,
            RequirementMetricValue::MyReplaysWatched(_) => false,
            RequirementMetricValue::ReplaysIWatched(_) => false,
            RequirementMetricValue::Clan(requirement_clans) => {
                if let PlayerMetricValue::Clan(player_clans) = other {
                    requirement_clans
                        .iter()
                        .all(|clan| player_clans.contains(clan))
                } else {
                    false
                }
            }
            RequirementMetricValue::TopStars(_) => false,
            RequirementMetricValue::LastPause(_) => false,
        }
    }

    fn reverse_ordering(ord: Option<Ordering>) -> Option<Ordering> {
        ord.map(|ord| match ord {
            Ordering::Less => Ordering::Greater,
            Ordering::Equal => Ordering::Equal,
            Ordering::Greater => Ordering::Less,
        })
    }
}

impl PartialEq<PlayerMetricValue> for RequirementMetricValue {
    fn eq(&self, other: &PlayerMetricValue) -> bool {
        if std::mem::discriminant(&Metric::from(self))
            != std::mem::discriminant(&Metric::from(other))
        {
            return false;
        }

        match self {
            RequirementMetricValue::TopPp(v) => {
                if let PlayerMetricValue::TopPp(player_metric_value) = other {
                    v == player_metric_value
                } else {
                    false
                }
            }
            RequirementMetricValue::TopAcc(v) => {
                if let PlayerMetricValue::TopAcc(player_metric_value) = other {
                    v == player_metric_value
                } else {
                    false
                }
            }
            RequirementMetricValue::TotalPp(v) => {
                if let PlayerMetricValue::TotalPp(player_metric_value) = other {
                    v == player_metric_value
                } else {
                    false
                }
            }
            RequirementMetricValue::Rank(v) => {
                if let PlayerMetricValue::Rank(player_metric_value) = other {
                    v == player_metric_value
                } else {
                    false
                }
            }
            RequirementMetricValue::CountryRank(v) => {
                if let PlayerMetricValue::CountryRank(player_metric_value) = other {
                    v == player_metric_value
                } else {
                    false
                }
            }
            RequirementMetricValue::MaxStreak(v) => {
                if let PlayerMetricValue::MaxStreak(player_metric_value) = other {
                    v == player_metric_value
                } else {
                    false
                }
            }
            RequirementMetricValue::Top1Count(v) => {
                if let PlayerMetricValue::Top1Count(player_metric_value) = other {
                    v == player_metric_value
                } else {
                    false
                }
            }
            RequirementMetricValue::MyReplaysWatched(v) => {
                if let PlayerMetricValue::MyReplaysWatched(player_metric_value) = other {
                    v == player_metric_value
                } else {
                    false
                }
            }
            RequirementMetricValue::ReplaysIWatched(v) => {
                if let PlayerMetricValue::ReplaysIWatched(player_metric_value) = other {
                    v == player_metric_value
                } else {
                    false
                }
            }
            RequirementMetricValue::Clan(v) => {
                if let PlayerMetricValue::Clan(player_metric_value) = other {
                    v.iter().all(|clan| player_metric_value.contains(clan))
                } else {
                    false
                }
            }
            RequirementMetricValue::TopStars(v) => {
                if let PlayerMetricValue::TopStars(player_metric_value) = other {
                    v == player_metric_value
                } else {
                    false
                }
            }
            RequirementMetricValue::LastPause(v) => {
                if let PlayerMetricValue::LastPause(Some(last_pause_date)) = other {
                    (Utc::now() - Duration::days(*v as i64)) == *last_pause_date
                } else {
                    false
                }
            }
        }
    }
}

impl PartialOrd<PlayerMetricValue> for RequirementMetricValue {
    fn partial_cmp(&self, other: &PlayerMetricValue) -> Option<Ordering> {
        match self {
            RequirementMetricValue::TopPp(v) => {
                if let PlayerMetricValue::TopPp(player_metric_value) = other {
                    v.partial_cmp(player_metric_value)
                } else {
                    None
                }
            }
            RequirementMetricValue::TopAcc(v) => {
                if let PlayerMetricValue::TopAcc(player_metric_value) = other {
                    v.partial_cmp(player_metric_value)
                } else {
                    None
                }
            }
            RequirementMetricValue::TotalPp(v) => {
                if let PlayerMetricValue::TotalPp(player_metric_value) = other {
                    v.partial_cmp(player_metric_value)
                } else {
                    None
                }
            }
            RequirementMetricValue::Rank(v) => {
                if let PlayerMetricValue::Rank(player_metric_value) = other {
                    RequirementMetricValue::reverse_ordering(v.partial_cmp(player_metric_value))
                } else {
                    None
                }
            }
            RequirementMetricValue::CountryRank(v) => {
                if let PlayerMetricValue::CountryRank(player_metric_value) = other {
                    RequirementMetricValue::reverse_ordering(v.partial_cmp(player_metric_value))
                } else {
                    None
                }
            }
            RequirementMetricValue::MaxStreak(v) => {
                if let PlayerMetricValue::MaxStreak(player_metric_value) = other {
                    v.partial_cmp(player_metric_value)
                } else {
                    None
                }
            }
            RequirementMetricValue::Top1Count(v) => {
                if let PlayerMetricValue::Top1Count(player_metric_value) = other {
                    v.partial_cmp(player_metric_value)
                } else {
                    None
                }
            }
            RequirementMetricValue::MyReplaysWatched(v) => {
                if let PlayerMetricValue::MyReplaysWatched(player_metric_value) = other {
                    v.partial_cmp(player_metric_value)
                } else {
                    None
                }
            }
            RequirementMetricValue::ReplaysIWatched(v) => {
                if let PlayerMetricValue::ReplaysIWatched(player_metric_value) = other {
                    v.partial_cmp(player_metric_value)
                } else {
                    None
                }
            }
            RequirementMetricValue::Clan(_v) => None,
            RequirementMetricValue::TopStars(v) => {
                if let PlayerMetricValue::TopStars(player_metric_value) = other {
                    v.partial_cmp(player_metric_value)
                } else {
                    None
                }
            }
            RequirementMetricValue::LastPause(v) => {
                if let PlayerMetricValue::LastPause(Some(last_pause_date)) = other {
                    RequirementMetricValue::reverse_ordering(
                        (Utc::now() - Duration::days(*v as i64)).partial_cmp(last_pause_date),
                    )
                } else {
                    Some(Ordering::Less)
                }
            }
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, PartialOrd)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub enum PlayerMetricValue {
    TopPp(f64),
    TopAcc(f64),
    TotalPp(f64),
    Rank(u32),
    CountryRank(u32),
    MaxStreak(u32),
    Top1Count(u32),
    MyReplaysWatched(u32),
    ReplaysIWatched(u32),
    Clan(Vec<String>),
    TopStars(f64),
    LastPause(Option<DateTime<Utc>>),
}

impl From<&PlayerMetricValue> for Metric {
    fn from(value: &PlayerMetricValue) -> Self {
        match value {
            PlayerMetricValue::TopPp(_) => Metric::TopPp,
            PlayerMetricValue::TopAcc(_) => Metric::TopAcc,
            PlayerMetricValue::TotalPp(_) => Metric::TotalPp,
            PlayerMetricValue::Rank(_) => Metric::Rank,
            PlayerMetricValue::CountryRank(_) => Metric::CountryRank,
            PlayerMetricValue::MaxStreak(_) => Metric::MaxStreak,
            PlayerMetricValue::Top1Count(_) => Metric::Top1Count,
            PlayerMetricValue::MyReplaysWatched(_) => Metric::MyReplaysWatched,
            PlayerMetricValue::ReplaysIWatched(_) => Metric::ReplaysIWatched,
            PlayerMetricValue::Clan(_) => Metric::Clan,
            PlayerMetricValue::TopStars(_) => Metric::TopStars,
            PlayerMetricValue::LastPause(_) => Metric::LastPause,
        }
    }
}

pub(crate) type RoleGroup = String;

type RoleRequirementId = u32;

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Requirement {
    condition: Condition,
    value: RequirementMetricValue,
}

impl Requirement {
    pub fn is_fulfilled_for(&self, player_metric: &PlayerMetricValue) -> bool {
        if std::mem::discriminant(&Metric::from(&self.value))
            != std::mem::discriminant(&Metric::from(player_metric))
        {
            return false;
        }

        match self.condition {
            Condition::WorseThan => self.value.gt(player_metric),
            Condition::WorseThanOrEqualTo => self.value.ge(player_metric),
            Condition::EqualTo => self.value.eq(player_metric),
            Condition::BetterThan => self.value.lt(player_metric),
            Condition::BetterThanOrEqualTo => self.value.le(player_metric),
            Condition::Contains => self.value.is_contained_by(player_metric),
        }
    }
}

impl std::fmt::Display for Requirement {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match &self.value {
                RequirementMetricValue::TopPp(v) => format!(
                    "**Top PP** *{}* **{}**",
                    self.condition.to_string().to_lowercase(),
                    v
                ),
                RequirementMetricValue::TopAcc(v) => format!(
                    "**Top Acc** *{}* **{}**",
                    self.condition.to_string().to_lowercase(),
                    v
                ),
                RequirementMetricValue::TotalPp(v) => format!(
                    "**Total PP** *{}* **{}**",
                    self.condition.to_string().to_lowercase(),
                    v
                ),
                RequirementMetricValue::Rank(v) => format!(
                    "**Rank** *{}* **{}**",
                    self.condition.to_string().to_lowercase(),
                    v
                ),
                RequirementMetricValue::CountryRank(v) => format!(
                    "**Country rank** *{}* **{}**",
                    self.condition.to_string().to_lowercase(),
                    v
                ),
                RequirementMetricValue::MaxStreak(v) => format!(
                    "**Max streak** *{}* **{}**",
                    self.condition.to_string().to_lowercase(),
                    v
                ),
                RequirementMetricValue::Top1Count(v) => format!(
                    "**#1 count** *{}* **{}**",
                    self.condition.to_string().to_lowercase(),
                    v
                ),
                RequirementMetricValue::MyReplaysWatched(v) => format!(
                    "**My replays watched** *{}* **{}**",
                    self.condition.to_string().to_lowercase(),
                    v
                ),
                RequirementMetricValue::ReplaysIWatched(v) => format!(
                    "**I watched replays** *{}* **{}**",
                    self.condition.to_string().to_lowercase(),
                    v
                ),
                RequirementMetricValue::Clan(v) => format!(
                    "**Clan** *{}* **{}**",
                    self.condition.to_string().to_lowercase(),
                    v.join(", "),
                ),
                RequirementMetricValue::TopStars(v) => format!(
                    "**Top Stars** *{}* **{}**",
                    self.condition.to_string().to_lowercase(),
                    v
                ),
                RequirementMetricValue::LastPause(v) => format!(
                    "**Last pause** *{}* **{} days**",
                    self.condition.to_string().to_lowercase(),
                    v
                ),
            }
        )
    }
}

#[derive(Default, Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct RoleSettings {
    role_id: RoleId,
    conditions: HashMap<RoleRequirementId, Requirement>,
    weight: u32,
}

impl RoleSettings {
    pub fn new(role_id: RoleId, weight: u32) -> Self {
        Self {
            role_id,
            conditions: HashMap::new(),
            weight,
        }
    }

    fn get_next_condition_id(&self) -> RoleRequirementId {
        self.conditions
            .keys()
            .fold(0, |acc, condition_id| acc.max(*condition_id))
            + 1
    }

    pub(crate) fn add_requirement(&mut self, condition: Condition, value: RequirementMetricValue) {
        let rc = Requirement { condition, value };

        self.conditions
            .entry(self.get_next_condition_id())
            .or_insert(rc);
    }

    pub fn is_fulfilled_for(&self, player: &Player) -> bool {
        self.conditions.iter().all(|(_role_id, role_requirement)| {
            role_requirement.is_fulfilled_for(
                &player.get_metric_with_value(Metric::from(&role_requirement.value)),
            )
        })
    }
}

impl std::fmt::Display for RoleSettings {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        let mut cond_vec = self
            .conditions
            .iter()
            .map(|(cond_id, cond)| (*cond_id, cond.clone()))
            .collect::<Vec<(RoleRequirementId, Requirement)>>();
        cond_vec.sort_unstable_by(|a, b| Ord::cmp(&a.0, &b.0));

        write!(
            f,
            "* <@&{}> (*weight: {}*)\n{}",
            self.role_id,
            self.weight,
            cond_vec
                .iter()
                .map(|(_role_cond_id, role_cond)| format!(" * {}", role_cond))
                .fold(String::new(), |out, rs| out + &*format!("{}\n", rs))
                .trim_end()
        )
    }
}

#[derive(Default, Debug, Clone)]
pub struct UserRoleChanges {
    pub guild_id: GuildId,
    pub user_id: UserId,
    pub name: String,
    pub to_add: Vec<RoleId>,
    pub to_remove: Vec<RoleId>,
}

impl UserRoleChanges {
    pub async fn apply(
        &self,
        http: &Arc<poise::serenity_prelude::Http>,
    ) -> Result<&UserRoleChanges, Error> {
        info!("Updating user {} ({}) roles...", self.user_id, self.name);

        if self.to_add.is_empty() && self.to_remove.is_empty() {
            info!(
                "No roles to add or remove for user {} ({}).",
                self.user_id, self.name
            );
            return Ok(self);
        }

        info!(
            "{} role(s) to add to user {} ({})",
            self.to_add.len(),
            self.user_id,
            self.name
        );

        for role_id in self.to_add.iter() {
            trace!(
                "Adding role {} to user {} ({})",
                role_id,
                self.user_id,
                self.name
            );

            if let Err(e) = http
                .add_member_role(
                    self.guild_id.into(),
                    self.user_id.into(),
                    (*role_id).into(),
                    None,
                )
                .await
            {
                error!(
                    "Can not add role {} to user {} ({}): {}",
                    role_id, self.user_id, self.name, e
                );
                continue;
            }

            trace!(
                "Role {} added to user {} ({})",
                role_id,
                self.user_id,
                self.name
            );
        }

        info!(
            "{} role(s) to remove from user {} ({})",
            self.to_remove.len(),
            self.user_id,
            self.name
        );

        for role_id in self.to_remove.iter() {
            trace!(
                "Removing role {} from user {} ({})",
                role_id,
                self.user_id,
                self.name
            );

            if let Err(e) = http
                .remove_member_role(
                    self.guild_id.into(),
                    self.user_id.into(),
                    (*role_id).into(),
                    None,
                )
                .await
            {
                error!(
                    "Can not remove role {} from user {} ({}): {}",
                    role_id, self.user_id, self.name, e
                );
                continue;
            }

            trace!(
                "Role {} removed from user {} ({})",
                role_id,
                self.user_id,
                self.name
            );
        }

        Ok(self)
    }

    pub fn is_changed(&self) -> bool {
        !self.to_add.is_empty() || !self.to_remove.is_empty()
    }
}
impl std::fmt::Display for UserRoleChanges {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        let to_add_list = if !self.to_add.is_empty() {
            self.to_add
                .iter()
                .map(|role| format!("<@&{}>", role))
                .collect::<Vec<String>>()
                .join(", ")
        } else {
            "None".to_string()
        };

        let to_remove_list = if !self.to_remove.is_empty() {
            self.to_remove
                .iter()
                .map(|role| format!("<@&{}>", role))
                .collect::<Vec<String>>()
                .join(", ")
        } else {
            "None".to_string()
        };

        write!(
            f,
            "The roles of user <@{}> have been updated\n**Added roles:** {}\n**Removed roles:** {}",
            self.user_id, to_add_list, to_remove_list,
        )
    }
}

#[derive(Default, Debug, Clone)]
pub struct UserRoleStatus {
    should_have: Vec<RoleId>,
    should_not_have: Vec<RoleId>,
}

impl UserRoleStatus {
    pub fn get_role_changes(
        &self,
        guild_id: GuildId,
        player: &Player,
        current_roles: &[RoleId],
    ) -> UserRoleChanges {
        UserRoleChanges {
            guild_id,
            user_id: player.user_id,
            name: player.name.clone(),
            to_add: self
                .should_have
                .iter()
                .filter_map(|role_id| {
                    if !current_roles.contains(role_id) {
                        Some(role_id.to_owned())
                    } else {
                        None
                    }
                })
                .collect::<HashSet<_>>()
                .into_iter()
                .collect(),
            to_remove: current_roles
                .iter()
                .filter_map(|role_id| {
                    if self.should_not_have.contains(role_id) {
                        Some(role_id.to_owned())
                    } else {
                        None
                    }
                })
                .collect::<HashSet<_>>()
                .into_iter()
                .collect(),
        }
    }
}

#[derive(Serialize, Default, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
#[serde(default)]
pub struct GuildSettings {
    guild_id: GuildId,
    bot_channel_id: Option<ChannelId>,
    requires_verified_profile: bool,
    role_groups: HashMap<RoleGroup, HashMap<RoleId, RoleSettings>>,
    clan_settings: Option<ClanSettings>,
}

impl StorageKey for GuildId {}
impl StorageValue<GuildId> for GuildSettings {
    fn get_key(&self) -> GuildId {
        self.guild_id
    }
}

impl GuildSettings {
    pub fn new(guild_id: GuildId) -> Self {
        Self {
            guild_id,
            ..Default::default()
        }
    }

    pub fn get_key(&self) -> GuildId {
        self.guild_id
    }

    pub fn get_channel(&self) -> Option<ChannelId> {
        self.bot_channel_id
    }

    pub fn set_channel(&mut self, channel_id: Option<ChannelId>) {
        self.bot_channel_id = channel_id;
    }

    pub fn get_clan_settings(&self) -> Option<ClanSettings> {
        self.clan_settings.clone()
    }

    pub fn set_clan_settings(&mut self, clan_settings: Option<ClanSettings>) {
        self.clan_settings = clan_settings;
    }

    pub fn set_oauth_token(&mut self, oauth_token: bool) {
        if let Some(ref mut clan_settings) = self.clan_settings {
            clan_settings.set_oauth_token(oauth_token);
        }
    }

    pub fn set_verified_profile_requirement(&mut self, requires_verified_profile: bool) {
        self.requires_verified_profile = requires_verified_profile;
    }

    pub fn add(&mut self, role_group: RoleGroup, role_settings: RoleSettings) -> &mut Self {
        let role_settings_clone = role_settings.clone();
        self.role_groups
            .entry(role_group)
            .or_default()
            .entry(role_settings.role_id)
            .and_modify(|rs| *rs = role_settings)
            .or_insert(role_settings_clone);

        self
    }

    pub fn merge(&mut self, role_group: RoleGroup, role_settings: RoleSettings) -> &mut Self {
        let role_settings_clone = role_settings.clone();
        self.role_groups
            .entry(role_group)
            .or_default()
            .entry(role_settings.role_id)
            .and_modify(|rs| {
                rs.weight = role_settings.weight;

                role_settings
                    .conditions
                    .values()
                    .for_each(|rc| rs.add_requirement(rc.condition.clone(), rc.value.clone()));
            })
            .or_insert(role_settings_clone);

        self
    }

    pub fn remove(&mut self, role_group: RoleGroup, role_id: RoleId) {
        let role_group_clone = role_group.clone();

        self.role_groups.entry(role_group).and_modify(|rs| {
            rs.remove(&role_id);
        });

        if self.role_groups.contains_key(&role_group_clone)
            && self.role_groups.get(&role_group_clone).unwrap().is_empty()
        {
            self.role_groups.remove(&role_group_clone);
        }
    }

    pub fn all_roles(&self) -> Vec<&RoleId> {
        self.role_groups
            .iter()
            .flat_map(|(_rg, rs)| rs.keys())
            .collect()
    }

    pub fn contains_in_group(&self, role_group: RoleGroup, role_id: RoleId) -> bool {
        self.role_groups.contains_key(&role_group)
            && self
                .role_groups
                .get(&role_group)
                .unwrap()
                .contains_key(&role_id)
    }

    pub fn contains(&self, role_id: RoleId) -> bool {
        self.all_roles().iter().any(|&&r| r == role_id)
    }

    pub fn get_groups(&self) -> Vec<String> {
        self.role_groups.keys().cloned().collect()
    }

    pub(crate) fn get_role_updates(
        &self,
        guild_id: GuildId,
        player: &Player,
        current_roles: &[RoleId],
    ) -> UserRoleChanges {
        #[derive(Debug)]
        struct RoleFulfillmentStatus {
            role_id: RoleId,
            fulfilled: bool,
            weight: u32,
        }

        let mut ru = UserRoleStatus::default();

        self.role_groups
            .values()
            .map(|roles| {
                let mut roles_fulfillment = roles
                    .iter()
                    .map(|(role_id, role_settings)| RoleFulfillmentStatus {
                        role_id: *role_id,
                        fulfilled: role_settings.is_fulfilled_for(player),
                        weight: role_settings.weight,
                    })
                    .collect::<Vec<RoleFulfillmentStatus>>();

                roles_fulfillment.sort_unstable_by(|a, b| b.weight.cmp(&a.weight));

                let role_updates = &mut UserRoleStatus::default();

                roles_fulfillment
                    .iter()
                    .fold(role_updates, |acc, rf| {
                        if rf.fulfilled && acc.should_have.is_empty() {
                            acc.should_have.push(rf.role_id);
                        } else {
                            acc.should_not_have.push(rf.role_id);
                        }
                        acc
                    })
                    .clone()
            })
            .fold(&mut ru, |acc, mut role_updates| {
                acc.should_have.append(&mut role_updates.should_have);
                acc.should_not_have
                    .append(&mut role_updates.should_not_have);

                acc
            })
            .get_role_changes(guild_id, player, current_roles)
    }
}

impl std::fmt::Display for GuildSettings {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        let mut rg_vec = self
            .role_groups
            .iter()
            .collect::<Vec<(&RoleGroup, &HashMap<RoleId, RoleSettings>)>>();
        rg_vec.sort_unstable_by(|a, b| Ord::cmp(a.0, b.0));

        write!(
            f,
            "# __Current settings__\nBot log channel: {}\nVerified profiles only: {}\nClan setting: {}\n## Auto roles:\n{}",
            self.bot_channel_id.map_or_else(
                || "**None**".to_owned(),
                |channel_id| format!("<#{}>", channel_id.to_owned())
            ),
            if self.requires_verified_profile {"Yes"} else {"No"},
            if self.clan_settings.is_some() {self.clan_settings.clone().unwrap().to_string()} else {"Not set up".to_owned()},
            {
                let roles = rg_vec
                    .iter()
                    .map(|(rg, rs_hm)| {
                        let mut rs_vec = rs_hm.values().cloned().collect::<Vec<RoleSettings>>();
                        rs_vec.sort_unstable_by(|a, b| Ord::cmp(&b.weight, &a.weight));

                        format!(
                            "### Group: __{}__\n{}",
                            rg,
                            rs_vec
                                .iter()
                                .map(|rs| format!("{}", rs))
                                .fold(String::new(), |out, rs| out + &*format!("{}\n", rs))
                                .trim_end()
                        )
                    })
                    .fold(String::new(), |out, rg| out + &*format!("{}\n", rg))
                    .trim()
                    .to_owned();

                if !roles.is_empty() {
                    roles
                } else {
                    "None".to_owned()
                }
            }
        )
    }
}

#[derive(Serialize, Default, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
#[serde(default)]
pub struct ClanSettings {
    user_id: UserId,
    owner_id: PlayerId,
    clan: ClanTag,
    self_invite: bool,
    oauth_token_is_set: bool,
}

impl ClanSettings {
    pub fn new(
        user_id: UserId,
        owner_id: PlayerId,
        clan: ClanTag,
        self_invite: bool,
    ) -> ClanSettings {
        ClanSettings {
            user_id,
            owner_id,
            clan,
            self_invite,
            oauth_token_is_set: false,
        }
    }

    pub fn get_clan(&self) -> ClanTag {
        self.clan.clone()
    }

    pub fn supports_self_invitation(&self) -> bool {
        self.self_invite
    }

    pub fn is_oauth_token_set(&self) -> bool {
        self.oauth_token_is_set
    }

    pub fn set_oauth_token(&mut self, oauth_token_is_set: bool) {
        self.oauth_token_is_set = oauth_token_is_set;
    }
}

impl std::fmt::Display for ClanSettings {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        if self.oauth_token_is_set {
            write!(
                f,
                "Set up for the clan {}. Users can{} send themselves invitations.",
                self.clan,
                if !self.supports_self_invitation() {
                    " NOT"
                } else {
                    ""
                }
            )
        } else {
            write!(f, "Unfinished setup for clan {}!", self.get_clan())
        }
    }
}

#[derive(Clone)]
pub(crate) struct GuildOAuthTokenRepository {
    owner_id: PlayerId,
    player_oauth_token_repository: Arc<PlayerOAuthTokenRepository>,
}

impl GuildOAuthTokenRepository {
    pub fn new(
        player_id: PlayerId,
        player_oauth_token_repository: Arc<PlayerOAuthTokenRepository>,
    ) -> GuildOAuthTokenRepository {
        GuildOAuthTokenRepository {
            owner_id: player_id,
            player_oauth_token_repository,
        }
    }
}

#[async_trait]
impl OAuthTokenRepository for GuildOAuthTokenRepository {
    async fn get(&self) -> Result<Option<OAuthToken>, BlError> {
        trace!("Fetching OAuth token from repository...");

        match self.player_oauth_token_repository.get(&self.owner_id).await {
            Some(player_oauth_token) => {
                trace!("OAuth token fetched from repository.");

                Ok(Some(player_oauth_token.into()))
            }
            None => {
                trace!("No OAuth token in repository.");

                Err(BlError::OAuthStorage)
            }
        }
    }

    async fn store<ModifyFunc>(&self, modify_func: ModifyFunc) -> Result<OAuthToken, BlError>
    where
        ModifyFunc: for<'b> FnOnce(&'b mut OAuthToken) -> BoxFuture<'b, ()> + Send + 'static,
    {
        trace!("Storing OAuth token in repository...");

        match self
            .player_oauth_token_repository
            .set(&self.owner_id, |token| {
                Box::pin(async {
                    modify_func(&mut token.oauth_token).await;
                })
            })
            .await
        {
            Ok(player_oauth_token) => Ok(player_oauth_token.into()),
            Err(_) => Err(BlError::OAuthStorage),
        }
    }
}

pub async fn get_binary_file(url: &str) -> crate::beatleader::Result<Bytes> {
    let client_builder = reqwest::Client::builder()
        .https_only(true)
        .gzip(true)
        .brotli(true)
        .user_agent(APP_USER_AGENT)
        .build();

    let Ok(client) = client_builder else {
        return Err(BlError::Unknown);
    };

    let request = client
        .request(Method::GET, url)
        .timeout(TimeDuration::from_secs(30))
        .build();

    if let Err(err) = request {
        return Err(BlError::Request(err));
    }

    let response = client.execute(request.unwrap()).await;

    match response {
        Err(err) => Err(BlError::Network(err)),
        Ok(response) => match response.status().as_u16() {
            200..=299 => match response.bytes().await {
                Ok(b) => Ok(b),
                Err(_err) => Err(BlError::Unknown),
            },
            401 | 403 => Err(BlError::Unauthorized),
            404 => Err(BlError::NotFound),
            400..=499 => Err(BlError::Client(
                response.text_with_charset("utf-8").await.ok(),
            )),
            500..=599 => Err(BlError::Server),
            _ => Err(BlError::Unknown),
        },
    }
}

#[cfg(test)]
mod tests {
    use chrono::{DateTime, Duration, Utc};
    use poise::serenity_prelude::UserId;

    use crate::bot::{
        Condition, GuildId, GuildSettings, Metric, Player, PlayerMetricValue, Requirement,
        RequirementMetricValue, RoleId, RoleRequirementId, RoleSettings,
    };

    fn create_5kpp_ss_50_country_role_settings() -> RoleSettings {
        let mut rs = RoleSettings::new(RoleId(1), 100);

        rs.add_requirement(
            Condition::BetterThanOrEqualTo,
            RequirementMetricValue::TotalPp(5000.0),
        );

        rs.add_requirement(
            Condition::BetterThanOrEqualTo,
            RequirementMetricValue::TopAcc(90.0),
        );

        rs.add_requirement(
            Condition::BetterThanOrEqualTo,
            RequirementMetricValue::CountryRank(50),
        );

        rs
    }

    fn create_10kpp_role_settings() -> RoleSettings {
        let mut rs = RoleSettings::new(RoleId(2), 200);

        rs.add_requirement(
            Condition::BetterThanOrEqualTo,
            RequirementMetricValue::TotalPp(10000.0),
        );

        rs
    }
    fn create_1k_rank_role_settings() -> RoleSettings {
        let mut rs = RoleSettings::new(RoleId(3), 100);

        rs.add_requirement(
            Condition::BetterThanOrEqualTo,
            RequirementMetricValue::Rank(1000),
        );

        rs
    }
    fn create_500_rank_role_settings() -> RoleSettings {
        let mut rs = RoleSettings::new(RoleId(4), 200);

        rs.add_requirement(
            Condition::BetterThanOrEqualTo,
            RequirementMetricValue::Rank(500),
        );

        rs
    }
    fn create_100_rank_role_settings() -> RoleSettings {
        let mut rs = RoleSettings::new(RoleId(5), 300);

        rs.add_requirement(
            Condition::BetterThanOrEqualTo,
            RequirementMetricValue::Rank(100),
        );

        rs
    }

    fn create_clan_member_role_settings() -> RoleSettings {
        let mut rs = RoleSettings::new(RoleId(6), 100);

        rs.add_requirement(
            Condition::Contains,
            RequirementMetricValue::Clan(vec!["Clan1".to_string()]),
        );

        rs
    }

    fn create_no_pause_role_settings() -> RoleSettings {
        let mut rs = RoleSettings::new(RoleId(7), 100);

        rs.add_requirement(
            Condition::BetterThanOrEqualTo,
            RequirementMetricValue::LastPause(30),
        );

        rs
    }

    fn create_empty_guild_settings() -> GuildSettings {
        GuildSettings::new(GuildId(1))
    }

    fn create_guild_settings() -> GuildSettings {
        let mut gs = create_empty_guild_settings();

        gs.add("pp".to_string(), create_5kpp_ss_50_country_role_settings())
            .add("pp".to_string(), create_10kpp_role_settings())
            // just to test if it overwrite previous one
            .add("pp".to_string(), create_10kpp_role_settings())
            .add("rank".to_string(), create_1k_rank_role_settings())
            .add("rank".to_string(), create_500_rank_role_settings())
            .add("rank".to_string(), create_100_rank_role_settings())
            .add("clan".to_string(), create_clan_member_role_settings())
            .add("no-pause".to_string(), create_no_pause_role_settings());

        gs
    }

    #[test]
    fn it_properly_compares_player_metrics_with_requirement_metric() {
        let requirement_metric = RequirementMetricValue::TopPp(100.0);
        let better = PlayerMetricValue::TopPp(101.0);
        let worse = PlayerMetricValue::TopPp(99.0);

        assert!(requirement_metric > worse);
        assert!(requirement_metric < better);
        assert_eq!(requirement_metric, requirement_metric);

        let requirement_metric = RequirementMetricValue::Rank(100);
        let better = PlayerMetricValue::Rank(99);
        let worse = PlayerMetricValue::Rank(101);

        assert!(requirement_metric > worse);
        assert!(requirement_metric < better);
        assert_eq!(requirement_metric, requirement_metric);

        let requirement_metric =
            RequirementMetricValue::Clan(vec!["Clan1".to_string(), "Clan2".to_string()]);
        let ok = PlayerMetricValue::Clan(vec![
            "Clan1".to_string(),
            "Clan2".to_string(),
            "Other1".to_string(),
        ]);
        let fail = PlayerMetricValue::Clan(vec!["Clan1".to_string(), "Other1".to_string()]);

        assert!(requirement_metric.is_contained_by(&ok));
        assert!(!requirement_metric.is_contained_by(&fail));

        let requirement_metric = RequirementMetricValue::LastPause(30);
        let no_pause = PlayerMetricValue::LastPause(None);
        let more_than_30_days_ago =
            PlayerMetricValue::LastPause(Some(Utc::now() - Duration::days(50)));
        let less_than_30_days_ago =
            PlayerMetricValue::LastPause(Some(Utc::now() - Duration::days(3)));

        assert!(requirement_metric < no_pause);
        assert!(requirement_metric < more_than_30_days_ago);
        assert!(requirement_metric > less_than_30_days_ago);
    }

    #[test]
    fn it_check_if_requirement_is_fulfilled() {
        let requirement = Requirement {
            condition: Condition::BetterThanOrEqualTo,
            value: RequirementMetricValue::TopPp(100.0),
        };
        assert!(requirement.is_fulfilled_for(&PlayerMetricValue::TopPp(100.0)));
        assert!(requirement.is_fulfilled_for(&PlayerMetricValue::TopPp(150.0)));
        assert!(!requirement.is_fulfilled_for(&PlayerMetricValue::TopPp(90.0)));

        let requirement = Requirement {
            condition: Condition::BetterThanOrEqualTo,
            value: RequirementMetricValue::Rank(100),
        };
        assert!(requirement.is_fulfilled_for(&PlayerMetricValue::Rank(100)));
        assert!(requirement.is_fulfilled_for(&PlayerMetricValue::Rank(90)));
        assert!(!requirement.is_fulfilled_for(&PlayerMetricValue::Rank(101)));

        let requirement = Requirement {
            condition: Condition::BetterThanOrEqualTo,
            value: RequirementMetricValue::LastPause(30),
        };
        let no_pause = PlayerMetricValue::LastPause(None);
        let exactly_30_days_ago =
            PlayerMetricValue::LastPause(Some(Utc::now() - Duration::days(30)));
        let more_than_30_days_ago =
            PlayerMetricValue::LastPause(Some(Utc::now() - Duration::days(50)));
        let less_than_30_days_ago =
            PlayerMetricValue::LastPause(Some(Utc::now() - Duration::days(3)));

        assert!(requirement.is_fulfilled_for(&no_pause));
        assert!(requirement.is_fulfilled_for(&more_than_30_days_ago));
        assert!(requirement.is_fulfilled_for(&exactly_30_days_ago));
        assert!(!requirement.is_fulfilled_for(&less_than_30_days_ago));
    }

    #[test]
    fn it_generates_next_role_condition_id() {
        let rs = create_5kpp_ss_50_country_role_settings();

        assert_eq!(rs.conditions.len(), 3);

        let mut vec = rs
            .conditions
            .into_keys()
            .collect::<Vec<RoleRequirementId>>();
        vec.sort_unstable();

        assert_eq!(vec, [1, 2, 3]);
    }

    #[test]
    fn it_can_get_player_metric_value_from_player() {
        let player = Player {
            pp: 12000.0,
            top_pp: 400.0,
            top_accuracy: 91.0,
            country_rank: 20,
            rank: 1000,
            total_replay_watched: 200,
            watched_replays: 1000,
            top1_count: 10,
            top_stars: 11.5,
            max_streak: 5,
            last_ranked_paused_at: None,
            clans: vec!["Clan1".to_string()],
            ..Default::default()
        };

        assert_eq!(
            player.get_metric_with_value(Metric::TotalPp),
            PlayerMetricValue::TotalPp(12000.0)
        );
        assert_eq!(
            player.get_metric_with_value(Metric::TopPp),
            PlayerMetricValue::TopPp(400.0)
        );
        assert_eq!(
            player.get_metric_with_value(Metric::TopAcc),
            PlayerMetricValue::TopAcc(91.0)
        );
        assert_eq!(
            player.get_metric_with_value(Metric::CountryRank),
            PlayerMetricValue::CountryRank(20)
        );
        assert_eq!(
            player.get_metric_with_value(Metric::Rank),
            PlayerMetricValue::Rank(1000)
        );

        assert_eq!(
            player.get_metric_with_value(Metric::Clan),
            PlayerMetricValue::Clan(vec!["Clan1".to_string()])
        );

        assert_eq!(
            player.get_metric_with_value(Metric::MaxStreak),
            PlayerMetricValue::MaxStreak(5)
        );

        assert_eq!(
            player.get_metric_with_value(Metric::ReplaysIWatched),
            PlayerMetricValue::ReplaysIWatched(1000)
        );

        assert_eq!(
            player.get_metric_with_value(Metric::MyReplaysWatched),
            PlayerMetricValue::MyReplaysWatched(200)
        );

        assert_eq!(
            player.get_metric_with_value(Metric::Top1Count),
            PlayerMetricValue::Top1Count(10)
        );

        assert_eq!(
            player.get_metric_with_value(Metric::TopStars),
            PlayerMetricValue::TopStars(11.5)
        );

        assert_eq!(
            player.get_metric_with_value(Metric::LastPause),
            PlayerMetricValue::LastPause(None)
        );
    }

    #[test]
    fn it_check_if_player_metric_fulfills_role_setting_conditions() {
        let rs_5k = create_5kpp_ss_50_country_role_settings();
        let rs_10k = create_10kpp_role_settings();
        let rs_clan = create_clan_member_role_settings();
        let rs_no_pause = create_no_pause_role_settings();

        let mut player = Player {
            pp: 12000.0,
            top_accuracy: 91.0,
            country_rank: 20,
            clans: vec!["Clan1".to_string(), "Clan2".to_string()],
            last_ranked_paused_at: Some(Utc::now() - Duration::days(50)),
            ..Default::default()
        };

        assert!(rs_5k.is_fulfilled_for(&player));
        assert!(rs_10k.is_fulfilled_for(&player));
        assert!(rs_clan.is_fulfilled_for(&player));
        assert!(rs_no_pause.is_fulfilled_for(&player));

        player.top_accuracy = 89.0;
        assert!(!rs_5k.is_fulfilled_for(&player));

        player.top_accuracy = 91.0;
        player.country_rank = 100;
        assert!(!rs_5k.is_fulfilled_for(&player));

        player.pp = 7000.0;
        player.country_rank = 10;

        assert!(rs_5k.is_fulfilled_for(&player));
        assert!(!rs_10k.is_fulfilled_for(&player));

        player.clans = vec!["Other clan".to_string()];
        assert!(!rs_clan.is_fulfilled_for(&player));

        player.last_ranked_paused_at = Some(Utc::now() - Duration::days(3));
        assert!(!rs_no_pause.is_fulfilled_for(&player));
    }

    #[test]
    fn it_can_add_role_settings_to_guild() {
        let gs = create_guild_settings();

        assert_eq!(gs.role_groups.keys().len(), 4);
        assert_eq!(gs.role_groups.get("pp").unwrap().keys().len(), 2);
        assert_eq!(gs.role_groups.get("rank").unwrap().keys().len(), 3);
        assert_eq!(gs.role_groups.get("clan").unwrap().keys().len(), 1);
        assert_eq!(gs.role_groups.get("no-pause").unwrap().keys().len(), 1);
    }

    #[test]
    fn it_can_merge_role_conditions() {
        let mut gs = create_empty_guild_settings();

        let mut rs = RoleSettings::new(RoleId(1), 1000);

        rs.add_requirement(
            Condition::BetterThanOrEqualTo,
            RequirementMetricValue::TotalPp(5000.0),
        );

        gs.merge("pp".to_string(), create_5kpp_ss_50_country_role_settings())
            .merge("pp".to_string(), rs);

        let role_conditions = gs.role_groups.get("pp").unwrap().get(&RoleId(1)).unwrap();

        assert_eq!(gs.role_groups.keys().len(), 1);
        assert_eq!(gs.role_groups.get("pp").unwrap().keys().len(), 1);
        assert_eq!(role_conditions.conditions.len(), 4);
        assert_eq!(role_conditions.weight, 1000);
    }

    #[test]
    fn it_can_remove_role_settings_from_guild() {
        let mut gs = create_guild_settings();

        gs.remove("invalid-group".to_string(), RoleId(1));
        gs.remove("rank".to_string(), RoleId(1));
        gs.remove("rank".to_string(), RoleId(3));
        gs.remove("rank".to_string(), RoleId(5));

        assert_eq!(gs.role_groups.keys().len(), 4);
        assert_eq!(gs.role_groups.get("pp").unwrap().keys().len(), 2);
        assert_eq!(gs.role_groups.get("rank").unwrap().keys().len(), 1);
        assert_eq!(gs.role_groups.get("clan").unwrap().keys().len(), 1);
        assert_eq!(gs.role_groups.get("no-pause").unwrap().keys().len(), 1);

        gs.remove("rank".to_string(), RoleId(4));
        assert!(!gs.role_groups.contains_key("rank"));
    }

    #[test]
    fn it_can_check_if_role_exists_in_guild_role_group() {
        let gs = create_guild_settings();

        assert!(!gs.contains_in_group("invalid".to_string(), RoleId(1)));
        assert!(!gs.contains_in_group("rank".to_string(), RoleId(1000)));
        assert!(gs.contains_in_group("rank".to_string(), RoleId(3)));
        assert!(gs.contains_in_group("rank".to_string(), RoleId(5)));
    }

    #[test]
    fn it_can_get_all_roles_set_in_guild() {
        let gs = create_guild_settings();

        let mut roles = gs.all_roles();
        roles.sort_unstable();

        assert_eq!(
            roles,
            vec![
                &RoleId(1),
                &RoleId(2),
                &RoleId(3),
                &RoleId(4),
                &RoleId(5),
                &RoleId(6),
                &RoleId(7)
            ]
        );
    }

    #[test]
    fn it_can_check_if_role_exists_in_any_guild_role_group() {
        let gs = create_guild_settings();

        assert!(!gs.contains(RoleId(1000)));
        assert!(gs.contains(RoleId(1)));
        assert!(gs.contains(RoleId(5)));
    }

    #[test]
    fn it_resolves_which_roles_should_be_added_and_removed() {
        let gs = create_guild_settings();

        let mut player = Player {
            pp: 7000.0,
            top_accuracy: 91.0,
            rank: 1001,
            country_rank: 20,
            last_ranked_paused_at: Some(Utc::now() - Duration::days(1)),
            ..Default::default()
        };

        let mut roles_updates =
            gs.get_role_updates(GuildId(1), &player, &vec![RoleId(1), RoleId(3), RoleId(7)]);

        roles_updates.to_add.sort_unstable();
        roles_updates.to_remove.sort_unstable();

        assert_eq!(roles_updates.to_add, Vec::<RoleId>::new());
        assert_eq!(roles_updates.to_remove, vec![RoleId(3), RoleId(7)]);

        player.top_accuracy = 89.0;

        let mut roles_updates = gs.get_role_updates(GuildId(1), &player, &vec![RoleId(1)]);

        roles_updates.to_add.sort_unstable();
        roles_updates.to_remove.sort_unstable();

        assert_eq!(roles_updates.to_add, Vec::<RoleId>::new());
        assert_eq!(roles_updates.to_remove, vec![RoleId(1)]);

        player.pp = 10000.0;

        let mut roles_updates = gs.get_role_updates(GuildId(1), &player, &vec![]);

        roles_updates.to_add.sort_unstable();
        roles_updates.to_remove.sort_unstable();

        assert_eq!(roles_updates.to_add, vec![RoleId(2)]);
        assert_eq!(roles_updates.to_remove, Vec::<RoleId>::new());

        player.rank = 1000;

        let mut roles_updates = gs.get_role_updates(GuildId(1), &player, &vec![RoleId(2)]);

        roles_updates.to_add.sort_unstable();
        roles_updates.to_remove.sort_unstable();

        assert_eq!(roles_updates.to_add, vec![RoleId(3)]);
        assert_eq!(roles_updates.to_remove, Vec::<RoleId>::new());

        player.rank = 500;

        let mut roles_updates =
            gs.get_role_updates(GuildId(1), &player, &vec![RoleId(2), RoleId(3)]);

        roles_updates.to_add.sort_unstable();
        roles_updates.to_remove.sort_unstable();

        assert_eq!(roles_updates.to_add, vec![RoleId(4)]);
        assert_eq!(roles_updates.to_remove, vec![RoleId(3)]);

        player.clans = vec!["Clan1".to_string()];

        let mut roles_updates =
            gs.get_role_updates(GuildId(1), &player, &vec![RoleId(2), RoleId(3)]);

        roles_updates.to_add.sort_unstable();
        roles_updates.to_remove.sort_unstable();

        assert_eq!(roles_updates.to_add, vec![RoleId(4), RoleId(6)]);
        assert_eq!(roles_updates.to_remove, vec![RoleId(3)]);

        player.clans = vec!["Other".to_string()];

        let mut roles_updates =
            gs.get_role_updates(GuildId(1), &player, &vec![RoleId(2), RoleId(3), RoleId(6)]);

        roles_updates.to_add.sort_unstable();
        roles_updates.to_remove.sort_unstable();

        assert_eq!(roles_updates.to_add, vec![RoleId(4)]);
        assert_eq!(roles_updates.to_remove, vec![RoleId(3), RoleId(6)]);

        player.last_ranked_paused_at = Some(Utc::now() - Duration::days(50));

        let mut roles_updates =
            gs.get_role_updates(GuildId(1), &player, &vec![RoleId(2), RoleId(3), RoleId(6)]);

        roles_updates.to_add.sort_unstable();
        roles_updates.to_remove.sort_unstable();

        assert_eq!(roles_updates.to_add, vec![RoleId(4), RoleId(7)]);
        assert_eq!(roles_updates.to_remove, vec![RoleId(3), RoleId(6)]);
    }
}
