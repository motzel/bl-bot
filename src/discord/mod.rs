use std::borrow::Cow;
use std::cmp::Ordering;
use std::collections::HashMap;
use std::sync::Arc;

use chrono::Utc;
use log::{debug, error, info, warn};
pub(crate) use poise::serenity_prelude as serenity;
use poise::serenity_prelude::{AttachmentType, ChannelId, SerenityError};
use poise::Framework;
use serenity::{http, model::id::GuildId};
use tokio_util::sync::CancellationToken;
use tokio_util::task::TaskTracker;

use crate::beatleader::clan::{ClanMapsParam, ClanMapsSort, ClanRankingParam};
use crate::beatleader::oauth::OAuthAppCredentials;
use crate::beatleader::pp::{
    calculate_acc_from_pp, calculate_pp_boundary, StarRating, CLAN_WEIGHT_COEFFICIENT,
};
use crate::beatleader::BlContext;
use crate::beatleader::SortOrder;
use crate::config::CommonData;
use crate::discord::bot::commands::player::get_player_embed;
use crate::discord::bot::commands::{
    cmd_add_auto_role, cmd_clan_invitation, cmd_clan_wars_playlist, cmd_export, cmd_import,
    cmd_link, cmd_profile, cmd_refresh_scores, cmd_register, cmd_remove_auto_role, cmd_replay,
    cmd_set_clan_invitation, cmd_set_clan_invitation_code, cmd_set_clan_wars_maps_channel,
    cmd_set_log_channel, cmd_set_profile_verification, cmd_show_settings, cmd_unlink,
};
use crate::discord::bot::{GuildOAuthTokenRepository, GuildSettings, UserRoleChanges};
use crate::storage::guild::GuildSettingsRepository;
use crate::storage::player::PlayerRepository;
use crate::storage::player_oauth_token::PlayerOAuthTokenRepository;
use crate::storage::player_scores::PlayerScoresRepository;
use crate::BL_CLIENT;

pub mod bot;

pub(crate) struct Data {
    pub guild_settings_repository: Arc<GuildSettingsRepository>,
    pub players_repository: Arc<PlayerRepository>,
    pub player_scores_repository: Arc<PlayerScoresRepository>,
    pub player_oauth_token_repository: Arc<PlayerOAuthTokenRepository>,
    pub oauth_credentials: Option<OAuthAppCredentials>,
}

impl From<CommonData> for Data {
    fn from(value: CommonData) -> Self {
        let oauth_credentials =
            value
                .settings
                .oauth
                .as_ref()
                .map(|oauth_settings| OAuthAppCredentials {
                    client_id: oauth_settings.client_id.clone(),
                    client_secret: oauth_settings.client_secret.clone(),
                    redirect_uri: oauth_settings.redirect_uri.clone(),
                });

        Self {
            guild_settings_repository: value.guild_settings_repository,
            players_repository: value.players_repository,
            player_scores_repository: value.player_scores_repository,
            player_oauth_token_repository: value.player_oauth_token_repository,
            oauth_credentials,
        }
    }
}

pub(crate) type Context<'a> = poise::Context<'a, Data, crate::Error>;

async fn on_error(error: poise::FrameworkError<'_, Data, crate::Error>) {
    match error {
        poise::FrameworkError::Setup { error, .. } => panic!("Failed to start bot: {:?}", error),
        poise::FrameworkError::Command { error, ctx } => {
            info!("Error in command `{}`: {:?}", ctx.command().name, error,);
        }
        error => {
            if let Err(e) = poise::builtins::on_error(error).await {
                info!("Error while handling error: {}", e)
            }
        }
    }
}

