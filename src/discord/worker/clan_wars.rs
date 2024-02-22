use crate::beatleader::clan::{ClanMapsParam, ClanMapsSort, ClanRankingParam};
use crate::beatleader::oauth::OAuthAppCredentials;
use crate::beatleader::pp::{
    calculate_acc_from_pp, calculate_pp_boundary, StarRating, CLAN_WEIGHT_COEFFICIENT,
};
use crate::beatleader::{BlContext, SortOrder};
use crate::discord::{serenity, BotData};
use crate::storage::guild::GuildSettingsRepository;
use crate::storage::player_oauth_token::PlayerOAuthTokenRepository;
use crate::BL_CLIENT;
use chrono::Utc;
use poise::serenity_prelude::ChannelId;
use std::cmp::Ordering;
use std::sync::Arc;
use tokio_util::sync::CancellationToken;
use tracing::{error, info};

pub struct BlClanWarsMapsWorker {
    context: serenity::Context,
    guild_settings_repository: Arc<GuildSettingsRepository>,
    player_oauth_token_repository: Arc<PlayerOAuthTokenRepository>,
    oauth_credentials: Option<OAuthAppCredentials>,
    refresh_interval: chrono::Duration,
    token: CancellationToken,
}

impl BlClanWarsMapsWorker {
    pub fn new(
        context: serenity::Context,
        data: BotData,
        refresh_interval: chrono::Duration,
        token: CancellationToken,
    ) -> Self {
        let oauth_credentials = data.oauth_credentials();

        Self {
            context,
            guild_settings_repository: data.guild_settings_repository,
            player_oauth_token_repository: data.player_oauth_token_repository,
            oauth_credentials,
            refresh_interval,
            token,
        }
    }

