#![allow(dead_code)]
#![allow(unused_imports)]

use std::cmp::Ordering;
use std::collections::HashMap;
use std::collections::HashSet;
use std::sync::Arc;

use log::{debug, error, info};
use poise::serenity_prelude as serenity;
use poise::serenity_prelude::{ChannelId, User, UserId};
use poise::SlashArgument;
use serde::{Deserialize, Serialize};
use serenity::model::gateway::Activity;
use serenity::model::id::GuildId;
use serenity::model::prelude::RoleId;
use shuttle_poise::ShuttlePoise;
use shuttle_secrets::SecretStore;

use crate::beatleader::player::PlayerId;
use crate::bot::beatleader::{fetch_scores, Player};
use crate::Context;
use crate::Error;

pub(crate) mod beatleader;
pub(crate) mod commands;

#[derive(Serialize, Deserialize, Clone, Debug, poise::ChoiceParameter)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub(crate) enum PlayerMetric {
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
}

impl From<&PlayerMetricWithValue> for PlayerMetric {
    fn from(value: &PlayerMetricWithValue) -> Self {
        match value {
            PlayerMetricWithValue::TopPp(_) => PlayerMetric::TopPp,
            PlayerMetricWithValue::TopAcc(_) => PlayerMetric::TopAcc,
            PlayerMetricWithValue::TotalPp(_) => PlayerMetric::TotalPp,
            PlayerMetricWithValue::Rank(_) => PlayerMetric::Rank,
            PlayerMetricWithValue::CountryRank(_) => PlayerMetric::CountryRank,
            PlayerMetricWithValue::MaxStreak(_) => PlayerMetric::MaxStreak,
            PlayerMetricWithValue::Top1Count(_) => PlayerMetric::Top1Count,
            PlayerMetricWithValue::MyReplaysWatched(_) => PlayerMetric::MyReplaysWatched,
            PlayerMetricWithValue::ReplaysIWatched(_) => PlayerMetric::ReplaysIWatched,
            PlayerMetricWithValue::Clan(_) => PlayerMetric::Clan,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, poise::ChoiceParameter)]
#[serde(rename_all = "camelCase")]
pub(crate) enum MetricCondition {
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

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
#[non_exhaustive]
pub(crate) enum PlayerMetricWithValue {
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
}

impl PlayerMetricWithValue {
    pub fn new(metric: PlayerMetric, value: &str) -> Result<Self, Error> {
        match metric {
            PlayerMetric::TotalPp => Ok(PlayerMetricWithValue::TotalPp(value.parse::<f64>()?)),
            PlayerMetric::TopPp => Ok(PlayerMetricWithValue::TopPp(value.parse::<f64>()?)),
            PlayerMetric::Rank => Ok(PlayerMetricWithValue::Rank(value.parse::<u32>()?)),
            PlayerMetric::CountryRank => {
                Ok(PlayerMetricWithValue::CountryRank(value.parse::<u32>()?))
            }
            PlayerMetric::TopAcc => Ok(PlayerMetricWithValue::TopAcc(value.parse::<f64>()?)),
            PlayerMetric::MaxStreak => Ok(PlayerMetricWithValue::MaxStreak(value.parse::<u32>()?)),
            PlayerMetric::Top1Count => Ok(PlayerMetricWithValue::Top1Count(value.parse::<u32>()?)),
            PlayerMetric::MyReplaysWatched => Ok(PlayerMetricWithValue::MyReplaysWatched(
                value.parse::<u32>()?,
            )),
            PlayerMetric::ReplaysIWatched => Ok(PlayerMetricWithValue::ReplaysIWatched(
                value.parse::<u32>()?,
            )),
            PlayerMetric::Clan => {
                if value.len() < 2 || value.len() > 4 {
                    return Err(From::from("name of the clan should have 2 to 4 characters"));
                }

                Ok(PlayerMetricWithValue::Clan(vec![value.to_string()]))
            }
        }
    }

    pub fn is_fulfilled_for(
        &self,
        condition: &MetricCondition,
        value: &PlayerMetricWithValue,
    ) -> bool {
        if std::mem::discriminant(&PlayerMetric::from(self))
            != std::mem::discriminant(&PlayerMetric::from(value))
        {
            return false;
        }

        match condition {
            MetricCondition::WorseThan => self.lt(value),
            MetricCondition::WorseThanOrEqualTo => self.le(value),
            MetricCondition::EqualTo => self.eq(value),
            MetricCondition::BetterThan => self.gt(value),
            MetricCondition::BetterThanOrEqualTo => self.ge(value),
            MetricCondition::Contains => self.contains(value),
        }
    }

