use std::path::PathBuf;
use std::sync::Arc;

use config::{Config, ConfigError, Environment, File};
use log::info;
use serde::{Deserialize, Serialize};

use crate::beatleader::BlContext;
use crate::storage::guild::GuildSettingsRepository;
use crate::storage::persist::PersistInstance;
use crate::storage::player::PlayerRepository;
use crate::storage::player_oauth_token::PlayerOAuthTokenRepository;
use crate::storage::player_scores::PlayerScoresRepository;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(unused)]
pub(crate) struct OAuthSettings {
    pub client_id: String,
    pub client_secret: String,
    pub redirect_uri: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(unused)]
pub(crate) struct Settings {
    pub discord_token: String,
    pub refresh_interval: u64,
    pub storage_path: String,
    pub clan_wars_interval: u64,
    pub clan_wars_maps_count: u16,
    pub oauth: Option<OAuthSettings>,
}

impl Settings {
    pub fn new() -> Result<Self, ConfigError> {
        info!("Creating settings from configuration file...");

        let s = Config::builder()
            .set_default("refresh_interval", 600)?
            .set_default("storage_path", "./.storage")?
            .set_default("clan_wars_interval", 360)?
            .set_default("clan_wars_maps_count", 30)?
            .add_source(File::with_name("config").required(false))
            .add_source(File::with_name("config.dev").required(false))
            .add_source(Environment::with_prefix("BLBOT"))
            .build()?;

        match s.try_deserialize::<Self>() {
            Ok(config) => {
                if config.refresh_interval < 30 {
                    return Err(ConfigError::Message(
                        "REFRESH_INTERVAL should be at least 30 seconds".to_owned(),
                    ));
                }

                if config.clan_wars_interval < 30 {
                    return Err(ConfigError::Message(
                        "CLAN_WARS_INTERVAL should be at least 30 minutes".to_owned(),
                    ));
                }

                if config.clan_wars_maps_count > 100 {
                    return Err(ConfigError::Message(
                        "CLAN_WARS_MAPS_COUNT should not be greater than 100".to_owned(),
                    ));
                }

                info!("Settings created.");

                Ok(config)
            }
            Err(e) => Err(e),
        }
    }
}

#[derive(Clone)]
pub(crate) struct CommonData {
    pub guild_settings_repository: Arc<GuildSettingsRepository>,
    pub players_repository: Arc<PlayerRepository>,
    pub player_scores_repository: Arc<PlayerScoresRepository>,
    pub player_oauth_token_repository: Arc<PlayerOAuthTokenRepository>,
    pub settings: Settings,
}

pub async fn init() -> CommonData {
    let settings = Settings::new().unwrap();

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

    CommonData {
        guild_settings_repository,
        players_repository,
        player_oauth_token_repository,
        player_scores_repository,
        settings,
    }
}
