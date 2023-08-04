/*
use crate::beatleader::error::Error::DbError;
use crate::beatleader::player::PlayerId;
use crate::beatleader::Client;
use crate::bot::beatleader::Player as BotPlayer;
use crate::bot::{
    GuildSettings, MetricCondition, PlayerMetric, PlayerMetricWithValue, RoleGroup, RoleSettings,
};
use crate::Error;
use crate::BL_CLIENT;
use log::{debug, error, info, warn};
use poise::serenity_prelude::ButtonStyle::Link;
use poise::serenity_prelude::{ChannelId, GuildId, RoleId, UserId};
use serde::{Deserialize, Serialize};
use shuttle_persist::PersistInstance;
use std::sync::Arc;
use tokio::sync::{Mutex, MutexGuard};
use tracing::field::debug;

pub(crate) async fn fetch_and_update_all_players(
    persist: &PersistInstance,
) -> Result<Vec<(BotPlayer, UserId)>, Error> {
    info!("Updating profiles of all players...");

    match get_linked_players(persist).await {
        Ok(linked_players) => {
            let links_count = linked_players.players.len();
            info!("Players links loaded, {} link(s) found.", links_count);

            let mut players =
                Vec::<(BotPlayer, UserId)>::with_capacity(linked_players.players.len());

            for linked_player in linked_players.players {
                debug!("Updating player {}...", linked_player.player_id.clone());

                match fetch_and_update_player(persist, linked_player.player_id).await {
                    Ok(player) => {
                        info!("Player {} ({}) updated.", player.id, player.name);
                        players.push((player, linked_player.discord_user_id));
                    }
                    Err(e) => {
                        error!("Error updating player: {}", e);
                    }
                };
            }

            if players.len() < links_count {
                warn!(
                    "Fewer profiles updated ({}) than expected ({})",
                    players.len(),
                    links_count
                );
            }

            Ok(players)
        }
        Err(e) => {
            error!("Can not get linked players: {}", e);

            Err(e)
        }
    }
}
*/