pub(crate) async fn init(
    data: CommonData,
    tracker: TaskTracker,
    token: CancellationToken,
) -> Result<Arc<Framework<Data, crate::Error>>, serenity::Error> {
    let settings = data.settings.clone();
    let data: Data = data.into();

    let options = poise::FrameworkOptions {
        commands: vec![
            cmd_replay(),
            cmd_profile(),
            cmd_link(),
            cmd_unlink(),
            cmd_show_settings(),
            cmd_add_auto_role(),
            cmd_remove_auto_role(),
            cmd_set_log_channel(),
            cmd_set_profile_verification(),
            cmd_set_clan_invitation(),
            cmd_set_clan_invitation_code(),
            cmd_clan_invitation(),
            cmd_clan_wars_playlist(),
            cmd_set_clan_wars_maps_channel(),
            // cmd_invite_player(),
            cmd_register(),
            cmd_export(),
            cmd_import(),
            cmd_refresh_scores(),
        ],
        pre_command: |ctx| {
            Box::pin(async move {
                info!("Executing command {}...", ctx.command().qualified_name);
            })
        },
        // This code is run after a command if it was successful (returned Ok)
        post_command: |ctx| {
            Box::pin(async move {
                info!("Executed command {}!", ctx.command().qualified_name);
            })
        },
        on_error: |error| Box::pin(on_error(error)),
        ..Default::default()
    };

    Framework::builder()
        .options(options)
        .token(settings.discord_token.clone())
        .intents(serenity::GatewayIntents::non_privileged()) // | serenity::GatewayIntents::MESSAGE_CONTENT
        .setup(move |ctx, _ready, _framework| {
            Box::pin(async move {
                info!("Bot logged in as {}", _ready.user.name);

                info!("Setting bot status...");
                ctx.set_presence(
                    Some(serenity::model::gateway::Activity::playing("Beat Leader")),
                    serenity::model::user::OnlineStatus::Online,
                )
                .await;

                let global_ctx = ctx.clone();

                let player_oauth_token_repository_worker =
                    Arc::clone(&data.player_oauth_token_repository);
                let guild_settings_repository_worker = Arc::clone(&data.guild_settings_repository);
                let players_repository_worker = Arc::clone(&data.players_repository);
                let player_scores_repository_worker = Arc::clone(&data.player_scores_repository);

                let oauth_credentials_clone = data.oauth_credentials.clone();

                tracker.spawn(async move {
                    let interval = std::time::Duration::from_secs(settings.refresh_interval);
                    info!("Run a task that updates data every {:?}", interval);

                    'outer: loop {
                        info!("Refreshing expired OAuth tokens...");

                        if let Some(ref oauth_credentials) = oauth_credentials_clone {
                            for guild in guild_settings_repository_worker.all().await {
                                if let Some(clan_settings) = guild.get_clan_settings() {
                                    if clan_settings.is_oauth_token_set() {
                                        info!("Refreshing OAuth token for a clan {}...", clan_settings.get_clan());

                                        let clan_owner_id = clan_settings.get_owner();

                                        let oauth_token_option = player_oauth_token_repository_worker.get(&clan_owner_id).await;

                                        if let Some(oauth_token) = oauth_token_option {
                                            if !oauth_token.oauth_token.is_valid_for(chrono::Duration::seconds(settings.refresh_interval as i64 + 30)) {
                                                let guild_oauth_token_repository = GuildOAuthTokenRepository::new(
                                                    clan_owner_id,
                                                    Arc::clone(&player_oauth_token_repository_worker),
                                                );
                                                let oauth_client = BL_CLIENT.with_oauth(
                                                    oauth_credentials.clone(),
                                                    guild_oauth_token_repository,
                                                );

                                                match oauth_client.refresh_token_if_needed().await {
                                                    Ok(oauth_token) => {
                                                        info!("OAuth token refreshed, expiration date: {}", oauth_token.get_expiration());
                                                    },
                                                    Err(err) => {
                                                        error!("OAuth token refreshing error: {}", err);
                                                    }
                                                }
                                            } else {
                                                info!("OAuth token is still valid, skip refreshing.");
                                            }
                                        } else {
                                            warn!("No OAuth token for a clan {} found.", clan_settings.get_clan());
                                        }
                                    }
                                }

                                if token.is_cancelled() {
                                    warn!("Update task is shutting down...");
                                    break 'outer;
                                }
                            }

                            info!("OAuth tokens refreshed.");
                        } else {
                            info!("No OAuth credentials, skipping.");
                        }

                        if let Ok(bot_players) =
                            players_repository_worker.update_all_players_stats(&player_scores_repository_worker, false, Some(token.clone())).await
                        {
                            info!("Updating players roles ({})...", bot_players.len());

                            let mut current_players_roles = Vec::new();
                            for bot_player in bot_players {
                                debug!(
                                    "Fetching user {} ({}) roles...",
                                    &bot_player.user_id, &bot_player.name
                                );

                                let mut guilds_to_unlink = vec![];
                                for guild_id in &bot_player.linked_guilds {
                                    // TODO: do not get user roles if guild does not have automatic roles enabled
                                    let member = match global_ctx
                                        .http
                                        .get_member(u64::from(*guild_id), bot_player.user_id.into())
                                        .await {
                                        Ok(member) => member,
                                        Err(err) => {
                                            error!("Can not fetch user {} membership in {} guild due to an error: {:?}.", bot_player.user_id, &guild_id, err);

                                            match err {
                                                SerenityError::Http(http_err) => {
                                                    match *http_err {
                                                        http::HttpError::UnsuccessfulRequest(http::error::ErrorResponse {error : discord_err, ..}) => {
                                                            // see: https://discord.com/developers/docs/topics/opcodes-and-status-codes#json
                                                            if discord_err.code == 10007 {
                                                                debug!("User {} ({}) is not a member of the guild {} anymore.", &bot_player.user_id, &bot_player.name, &guild_id);
                                                                guilds_to_unlink.push(u64::from(*guild_id));
                                                            }

                                                            continue
                                                        }
                                                        _ => continue
                                                    }

                                                }
                                                _ => continue
                                            }
                                        }
                                    };

                                    current_players_roles.push((
                                        *guild_id,
                                        bot_player.clone(),
                                        member.roles,
                                    ));

                                    if token.is_cancelled() {
                                        warn!("Update task is shutting down...");
                                        break 'outer;
                                    }
                                }

                                if !guilds_to_unlink.is_empty() {
                                    info!("Unlinking user {} ({}) from guilds {:?}...", &bot_player.user_id, &bot_player.name, &guilds_to_unlink);

                                    let _ = players_repository_worker.unlink_guilds(&bot_player.user_id, guilds_to_unlink).await;
                                }
                            }

                            let mut guild_ids = current_players_roles.iter().map(|(guild_id, _player, _roles)| *guild_id).collect::<Vec<GuildId>>();
                            guild_ids.sort_unstable();
                            guild_ids.dedup();

                            let mut guilds : HashMap<GuildId, GuildSettings> = HashMap::new();

                            for guild_id in &guild_ids {
                                if let Ok(guild_settings) = guild_settings_repository_worker.get(guild_id).await {
                                    guilds.insert(*guild_id, guild_settings);
                                }
                            }

                            let role_changes = current_players_roles
                                .iter()
                                .filter_map(|(guild_id, player, roles)| {
                                    guilds.get(guild_id).map(|guild_settings| guild_settings.get_role_updates(*guild_id, player, roles))
                                })
                                .collect::<Vec<UserRoleChanges>>();

                            for rc in role_changes {
                                match rc.apply(&global_ctx.http).await {
                                    Ok(rc) => {
                                        if rc.is_changed() {
                                            if let Some(bot_channel_id) = guilds.get(&rc.guild_id).map_or_else(|| None, |guild_settings| guild_settings.get_channel()) {
                                                info!(
                                                    "Logging changes to channel #{}",
                                                    bot_channel_id
                                                );

                                                match players_repository_worker.get(&rc.user_id).await {
                                                    Some(player) => {
                                                        let embed_image = get_player_embed(&player).await;

                                                        match bot_channel_id
                                                            .send_message(global_ctx.clone(), |m| {
                                                                if let Some(embed_buffer) = embed_image {
                                                                    m.add_file(AttachmentType::Bytes {
                                                                        data: Cow::<[u8]>::from(embed_buffer),
                                                                        filename: "embed.png".to_string(),
                                                                    });
                                                                }

                                                                m.content(format!("{}", rc))
                                                                    .allowed_mentions(|am| am.empty_parse())
                                                            })
                                                            .await {
                                                            Ok(_) => {}
                                                            Err(err) => {
                                                                info!("Can not post log update to channel #{}: {}", bot_channel_id, err);
                                                            }
                                                        };
                                                    }
                                                    None => {
                                                        match bot_channel_id
                                                            .send_message(global_ctx.clone(), |m| {
                                                                m.content(format!("{}", rc))
                                                                    .allowed_mentions(|am| am.empty_parse())
                                                            })
                                                            .await {
                                                            Ok(_) => {}
                                                            Err(err) => {
                                                                info!("Can not post log update to channel #{}: {}", bot_channel_id, err);
                                                            }
                                                        };
                                                    }
                                                }
                                            }
                                        }
                                    }
                                    Err(e) => {
                                        error!(
                                            "Failed to update roles for user {}: {}",
                                            rc.user_id, e
                                        );
                                    }
                                }

                                if token.is_cancelled() {
                                    warn!("Update task is shutting down...");
                                    break 'outer;
                                }
                            }

                            info!("Players roles updated.");
                        }

                        for guild in guild_settings_repository_worker.all().await {
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

                                    if last_posted_at.is_none() || last_posted_at.unwrap().le(&(Utc::now() - chrono::Duration::minutes(settings.clan_wars_interval as i64))) {
                                        match guild_settings_repository_worker
                                            .set_clan_wars_posted_at(&guild.get_key(), Utc::now())
                                            .await
                                        {
                                            Ok(_) => {
                                                info!("{} clan wars maps posted time set.", clan_settings.get_clan());

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
                                                    .await {
                                                    Ok(maps_list) => {
                                                        let mut maps = vec![];
                                                        for map in maps_list.data.into_iter() {
                                                            if let Ok(scores) = BL_CLIENT
                                                                .clan()
                                                                .scores(
                                                                    &map.leaderboard.id,
                                                                    map.id,
                                                                    &[ClanRankingParam::Count(100), ClanRankingParam::Page(1)],
                                                                )
                                                                .await {
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
                                                                let pp = calculate_pp_boundary(CLAN_WEIGHT_COEFFICIENT, &mut pps, -map.pp);
                                                                let acc = match calculate_acc_from_pp(
                                                                    pp,
                                                                    StarRating {
                                                                        pass: map.leaderboard.difficulty.pass_rating,
                                                                        tech: map.leaderboard.difficulty.tech_rating,
                                                                        acc: map.leaderboard.difficulty.acc_rating,
                                                                    },
                                                                    map.leaderboard.difficulty.mode_name.as_str(),
                                                                ) {
                                                                    None => "Not possible".to_owned(),
                                                                    Some(acc) => format!("{:.2}%", acc * 100.0),
                                                                };
                                                                let acc_fs = match map.leaderboard.difficulty.modifiers_rating.as_ref() {
                                                                    None => "No ratings".to_owned(),
                                                                    Some(ratings) => match calculate_acc_from_pp(
                                                                        pp,
                                                                        StarRating {
                                                                            pass: ratings.fs_pass_rating,
                                                                            tech: ratings.fs_tech_rating,
                                                                            acc: ratings.fs_acc_rating,
                                                                        },
                                                                        map.leaderboard.difficulty.mode_name.as_str(),
                                                                    ) {
                                                                        None => "Not possible".to_owned(),
                                                                        Some(acc) => format!("{:.2}%", acc * 100.0),
                                                                    },
                                                                };
                                                                let acc_sfs = match map.leaderboard.difficulty.modifiers_rating.as_ref() {
                                                                    None => "No ratings".to_owned(),
                                                                    Some(ratings) => match calculate_acc_from_pp(
                                                                        pp,
                                                                        StarRating {
                                                                            pass: ratings.sf_pass_rating,
                                                                            tech: ratings.sf_tech_rating,
                                                                            acc: ratings.sf_acc_rating,
                                                                        },
                                                                        map.leaderboard.difficulty.mode_name.as_str(),
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

                                                        out.sort_unstable_by(|a, b| a.6.partial_cmp(&b.6).unwrap_or(Ordering::Equal));

                                                        info!(
                                                            "{} clan wars maps found. Posting maps to channel #{}",
                                                            out.len(), clan_wars_channel_id
                                                        );

                                                        async fn post_msg(global_ctx: &serenity::Context, channel_id: ChannelId, description: &str, content: &str) {
                                                            match channel_id
                                                                .send_message(global_ctx.clone(), |m| {
                                                                    m.embed(|e| {
                                                                        e.description(description)
                                                                    })
                                                                        .allowed_mentions(|am| am.empty_parse());

                                                                    if !content.is_empty() {
                                                                        m.content(content);
                                                                    }

                                                                    m
                                                                })
                                                                .await {
                                                                Ok(_) => {}
                                                                Err(err) => {
                                                                    info!("Can not post clan wars map to channel #{}: {}", channel_id, err);
                                                                }
                                                            };
                                                        }

                                                        const MAX_DISCORD_MSG_LENGTH: usize = 2000;
                                                        let mut msg_count = 0;
                                                        let header = format!("### **{} clan wars maps**", clan_settings.get_clan());
                                                        let mut description = String::new();
                                                        for item in out {
                                                            let map_description = format!("### **#{} [{} / {}](https://www.beatleader.xyz/leaderboard/clanranking/{}/{})**\n{} score{} / {:.2}pp / **{:.2} raw pp**\n {} / {} FS / {} SF\n",
                                                                                          item.1, item.2, item.3, item.0, ((item.1 - 1) / 10 + 1),
                                                                                          item.5, if item.5 > 1 { "s" } else { "" }, item.4, item.6, item.7, item.8, item.9);

                                                            if description.len() + "\n\n".len() + map_description.len() + (if msg_count > 0 { 0 } else { header.len() }) < MAX_DISCORD_MSG_LENGTH {
                                                                description.push_str(&map_description);
                                                            } else {
                                                                post_msg(
                                                                    &global_ctx,
                                                                    clan_wars_channel_id,
                                                                    description.as_str(),
                                                                    if msg_count == 0 { header.as_str() } else { "" },
                                                                ).await;

                                                                description = String::new();
                                                                msg_count += 1;
                                                            }
                                                        }

                                                        if !description.is_empty() {
                                                            post_msg(
                                                                &global_ctx,
                                                                clan_wars_channel_id,
                                                                description.as_str(),
                                                                if msg_count == 0 { header.as_str() } else { "" },
                                                            ).await;
                                                        }


                                                    }
                                                    Err(err) => {
                                                        error!("Can not fetch clan wars map list: {:?}", err);
                                                    }
                                                }

                                                info!("Clan wars maps for a clan {} refreshed and posted.",clan_settings.get_clan());
                                            }
                                            Err(err) => {
                                                error!("Can not set clan wars posted time for clan {}: {:?}", clan_settings.get_clan(), err);
                                            }
                                        }
                                    } else {
                                        info!("Clan {} wars maps do not require posting yet.", clan_settings.get_clan());
                                    }
                                }
                            }
                        }

                        tokio::select! {
                            _ = token.cancelled() => {
                                warn!("Update task is shutting down...");
                                break 'outer;
                            }
                            _ = tokio::time::sleep(interval) => {}
                        }
                    }

                    info!("Update task shut down.");
                });

                Ok(data)
            })
        })
        .build()
        .await
}
