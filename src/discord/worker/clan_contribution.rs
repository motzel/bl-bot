use std::cmp::Ordering;
use std::collections::HashMap;
use std::sync::Arc;

use chrono::Utc;
use cli_table::{format::Justify, Cell, ColorChoice, Table};
use poise::serenity_prelude::{
    AutoArchiveDuration, ChannelType, CreateAllowedMentions, CreateAttachment, CreateMessage,
    CreateThread, UserId,
};
use serde::{Deserialize, Serialize};
use tokio_util::sync::CancellationToken;

use crate::beatleader::clan::{ClanPlayer, ClanTag};
use crate::beatleader::player::PlayerId;
use crate::beatleader::pp::{curve_at_value, CLAN_WEIGHT_COEFFICIENT};
use crate::discord::bot::beatleader::clan::{ClanWars, ClanWarsSort};
use crate::discord::{serenity, BotData};
use crate::storage::guild::GuildSettingsRepository;
use crate::storage::player::PlayerRepository;
use crate::storage::player_scores::PlayerScoresRepository;

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
    pub map_percentages: f64,
    pub points: f64,
    pub bonus_maps_count: u32,
    pub bonus_points: f64,
    pub total_points: f64,
}

pub struct BlClanContributionWorker {
    context: serenity::Context,
    guild_settings_repository: Arc<GuildSettingsRepository>,
    player_repository: Arc<PlayerRepository>,
    player_scores_repository: Arc<PlayerScoresRepository>,
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
            player_repository: data.players_repository,
            player_scores_repository: data.player_scores_repository,
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
                            .le(&(Utc::now() - self.refresh_interval))
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

                                let mut soldiers = HashMap::<PlayerId, UserId>::new();
                                for user_id in clan_settings.get_clan_wars_soldiers().iter() {
                                    match self.player_repository.get(user_id).await {
                                        Some(player) => {
                                            if player
                                                .is_primary_clan_member(&clan_settings.get_clan())
                                            {
                                                soldiers.insert(player.id, *user_id);
                                            }
                                        }
                                        None => {
                                            tracing::warn!(
                                                "Can not get player for user @{}",
                                                user_id
                                            )
                                        }
                                    };
                                }

