use std::path::PathBuf;
use std::sync::Arc;
use tracing::info;

use crate::beatleader::BlContext;
use crate::config::Settings;
use crate::storage::bsmaps::BsMapsRepository;
use crate::storage::clan_peak::ClanPeakRepository;
use crate::storage::guild::GuildSettingsRepository;
use crate::storage::persist::PersistInstance;
use crate::storage::player::PlayerRepository;
use crate::storage::player_oauth_token::PlayerOAuthTokenRepository;
use crate::storage::player_scores::PlayerScoresRepository;
use crate::storage::playlist::PlaylistRepository;

#[derive(Clone)]
pub struct CommonData {
    pub guild_settings_repository: Arc<GuildSettingsRepository>,
    pub players_repository: Arc<PlayerRepository>,
    pub player_scores_repository: Arc<PlayerScoresRepository>,
    pub player_oauth_token_repository: Arc<PlayerOAuthTokenRepository>,
    pub playlists_repository: Arc<PlaylistRepository>,
    pub maps_repository: Arc<BsMapsRepository>,
    pub clan_peak_repository: Arc<ClanPeakRepository>,
    pub settings: Settings,
}

pub async fn init(settings: Settings) -> CommonData {
    let persist = Arc::new(PersistInstance::new(PathBuf::from(&settings.storage_path)).unwrap());

    info!("Initializing player OAuth tokens repository...");
    let player_oauth_token_repository = Arc::new(
        PlayerOAuthTokenRepository::new(Arc::clone(&persist))
            .await
            .unwrap(),
    );
    info!(
        "Player OAuth tokens repository initialized, length: {}.",
        player_oauth_token_repository.len().await
    );

    info!("Initializing guild settings repository...");
    let guild_settings_repository = Arc::new(
        GuildSettingsRepository::new(Arc::clone(&persist))
            .await
            .unwrap(),
    );
    info!(
        "Guild settings repository initialized, length: {}.",
        guild_settings_repository.len().await
    );

    info!("Initializing players repository...");
    let players_repository = Arc::new(PlayerRepository::new(Arc::clone(&persist)).await.unwrap());
    info!(
        "Players repository initialized, length: {}.",
        players_repository.len().await
    );

    info!("Initializing players scores repository...");
    let player_scores_repository = Arc::new(
        PlayerScoresRepository::new(Arc::clone(&persist), BlContext::General)
            .await
            .unwrap(),
    );
    info!("Players scores repository initialized.");

    info!("Initializing playlists repository...");
    let playlists_repository =
        Arc::new(PlaylistRepository::new(Arc::clone(&persist)).await.unwrap());
    info!("Playlists repository initialized.");

    info!("Initializing maps repository...");
    let maps_repository = Arc::new(BsMapsRepository::new(Arc::clone(&persist)).await.unwrap());
    info!("Maps repository initialized.");

    info!("Initializing clan peak repository...");
    let clan_peak_repository =
        Arc::new(ClanPeakRepository::new(Arc::clone(&persist)).await.unwrap());
    info!("Clan peak repository initialized.");

    CommonData {
        guild_settings_repository,
        players_repository,
        player_oauth_token_repository,
        player_scores_repository,
        playlists_repository,
        maps_repository,
        clan_peak_repository,
        settings,
    }
}
