use reqwest::Method;
use serde::Deserialize;
use serde_with::{serde_as, DefaultOnNull};

use crate::beatleader;
use crate::beatleader::error::Error::{JsonDecode, Request};
use crate::beatleader::{Client, QueryParam, SortOrder};

pub struct PlayerRequest<'a> {
    client: &'a Client,
}

impl<'a> PlayerRequest<'a> {
    pub fn new(client: &'a Client) -> Self {
        Self { client }
    }

    pub async fn get_by_id(&self, id: &PlayerId) -> beatleader::Result<Player> {
        match self
            .client
            .get(&(format!("/player/{}", id)))
            .await?
            .json::<Player>()
            .await
        {
            Ok(player) => Ok(player),
            Err(e) => Err(JsonDecode(e)),
        }
    }

    pub async fn get_scores(
        &self,
        id: &PlayerId,
        params: &[PlayerScoreParam],
    ) -> beatleader::Result<Scores> {
        let request = self
            .client
            .request_builder(
                Method::GET,
                format!("/player/{}/scores", id),
                self.client.timeout,
            )
            .query(
                &(params
                    .iter()
                    .map(|param| param.as_query_param())
                    .collect::<Vec<(String, String)>>()),
            )
            .build();

        if let Err(err) = request {
            return Err(Request(err));
        }

        match self
            .client
            .send_request(request.unwrap())
            .await?
            .json::<Scores>()
            .await
        {
            Ok(player_scores) => Ok(player_scores),
            Err(e) => Err(JsonDecode(e)),
        }
    }
}

#[allow(dead_code)]
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
pub enum PlayerScoreParam {
    Page(u32),
    Sort(PlayerScoreSort),
    Order(SortOrder),
    Count(u32),
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
            PlayerScoreParam::Order(order) => (
                "order".to_owned(),
                match order {
                    SortOrder::Ascending => "asc".to_owned(),
                    SortOrder::Descending => "desc".to_owned(),
                },
            ),
            PlayerScoreParam::Count(count) => ("count".to_owned(), count.to_string()),
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
    pub clans: Vec<Clan>,
    pub socials: Vec<Social>,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Clan {
    pub id: u32,
    pub tag: String,
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
    pub last_ranked_score_time: u32,
    pub last_unranked_score_time: u32,
    pub last_score_time: u32,
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

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct MetaData {
    pub items_per_page: u32,
    pub page: u32,
    pub total: u32,
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
}

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

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Scores {
    #[serde(rename = "data")]
    pub scores: Vec<Score>,
    pub metadata: MetaData,
}
