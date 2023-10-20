use reqwest::Method;
use serde::Deserialize;
use serde_repr::{Deserialize_repr, Serialize_repr};
use serde_with::{serde_as, DefaultOnError, DefaultOnNull, TimestampSeconds};

use chrono::serde::ts_seconds;
use chrono::{DateTime, Utc};

use crate::beatleader;
use crate::beatleader::clan::ClanTag;
use crate::beatleader::{
    BlApiListResponse, BlApiResponse, BlContext, Client, List, QueryParam, SortOrder,
};

pub struct PlayerResource<'a> {
    client: &'a Client,
}

impl<'a> PlayerResource<'a> {
    pub fn new(client: &'a Client) -> Self {
        Self { client }
    }

    pub async fn get(&self, id: &PlayerId) -> beatleader::Result<Player> {
        self.client
            .get_json::<Player, Player, PlayerScoreParam>(
                Method::GET,
                &format!("/player/{}", id),
                &[],
            )
            .await
    }

    pub async fn scores(
        &self,
        id: &PlayerId,
        params: &[PlayerScoreParam],
    ) -> beatleader::Result<List<Score>> {
        self.client
            .get_json::<BlApiListResponse<Score>, List<Score>, PlayerScoreParam>(
                Method::GET,
                &format!("/player/{}/scores", id),
                params,
            )
            .await
    }
}

#[allow(dead_code)]
#[derive(Clone)]
pub enum PlayerScoreSort {
    Date,
    Pp,
    Acc,
    Stars,
    Rank,
    Pauses,
    MaxStreak,
    ReplaysWatched,
    Mistakes,
}

#[allow(dead_code)]
#[derive(Clone)]
pub enum MapType {
    All,
    Ranked,
    Unranked,
}

#[allow(dead_code)]
#[derive(Clone)]
pub enum PlayerScoreParam {
    Page(u32),
    Sort(PlayerScoreSort),
    Order(SortOrder),
    Count(u32),
    Type(MapType),
    TimeFrom(DateTime<Utc>),
    Context(BlContext),
}

impl QueryParam for PlayerScoreParam {
    fn as_query_param(&self) -> (String, String) {
        match self {
            PlayerScoreParam::Page(page) => ("page".to_owned(), page.to_string()),
            PlayerScoreParam::Sort(field) => (
                "sortBy".to_owned(),
                match field {
                    PlayerScoreSort::Date => "date".to_owned(),
                    PlayerScoreSort::Pp => "pp".to_owned(),
                    PlayerScoreSort::Acc => "acc".to_owned(),
                    PlayerScoreSort::Stars => "stars".to_owned(),
                    PlayerScoreSort::Rank => "rank".to_owned(),
                    PlayerScoreSort::Pauses => "pauses".to_owned(),
                    PlayerScoreSort::MaxStreak => "maxStreak".to_owned(),
                    PlayerScoreSort::ReplaysWatched => "replaysWatched".to_owned(),
                    PlayerScoreSort::Mistakes => "mistakes".to_owned(),
                },
            ),
            PlayerScoreParam::Order(order) => ("order".to_owned(), order.to_string()),
            PlayerScoreParam::Count(count) => ("count".to_owned(), count.to_string()),
            PlayerScoreParam::TimeFrom(time) => {
                ("from_time".to_owned(), time.timestamp().to_string())
            }
            PlayerScoreParam::Type(map_type) => (
                "type".to_owned(),
                match map_type {
                    MapType::All => "all".to_owned(),
                    MapType::Ranked => "ranked".to_owned(),
                    MapType::Unranked => "unranked".to_owned(),
                },
            ),
            PlayerScoreParam::Context(context) => {
                ("leaderboardContext".to_owned(), context.to_string())
            }
        }
    }
}

pub type PlayerId = String;

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Player {
    pub id: PlayerId,
    pub name: String,
    pub avatar: String,
    pub country: String,
    pub rank: u32,
    pub country_rank: u32,
    pub pp: f64,
    pub acc_pp: f64,
    pub tech_pp: f64,
    pub pass_pp: f64,
    pub score_stats: PlayerScoreStats,
    pub banned: bool,
    pub bot: bool,
    pub inactive: bool,
    pub clans: Vec<PlayerClan>,
    pub socials: Vec<Social>,
}

