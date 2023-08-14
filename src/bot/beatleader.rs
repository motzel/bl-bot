use poise::serenity_prelude::{GuildId, UserId};
use poise::CreateReply;
use serde::{Deserialize, Serialize};

use chrono::serde::{ts_seconds, ts_seconds_option};
use chrono::{DateTime, Utc};

use crate::beatleader::player::{MetaData, PlayerId};
use crate::beatleader::player::{
    Player as BlPlayer, PlayerScoreParam, PlayerScoreSort, Score as BlScore, Scores as BlScores,
};
use crate::beatleader::{error::Error as BlError, SortOrder};
use crate::bot::{PlayerMetric, PlayerMetricWithValue};
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
    pub top_accuracy: f64,
    pub top_ranked_accuracy: f64,
    pub top_unranked_accuracy: f64,
    pub top_acc_pp: f64,
    pub top_tech_pp: f64,
    pub top_pass_pp: f64,
    pub top_pp: f64,
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
}

impl Player {
    pub fn from_user_id_and_bl_player(
        user_id: UserId,
        guild_ids: Vec<GuildId>,
        bl_player: BlPlayer,
        last_fetch: Option<DateTime<Utc>>,
        last_scores_fetch: Option<DateTime<Utc>>,
    ) -> Self {
        Player {
            id: bl_player.id,
            user_id,
            linked_guilds: guild_ids,
            name: bl_player.name,
            active: !bl_player.inactive && !bl_player.banned && !bl_player.bot,
            avatar: bl_player.avatar,
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
            top_accuracy: bl_player.score_stats.top_accuracy * 100.0,
            top_ranked_accuracy: bl_player.score_stats.top_ranked_accuracy * 100.0,
            top_unranked_accuracy: bl_player.score_stats.top_unranked_accuracy * 100.0,
            top_acc_pp: bl_player.score_stats.top_acc_pp,
            top_tech_pp: bl_player.score_stats.top_tech_pp,
            top_pass_pp: bl_player.score_stats.top_pass_pp,
            top_pp: bl_player.score_stats.top_pp,
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
            last_fetch,
            last_scores_fetch,
        }
    }

    pub(crate) fn is_linked_to_any_guild(&self) -> bool {
        !self.linked_guilds.is_empty()
    }

    pub(crate) fn is_linked_to_guild(&self, guild_id: &GuildId) -> bool {
        self.linked_guilds.contains(guild_id)
    }