    pub fn contains(&self, other: &Self) -> bool {
        match self {
            PlayerMetricWithValue::TopPp(_) => false,
            PlayerMetricWithValue::TopAcc(_) => false,
            PlayerMetricWithValue::TotalPp(_) => false,
            PlayerMetricWithValue::Rank(_) => false,
            PlayerMetricWithValue::CountryRank(_) => false,
            PlayerMetricWithValue::MaxStreak(_) => false,
            PlayerMetricWithValue::Top1Count(_) => false,
            PlayerMetricWithValue::MyReplaysWatched(_) => false,
            PlayerMetricWithValue::ReplaysIWatched(_) => false,
            PlayerMetricWithValue::Clan(player_clans) => {
                if let PlayerMetricWithValue::Clan(clans) = other {
                    clans.iter().all(|clan| player_clans.contains(clan))
                } else {
                    false
                }
            }
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
                    PlayerMetricWithValue::reverse_ordering(v.partial_cmp(o))
                } else {
                    None
                }
            }
            PlayerMetricWithValue::CountryRank(v) => {
                if let PlayerMetricWithValue::CountryRank(o) = other {
                    PlayerMetricWithValue::reverse_ordering(v.partial_cmp(o))
                } else {
                    None
                }
            }
            PlayerMetricWithValue::MaxStreak(v) => {
                if let PlayerMetricWithValue::MaxStreak(o) = other {
                    v.partial_cmp(o)
                } else {
                    None
                }
            }
            PlayerMetricWithValue::Top1Count(v) => {
                if let PlayerMetricWithValue::Top1Count(o) = other {
                    v.partial_cmp(o)
                } else {
                    None
                }
            }
            PlayerMetricWithValue::MyReplaysWatched(v) => {
                if let PlayerMetricWithValue::MyReplaysWatched(o) = other {
                    v.partial_cmp(o)
                } else {
                    None
                }
            }
            PlayerMetricWithValue::ReplaysIWatched(v) => {
                if let PlayerMetricWithValue::ReplaysIWatched(o) = other {
                    v.partial_cmp(o)
                } else {
                    None
                }
            }
            PlayerMetricWithValue::Clan(_) => None,
        }
    }
}

pub(crate) type RoleGroup = String;

type RoleConditionId = u32;

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct RoleCondition {
    condition: MetricCondition,
    value: PlayerMetricWithValue,
}

impl std::fmt::Display for RoleCondition {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match &self.value {
                PlayerMetricWithValue::TopPp(v) => format!(
                    "**Top PP** *{}* **{}**",
                    self.condition.to_string().to_lowercase(),
                    v
                ),
                PlayerMetricWithValue::TopAcc(v) => format!(
                    "**Top Acc** *{}* **{}**",
                    self.condition.to_string().to_lowercase(),
                    v
                ),
                PlayerMetricWithValue::TotalPp(v) => format!(
                    "**Total PP** *{}* **{}**",
                    self.condition.to_string().to_lowercase(),
                    v
                ),
                PlayerMetricWithValue::Rank(v) => format!(
                    "**Rank** *{}* **{}**",
                    self.condition.to_string().to_lowercase(),
                    v
                ),
                PlayerMetricWithValue::CountryRank(v) => format!(
                    "**Country rank** *{}* **{}**",
                    self.condition.to_string().to_lowercase(),
                    v
                ),
                PlayerMetricWithValue::MaxStreak(v) => format!(
                    "**Max streak** *{}* **{}**",
                    self.condition.to_string().to_lowercase(),
                    v
                ),
                PlayerMetricWithValue::Top1Count(v) => format!(
                    "**#1 count** *{}* **{}**",
                    self.condition.to_string().to_lowercase(),
                    v
                ),
                PlayerMetricWithValue::MyReplaysWatched(v) => format!(
                    "**My replays watched** *{}* **{}**",
                    self.condition.to_string().to_lowercase(),
                    v
                ),
                PlayerMetricWithValue::ReplaysIWatched(v) => format!(
                    "**I watched replays** *{}* **{}**",
                    self.condition.to_string().to_lowercase(),
                    v
                ),
                PlayerMetricWithValue::Clan(v) => format!(
                    "**Clan** *{}* **{}**",
                    self.condition.to_string().to_lowercase(),
                    v.join(", "),
                ),
            }
        )
    }
}