impl BlApiResponse for Player {}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct PlayerClan {
    pub id: u32,
    pub tag: ClanTag,
    pub color: String,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Social {
    pub id: u32,
    pub service: String,
    pub user: String,
    pub user_id: String,
    pub link: String,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct PlayerScoreStats {
    pub a_plays: u32,
    pub s_plays: u32,
    pub sp_plays: u32,
    pub ss_plays: u32,
    pub ssp_plays: u32,
    pub average_accuracy: f64,
    pub average_ranked_accuracy: f64,
    pub average_unranked_accuracy: f64,
    #[serde(with = "ts_seconds")]
    pub last_ranked_score_time: DateTime<Utc>,
    #[serde(with = "ts_seconds")]
    pub last_unranked_score_time: DateTime<Utc>,
    #[serde(with = "ts_seconds")]
    pub last_score_time: DateTime<Utc>,
    pub max_streak: u32,
    pub ranked_max_streak: u32,
    pub unranked_max_streak: u32,
    pub median_accuracy: f64,
    pub median_ranked_accuracy: f64,
    pub top_accuracy: f64,
    pub top_ranked_accuracy: f64,
    pub top_unranked_accuracy: f64,
    #[serde(rename = "topAccPP")]
    pub top_acc_pp: f64,
    #[serde(rename = "topTechPP")]
    pub top_tech_pp: f64,
    #[serde(rename = "topPassPP")]
    pub top_pass_pp: f64,
    pub top_pp: f64,
    pub total_play_count: u32,
    pub ranked_play_count: u32,
    pub unranked_play_count: u32,
    #[serde(rename = "anonimusReplayWatched")]
    pub anonymous_replay_watched: u32,
    pub authorized_replay_watched: u32,
    pub watched_replays: u32,
    pub peak_rank: u32,
    pub top1_count: u32,
}

#[serde_as]
#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Score {
    pub id: u32,
    #[serde_as(deserialize_as = "DefaultOnNull")]
    pub accuracy: f64,
    #[serde_as(deserialize_as = "DefaultOnNull")]
    pub fc_accuracy: f64,
    #[serde_as(deserialize_as = "DefaultOnNull")]
    pub pp: f64,
    #[serde_as(deserialize_as = "DefaultOnNull")]
    pub fc_pp: f64,
    #[serde_as(deserialize_as = "DefaultOnNull")]
    pub weight: f64,
    #[serde_as(deserialize_as = "DefaultOnNull")]
    pub rank: u32,
    #[serde_as(deserialize_as = "DefaultOnNull")]
    pub acc_left: f64,
    #[serde_as(deserialize_as = "DefaultOnNull")]
    pub acc_right: f64,
    #[serde_as(deserialize_as = "DefaultOnNull")]
    pub bad_cuts: u32,
    #[serde_as(deserialize_as = "DefaultOnNull")]
    pub bomb_cuts: u32,
    #[serde_as(deserialize_as = "DefaultOnNull")]
    pub missed_notes: u32,
    #[serde_as(deserialize_as = "DefaultOnNull")]
    pub walls_hit: u32,
    #[serde_as(deserialize_as = "DefaultOnNull")]
    pub full_combo: bool,
    #[serde_as(deserialize_as = "DefaultOnNull")]
    pub max_streak: u32,
    #[serde_as(deserialize_as = "DefaultOnNull")]
    pub max_combo: u32,
    #[serde_as(deserialize_as = "DefaultOnNull")]
    pub pauses: u32,
    #[serde_as(deserialize_as = "DefaultOnNull")]
    pub leaderboard: Leaderboard,
    #[serde_as(deserialize_as = "DefaultOnNull")]
    pub modifiers: String,
    #[serde_as(as = "TimestampSeconds<String>")]
    pub timeset: DateTime<Utc>,
    #[serde(with = "ts_seconds")]
    pub timepost: DateTime<Utc>,
}

impl BlApiResponse for Score {}

#[serde_as]
#[derive(Deserialize, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct Leaderboard {
    #[serde_as(deserialize_as = "DefaultOnNull")]
    pub id: String,
    #[serde_as(deserialize_as = "DefaultOnNull")]
    pub song: Song,
    #[serde_as(deserialize_as = "DefaultOnNull")]
    pub difficulty: Difficulty,
}