                                match Self::get_stats(
                                    clan_settings.get_clan(),
                                    ClanWarsSort::ToHold,
                                    None,
                                    &soldiers,
                                )
                                .await
                                {
                                    Some(mut captured_clan_stats) => {
                                        tracing::info!(
                                            "{} clan contribution captured maps found. Posting contribution to channel #{}",
                                            captured_clan_stats.maps_count,
                                            clan_wars_channel_id
                                        );

                                        const MIN_BONUS_MAPS_COUNT: u32 = 20;
                                        const MAX_BONUS_MAPS_COUNT: u32 = 50;
                                        let conquer_clan_stats = match Self::get_stats(
                                            clan_settings.get_clan(),
                                            ClanWarsSort::ToConquer,
                                            Some(
                                                if captured_clan_stats.maps_count
                                                    < MIN_BONUS_MAPS_COUNT
                                                {
                                                    MIN_BONUS_MAPS_COUNT
                                                } else {
                                                    captured_clan_stats
                                                        .maps_count
                                                        .min(MAX_BONUS_MAPS_COUNT)
                                                },
                                            ),
                                            &soldiers,
                                        )
                                        .await
                                        {
                                            None => None,
                                            Some(conquer_clan_stats) => {
                                                tracing::info!(
                                                    "{} clan contribution maps to conquer found.",
                                                    conquer_clan_stats.maps_count,
                                                );

                                                Some((
                                                    conquer_clan_stats.maps_count,
                                                    conquer_clan_stats
                                                        .soldiers
                                                        .into_iter()
                                                        .map(|s| (s.player.id.clone(), s))
                                                        .collect::<HashMap<_, _>>(),
                                                ))
                                            }
                                        };

                                        let bonus_maps_count = conquer_clan_stats
                                            .as_ref()
                                            .map_or_else(|| 0, |cs| cs.0);

                                        captured_clan_stats.soldiers.iter_mut().for_each(|s| {
                                            if let Some(bonus_stats) = &conquer_clan_stats {
                                                if let Some(bonus) = bonus_stats.1.get(&s.player.id)
                                                {
                                                    s.bonus_maps_count = bonus.maps_count;
                                                    s.bonus_points = bonus.points;
                                                    s.total_points = s.points
                                                        + CLAN_WEIGHT_COEFFICIENT * bonus.points;
                                                } else {
                                                    s.total_points = s.points;
                                                }
                                            } else {
                                                s.total_points = s.points;
                                            }
                                        });

                                        if let Some(bonus_stats) = &conquer_clan_stats {
                                            bonus_stats.1.iter().enumerate().for_each(
                                                |(_, (player_id, stats))| {
                                                    if !captured_clan_stats
                                                        .soldiers
                                                        .iter()
                                                        .any(|s| &s.player.id == player_id)
                                                    {
                                                        captured_clan_stats.soldiers.extend(vec![
                                                            ClanSoldierStats {
                                                                player: stats.player.clone(),
                                                                maps_count: 0,
                                                                total_pp: 0.0,
                                                                total_weighted_pp: 0.0,
                                                                efficiency: 0.0,
                                                                map_percentages: 0.0,
                                                                points: 0.0,
                                                                bonus_maps_count: stats.maps_count,
                                                                bonus_points: stats.points,
                                                                total_points: stats.points
                                                                    * CLAN_WEIGHT_COEFFICIENT,
                                                            },
                                                        ]);
                                                    }
                                                },
                                            );
                                        }

                                        captured_clan_stats.soldiers.sort_unstable_by(|a, b| {
                                            b.total_points
                                                .partial_cmp(&a.total_points)
                                                .unwrap_or(Ordering::Equal)
                                        });

                                        let table = captured_clan_stats
                                            .soldiers
                                            .iter()
                                            .map(|s| {
                                                vec![
                                                    s.player.name.as_str().cell(),
                                                    format!("{:.2}", s.points)
                                                        .cell()
                                                        .justify(Justify::Right),
                                                    format!(
                                                        "{:.2}",
                                                        s.bonus_points * CLAN_WEIGHT_COEFFICIENT
                                                    )
                                                    .cell()
                                                    .justify(Justify::Right),
                                                    format!("{:.2}", s.total_points)
                                                        .cell()
                                                        .justify(Justify::Right),
                                                    format!(
                                                        "{}/{}",
                                                        s.maps_count,
                                                        captured_clan_stats.maps_count
                                                    )
                                                    .cell()
                                                    .justify(Justify::Right),
                                                    format!(
                                                        "{}/{}",
                                                        s.bonus_maps_count, bonus_maps_count,
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
                                                    if captured_clan_stats.total_pp > 0.0 {
                                                        format!(
                                                            "{:.2}%",
                                                            s.total_weighted_pp
                                                                / captured_clan_stats.total_pp
                                                                * 100.0
                                                        )
                                                        .cell()
                                                        .justify(Justify::Right)
                                                    } else {
                                                        "0.00%".cell().justify(Justify::Center)
                                                    },
                                                ]
                                            })
                                            .collect::<Vec<_>>()
                                            .table()
                                            .title(vec![
                                                "Soldier".cell(),
                                                "Cap. points".cell(),
                                                "Bonus points".cell(),
                                                "Total points".cell(),
                                                "Cap. maps".cell(),
                                                "Bonus maps".cell(),
                                                "Total PP".cell(),
                                                "Contrib. PP".cell(),
                                                "PP eff.".cell(),
                                                "Contrib.".cell(),
                                            ])
                                            .color_choice(ColorChoice::Never);

                                        match table.display() {
                                            Ok(table_display) => {
                                                let file_contents = format!(
                                                    "// {} //\n\nCaptured maps: {}\nTotal captured PP: {:.2}\nBonus (to conquer) maps: {}\n\n{}",
                                                    captured_clan_stats.clan_tag,
                                                    captured_clan_stats.maps_count,
                                                    captured_clan_stats.total_pp,
                                                    bonus_maps_count,
                                                    table_display
                                                );

                                                // create new thread if possible
                                                tracing::debug!("Creating clan wars contribution thread for the clan {}...", captured_clan_stats.clan_tag.clone());

                                                let channel_id = match clan_wars_channel_id
                                                    .create_thread(
                                                        &self.context,
                                                        CreateThread::new(format!(
                                                            "{} clan wars contribution",
                                                            captured_clan_stats.clan_tag,
                                                        ))
                                                        .auto_archive_duration(
                                                            AutoArchiveDuration::OneHour,
                                                        )
                                                        .kind(ChannelType::PublicThread),
                                                    )
                                                    .await
                                                {
                                                    Ok(guild_channel) => {
                                                        tracing::debug!("Clan wars contribution thread for the {} clan created.", captured_clan_stats.clan_tag.clone());

                                                        guild_channel.id
                                                    }
                                                    Err(err) => {
                                                        tracing::error!("Can not create clan wars contribution thread for the {} clan on channel #{}: {}", captured_clan_stats.clan_tag.clone(),clan_wars_channel_id, err);

                                                        clan_wars_channel_id
                                                    }
                                                };

                                                let message = CreateMessage::new()
                                                    .content(
                                                        format!("Current player contributions to maps captured by the {} clan", captured_clan_stats.clan_tag.clone()),
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
                                                        tracing::debug!("Clan wars contribution for the {} clan posted to channel #{}.", captured_clan_stats.clan_tag.clone(), clan_wars_channel_id);
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
                                    None => {
                                        //
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

    async fn get_stats(
        clan_tag: ClanTag,
        sort: ClanWarsSort,
        count: Option<u32>,
        soldiers: &HashMap<PlayerId, UserId>,
    ) -> Option<ClanStats> {
        match ClanWars::fetch(clan_tag.clone(), sort.clone(), count).await {
            Ok(clan_wars) => {
                let mut clan_stats = ClanStats {
                    clan_tag,
                    ..Default::default()
                };

                let mut player_stats = HashMap::<PlayerId, ClanSoldierStats>::new();

                for map in clan_wars.maps.into_iter() {
                    clan_stats.maps_count += 1;

                    let max_map_pp = if !map.scores.is_empty() {
                        map.scores.first().unwrap().pp
                    } else {
                        0.0
                    };

                    for (idx, score) in map.scores.into_iter().enumerate() {
                        let weight = CLAN_WEIGHT_COEFFICIENT.powi(idx as i32);
                        let weighted_pp = score.pp * weight;

                        clan_stats.total_pp += weighted_pp;

                        player_stats
                            .entry(score.player_id)
                            .and_modify(|s| {
                                s.maps_count += 1;
                                s.total_pp += score.pp;
                                s.total_weighted_pp += weighted_pp;
                                s.map_percentages += if max_map_pp > 0.0 {
                                    score.pp / max_map_pp
                                } else {
                                    0.0
                                };
                            })
                            .or_insert(ClanSoldierStats {
                                player: score.player,
                                maps_count: 1,
                                total_pp: score.pp,
                                total_weighted_pp: weighted_pp,
                                map_percentages: if max_map_pp > 0.0 {
                                    score.pp / max_map_pp
                                } else {
                                    0.0
                                },
                                points: 0.0,
                                efficiency: 0.0,
                                bonus_maps_count: 0,
                                bonus_points: 0.0,
                                total_points: 0.0,
                            });
                    }
                }

                clan_stats.soldiers = player_stats
                    .into_values()
                    .filter_map(|mut s| {
                        if !soldiers.contains_key(&s.player.id) {
                            return None;
                        }

                        s.efficiency = if s.total_pp > 0.0 {
                            s.total_weighted_pp / s.total_pp * 100.0
                        } else {
                            0.0
                        };

                        s.points = if clan_stats.maps_count > 0 {
                            let percent_of_maps_played =
                                (s.maps_count as f64 / clan_stats.maps_count as f64 - 0.05)
                                    .max(0.01);
                            s.map_percentages * curve_at_value(percent_of_maps_played)
                        } else {
                            0.0
                        };

                        Some(s)
                    })
                    .collect();

                Some(clan_stats)
            }
            Err(err) => {
                tracing::error!(
                    "Can not fetch clan contribution map list ({}): {:?}",
                    sort,
                    err
                );

                None
            }
        }
    }
}
