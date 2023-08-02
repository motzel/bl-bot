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

#[derive(Deserialize, Serialize, Debug, Clone)]
pub(crate) struct PlayerLink {
    pub discord_user_id: UserId,
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
    discord_user_id: UserId,
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
    persist: &PersistInstance,
    player_id: PlayerId,
) -> Result<BotPlayer, Error> {
    debug!("Fetching BL player {}...", player_id);

    let bl_player_result = BL_CLIENT.player().get_by_id(&player_id).await;
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

pub(crate) async fn get_linked_players(persist: &PersistInstance) -> Result<LinkedPlayers, Error> {
    // LinkedPlayers object can not be deserialized as is for some reason
    match persist.load::<String>("linked-players-v1") {
        Ok(json) => match serde_json::from_str::<LinkedPlayers>(json.as_str()) {
            Ok(linked_players) => Ok(linked_players),
            Err(e) => {
                error!("Can not deserialize JSON linked_players: {}", e);

                Ok(LinkedPlayers::new())
            }
        },
        Err(e) => {
            error!("Can not load linked players: {}", e);

            Ok(LinkedPlayers::new())
        }
    }
}

pub(crate) async fn store_linked_players(
    persist: &PersistInstance,
    players: LinkedPlayers,
) -> Result<(), Error> {
    // LinkedPlayers object can not be serialized as is for some reason
    let json = serde_json::to_string(&players)?;

    match persist.save::<String>("linked-players-v1", json) {
        Ok(_) => Ok(()),
        Err(e) => {
            error!("Linked players save error: {}", e.to_string());

            Err(e.to_string())?
        }
    }
}

pub(crate) async fn link_player(
    persist: &PersistInstance,
    discord_user_id: UserId,
    player_id: PlayerId,
) -> Result<BotPlayer, Error> {
    info!(
        "Linking Discord user {} with BL player {}...",
        discord_user_id, player_id
    );

    let player = fetch_and_update_player(persist, player_id.clone()).await?;

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

pub(crate) async fn unlink_player(
    persist: &PersistInstance,
    discord_user_id: UserId,
) -> Result<(), Error> {
    info!("Unlinking discord user {}...", discord_user_id);

    let mut data = get_linked_players(persist).await?;

    let original_len = data.players.len();

    debug!("Players links loaded, {} link(s) found.", original_len);

    // filter existing link
    data.players
        .retain(|p| p.discord_user_id != discord_user_id);

    if data.players.len() == original_len {
        return Err("User is not linked to BL profile")?;
    }

    debug!("Saving new players links ({})...", data.players.len());

    store_linked_players(persist, data).await?;

    Ok(())
}

pub(crate) async fn get_guild_settings(
    persist: &PersistInstance,
    guild_id: GuildId,
) -> Result<GuildSettings, Error> {
    let guild_settings_key = format!("guild-settings-v1-{}", guild_id);

    debug!("Loading guid settings from {}...", guild_settings_key);

    // GuildSettings object can not be deserialized as is for some reason
    match persist.load::<String>(guild_settings_key.as_str()) {
        Ok(json) => match serde_json::from_str::<GuildSettings>(json.as_str()) {
            Ok(gs) => Ok(gs),
            Err(e) => {
                error!("Can not deserialize JSON guild settings: {}", e);

                Ok(GuildSettings::new(guild_id))
            }
        },
        Err(e) => {
            error!("Can not load guild settings: {}", e);

            Ok(GuildSettings::new(guild_id))
        }
    }
}

pub(crate) async fn store_guild_settings<'a>(
    persist: &PersistInstance,
    guild_settings: MutexGuard<'a, GuildSettings>,
) -> Result<(), Error> {
    let guild_settings_key = format!("guild-settings-v1-{}", guild_settings.guild_id);

    debug!("Saving guid settings as {}...", guild_settings_key);

    // GuildSettings object can not be serialized as is for some reason
    let json = serde_json::to_string::<GuildSettings>(&guild_settings)?;

    match persist.save::<String>(guild_settings_key.as_str(), json) {
        Ok(_) => Ok(()),
        Err(e) => {
            error!("Guild settings save error: {}", e.to_string());

            Err(e.to_string())?
        }
    }
}

pub(crate) async fn add_auto_role(
    persist: &PersistInstance,
    guild_settings: &Arc<Mutex<GuildSettings>>,
    role_group: RoleGroup,
    role_id: RoleId,
    metric_and_value: PlayerMetricWithValue,
    condition: MetricCondition,
    weight: u32,
) -> Result<(), Error> {
    info!("Adding auto role...");

    let mut lock = guild_settings.lock().await;

    let mut rs = RoleSettings::new(role_id, weight);
    rs.add_condition(condition, metric_and_value);

    lock.merge(role_group, rs);

    store_guild_settings(persist, lock).await?;

    info!("Role added.");

    Ok(())
}

pub(crate) async fn remove_auto_role(
    persist: &PersistInstance,
    guild_settings: &Arc<Mutex<GuildSettings>>,
    role_group: RoleGroup,
    role_id: RoleId,
) -> Result<(), Error> {
    info!("Removing auto role...");

    let mut lock = guild_settings.lock().await;

    lock.remove(role_group, role_id);

    store_guild_settings(persist, lock).await?;

    info!("Role removed.");

    Ok(())
}

pub(crate) async fn update_bot_log_channel(
    persist: &PersistInstance,
    guild_settings: &Arc<Mutex<GuildSettings>>,
    bot_channel_id: Option<ChannelId>,
) -> Result<(), Error> {
    info!("Updating bot log channel...");

    let mut lock = guild_settings.lock().await;

    lock.bot_channel_id = bot_channel_id;

    store_guild_settings(persist, lock).await?;

    info!("Channel updated.");

    Ok(())
}
