use crate::beatleader::oauth::OAuthAppCredentials;
use crate::discord::bot::GuildOAuthTokenRepository;
use crate::discord::BotData;
use crate::storage::guild::GuildSettingsRepository;
use crate::storage::player_oauth_token::PlayerOAuthTokenRepository;
use crate::BL_CLIENT;
use std::sync::Arc;
use tokio_util::sync::CancellationToken;
use tracing::{error, info, warn};

pub struct BlOauthTokenRefreshWorker {
    guild_settings_repository: Arc<GuildSettingsRepository>,
    player_oauth_token_repository: Arc<PlayerOAuthTokenRepository>,
    oauth_credentials: Option<OAuthAppCredentials>,
    refresh_interval: chrono::Duration,
    token: CancellationToken,
}

impl BlOauthTokenRefreshWorker {
    pub fn new(
        data: BotData,
        refresh_interval: chrono::Duration,
        token: CancellationToken,
    ) -> Self {
        let oauth_credentials = data.oauth_credentials();

        Self {
            guild_settings_repository: data.guild_settings_repository,
            player_oauth_token_repository: data.player_oauth_token_repository,
            oauth_credentials,
            refresh_interval,
            token,
        }
    }

    pub async fn run(&self) {
        info!("Refreshing expired OAuth tokens...");

        if let Some(ref oauth_credentials) = self.oauth_credentials {
            for guild in self.guild_settings_repository.all().await {
                if let Some(clan_settings) = guild.get_clan_settings() {
                    if clan_settings.is_oauth_token_set() {
                        info!(
                            "Refreshing OAuth token for a clan {}...",
                            clan_settings.get_clan()
                        );

                        let clan_owner_id = clan_settings.get_owner();

                        let oauth_token_option =
                            self.player_oauth_token_repository.get(&clan_owner_id).await;

                        if let Some(oauth_token) = oauth_token_option {
                            if !oauth_token.oauth_token.is_valid_for(self.refresh_interval) {
                                let guild_oauth_token_repository = GuildOAuthTokenRepository::new(
                                    clan_owner_id,
                                    Arc::clone(&self.player_oauth_token_repository),
                                );
                                let oauth_client = BL_CLIENT.with_oauth(
                                    oauth_credentials.clone(),
                                    guild_oauth_token_repository,
                                );

                                match oauth_client.refresh_token_if_needed().await {
                                    Ok(oauth_token) => {
                                        info!(
                                            "OAuth token refreshed, expiration date: {}",
                                            oauth_token.get_expiration()
                                        );
                                    }
                                    Err(err) => {
                                        error!("OAuth token refreshing error: {}", err);
                                    }
                                }
                            } else {
                                info!("OAuth token is still valid, skip refreshing.");
                            }
                        } else {
                            warn!(
                                "No OAuth token for a clan {} found.",
                                clan_settings.get_clan()
                            );
                        }
                    }
                }

                if self.token.is_cancelled() {
                    warn!("Oauth token update task is shutting down...");
                    return;
                }
            }

            info!("OAuth tokens refreshed.");
        } else {
            info!("No OAuth credentials, skipping.");
        }
    }
}
