use crate::beatleader::player::PlayerId;
use crate::beatleader::Client;
use crate::bot::beatleader::Player as BotPlayer;
use crate::Error;
use log::{debug, error, info, warn};
use serde::{Deserialize, Serialize};
use shuttle_persist::PersistInstance;

#[derive(Deserialize, Serialize, Debug, Clone)]
pub(crate) struct PlayerLink {
    pub discord_user_id: u64,
    pub player_id: PlayerId,
}

#[derive(Deserialize, Serialize)]
pub(crate) struct LinkedPlayers {
    players: Vec<PlayerLink>,
}

impl LinkedPlayers {
    pub fn new() -> Self {
        Self {
            players: Vec::new(),
        }
    }
}

pub(crate) async fn get_player_id(
    persist: &PersistInstance,
    discord_user_id: u64,
) -> Result<PlayerId, Error> {
    match persist.load::<LinkedPlayers>("linked-players-v1") {
        Ok(players) => players
            .players
            .into_iter()
            .find(|p| p.discord_user_id == discord_user_id)
            .map_or(Err("Player is not linked".into()), |p| Ok(p.player_id)),
        Err(e) => Err(e)?,
    }
}

pub(crate) async fn fetch_and_update_player(
    bl_client: &Client,
    persist: &PersistInstance,
    player_id: PlayerId,
) -> Result<BotPlayer, Error> {
    info!("Fetching BL player {}...", player_id);

    let bl_player_result = bl_client.player().get_by_id(&player_id).await;
    if let Err(e) = bl_player_result {
        warn!("BL player ({}) fetching error: {}", player_id, e);

        return Err(e.to_string())?;
    }

    let player = BotPlayer::from(bl_player_result.unwrap());
    let player_clone = player.clone();

    info!(
        "BL player ({}) fetched. Player name: {}",
        player.id, player.name
    );

    let player_key = format!("player-v1-{}", player_id);

    info!("Saving player data ({}) to {}...", player_id, player_key);

    if let Err(e) = persist.save::<BotPlayer>(player_key.as_str(), player) {
        error!(
            "Saving player data ({}) error: {}",
            player_id,
            e.to_string()
        );

        return Err(e.to_string())?;
    }

    info!("Player data ({}) saved to {}.", player_id, player_key);

    Ok(player_clone)
}

pub(crate) async fn link_player(
    bl_client: &Client,
    persist: &PersistInstance,
    discord_user_id: u64,
    player_id: PlayerId,
) -> Result<BotPlayer, Error> {
    info!(
        "Linking Discord user {} with BL player {}...",
        discord_user_id, player_id
    );

    let player = fetch_and_update_player(bl_client, persist, player_id.clone()).await?;

    let mut data = match persist.load::<LinkedPlayers>("linked-players-v1") {
        Ok(players) => players,
        Err(_) => LinkedPlayers::new(),
    };

    info!(
        "Players links db loaded, {} link(s) found.",
        data.players.len()
    );

    // filter existing link
    data.players
        .retain(|p| p.discord_user_id != discord_user_id);

    let player_link = PlayerLink {
        discord_user_id,
        player_id,
    };

    data.players.push(player_link);

    info!("Saving new links...");

    match persist.save::<LinkedPlayers>("linked-players-v1", data) {
        Ok(_) => Ok(player),
        Err(e) => {
            error!("Save links error: {}", e.to_string());

            Err(e.to_string())?
        }
    }
}