#[derive(Default, Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct RoleSettings {
    role_id: RoleId,
    conditions: HashMap<RoleConditionId, RoleCondition>,
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

    fn get_next_condition_id(&self) -> RoleConditionId {
        self.conditions
            .keys()
            .fold(0, |acc, condition_id| acc.max(*condition_id))
            + 1
    }

    pub(crate) fn add_condition(
        &mut self,
        condition: MetricCondition,
        value: PlayerMetricWithValue,
    ) {
        let rc = RoleCondition { condition, value };

        self.conditions
            .entry(self.get_next_condition_id())
            .or_insert(rc);
    }

    pub fn is_fulfilled_for(&self, player: &Player) -> bool {
        self.conditions.iter().all(|(_role_id, role_condition)| {
            player
                .get_metric_with_value(PlayerMetric::from(&role_condition.value))
                .is_fulfilled_for(&role_condition.condition, &role_condition.value)
        })
    }
}

impl std::fmt::Display for RoleSettings {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        let mut cond_vec = self
            .conditions
            .iter()
            .map(|(cond_id, cond)| (*cond_id, cond.clone()))
            .collect::<Vec<(RoleConditionId, RoleCondition)>>();
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
            debug!(
                "Adding role {} to user {} ({})",
                role_id, self.user_id, self.name
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

            debug!(
                "Role {} added to user {} ({})",
                role_id, self.user_id, self.name
            );
        }

        info!(
            "{} role(s) to remove from user {} ({})",
            self.to_remove.len(),
            self.user_id,
            self.name
        );

