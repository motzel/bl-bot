use std::{fs::OpenOptions, io::Write};

use std::sync::Arc;

use chrono::Utc;
use poise::serenity_prelude::{CreateAllowedMentions, CreateMessage};
use tokio_util::sync::CancellationToken;

use crate::discord::bot::beatleader::clan::fetch_clan;
use crate::discord::{serenity, BotData};
use crate::storage::clan_peak::{ClanPeak, ClanPeakRepository};
use crate::storage::guild::GuildSettingsRepository;

pub struct BlClanPeakWorker {
    context: serenity::Context,
    guild_settings_repository: Arc<GuildSettingsRepository>,
    clan_peak_repository: Arc<ClanPeakRepository>,
    token: CancellationToken,
}

impl BlClanPeakWorker {
    pub fn new(context: serenity::Context, data: BotData, token: CancellationToken) -> Self {
        Self {
            context,
            guild_settings_repository: data.guild_settings_repository,
            clan_peak_repository: data.clan_peak_repository,
            token,
        }
    }

    pub async fn run(&self) {
        for guild in self.guild_settings_repository.all().await {
            if let Some(clan_settings) = guild.get_clan_settings() {
                let clan_tag = clan_settings.get_clan();

                if let Some(clan_wars_channel_id) =
                    clan_settings.get_clan_wars_contribution_channel()
                {
                    let last_posted_at = clan_settings.get_clan_peak_posted_at();

                    tracing::info!(
                        "Refreshing clan {} peak, last posted at: {}...",
                        &clan_tag,
                        if last_posted_at.is_some() {
                            format!("{}", last_posted_at.unwrap())
                        } else {
                            "never".to_owned()
                        }
                    );

                    match fetch_clan(&clan_tag).await {
                        Ok(clan) => {
                            tracing::info!("Refreshing clan {} peak...", &clan_tag);

                            match self
                                .clan_peak_repository
                                .get(&clan_settings.get_clan_id())
                                .await
                            {
                                Ok(clan_peak) => {
                                    let current_peak = match clan_peak {
                                        None => 0,
                                        Some(clan_peak) => clan_peak.peak,
                                    };

                                    if clan.capture_leaderboards_count > 0
                                        && current_peak < clan.capture_leaderboards_count
                                    {
                                        let clan_peak = ClanPeak::new(
                                            clan_settings.get_clan_id(),
                                            clan_tag.clone(),
                                            clan.capture_leaderboards_count,
                                            Utc::now(),
                                        );

                                        match self.clan_peak_repository.set(clan_peak).await {
                                            Ok(clan_peak) => {
                                                tracing::info!(
                                                    "Clan peak for {} set to {}.",
                                                    &clan_tag,
                                                    clan_peak.peak
                                                );

                                                tracing::info!(
                                                    "Posting {} clan peak...",
                                                    &clan_tag
                                                );

                                                let message = CreateMessage::new()
                                                    .content(format!(
                                                        "# {} new peak: {} maps 🥳",
                                                        &clan_tag, clan_peak.peak
                                                    ))
                                                    .allowed_mentions(CreateAllowedMentions::new());

                                                match clan_wars_channel_id
                                                    .send_message(&self.context, message)
                                                    .await
                                                {
                                                    Ok(_) => {
                                                        match self
                                                            .guild_settings_repository
                                                            .set_clan_peak_posted_at(
                                                                &guild.get_key(),
                                                                Utc::now(),
                                                            )
                                                            .await
                                                        {
                                                            Ok(_) => {
                                                                tracing::info!(
                                                                    "{} clan peak posted time set.",
                                                                    &clan_tag
                                                                );
                                                            }
                                                            Err(err) => {
                                                                tracing::error!(
                                                                    "Can not set {} clan peak posted time: {}",
                                                                    &clan_tag,
                                                                    err
                                                                );
                                                            }
                                                        }

                                                        tracing::debug!(
                                                            "{} clan peak posted to channel #{}.",
                                                            &clan_tag,
                                                            clan_wars_channel_id
                                                        );
                                                    }
                                                    Err(err) => {
                                                        tracing::error!(
                                                            "Can not post {} clan peak: {}",
                                                            &clan_tag,
                                                            err
                                                        );
                                                    }
                                                }
                                            }
                                            Err(err) => {
                                                tracing::error!(
                                                    "Can not set {} clan peak: {}",
                                                    &clan_tag,
                                                    err
                                                );
                                            }
                                        }
                                    } else {
                                        tracing::info!("Clan {} peak is up to date.", &clan_tag);
                                    }

                                    let clan_peak = ClanPeak::new(
                                        clan_settings.get_clan_id(),
                                        clan_tag.clone(),
                                        clan.capture_leaderboards_count,
                                        Utc::now(),
                                    );
                                    if let Ok(mut json) =
                                        serde_json::to_string::<ClanPeak>(&clan_peak)
                                    {
                                        match OpenOptions::new()
                                            .create(true)
                                            .append(true)
                                            .open(".storage/clan-peak-history.ndjson")
                                        {
                                            Ok(mut file) => {
                                                json.push('\n');

                                                if let Err(e) = file.write_all(json.as_bytes()) {
                                                    tracing::error!("Couldn't write clan peak history to file: {}", e);
                                                }
                                            }
                                            Err(e) => {
                                                tracing::error!(
                                                    "Couldn't create clan peak history file: {}",
                                                    e
                                                );
                                            }
                                        }
                                    }
                                }
                                Err(err) => {
                                    tracing::error!(
                                        "Can not get {} clan current peak: {}",
                                        &clan_tag,
                                        err
                                    );
                                }
                            }
                        }
                        Err(err) => {
                            tracing::error!("Can not fetch {} clan info: {}", &clan_tag, err);
                        }
                    };

                    tracing::info!("Clan peak for a clan {} refreshed.", &clan_tag);
                }
            }
        }
    }
}
