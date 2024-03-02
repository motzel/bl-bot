use chrono::serde::{ts_seconds, ts_seconds_option};
use chrono::{DateTime, Utc};
use poise::serenity_prelude::{GuildId, UserId};
use serde::{Deserialize, Serialize};
use tracing::{debug, info, trace};

use crate::beatleader::clan::ClanTag;
use crate::beatleader::error::Error as BlError;
use crate::beatleader::player::{
    MapType, Player as BlPlayer, PlayerId, PlayerScoreParam, PlayerScoreSort,
};
use crate::beatleader::{BlContext, SortOrder};
use crate::discord::bot::beatleader::score::{fetch_scores, Score};
use crate::discord::bot::{Metric, PlayerMetricValue};
use crate::storage::{StorageKey, StorageValue};
use crate::BL_CLIENT;

#[derive(Serialize, Deserialize, Default, Debug, Clone)]
#[serde(rename_all = "camelCase")]
#[serde(default)]
#[non_exhaustive]
pub struct Player {
    pub id: PlayerId,
    pub user_id: UserId,
    pub linked_guilds: Vec<GuildId>,
    pub name: String,
    pub active: bool,
    pub avatar: String,
    pub profile_cover: Option<String>,
    pub country: String,
    pub rank: u32,
    pub country_rank: u32,
    pub pp: f64,
    pub acc_pp: f64,
    pub tech_pp: f64,
    pub pass_pp: f64,
    pub max_streak: u32,
    pub ranked_max_streak: u32,
    pub unranked_max_streak: u32,
    pub avg_accuracy: f64,
    pub avg_ranked_accuracy: f64,
    pub avg_unranked_accuracy: f64,
    pub top_accuracy: f64,
    pub top_ranked_accuracy: f64,
    pub top_unranked_accuracy: f64,
    pub top_acc_pp: f64,
    pub top_tech_pp: f64,
    pub top_pass_pp: f64,
    pub top_pp: f64,
    pub top_stars: f64,
    pub plus_1pp: f64,
    pub total_play_count: u32,
    pub ranked_play_count: u32,
    pub unranked_play_count: u32,
    pub peak_rank: u32,
    pub top1_count: u32,
    pub anonymous_replay_watched: u32,
    pub authorized_replay_watched: u32,
    pub total_replay_watched: u32,
    pub watched_replays: u32,
    pub clans: Vec<String>,
    pub is_verified: bool,
    #[serde(with = "ts_seconds")]
    pub last_ranked_score_time: DateTime<Utc>,
    #[serde(with = "ts_seconds")]
    pub last_unranked_score_time: DateTime<Utc>,
    #[serde(with = "ts_seconds")]
    pub last_score_time: DateTime<Utc>,
    #[serde(with = "ts_seconds_option")]
    pub last_fetch: Option<DateTime<Utc>>,
    #[serde(with = "ts_seconds_option")]
    pub last_scores_fetch: Option<DateTime<Utc>>,
    #[serde(with = "ts_seconds_option")]
    pub last_ranked_paused_at: Option<DateTime<Utc>>,
}

impl StorageKey for UserId {}

impl StorageValue<UserId> for Player {
    fn get_key(&self) -> UserId {
        self.user_id
    }
}

impl Player {
    pub fn get_key(&self) -> UserId {
        self.user_id
    }

