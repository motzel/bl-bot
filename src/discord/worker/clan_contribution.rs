use std::cmp::Ordering;
use std::collections::HashMap;
use std::sync::Arc;

use chrono::Utc;
use cli_table::{format::Justify, Cell, ColorChoice, Table};
use poise::serenity_prelude::{
    AutoArchiveDuration, ChannelType, CreateAllowedMentions, CreateAttachment, CreateMessage,
    CreateThread,
};
use serde::{Deserialize, Serialize};
use tokio_util::sync::CancellationToken;

use crate::beatleader::clan::{ClanPlayer, ClanTag};
use crate::beatleader::player::PlayerId;
use crate::beatleader::pp::{curve_at_value, CLAN_WEIGHT_COEFFICIENT};
use crate::discord::bot::beatleader::clan::{ClanWars, ClanWarsSort};
use crate::discord::{serenity, BotData};
use crate::storage::guild::GuildSettingsRepository;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub(crate) struct ClanStats {
    pub clan_tag: ClanTag,
    pub maps_count: u32,
    pub total_pp: f64,
    pub soldiers: Vec<ClanSoldierStats>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ClanSoldierStats {
    pub player: ClanPlayer,
    pub maps_count: u32,
    pub total_pp: f64,
    pub total_weighted_pp: f64,
    pub efficiency: f64,
    pub score: f64,
}

pub struct BlClanContributionWorker {
    context: serenity::Context,
    guild_settings_repository: Arc<GuildSettingsRepository>,
    refresh_interval: chrono::Duration,
    token: CancellationToken,
}

impl BlClanContributionWorker {
    pub fn new(
        context: serenity::Context,
        data: BotData,
        refresh_interval: chrono::Duration,
        token: CancellationToken,
    ) -> Self {
        Self {
            context,
            guild_settings_repository: data.guild_settings_repository,
            refresh_interval,
            token,
        }
    }

    pub async fn run(&self) {
        for guild in self.guild_settings_repository.all().await {
            if let Some(clan_settings) = guild.get_clan_settings() {
                if let Some(clan_wars_channel_id) =
                    clan_settings.get_clan_wars_contribution_channel()
                {
                    let last_posted_at = clan_settings.get_clan_wars_contribution_posted_at();

                    tracing::info!(
                        "Refreshing clan {} contribution, last posted at: {}...",
                        clan_settings.get_clan(),
                        if last_posted_at.is_some() {
                            format!("{}", last_posted_at.unwrap())
                        } else {
                            "never".to_owned()
                        }
                    );

                    if last_posted_at.is_none()
                        || last_posted_at
                            .unwrap()
                            .le(&(Utc::now() - chrono::Duration::minutes(60)))
                    {
                        match self
                            .guild_settings_repository
                            .set_clan_wars_contribution_posted_at(&guild.get_key(), Utc::now())
                            .await
                        {
                            Ok(_) => {
                                tracing::info!(
                                    "{} clan contribution posted time set.",
                                    clan_settings.get_clan()
                                );

                                match ClanWars::fetch(
                                    clan_settings.get_clan(),
                                    ClanWarsSort::ToHold,
                                    None,
                                )
                                .await
                                {
                                    Ok(clan_wars) => {
                                        tracing::info!(
                                            "{} clan contribution maps found. Posting contribution to channel #{}",
                                            clan_wars.maps.len(),
                                            clan_wars_channel_id
                                        );

                                        let mut clan_stats = ClanStats {
                                            clan_tag: clan_settings.get_clan(),
                                            ..Default::default()
                                        };

                                        let mut soldiers =
                                            HashMap::<PlayerId, ClanSoldierStats>::new();

                                        for map in clan_wars.maps.into_iter() {
                                            clan_stats.maps_count += 1;

                                            for (idx, score) in map.scores.into_iter().enumerate() {
                                                let weight =
                                                    CLAN_WEIGHT_COEFFICIENT.powi(idx as i32);
                                                let weighted_pp = score.pp * weight;

                                                clan_stats.total_pp += weighted_pp;

                                                soldiers
                                                    .entry(score.player_id)
                                                    .and_modify(|s| {
                                                        s.maps_count += 1;
                                                        s.total_pp += score.pp;
                                                        s.total_weighted_pp += weighted_pp;
                                                    })
                                                    .or_insert(ClanSoldierStats {
                                                        player: score.player,
                                                        maps_count: 1,
                                                        total_pp: score.pp,
                                                        total_weighted_pp: weighted_pp,
                                                        score: 0.0,
                                                        efficiency: 0.0,
                                                    });
                                            }
                                        }

                                        clan_stats.soldiers = soldiers
                                            .into_values()
                                            .map(|mut s| {
                                                s.efficiency = if s.total_pp > 0.0 {
                                                    s.total_weighted_pp / s.total_pp * 100.0
                                                } else {
                                                    0.0
                                                };

                                                s.score = if clan_stats.maps_count > 0 {
                                                    let percent_of_maps_played = (s.maps_count
                                                        as f64
                                                        / clan_stats.maps_count as f64
                                                        - 0.1)
                                                        .max(0.01);
                                                    s.total_weighted_pp
                                                        * curve_at_value(percent_of_maps_played)
                                                } else {
                                                    0.0
                                                };

                                                s
                                            })
                                            .collect();

                                        clan_stats.soldiers.sort_unstable_by(|a, b| {
                                            b.score.partial_cmp(&a.score).unwrap_or(Ordering::Equal)
                                        });

                                        let table = clan_stats
                                            .soldiers
                                            .iter()
                                            .map(|s| {
                                                vec![
                                                    s.player.name.as_str().cell(),
                                                    format!(
                                                        "{}/{}",
                                                        s.maps_count, clan_stats.maps_count
                                                    )
                                                    .cell()
                                                    .justify(Justify::Right),
                                                    format!("{:.2}", s.total_pp)
                                                        .cell()
                                                        .justify(Justify::Right),
                                                    format!("{:.2}", s.total_weighted_pp)
                                                        .cell()
                                                        .justify(Justify::Right),
                                                    format!("{:.2}%", s.efficiency)
                                                        .cell()
                                                        .justify(Justify::Right),
                                                    if clan_stats.total_pp > 0.0 {
                                                        format!(
                                                            "{:.2}%",
                                                            s.total_weighted_pp
                                                                / clan_stats.total_pp
                                                                * 100.0
                                                        )
                                                        .cell()
                                                        .justify(Justify::Right)
                                                    } else {
                                                        "0.00%".cell().justify(Justify::Center)
                                                    },
                                                    format!("{:.2}", s.score)
                                                        .cell()
                                                        .justify(Justify::Right),
                                                ]
                                            })
                                            .collect::<Vec<_>>()
                                            .table()
                                            .title(vec![
                                                "Soldier".cell(),
                                                "Maps".cell(),
                                                "Total PP".cell(),
                                                "Contributed PP".cell(),
                                                "PP efficiency".cell(),
                                                "Contribution".cell(),
                                                "Credits".cell(),
                                            ])
                                            .color_choice(ColorChoice::Never);

                                        match table.display() {
                                            Ok(table_display) => {
                                                let file_contents = format!(
                                                    "// {} //\n\nCaptured maps: {}\nTotal captured PP: {:.2}\n\n{}",
                                                    clan_stats.clan_tag,
                                                    clan_stats.maps_count,
                                                    clan_stats.total_pp,
                                                    table_display
                                                );

                                                // create new thread if possible
                                                tracing::debug!("Creating clan wars contribution thread for the clan {}...", clan_stats.clan_tag.clone());

                                                let channel_id = match clan_wars_channel_id
                                                    .create_thread(
                                                        &self.context,
                                                        CreateThread::new(format!(
                                                            "{} clan wars contribution",
                                                            clan_stats.clan_tag,
                                                        ))
                                                        .auto_archive_duration(
                                                            AutoArchiveDuration::OneHour,
                                                        )
                                                        .kind(ChannelType::PublicThread),
                                                    )
                                                    .await
                                                {
                                                    Ok(guild_channel) => {
                                                        tracing::debug!("Clan wars contribution thread for the {} clan created.", clan_stats.clan_tag.clone());

                                                        guild_channel.id
                                                    }
                                                    Err(err) => {
                                                        tracing::error!("Can not create clan wars contribution thread for the {} clan on channel #{}: {}", clan_stats.clan_tag.clone(),clan_wars_channel_id, err);

                                                        clan_wars_channel_id
                                                    }
                                                };

                                                let message = CreateMessage::new()
                                                    .content(
                                                        format!("Current player contributions to maps captured by the {} clan", clan_stats.clan_tag.clone()),
                                                    )
                                                    .add_file(CreateAttachment::bytes(
                                                        file_contents,
                                                        "contribution.txt".to_string(),
                                                    ))
                                                    .allowed_mentions(CreateAllowedMentions::new());
                                                match channel_id
                                                    .send_message(&self.context, message)
                                                    .await
                                                {
                                                    Ok(_) => {
                                                        tracing::debug!("Clan wars contribution for the {} clan posted to channel #{}.", clan_stats.clan_tag.clone(), clan_wars_channel_id);
                                                    }
                                                    Err(err) => {
                                                        tracing::error!("Can not post clan wars contribution to channel #{}: {}", clan_wars_channel_id, err);
                                                    }
                                                };
                                            }
                                            Err(err) => {
                                                tracing::error!(
                                                    "Can not create contribution table: {}",
                                                    err
                                                );
                                            }
                                        };
                                    }
                                    Err(err) => {
                                        tracing::error!(
                                            "Can not fetch clan contribution map list: {:?}",
                                            err
                                        );
                                    }
                                }

                                tracing::info!(
                                    "Clan contribution for a clan {} refreshed and posted.",
                                    clan_settings.get_clan()
                                );
                            }
                            Err(err) => {
                                tracing::error!(
                                    "Can not set clan contribution posted time for clan {}: {:?}",
                                    clan_settings.get_clan(),
                                    err
                                );
                            }
                        }
                    } else {
                        tracing::info!(
                            "Clan {} contribution do not require posting yet.",
                            clan_settings.get_clan()
                        );
                    }
                }
            }
        }
    }
}
