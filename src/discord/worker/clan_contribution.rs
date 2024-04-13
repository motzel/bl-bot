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

use crate::beatleader::clan::{ClanMapScore, ClanPlayer, ClanTag};
use crate::beatleader::player::{LeaderboardId, PlayerId};
use crate::beatleader::pp::{curve_at_value, CLAN_WEIGHT_COEFFICIENT};
use crate::discord::bot::beatleader::clan::{ClanWars, ClanWarsSort};
use crate::discord::bot::beatleader::player::Player;
use crate::discord::bot::post_long_msg_in_parts;
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

                                let mut soldiers = HashMap::<PlayerId, Player>::new();
                                for user_id in clan_settings.get_clan_wars_soldiers().iter() {
                                    match self.player_repository.get(user_id).await {
                                        Some(player) => {
                                            if player
                                                .is_primary_clan_member(&clan_settings.get_clan())
                                            {
                                                soldiers.insert(player.id.clone(), player);
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

                                match self
                                    .get_stats(
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
                                        let conquer_clan_stats = match self
                                            .get_stats(
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
                                                    format!(
                                                        "{}/{}",
                                                        s.maps_count,
                                                        captured_clan_stats.maps_count
                                                    )
                                                    .cell()
                                                    .justify(Justify::Right),
                                                    format!("{:.2}", s.points)
                                                        .cell()
                                                        .justify(Justify::Right),
                                                    format!(
                                                        "{}/{}",
                                                        s.bonus_maps_count, bonus_maps_count,
                                                    )
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
                                                ]
                                            })
                                            .collect::<Vec<_>>()
                                            .table()
                                            .title(vec![
                                                "Soldier".cell(),
                                                "Cap. maps".cell(),
                                                "Cap. points".cell(),
                                                "Bonus maps".cell(),
                                                "Bonus points".cell(),
                                                "Total points".cell(),
                                            ])
                                            .color_choice(ColorChoice::Never);

                                        match table.display() {
                                            Ok(table_display) => {
                                                let file_contents = format!(
                                                    "// {} //\n\nCaptured maps: {}\nBonus (to conquer) maps: {}\n\n{}",
                                                    captured_clan_stats.clan_tag,
                                                    captured_clan_stats.maps_count,
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
                                                        tracing::debug!("Clan wars contribution file for the {} clan posted to channel #{}.", captured_clan_stats.clan_tag.clone(), clan_wars_channel_id);

                                                        let soldiers_pad = captured_clan_stats
                                                            .soldiers
                                                            .len()
                                                            .div_ceil(10);
                                                        let content = captured_clan_stats
                                                            .soldiers
                                                            .iter()
                                                            .enumerate()
                                                            .map(|(idx, s)| {
                                                                format!(
                                                                    "{:0pad$}. {} **{:.2} points**\n",
                                                                    idx + 1,
                                                                    s.player.name.clone(),
                                                                    s.total_points,
                                                                    pad = soldiers_pad
                                                                )
                                                            })
                                                            .collect::<Vec<_>>();

                                                        match post_long_msg_in_parts(
                                                            &self.context,
                                                            channel_id,
                                                            content,
                                                        )
                                                        .await
                                                        {
                                                            Ok(_) => {
                                                                tracing::debug!("Clan wars contribution ranking for the {} clan posted to channel #{}.", captured_clan_stats.clan_tag.clone(), clan_wars_channel_id);
                                                            }
                                                            Err(err) => {
                                                                tracing::error!("Can not post clan wars contribution ranking to channel #{}: {}", clan_wars_channel_id, err);
                                                            }
                                                        }
                                                    }
                                                    Err(err) => {
                                                        tracing::error!("Can not post clan wars contribution file to channel #{}: {}", clan_wars_channel_id, err);
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
        &self,
        clan_tag: ClanTag,
        sort: ClanWarsSort,
        count: Option<u32>,
        soldiers: &HashMap<PlayerId, Player>,
    ) -> Option<ClanStats> {
        match ClanWars::fetch(clan_tag.clone(), sort.clone(), count, true, None).await {
            Ok(mut clan_wars) => {
                let mut clan_stats = ClanStats {
                    clan_tag,
                    ..Default::default()
                };

                // get all relevant leaderboard ids
                let leaderboard_ids = clan_wars
                    .maps
                    .iter()
                    .map(|m| m.map.leaderboard.id.clone())
                    .collect::<Vec<_>>();

                // fetch soldiers scores for relevant leaderboards and add to the clan wars
                for (player_id, player) in soldiers.iter() {
                    match self.player_scores_repository.get(player_id).await {
                        Some(player_scores) => player_scores
                            .scores
                            .into_iter()
                            .filter_map(|score| {
                                if !leaderboard_ids.contains(&score.leaderboard_id) {
                                    return None;
                                }

                                Some((
                                    score.leaderboard_id,
                                    ClanMapScore {
                                        id: 0,
                                        player_id: score.player_id.clone(),
                                        player: ClanPlayer {
                                            id: score.player_id.clone(),
                                            name: player.name.clone(),
                                            avatar: player.avatar.clone(),
                                            country: player.country.clone(),
                                            rank: player.rank,
                                            country_rank: player.country_rank,
                                            pp: player.pp,
                                        },
                                        accuracy: score.accuracy,
                                        pp: score.pp,
                                        rank: score.rank,
                                        bad_cuts: 0,
                                        bomb_cuts: 0,
                                        missed_notes: 0,
                                        walls_hit: 0,
                                        full_combo: score.full_combo,
                                        modifiers: score.modifiers,
                                        timeset: score.timeset,
                                        timepost: score.timepost,
                                    },
                                ))
                            })
                            .collect::<HashMap<LeaderboardId, ClanMapScore>>(),
                        None => HashMap::new(),
                    }
                    .into_iter()
                    .for_each(|(leaderboard_id, clan_map_score)| {
                        match clan_wars
                            .maps
                            .iter()
                            .position(|m| m.map.leaderboard.id == leaderboard_id)
                        {
                            None => {
                                tracing::warn!(
                                    "Can not find an index for clan wars maps for leaderboardId {}",
                                    &leaderboard_id
                                );
                            }
                            Some(idx) => {
                                clan_wars.maps[idx].scores.push(clan_map_score);
                            }
                        };
                    });
                }

                // sort scores for all clan wars maps by pp desc
                for map in clan_wars.maps.iter_mut() {
                    map.scores.sort_unstable_by(|a, b| {
                        b.pp.partial_cmp(&a.pp).unwrap_or(Ordering::Equal)
                    });
                }

                drop(leaderboard_ids);

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
                            // map 0-1 range into 70-98% acc
                            let percent_of_maps_played =
                                (s.maps_count as f64 / clan_stats.maps_count as f64) / 100.0 * 0.28
                                    + 0.7;
                            s.map_percentages
                                * curve_at_value(percent_of_maps_played)
                                * std::f64::consts::PI
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