    pub fn from_user_id_and_bl_player(
        user_id: UserId,
        guild_ids: Vec<GuildId>,
        bl_player: BlPlayer,
        previous: Option<&Player>,
    ) -> Self {
        Player {
            id: bl_player.id,
            user_id,
            linked_guilds: guild_ids,
            name: bl_player.name,
            active: !bl_player.inactive && !bl_player.banned && !bl_player.bot,
            avatar: bl_player.avatar,
            profile_cover: bl_player.profile_settings.profile_cover,
            country: bl_player.country,
            clans: bl_player
                .clans
                .iter()
                .map(|clan| clan.tag.clone())
                .collect::<Vec<String>>(),
            rank: bl_player.rank,
            country_rank: bl_player.country_rank,
            pp: bl_player.pp,
            acc_pp: bl_player.acc_pp,
            tech_pp: bl_player.tech_pp,
            pass_pp: bl_player.pass_pp,
            max_streak: bl_player.score_stats.max_streak,
            ranked_max_streak: bl_player.score_stats.ranked_max_streak,
            unranked_max_streak: bl_player.score_stats.unranked_max_streak,
            avg_accuracy: bl_player.score_stats.average_accuracy * 100.0,
            avg_ranked_accuracy: bl_player.score_stats.average_ranked_accuracy * 100.0,
            avg_unranked_accuracy: bl_player.score_stats.average_unranked_accuracy * 100.0,
            top_accuracy: bl_player.score_stats.top_accuracy * 100.0,
            top_ranked_accuracy: bl_player.score_stats.top_ranked_accuracy * 100.0,
            top_unranked_accuracy: bl_player.score_stats.top_unranked_accuracy * 100.0,
            top_acc_pp: bl_player.score_stats.top_acc_pp,
            top_tech_pp: bl_player.score_stats.top_tech_pp,
            top_pass_pp: bl_player.score_stats.top_pass_pp,
            top_pp: bl_player.score_stats.top_pp,
            top_stars: if let Some(old_player) = previous {
                old_player.top_stars
            } else {
                0.0
            },
            plus_1pp: if let Some(old_player) = previous {
                old_player.plus_1pp
            } else {
                0.0
            },
            total_play_count: bl_player.score_stats.total_play_count,
            ranked_play_count: bl_player.score_stats.ranked_play_count,
            unranked_play_count: bl_player.score_stats.unranked_play_count,
            peak_rank: bl_player.score_stats.peak_rank,
            top1_count: bl_player.score_stats.top1_count,
            anonymous_replay_watched: bl_player.score_stats.anonymous_replay_watched,
            authorized_replay_watched: bl_player.score_stats.authorized_replay_watched,
            total_replay_watched: bl_player.score_stats.anonymous_replay_watched
                + bl_player.score_stats.authorized_replay_watched,
            watched_replays: bl_player.score_stats.watched_replays,
            is_verified: bl_player
                .socials
                .iter()
                .any(|social| social.service == "Discord" && social.user_id == user_id.to_string()),
            last_ranked_score_time: bl_player.score_stats.last_ranked_score_time,
            last_unranked_score_time: bl_player.score_stats.last_unranked_score_time,
            last_score_time: bl_player.score_stats.last_score_time,
            last_fetch: if let Some(player) = previous {
                player.last_fetch
            } else {
                None
            },
            last_scores_fetch: if let Some(player) = previous {
                player.last_scores_fetch
            } else {
                None
            },
            last_ranked_paused_at: if let Some(player) = previous {
                player.last_ranked_paused_at
            } else {
                None
            },
        }
    }

    pub(crate) fn is_clan_member(&self, clan_tag: &ClanTag) -> bool {
        !self.clans.is_empty() && self.clans.contains(clan_tag)
    }

    pub(crate) fn is_primary_clan_member(&self, clan_tag: &ClanTag) -> bool {
        !self.clans.is_empty() && self.clans.first().unwrap() == clan_tag
    }

    pub(crate) fn is_linked_to_any_guild(&self) -> bool {
        !self.linked_guilds.is_empty()
    }

    pub(crate) fn is_linked_to_guild(&self, guild_id: &GuildId) -> bool {
        self.linked_guilds.contains(guild_id)
    }

