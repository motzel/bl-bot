use serde::Deserialize;

use crate::beatleader;
use crate::beatleader::Client;
use crate::beatleader::error::BlError::JsonDecodeError;

pub struct PlayerRequest<'a> {
    client: &'a Client,
}

impl<'a> PlayerRequest<'a> {
    pub fn new(client: &'a Client) -> Self {
        Self { client }
    }

    pub async fn get_by_id(&self, id: PlayerId) -> beatleader::Result<Player> {
        match self
            .client
            .send_get_request(&(format!("/player/{}", id)))
            .await?
            .json::<Player>()
            .await
        {
            Ok(player) => Ok(player),
            Err(_err) => Err(JsonDecodeError),
        }
    }
}

pub type PlayerId = String;

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Player {
    pub id: PlayerId,
    pub name: String,
    pub avatar: String,
    pub country: String,
    pub rank: i32,
    pub country_rank: i32,
    pub pp: f64,
    pub acc_pp: f64,
    pub tech_pp: f64,
    pub pass_pp: f64,
    pub score_stats: PlayerScoreStats,
    pub banned: bool,
    pub bot: bool,
    pub inactive: bool,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct PlayerScoreStats {
    pub a_plays: i32,
    pub s_plays: i32,
    pub sp_plays: i32,
    pub ss_plays: i32,
    pub ssp_plays: i32,
    pub average_accuracy: f64,
    pub average_ranked_accuracy: f64,
    pub average_unranked_accuracy: f64,
    pub last_ranked_score_time: i32,
    pub last_unranked_score_time: i32,
    pub last_score_time: i32,
    pub max_streak: i32,
    pub ranked_max_streak: i32,
    pub unranked_max_streak: i32,
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
    pub total_play_count: i32,
    pub ranked_play_count: i32,
    pub unranked_play_count: i32,
    #[serde(rename = "anonimusReplayWatched")]
    pub anonymous_replay_watched: i32,
    pub authorized_replay_watched: i32,
    pub watched_replays: i32,
}