    pub async fn run(&self) {
        for guild in self.guild_settings_repository.all().await {
            if let Some(clan_settings) = guild.get_clan_settings() {
                if let Some(clan_wars_channel_id) = clan_settings.get_clan_wars_maps_channel() {
                    let last_posted_at = clan_settings.get_clan_wars_posted_at();

                    info!(
                        "Refreshing clan {} wars maps, last posted at: {}...",
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
                            .set_clan_wars_posted_at(&guild.get_key(), Utc::now())
                            .await
                        {
                            Ok(_) => {
                                info!(
                                    "{} clan wars maps posted time set.",
                                    clan_settings.get_clan()
                                );

                                match BL_CLIENT
                                    .clan()
                                    .maps(
                                        &clan_settings.get_clan(),
                                        &[
                                            ClanMapsParam::Count(30),
                                            ClanMapsParam::Page(1),
                                            ClanMapsParam::Order(SortOrder::Descending),
                                            ClanMapsParam::Context(BlContext::General),
                                            ClanMapsParam::Sort(ClanMapsSort::ToConquer),
                                        ],
                                    )
                                    .await
                                {
                                    Ok(maps_list) => {
                                        let mut maps = vec![];
                                        for map in maps_list.data.into_iter() {
                                            if let Ok(scores) = BL_CLIENT
                                                .clan()
                                                .scores(
                                                    &map.leaderboard.id,
                                                    map.id,
                                                    &[
                                                        ClanRankingParam::Count(100),
                                                        ClanRankingParam::Page(1),
                                                    ],
                                                )
                                                .await
                                            {
                                                maps.push((
                                                    map,
                                                    scores
                                                        .data
                                                        .into_iter()
                                                        .map(|score| score.pp)
                                                        .collect::<Vec<_>>(),
                                                ));
                                            }
                                        }

                                        let mut out = maps
                                            .into_iter()
                                            .map(|(map, mut pps)| {
                                                let pp = calculate_pp_boundary(
                                                    CLAN_WEIGHT_COEFFICIENT,
                                                    &mut pps,
                                                    -map.pp,
                                                );
                                                let acc = match calculate_acc_from_pp(
                                                    pp,
                                                    StarRating {
                                                        pass: map
                                                            .leaderboard
                                                            .difficulty
                                                            .pass_rating,
                                                        tech: map
                                                            .leaderboard
                                                            .difficulty
                                                            .tech_rating,
                                                        acc: map.leaderboard.difficulty.acc_rating,
                                                    },
                                                    map.leaderboard.difficulty.mode_name.as_str(),
                                                ) {
                                                    None => "Not possible".to_owned(),
                                                    Some(acc) => format!("{:.2}%", acc * 100.0),
                                                };
                                                let acc_fs = match map
                                                    .leaderboard
                                                    .difficulty
                                                    .modifiers_rating
                                                    .as_ref()
                                                {
                                                    None => "No ratings".to_owned(),
                                                    Some(ratings) => match calculate_acc_from_pp(
                                                        pp,
                                                        StarRating {
                                                            pass: ratings.fs_pass_rating,
                                                            tech: ratings.fs_tech_rating,
                                                            acc: ratings.fs_acc_rating,
                                                        },
                                                        map.leaderboard
                                                            .difficulty
                                                            .mode_name
                                                            .as_str(),
                                                    ) {
                                                        None => "Not possible".to_owned(),
                                                        Some(acc) => format!("{:.2}%", acc * 100.0),
                                                    },
                                                };
                                                let acc_sfs = match map
                                                    .leaderboard
                                                    .difficulty
                                                    .modifiers_rating
                                                    .as_ref()
                                                {
                                                    None => "No ratings".to_owned(),
                                                    Some(ratings) => match calculate_acc_from_pp(
                                                        pp,
                                                        StarRating {
                                                            pass: ratings.sf_pass_rating,
                                                            tech: ratings.sf_tech_rating,
                                                            acc: ratings.sf_acc_rating,
                                                        },
                                                        map.leaderboard
                                                            .difficulty
                                                            .mode_name
                                                            .as_str(),
                                                    ) {
                                                        None => "Not possible".to_owned(),
                                                        Some(acc) => format!("{:.2}%", acc * 100.0),
                                                    },
                                                };
                                                (
                                                    map.leaderboard.id,
                                                    map.rank,
                                                    map.leaderboard.song.name,
                                                    map.leaderboard.difficulty.difficulty_name,
                                                    map.pp,
                                                    pps.len(),
                                                    pp,
                                                    acc,
                                                    acc_fs,
                                                    acc_sfs,
                                                )
                                            })
                                            .collect::<Vec<_>>();

                                        out.sort_unstable_by(|a, b| {
                                            a.6.partial_cmp(&b.6).unwrap_or(Ordering::Equal)
                                        });

                                        info!(
                                            "{} clan wars maps found. Posting maps to channel #{}",
                                            out.len(),
                                            clan_wars_channel_id
                                        );

                                        async fn post_msg(
                                            global_ctx: &serenity::Context,
                                            channel_id: ChannelId,
                                            description: &str,
                                            content: &str,
                                        ) {
                                            match channel_id
                                                .send_message(global_ctx.clone(), |m| {
                                                    m.embed(|e| e.description(description))
                                                        .allowed_mentions(|am| am.empty_parse());

                                                    if !content.is_empty() {
                                                        m.content(content);
                                                    }

                                                    m
                                                })
                                                .await
                                            {
                                                Ok(_) => {}
                                                Err(err) => {
                                                    info!("Can not post clan wars map to channel #{}: {}", channel_id, err);
                                                }
                                            };
                                        }

                                        const MAX_DISCORD_MSG_LENGTH: usize = 2000;
                                        let mut msg_count = 0;
                                        let header = format!(
                                            "### **{} clan wars maps**",
                                            clan_settings.get_clan()
                                        );
                                        let mut description = String::new();
                                        for item in out {
                                            let map_description = format!("### **#{} [{} / {}](https://www.beatleader.xyz/leaderboard/clanranking/{}/{})**\n{} score{} / {:.2}pp / **{:.2} raw pp**\n {} / {} FS / {} SF\n",
                                                                          item.1, item.2, item.3, item.0, ((item.1 - 1) / 10 + 1),
                                                                          item.5, if item.5 > 1 { "s" } else { "" }, item.4, item.6, item.7, item.8, item.9);

                                            if description.len()
                                                + "\n\n".len()
                                                + map_description.len()
                                                + (if msg_count > 0 { 0 } else { header.len() })
                                                < MAX_DISCORD_MSG_LENGTH
                                            {
                                                description.push_str(&map_description);
                                            } else {
                                                post_msg(
                                                    &self.context,
                                                    clan_wars_channel_id,
                                                    description.as_str(),
                                                    if msg_count == 0 {
                                                        header.as_str()
                                                    } else {
                                                        ""
                                                    },
                                                )
                                                .await;

                                                description = String::new();
                                                msg_count += 1;
                                            }
                                        }

                                        if !description.is_empty() {
                                            post_msg(
                                                &self.context,
                                                clan_wars_channel_id,
                                                description.as_str(),
                                                if msg_count == 0 { header.as_str() } else { "" },
                                            )
                                            .await;
                                        }
                                    }
                                    Err(err) => {
                                        error!("Can not fetch clan wars map list: {:?}", err);
                                    }
                                }

                                info!(
                                    "Clan wars maps for a clan {} refreshed and posted.",
                                    clan_settings.get_clan()
                                );
                            }
                            Err(err) => {
                                error!(
                                    "Can not set clan wars posted time for clan {}: {:?}",
                                    clan_settings.get_clan(),
                                    err
                                );
                            }
                        }
                    } else {
                        info!(
                            "Clan {} wars maps do not require posting yet.",
                            clan_settings.get_clan()
                        );
                    }
                }
            }
        }
    }
}
