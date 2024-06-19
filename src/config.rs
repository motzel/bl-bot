use config::{Config, ConfigError, Environment, File, Value, ValueKind};
use serde::{Deserialize, Serialize};
use std::net::Ipv4Addr;
use tracing::info;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TracingLevel(String);

impl Default for TracingLevel {
    fn default() -> Self {
        Self("info".to_owned())
    }
}

impl From<TracingLevel> for tracing::Level {
    fn from(value: TracingLevel) -> Self {
        match value.0.as_str() {
            "trace" => tracing::Level::TRACE,
            "debug" => tracing::Level::DEBUG,
            "warn" => tracing::Level::WARN,
            "error" => tracing::Level::ERROR,
            _ => tracing::Level::INFO,
        }
    }
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub enum TracingFormat {
    #[default]
    #[serde(rename = "compact")]
    Compact,
    #[serde(rename = "pretty")]
    Pretty,
    #[serde(rename = "json")]
    Json,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
#[allow(unused)]
#[serde(default)]
pub(crate) struct TracingSettings {
    pub stdout_enabled: bool,
    pub stdout_target: bool,
    pub stdout_default_level: TracingLevel,
    pub stdout_level: TracingLevel,
    pub stdout_format: TracingFormat,
    pub log_enabled: bool,
    pub log_target: bool,
    pub log_dir: String,
    pub log_default_level: TracingLevel,
    pub log_level: TracingLevel,
    pub log_format: TracingFormat,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(unused)]
pub(crate) struct OAuthSettings {
    pub client_id: String,
    pub client_secret: String,
    pub redirect_uri: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(unused)]
pub(crate) struct ServerSettings {
    pub ip: Ipv4Addr,
    pub port: u16,
    pub timeout: u32,
    pub url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(unused)]
pub(crate) struct Settings {
    pub discord_token: String,
    pub refresh_interval: u64,
    pub storage_path: String,
    pub clan_wars_interval: u64,
    pub clan_wars_maps_count: u16,
    pub clan_wars_contribution_interval: u64,
    pub clan_peak_interval: u64,
    pub commander_orders_retention: u64,
    pub oauth: Option<OAuthSettings>,
    pub server: ServerSettings,
    pub tracing: TracingSettings,
}

impl Settings {
    pub fn new() -> Result<Self, ConfigError> {
        info!("Creating settings from configuration file...");

        let s = Config::builder()
            .set_default("refresh_interval", 600)?
            .set_default("storage_path", "./.storage")?
            .set_default("clan_wars_interval", 360)?
            .set_default("clan_wars_maps_count", 30)?
            .set_default("clan_wars_contribution_interval", 180)?
            .set_default("clan_peak_interval", 10)?
            .set_default("commander_orders_retention", 30)?
            .set_default(
                "server",
                ValueKind::Array(vec![
                    Value::new(Some(&"ip".to_owned()), "0.0.0.0"),
                    Value::new(Some(&"port".to_owned()), 3000),
                    Value::new(Some(&"timeout".to_owned()), 30),
                ]),
            )?
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

                if config.clan_wars_contribution_interval < 30 {
                    return Err(ConfigError::Message(
                        "CLAN_WARS_CONTRIBUTION_INTERVAL should be at least 30 minutes".to_owned(),
                    ));
                }

                if config.clan_peak_interval < 5 {
                    return Err(ConfigError::Message(
                        "CLAN_PEAK_INTERVAL should be at least 5 minutes".to_owned(),
                    ));
                }

                info!("Settings created.");

                Ok(config)
            }
            Err(e) => Err(e),
        }
    }
}