#[serde_as]
#[derive(Deserialize, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct Song {
    #[serde_as(deserialize_as = "DefaultOnNull")]
    pub id: String,
    #[serde_as(deserialize_as = "DefaultOnNull")]
    pub hash: String,
    #[serde_as(deserialize_as = "DefaultOnNull")]
    pub name: String,
    #[serde_as(deserialize_as = "DefaultOnNull")]
    pub sub_name: String,
    #[serde_as(deserialize_as = "DefaultOnNull")]
    pub mapper: String,
    #[serde_as(deserialize_as = "DefaultOnNull")]
    pub author: String,
    #[serde_as(deserialize_as = "DefaultOnNull")]
    pub duration: u32,
    #[serde_as(deserialize_as = "DefaultOnNull")]
    pub bpm: f32,
    #[serde_as(deserialize_as = "DefaultOnNull")]
    pub cover_image: String,
    #[serde_as(deserialize_as = "DefaultOnNull")]
    pub full_cover_image: String,
}

#[serde_as]
#[derive(Deserialize, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct Difficulty {
    #[serde_as(deserialize_as = "DefaultOnNull")]
    pub id: u32,
    #[serde_as(deserialize_as = "DefaultOnNull")]
    pub value: u32,
    #[serde_as(deserialize_as = "DefaultOnNull")]
    pub difficulty_name: String,
    #[serde_as(deserialize_as = "DefaultOnNull")]
    pub mode: u32,
    #[serde_as(deserialize_as = "DefaultOnNull")]
    pub mode_name: String,
    #[serde_as(deserialize_as = "DefaultOnNull")]
    pub acc_rating: f64,
    #[serde_as(deserialize_as = "DefaultOnNull")]
    pub pass_rating: f64,
    #[serde_as(deserialize_as = "DefaultOnNull")]
    pub tech_rating: f64,
    #[serde_as(deserialize_as = "DefaultOnNull")]
    pub stars: f64,
    #[serde_as(deserialize_as = "DefaultOnNull")]
    pub notes: u32,
    #[serde_as(deserialize_as = "DefaultOnNull")]
    pub modifiers_rating: Option<ModifiersRatings>,
    #[serde_as(deserialize_as = "DefaultOnError")]
    #[serde(default)]
    pub status: DifficultyStatus,
}

#[allow(dead_code)]
#[derive(Serialize_repr, Deserialize_repr, PartialEq, Default, Debug, Clone)]
#[repr(u8)]
pub enum DifficultyStatus {
    Unranked = 0,
    Nominated,
    Qualified,
    Ranked,
    Unrankable,
    Outdated,
    InEvent,
    #[default]
    Unknown = 255,
}

impl std::fmt::Display for DifficultyStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DifficultyStatus::Unranked => write!(f, "Unranked"),
            DifficultyStatus::Nominated => write!(f, "Nominated"),
            DifficultyStatus::Qualified => write!(f, "Qualified"),
            DifficultyStatus::Ranked => write!(f, "Ranked"),
            DifficultyStatus::Unrankable => write!(f, "Unrankable"),
            DifficultyStatus::Outdated => write!(f, "Outdated"),
            DifficultyStatus::InEvent => write!(f, "InEvent"),
            DifficultyStatus::Unknown => write!(f, "Unknown"),
        }
    }
}

#[serde_as]
#[derive(Deserialize, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct ModifiersRatings {
    #[serde_as(deserialize_as = "DefaultOnNull")]
    pub id: u32,
    #[serde_as(deserialize_as = "DefaultOnNull")]
    pub ss_stars: f64,
    #[serde_as(deserialize_as = "DefaultOnNull")]
    pub fs_stars: f64,
    #[serde_as(deserialize_as = "DefaultOnNull")]
    pub sf_stars: f64,
}