        for role_id in self.to_remove.iter() {
            debug!(
                "Removing role {} from user {} ({})",
                role_id, self.user_id, self.name
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

            debug!(
                "Role {} removed from user {} ({})",
                role_id, self.user_id, self.name
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
    role_groups: HashMap<RoleGroup, HashMap<RoleId, RoleSettings>>,
}

impl GuildSettings {
    pub fn new(guild_id: GuildId) -> Self {
        Self {
            guild_id,
            bot_channel_id: None,
            role_groups: HashMap::new(),
        }
    }

    pub fn get_channel(&self) -> Option<ChannelId> {
        self.bot_channel_id
    }

    pub fn set_channel(&mut self, channel_id: Option<ChannelId>) {
        self.bot_channel_id = channel_id;
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
                    .for_each(|rc| rs.add_condition(rc.condition.clone(), rc.value.clone()));
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
            "# __Current settings__\nBot log channel: {}\n## Auto roles:\n{}",
            self.bot_channel_id.map_or_else(
                || "**None**".to_owned(),
                |channel_id| format!("<#{}>", channel_id.to_owned())
            ),
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

#[cfg(test)]
mod tests {
    use poise::serenity_prelude::UserId;

    use crate::bot::{
        GuildId, GuildSettings, MetricCondition, Player, PlayerMetric, PlayerMetricWithValue,
        RoleConditionId, RoleId, RoleSettings,
    };

    fn create_5kpp_ss_50_country_role_settings() -> RoleSettings {
        let mut rs = RoleSettings::new(RoleId(1), 100);

        rs.add_condition(
            MetricCondition::BetterThanOrEqualTo,
            PlayerMetricWithValue::TotalPp(5000.0),
        );

        rs.add_condition(
            MetricCondition::BetterThanOrEqualTo,
            PlayerMetricWithValue::TopAcc(90.0),
        );

        rs.add_condition(
            MetricCondition::BetterThanOrEqualTo,
            PlayerMetricWithValue::CountryRank(50),
        );

        rs
    }
    fn create_10kpp_role_settings() -> RoleSettings {
        let mut rs = RoleSettings::new(RoleId(2), 200);

        rs.add_condition(
            MetricCondition::BetterThanOrEqualTo,
            PlayerMetricWithValue::TotalPp(10000.0),
        );

        rs
    }
    fn create_1k_rank_role_settings() -> RoleSettings {
        let mut rs = RoleSettings::new(RoleId(3), 100);

        rs.add_condition(
            MetricCondition::BetterThanOrEqualTo,
            PlayerMetricWithValue::Rank(1000),
        );

        rs
    }
    fn create_500_rank_role_settings() -> RoleSettings {
        let mut rs = RoleSettings::new(RoleId(4), 200);

        rs.add_condition(
            MetricCondition::BetterThanOrEqualTo,
            PlayerMetricWithValue::Rank(500),
        );

        rs
    }
    fn create_100_rank_role_settings() -> RoleSettings {
        let mut rs = RoleSettings::new(RoleId(5), 300);

        rs.add_condition(
            MetricCondition::BetterThanOrEqualTo,
            PlayerMetricWithValue::Rank(100),
        );

        rs
    }

    fn create_clan_member_role_settings() -> RoleSettings {
        let mut rs = RoleSettings::new(RoleId(6), 100);

        rs.add_condition(
            MetricCondition::Contains,
            PlayerMetricWithValue::Clan(vec!["Clan1".to_string()]),
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
            .add("clan".to_string(), create_clan_member_role_settings());

        gs
    }

    #[test]
    fn it_properly_compares_player_metrics_with_value() {
        let val = PlayerMetricWithValue::TopPp(100.0);
        let better = PlayerMetricWithValue::TopPp(101.0);
        let worse = PlayerMetricWithValue::TopPp(99.0);

        assert_eq!(val > worse, true);
        assert_eq!(val < better, true);
        assert_eq!(val == val, true);

        let val = PlayerMetricWithValue::Rank(100);
        let better = PlayerMetricWithValue::Rank(99);
        let worse = PlayerMetricWithValue::Rank(101);

        assert_eq!(val > worse, true);
        assert_eq!(val < better, true);
        assert_eq!(val == val, true);

        let val = PlayerMetricWithValue::Clan(vec![
            "Clan1".to_string(),
            "Clan2".to_string(),
            "Clan3".to_string(),
        ]);
        let ok = PlayerMetricWithValue::Clan(vec!["Clan1".to_string(), "Clan2".to_string()]);
        let fail = PlayerMetricWithValue::Clan(vec!["Other1".to_string(), "Other2".to_string()]);

        assert!(val.contains(&ok));
        assert!(!val.contains(&fail));
    }

    #[test]
    fn it_check_if_condition_is_fulfilled() {
        let condition_metric = PlayerMetricWithValue::TopPp(100.0);

        let player_metric = PlayerMetricWithValue::TopPp(100.0);
        assert_eq!(
            player_metric
                .is_fulfilled_for(&MetricCondition::BetterThanOrEqualTo, &condition_metric),
            true
        );

        let player_metric = PlayerMetricWithValue::TopPp(150.0);
        assert_eq!(
            player_metric
                .is_fulfilled_for(&MetricCondition::BetterThanOrEqualTo, &condition_metric),
            true
        );

        let player_metric = PlayerMetricWithValue::TopPp(90.0);
        assert_eq!(
            player_metric
                .is_fulfilled_for(&MetricCondition::BetterThanOrEqualTo, &condition_metric),
            false
        );

        let condition_metric = PlayerMetricWithValue::Rank(100);

        let player_metric = PlayerMetricWithValue::Rank(101);
        assert_eq!(
            player_metric
                .is_fulfilled_for(&MetricCondition::BetterThanOrEqualTo, &condition_metric),
            false
        );

        let player_metric = PlayerMetricWithValue::Rank(100);
        assert_eq!(
            player_metric
                .is_fulfilled_for(&MetricCondition::BetterThanOrEqualTo, &condition_metric),
            true
        );

        let player_metric = PlayerMetricWithValue::Rank(90);
        assert_eq!(
            player_metric
                .is_fulfilled_for(&MetricCondition::BetterThanOrEqualTo, &condition_metric),
            true
        );

        let player_metric = PlayerMetricWithValue::Rank(90);
        assert_eq!(
            player_metric.is_fulfilled_for(&MetricCondition::Contains, &condition_metric),
            false
        );
    }

    #[test]
    fn it_generates_next_role_condition_id() {
        let rs = create_5kpp_ss_50_country_role_settings();

        assert_eq!(rs.conditions.len(), 3);

        let mut vec = rs.conditions.into_keys().collect::<Vec<RoleConditionId>>();
        vec.sort_unstable();

        assert_eq!(vec, [1, 2, 3]);
    }

    #[test]
    fn it_can_get_player_metric_with_value_from_player() {
        let player = Player {
            pp: 12000.0,
            top_pp: 400.0,
            top_accuracy: 91.0,
            country_rank: 20,
            rank: 1000,
            clans: vec!["Clan1".to_string()],
            ..Default::default()
        };

        assert_eq!(
            player.get_metric_with_value(PlayerMetric::TotalPp),
            PlayerMetricWithValue::TotalPp(12000.0)
        );
        assert_eq!(
            player.get_metric_with_value(PlayerMetric::TopPp),
            PlayerMetricWithValue::TopPp(400.0)
        );
        assert_eq!(
            player.get_metric_with_value(PlayerMetric::TopAcc),
            PlayerMetricWithValue::TopAcc(91.0)
        );
        assert_eq!(
            player.get_metric_with_value(PlayerMetric::CountryRank),
            PlayerMetricWithValue::CountryRank(20)
        );
        assert_eq!(
            player.get_metric_with_value(PlayerMetric::Rank),
            PlayerMetricWithValue::Rank(1000)
        );

        assert_eq!(
            player.get_metric_with_value(PlayerMetric::Clan),
            PlayerMetricWithValue::Clan(vec!["Clan1".to_string()])
        );
    }

    #[test]
    fn it_check_if_player_metric_fulfills_role_setting_conditions() {
        let rs_5k = create_5kpp_ss_50_country_role_settings();
        let rs_10k = create_10kpp_role_settings();
        let rs_clan = create_clan_member_role_settings();

        let mut player = Player {
            pp: 12000.0,
            top_accuracy: 91.0,
            country_rank: 20,
            clans: vec!["Clan1".to_string(), "Clan2".to_string()],
            ..Default::default()
        };

        assert_eq!(rs_5k.is_fulfilled_for(&player), true);
        assert_eq!(rs_10k.is_fulfilled_for(&player), true);
        assert_eq!(rs_clan.is_fulfilled_for(&player), true);

        player.top_accuracy = 89.0;
        assert_eq!(rs_5k.is_fulfilled_for(&player), false);

        player.top_accuracy = 91.0;
        player.country_rank = 100;
        assert_eq!(rs_5k.is_fulfilled_for(&player), false);

        player.pp = 7000.0;
        player.country_rank = 10;

        assert_eq!(rs_5k.is_fulfilled_for(&player), true);
        assert_eq!(rs_10k.is_fulfilled_for(&player), false);

        player.clans = vec!["Other clan".to_string()];
        assert_eq!(rs_clan.is_fulfilled_for(&player), false);
    }

    #[test]
    fn it_can_add_role_settings_to_guild() {
        let gs = create_guild_settings();

        assert_eq!(gs.role_groups.keys().len(), 3);
        assert_eq!(gs.role_groups.get("pp").unwrap().keys().len(), 2);
        assert_eq!(gs.role_groups.get("rank").unwrap().keys().len(), 3);
        assert_eq!(gs.role_groups.get("clan").unwrap().keys().len(), 1);
    }

    #[test]
    fn it_can_merge_role_conditions() {
        let mut gs = create_empty_guild_settings();

        let mut rs = RoleSettings::new(RoleId(1), 1000);

        rs.add_condition(
            MetricCondition::BetterThanOrEqualTo,
            PlayerMetricWithValue::TotalPp(5000.0),
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

        assert_eq!(gs.role_groups.keys().len(), 3);
        assert_eq!(gs.role_groups.get("pp").unwrap().keys().len(), 2);
        assert_eq!(gs.role_groups.get("rank").unwrap().keys().len(), 1);
        assert_eq!(gs.role_groups.get("clan").unwrap().keys().len(), 1);

        gs.remove("rank".to_string(), RoleId(4));
        assert_eq!(gs.role_groups.contains_key("rank"), false);
    }

    #[test]
    fn it_can_check_if_role_exists_in_guild_role_group() {
        let gs = create_guild_settings();

        assert_eq!(
            gs.contains_in_group("invalid".to_string(), RoleId(1)),
            false
        );
        assert_eq!(
            gs.contains_in_group("rank".to_string(), RoleId(1000)),
            false
        );
        assert_eq!(gs.contains_in_group("rank".to_string(), RoleId(3)), true);
        assert_eq!(gs.contains_in_group("rank".to_string(), RoleId(5)), true);
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
                &RoleId(6)
            ]
        );
    }

    #[test]
    fn it_can_check_if_role_exists_in_any_guild_role_group() {
        let gs = create_guild_settings();

        assert_eq!(gs.contains(RoleId(1000)), false);
        assert_eq!(gs.contains(RoleId(1)), true);
        assert_eq!(gs.contains(RoleId(5)), true);
    }

    #[test]
    fn it_resolves_which_roles_should_be_added_and_removed() {
        let gs = create_guild_settings();

        let mut player = Player {
            pp: 7000.0,
            top_accuracy: 91.0,
            rank: 1001,
            country_rank: 20,
            ..Default::default()
        };

        let mut roles_updates =
            gs.get_role_updates(GuildId(1), &player, &vec![RoleId(1), RoleId(3)]);

        roles_updates.to_add.sort_unstable();
        roles_updates.to_remove.sort_unstable();

        assert_eq!(roles_updates.to_add, Vec::<RoleId>::new());
        assert_eq!(roles_updates.to_remove, vec![RoleId(3)]);

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
    }
}
