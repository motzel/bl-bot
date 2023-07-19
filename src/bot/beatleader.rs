use crate::beatleader::player::Player as BlPlayer;
use crate::beatleader::player::PlayerId;
use crate::beatleader::{error::Error as BlError, Client};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Player {
    pub id: PlayerId,
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
}

impl From<BlPlayer> for Player {
    fn from(bl_player: BlPlayer) -> Self {
        Player {
            id: bl_player.id,
            name: bl_player.name,
            active: !bl_player.inactive && !bl_player.banned && !bl_player.bot,
            avatar: bl_player.avatar,
            country: bl_player.country,
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
        }
    }
}

pub(crate) async fn fetch_player(
    bl_client: &Client,
    player_id: PlayerId,
) -> Result<Player, BlError> {
    Ok(Player::from(
        bl_client.player().get_by_id(&player_id).await?,
    ))
}

/*
use env_logger::Env;
use log::info;

use crate::beatleader::{
    player::PlayerScoreParam, player::PlayerScoreSort, Client, Result, SortOrder,
};
use crate::bot::Player as BotPlayer;

mod beatleader;
mod bot;
mod db;

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::Builder::from_env(Env::default().default_filter_or("info")).init();

    info!("Starting up...");

    let player_id = "76561198035381239".to_string();
    let client = Client::default();

    let player = BotPlayer::from(client.player().get_by_id(&player_id).await?);
    println!("Player: {:#?}", player);

    // let _player_scores = client
    //     .player()
    //     .get_scores(
    //         &player_id,
    //         &[
    //             PlayerScoreParam::Page(1),
    //             PlayerScoreParam::Count(3),
    //             PlayerScoreParam::Sort(PlayerScoreSort::Date),
    //             PlayerScoreParam::Order(SortOrder::Descending),
    //         ],
    //     )
    //     .await?;
    // println!("Scores: {:#?}", player_scores.data);

    info!("Shutting down...");

    Ok(())
}
*/
