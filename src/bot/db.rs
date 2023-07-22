use crate::beatleader::error::Error::DbError;
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
    pub players: Vec<PlayerLink>,
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
    get_linked_players(persist)
        .await?
        .players
        .into_iter()
        .find(|p| p.discord_user_id == discord_user_id)
        .map_or(Err("Player is not linked".into()), |p| Ok(p.player_id))
}

pub(crate) async fn store_player(
    persist: &PersistInstance,
    player: BotPlayer,
) -> Result<(), Error> {
    let player_key = format!("player-v1-{}", player.id);

    let player_id = player.id.clone();

    debug!("Saving player data ({}) to {}...", player_id, player_key);

    match persist.save::<BotPlayer>(player_key.as_str(), player) {
        Ok(_) => {
            debug!("Player data ({}) saved to {}.", player_id, player_key);

            Ok(())
        }
        Err(e) => {
            error!(
                "Saving player data ({}) error: {}",
                player_id,
                e.to_string()
            );

            Err(Box::new(DbError(e.to_string())))
        }
    }
}

pub(crate) async fn fetch_and_update_player(
    bl_client: &Client,
    persist: &PersistInstance,
    player_id: PlayerId,
) -> Result<BotPlayer, Error> {
    debug!("Fetching BL player {}...", player_id);

    let bl_player_result = bl_client.player().get_by_id(&player_id).await;
    if let Err(e) = bl_player_result {
        warn!("BL player ({}) fetching error: {}", player_id, e);

        return Err(e.to_string())?;
    }

    let player = BotPlayer::from(bl_player_result.unwrap());
    let player_clone = player.clone();

    debug!(
        "BL player ({}) fetched. Player name: {}",
        player.id, player.name
    );

    store_player(persist, player).await?;

    Ok(player_clone)
}

pub(crate) async fn fetch_and_update_all_players(
    bl_client: &Client,
    persist: &PersistInstance,
) -> Result<Vec<BotPlayer>, Error> {
    info!("Updating profiles of all players...");

    match get_linked_players(persist).await {
        Ok(linked_players) => {
            let links_count = linked_players.players.len();
            info!("Players links loaded, {} link(s) found.", links_count);

            let mut players = Vec::with_capacity(linked_players.players.len());

            for linked_player in linked_players.players {
                debug!("Updating player {}...", linked_player.player_id.clone());

                match fetch_and_update_player(bl_client, persist, linked_player.player_id).await {
                    Ok(player) => {
                        info!("Player {} ({}) updated.", player.id, player.name);
                        players.push(player);
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

pub(crate) async fn get_linked_players(persist: &PersistInstance) -> Result<LinkedPlayers, Error> {
    match persist.load::<LinkedPlayers>("linked-players-v1") {
        Ok(players) => Ok(players),
        Err(_) => Ok(LinkedPlayers::new()),
    }
}

pub(crate) async fn store_linked_players(
    persist: &PersistInstance,
    players: LinkedPlayers,
) -> Result<(), Error> {
    match persist.save::<LinkedPlayers>("linked-players-v1", players) {
        Ok(_) => Ok(()),
        Err(e) => {
            error!("Save links error: {}", e.to_string());

            Err(e.to_string())?
        }
    }
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

    let mut data = get_linked_players(persist).await?;

    debug!(
        "Players links loaded, {} link(s) found.",
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

    debug!("Saving new links ({})...", data.players.len());

    store_linked_players(persist, data).await?;

    Ok(player)
}