    pub(crate) fn get_metric_with_value(&self, metric: Metric) -> PlayerMetricValue {
        match metric {
            Metric::TopPp => PlayerMetricValue::TopPp(self.top_pp),
            Metric::TopAcc => PlayerMetricValue::TopAcc(self.top_accuracy),
            Metric::TotalPp => PlayerMetricValue::TotalPp(self.pp),
            Metric::Rank => PlayerMetricValue::Rank(self.rank),
            Metric::CountryRank => PlayerMetricValue::CountryRank(self.country_rank),

            Metric::MaxStreak => PlayerMetricValue::MaxStreak(self.max_streak),
            Metric::Top1Count => PlayerMetricValue::Top1Count(self.top1_count),
            Metric::MyReplaysWatched => {
                PlayerMetricValue::MyReplaysWatched(self.total_replay_watched)
            }
            Metric::ReplaysIWatched => PlayerMetricValue::ReplaysIWatched(self.watched_replays),
            Metric::Clan => PlayerMetricValue::Clan(self.clans.clone()),
            Metric::TopStars => PlayerMetricValue::TopStars(self.top_stars),
            Metric::LastPause => PlayerMetricValue::LastPause(self.last_ranked_paused_at),
        }
    }
}

pub(crate) async fn fetch_player_from_bl(player_id: &PlayerId) -> Result<BlPlayer, BlError> {
    BL_CLIENT.player().get(player_id).await
}

pub(crate) async fn fetch_player_from_bl_by_user_id(user_id: &UserId) -> Result<BlPlayer, BlError> {
    BL_CLIENT.player().get_by_discord(user_id).await
}

pub(crate) async fn fetch_all_player_scores(
    player: &Player,
    bl_context: BlContext,
    force: bool,
) -> Result<Option<Vec<Score>>, BlError> {
    info!("Fetching all ranked scores of {}...", player.name);

    if !force
        && player.last_scores_fetch.is_some()
        && player.last_scores_fetch.unwrap() > player.last_ranked_score_time
        && player.last_scores_fetch.unwrap() > Utc::now() - chrono::Duration::hours(24)
    {
        info!(
            "No new scores since last fetching ({}), skipping.",
            player.last_scores_fetch.unwrap()
        );

        return Ok(None);
    }

    const ITEMS_PER_PAGE: u32 = 100;

    let time_param: Vec<PlayerScoreParam> = match player.last_scores_fetch {
        Some(last_scores_fetch) => {
            if force {
                vec![]
            } else {
                vec![PlayerScoreParam::TimeFrom(last_scores_fetch)]
            }
        }
        None => vec![],
    };

    let mut player_scores = Vec::<Score>::with_capacity(player.ranked_play_count as usize);

    let mut page = 1;
    let mut page_count = 1;
    'outer: loop {
        trace!("Fetching scores page {} / {}...", page, page_count);

        match fetch_scores(
            &player.id,
            &[
                &[
                    PlayerScoreParam::Page(page),
                    PlayerScoreParam::Count(ITEMS_PER_PAGE),
                    PlayerScoreParam::Sort(PlayerScoreSort::Date),
                    PlayerScoreParam::Order(SortOrder::Ascending),
                    PlayerScoreParam::Type(MapType::Ranked),
                    PlayerScoreParam::Context(bl_context.clone()),
                ],
                &time_param[..],
            ]
            .concat(),
        )
        .await
        {
            Ok(scores_page) => {
                debug!("Scores page #{} fetched.", page);

                if scores_page.data.is_empty() {
                    break 'outer;
                }

                page_count = scores_page.total.div_ceil(ITEMS_PER_PAGE);

                for score in scores_page.data {
                    if score.modifiers.contains("NF")
                        || score.modifiers.contains("NB")
                        || score.modifiers.contains("NO")
                        || score.modifiers.contains("NA")
                        || score.modifiers.contains("OP")
                    {
                        continue;
                    }

                    player_scores.push(score);
                }
            }
            Err(e) => {
                return Err(e);
            }
        };

        page += 1;

        if page > page_count {
            break;
        }
    }

    info!("All ranked scores of {} fetched.", player.name);

    Ok(Some(player_scores))
}