    pub(crate) fn get_metric_with_value(&self, metric: PlayerMetric) -> PlayerMetricWithValue {
        match metric {
            PlayerMetric::TopPp => PlayerMetricWithValue::TopPp(self.top_pp),
            PlayerMetric::TopAcc => PlayerMetricWithValue::TopAcc(self.top_accuracy),
            PlayerMetric::TotalPp => PlayerMetricWithValue::TotalPp(self.pp),
            PlayerMetric::Rank => PlayerMetricWithValue::Rank(self.rank),
            PlayerMetric::CountryRank => PlayerMetricWithValue::CountryRank(self.country_rank),

            PlayerMetric::MaxStreak => PlayerMetricWithValue::MaxStreak(self.max_streak),
            PlayerMetric::Top1Count => PlayerMetricWithValue::Top1Count(self.top1_count),
            PlayerMetric::MyReplaysWatched => {
                PlayerMetricWithValue::MyReplaysWatched(self.total_replay_watched)
            }
            PlayerMetric::ReplaysIWatched => {
                PlayerMetricWithValue::ReplaysIWatched(self.watched_replays)
            }
            PlayerMetric::Clan => PlayerMetricWithValue::Clan(self.clans.clone()),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Score {
    pub id: u32,
    pub accuracy: f64,
    pub fc_accuracy: f64,
    pub acc_left: f64,
    pub acc_right: f64,
    pub pp: f64,
    pub fc_pp: f64,
    pub rank: u32,
    pub mistakes: u32,
    pub max_streak: u32,
    pub max_combo: u32,
    pub pauses: u32,
    pub full_combo: bool,
    pub modifiers: String,
    pub leaderboard_id: String,
    pub song_name: String,
    pub song_sub_name: String,
    pub song_mapper: String,
    pub song_author: String,
    pub song_cover: String,
    pub difficulty_name: String,
    pub difficulty_stars: f64,
    pub difficulty_stars_modified: bool,
    pub mode_name: String,
}

impl From<BlScore> for Score {
    fn from(bl_score: BlScore) -> Self {
        let mut modified_stars = false;
        let mut stars = bl_score.leaderboard.difficulty.stars;

        if let Some(ratings) = bl_score.leaderboard.difficulty.modifiers_rating {
            if bl_score.modifiers.contains("SS") && ratings.ss_stars > 0.00 {
                modified_stars = true;
                stars = ratings.ss_stars;
            } else if bl_score.modifiers.contains("FS") && ratings.fs_stars > 0.00 {
                modified_stars = true;
                stars = ratings.fs_stars;
            } else if bl_score.modifiers.contains("SF") && ratings.sf_stars > 0.00 {
                modified_stars = true;
                stars = ratings.sf_stars;
            }
        }

        Score {
            id: bl_score.id,
            accuracy: bl_score.accuracy * 100.0,
            fc_accuracy: bl_score.fc_accuracy * 100.0,
            acc_left: bl_score.acc_left,
            acc_right: bl_score.acc_right,
            pp: bl_score.pp,
            fc_pp: bl_score.fc_pp,
            rank: bl_score.rank,
            mistakes: bl_score.bad_cuts
                + bl_score.missed_notes
                + bl_score.bomb_cuts
                + bl_score.walls_hit,
            max_streak: bl_score.max_streak,
            max_combo: bl_score.max_combo,
            pauses: bl_score.pauses,
            full_combo: bl_score.full_combo,
            modifiers: bl_score.modifiers,
            leaderboard_id: bl_score.leaderboard.id,
            song_name: bl_score.leaderboard.song.name,
            song_sub_name: bl_score.leaderboard.song.sub_name,
            song_mapper: bl_score.leaderboard.song.mapper,
            song_author: bl_score.leaderboard.song.author,
            song_cover: bl_score.leaderboard.song.cover_image,
            difficulty_name: bl_score.leaderboard.difficulty.difficulty_name,
            difficulty_stars: stars,
            difficulty_stars_modified: modified_stars,
            mode_name: bl_score.leaderboard.difficulty.mode_name,
        }
    }
}

impl Score {
    pub(crate) fn add_embed(&self, reply: &mut CreateReply, player: &Player) {
        reply.embed(|f| {
            let mut desc = "**".to_owned() + &self.difficulty_name.clone();

            if self.difficulty_stars > 0.0 {
                let stars = format!(
                    " / {:.2}⭐{}",
                    self.difficulty_stars,
                    if self.difficulty_stars_modified {
                        "(M)"
                    } else {
                        ""
                    }
                );
                desc.push_str(&stars);
            }

            if !self.modifiers.is_empty() {
                desc.push_str(&(" / ".to_owned() + &self.modifiers.clone()));
            }

            desc.push_str("**");

            f.author(|a| {
                a.name(player.name.clone())
                    .icon_url(player.avatar.clone())
                    .url(format!("https://www.beatleader.xyz/u/{}", player.id))
            })
            .title(format!("{} {}", self.song_name, self.song_sub_name,))
            .description(desc)
            .url(format!(
                "https://replay.beatleader.xyz/?scoreId={}",
                self.id
            ))
            .thumbnail(self.song_cover.clone());

            if self.pp > 0.00 {
                if self.full_combo {
                    f.field("PP", format!("{:.2}", self.pp), true);
                } else {
                    f.field("PP", format!("{:.2} ({:.2} FC)", self.pp, self.fc_pp), true);
                }
            }

            if self.full_combo {
                f.field("Acc", format!("{:.2}%", self.accuracy), true);
            } else {
                f.field(
                    "Acc",
                    format!("{:.2}% ({:.2}% FC)", self.accuracy, self.fc_accuracy),
                    true,
                );
            }

            f.field("Rank", format!("#{}", self.rank), true)
                .field(
                    "Mistakes",
                    if self.mistakes == 0 {
                        "FC".to_string()
                    } else {
                        self.mistakes.to_string()
                    },
                    true,
                )
                .field("Acc Left", format!("{:.2}", self.acc_left), true)
                .field("Acc Right", format!("{:.2}", self.acc_right), true)
                .field("Pauses", self.pauses, true)
                .field("Max combo", self.max_combo, true)
                .field("Max streak", self.max_streak, true);

            f
        });
    }
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Scores {
    #[serde(rename = "data")]
    pub scores: Vec<Score>,
    pub metadata: MetaData,
}
impl From<BlScores> for Scores {
    fn from(bl_scores: BlScores) -> Self {
        Self {
            scores: bl_scores.scores.into_iter().map(Score::from).collect(),
            metadata: bl_scores.metadata,
        }
    }
}

pub(crate) async fn fetch_scores(
    player_id: &PlayerId,
    count: u32,
    sort_by: PlayerScoreSort,
) -> Result<Scores, BlError> {
    Ok(Scores::from(
        BL_CLIENT
            .player()
            .get_scores(
                player_id,
                &[
                    PlayerScoreParam::Page(1),
                    PlayerScoreParam::Count(count),
                    PlayerScoreParam::Sort(sort_by),
                    PlayerScoreParam::Order(SortOrder::Descending),
                ],
            )
            .await?,
    ))
}
