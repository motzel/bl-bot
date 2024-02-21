use config::{Config, ConfigError, Environment, File};
use serde::{Deserialize, Serialize};

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
    pub oauth: Option<OAuthSettings>,
}

impl Settings {
    pub fn new() -> Result<Self, ConfigError> {
        let s = Config::builder()
            .set_default("refresh_interval", 600)?
            .set_default("storage_path", "./.storage")?
            .set_default("clan_wars_interval", 360)?
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

                Ok(config)
            }
            Err(e) => Err(e),
        }
    }
}
