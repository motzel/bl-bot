use std::borrow::Cow;
use std::convert::From;
use std::sync::Arc;

use log::{debug, error, info, trace, warn};
use poise::serenity_prelude::{
    AttachmentType, CreateComponents, GuildId, MessageComponentInteraction, User, UserId,
};
use poise::{serenity_prelude as serenity, CreateReply, ReplyHandle};

use bytes::Bytes;

use crate::beatleader::player::{PlayerScoreParam, PlayerScoreSort};
use crate::beatleader::rating::Ratings;
use crate::beatleader::{BlContext, List as BlList, SortOrder};
use crate::bot::beatleader::{
    fetch_rating, fetch_scores, MapRating, MapRatingModifier, Player as BotPlayer, Player, Score,
};
use crate::bot::commands::guild::{get_guild_id, get_guild_settings};
use crate::bot::get_binary_file;
use crate::embed::{embed_profile, embed_score};
use crate::storage::player::PlayerRepository;
use crate::storage::PersistError;
use crate::{Context, Error};

#[derive(Debug, poise::ChoiceParameter, Default)]

pub(crate) enum Sort {
    #[name = "By Date"]
    #[default]
    Latest,
    #[name = "By PP"]
    ByPp,
    #[name = "By Acc"]
    ByAcc,
    #[name = "By Stars"]
    ByStars,
    #[name = "By Rank"]
    ByRank,
    #[name = "By Max Streak"]
    ByMaxStreak,
}

impl Sort {
    pub fn to_player_score_sort(&self) -> PlayerScoreSort {
        match self {
            Sort::Latest => PlayerScoreSort::Date,
            Sort::ByPp => PlayerScoreSort::Pp,
            Sort::ByAcc => PlayerScoreSort::Acc,
            Sort::ByStars => PlayerScoreSort::Stars,
            Sort::ByRank => PlayerScoreSort::Rank,
            Sort::ByMaxStreak => PlayerScoreSort::MaxStreak,
        }
    }
}

#[derive(Debug, poise::ChoiceParameter, Default)]
pub(crate) enum BlCommandContext {
    #[name = "General"]
    #[default]
    General,
    #[name = "No modifiers"]
    NoModifiers,
    #[name = "No pauses"]
    NoPauses,
    #[name = "Golf"]
    Golf,
}

impl BlCommandContext {
    pub fn to_bl_context(&self) -> BlContext {
        match self {
            BlCommandContext::General => BlContext::General,
            BlCommandContext::NoModifiers => BlContext::NoModifiers,
            BlCommandContext::NoPauses => BlContext::NoPauses,
            BlCommandContext::Golf => BlContext::Golf,
        }
    }
}

/// Link your account to your Beat Leader profile.
#[poise::command(slash_command, rename = "bl-link", guild_only)]
pub(crate) async fn cmd_link(
    ctx: Context<'_>,
    #[description = "Beat Leader PlayerID or profile URL"] bl_player_id: String,
    #[description = "Discord user (admin only, YOU if not specified)"] user: Option<User>,
) -> Result<(), Error> {
    let guild_id = get_guild_id(ctx, true).await?;

    let (selected_user_id, requires_verification) = match user {
        Some(user) => {
            if let Some(member) = ctx.author_member().await {
                match member.permissions {
                    None => {
                        say_without_ping(ctx, "Error: can not get user permissions", true).await?;

                        return Ok(());
                    }
                    Some(permissions) => {
                        if !permissions.administrator() {
                            say_without_ping(
                                ctx,
                                "Error: linking another user requires administrator privilege",
                                true,
                            )
                            .await?;

                            return Ok(());
                        }

                        (user.id, false)
                    }
                }
            } else {
                say_without_ping(ctx, "Error: can not get user permissions", true).await?;

                return Ok(());
            }
        }
        None => {
            let guild_settings = get_guild_settings(ctx, true).await?;

            (ctx.author().id, guild_settings.requires_verified_profile)
        }
    };

    let mut player_id = bl_player_id;
    let re = regex::Regex::new(r"beatleader.xyz/u/(?<player_id>[^\/\?$]+)").unwrap();
    if let Some(caps) = re.captures(&player_id) {
        player_id = caps["player_id"].to_owned().clone();
    }

    ctx.defer().await?;

    match ctx
        .data()
        .players_repository
        .link(
            guild_id,
            selected_user_id,
            player_id.to_owned(),
            requires_verification,
        )
        .await
    {
        Ok(player) => {
            let embed_image = get_player_embed(&player).await;

            ctx.send(|m| {
                if embed_image.is_none() {
                    add_profile_card(m, player);
                } else if let Some(embed_buffer) = embed_image {
                    m.attachment(AttachmentType::Bytes {
                        data: Cow::<[u8]>::from(embed_buffer),
                        filename: "embed.png".to_string(),
                    });
                }

                m.content(format!(
                    "<@{}> has been linked to the BL profile",
                    selected_user_id
                ))
                // https://docs.rs/serenity/latest/serenity/builder/struct.CreateAllowedMentions.html
                .allowed_mentions(|am| am.parse(serenity::builder::ParseValue::Users))
                .ephemeral(false)
            })
            .await?;

            Ok(())
        }
        Err(e) => {
            say_without_ping(ctx, format!("An error occurred: {}", e).as_str(), true).await?;

            Ok(())
        }
    }
}

/// Unlink your account from your Beat Leader profile.
#[poise::command(slash_command, rename = "bl-unlink", guild_only)]
pub(crate) async fn cmd_unlink(
    ctx: Context<'_>,
    #[description = "Discord user (admin only, YOU if not specified)"] user: Option<User>,
) -> Result<(), Error> {
    let guild_id = get_guild_id(ctx, true).await?;

    let selected_user_id = match user {
        Some(user) => {
            if let Some(member) = ctx.author_member().await {
                match member.permissions {
                    None => {
                        say_without_ping(ctx, "Error: can not get user permissions", true).await?;

                        return Ok(());
                    }
                    Some(permissions) => {
                        if !permissions.administrator() {
                            say_without_ping(
                                ctx,
                                "Error: unlinking another user requires administrator privilege",
                                true,
                            )
                            .await?;

                            return Ok(());
                        }

                        user.id
                    }
                }
            } else {
                say_without_ping(ctx, "Error: can not get user permissions", true).await?;

                return Ok(());
            }
        }
        None => ctx.author().id,
    };

    match ctx
        .data()
        .players_repository
        .unlink(&guild_id, &selected_user_id)
        .await
    {
        Ok(_) => {
            ctx.send(|m| {
                m.content(format!(
                    "<@{}> has been unlinked from BL profile",
                    selected_user_id
                ))
                // https://docs.rs/serenity/latest/serenity/builder/struct.CreateAllowedMentions.html
                .allowed_mentions(|am| {
                    am.parse(serenity::builder::ParseValue::Users)
                        .parse(serenity::builder::ParseValue::Roles)
                })
                .ephemeral(false)
            })
            .await?;

            Ok(())
        }
        Err(e) => match e {
            PersistError::NotFound(_) => {
                say_profile_not_linked(ctx, &selected_user_id).await?;

                Ok(())
            }
            _ => {
                ctx.send(|f| {
                    f.content(format!("An error has occurred: {}", e))
                        .ephemeral(true)
                })
                .await?;

                Ok(())
            }
        },
    }
}

/// Displays player's BL profile
#[poise::command(slash_command, rename = "bl-profile", guild_only)]
pub(crate) async fn cmd_profile(
    ctx: Context<'_>,
    #[description = "Discord user (YOU if not specified)"] user: Option<serenity::User>,
) -> Result<(), Error> {
    ctx.defer().await?;

    let guild_settings = get_guild_settings(ctx, true).await?;

    let selected_user = user.as_ref().unwrap_or_else(|| ctx.author());

    match link_user_if_needed(
        ctx,
        &guild_settings.guild_id,
        selected_user,
        guild_settings.requires_verified_profile,
    )
    .await
    {
        Some(player) => {
            if !player.is_linked_to_guild(&guild_settings.guild_id) {
                say_profile_not_linked(ctx, &selected_user.id).await?;

                return Ok(());
            }

            let embed_image = get_player_embed(&player).await;

            ctx.send(|m| {
                if embed_image.is_none() {
                    add_profile_card(m, player);
                } else if let Some(embed_buffer) = embed_image {
                    m.attachment(AttachmentType::Bytes {
                        data: Cow::<[u8]>::from(embed_buffer),
                        filename: "embed.png".to_string(),
                    });
                }

                m.allowed_mentions(|am| am.empty_parse()).ephemeral(false)
            })
            .await?;

            Ok(())
        }
        None => {
            say_profile_not_linked(ctx, &selected_user.id).await?;

            Ok(())
        }
    }
}

pub(crate) async fn link_user_if_needed(
    ctx: Context<'_>,
    guild_id: &GuildId,
    selected_user: &User,
    requires_verified_profile: bool,
) -> Option<Player> {
    trace!(
        "Checking if user {} should be linked to the guild {}...",
        selected_user.id,
        guild_id
    );

    match ctx.data().players_repository.get(&selected_user.id).await {
        Some(mut player) => {
            trace!(
                "User {} exists, checking if they should be linked to the guild {}...",
                selected_user.id,
                guild_id
            );
            if !player.is_linked_to_guild(guild_id)
                && ctx
                    .data()
                    .players_repository
                    .link_guild(&selected_user.id, *guild_id)
                    .await
                    .is_ok()
            {
                trace!(
                    "User {} linked to the guild {}.",
                    selected_user.id,
                    guild_id
                );

                player.linked_guilds.push(*guild_id);
            }

            Some(player)
        }
        None => {
            trace!(
                "User {} is not linked yet, trying to fetch player from BL using Discord id...",
                selected_user.id
            );

            if let Ok(bl_player) =
                PlayerRepository::fetch_player_from_bl_by_user_id(&selected_user.id).await
            {
                trace!(
                    "User {} is linked on the BL website, player name: {}. Trying to link...",
                    selected_user.id,
                    &bl_player.name
                );

                return match ctx
                    .data()
                    .players_repository
                    .link_player(
                        *guild_id,
                        selected_user.id,
                        bl_player,
                        requires_verified_profile,
                    )
                    .await
                {
                    Ok(player) => Some(player),
                    Err(_) => None,
                };
            };

            None
        }
    }
}

/// Post link to a replay, yours or another server user who has linked they BL account.
///
/// Enter any user of this server as a parameter. If you omit it then your replay will be searched for.
#[poise::command(slash_command, rename = "bl-replay", guild_only)]
pub(crate) async fn cmd_replay(
    ctx: Context<'_>,
    #[description = "Sort by (latest if not specified)"] sort: Option<Sort>,
    #[description = "BL context (General if not specified)"] context: Option<BlCommandContext>,
    #[description = "Discord user (YOU if not specified)"] user: Option<serenity::User>,
) -> Result<(), Error> {
    ctx.defer_ephemeral().await?;

    let guild_settings = get_guild_settings(ctx, true).await?;

    let current_user = ctx.author();
    let selected_user = user.as_ref().unwrap_or(current_user);

    match link_user_if_needed(
        ctx,
        &guild_settings.guild_id,
        selected_user,
        guild_settings.requires_verified_profile,
    )
    .await
    {
        Some(player) => {
            let player_score_sort = (sort.unwrap_or_default()).to_player_score_sort();
            let player_score_context = (context.unwrap_or_default()).to_bl_context();

            if !player.is_linked_to_guild(&guild_settings.guild_id) {
                say_profile_not_linked(ctx, &selected_user.id).await?;

                return Ok(());
            }

            let player_scores = match fetch_scores(
                &player.id,
                &[
                    PlayerScoreParam::Page(1),
                    PlayerScoreParam::Count(25),
                    PlayerScoreParam::Sort(player_score_sort),
                    PlayerScoreParam::Order(SortOrder::Descending),
                    PlayerScoreParam::Context(player_score_context.clone()),
                ],
            )
            .await
            {
                Ok(player_scores) => {
                    if player_scores.total == 0 {
                        say_without_ping(ctx, "No scores.", true).await?;
                        return Ok(());
                    }

                    player_scores
                }
                Err(e) => {
                    ctx.say(format!("Error fetching scores: {}", e)).await?;
                    return Ok(());
                }
            };

            let msg = ctx
                .send(|m| {
                    let selected_ids = Vec::new();
                    m.components(|c| add_replay_components(c, &player_scores, &selected_ids))
                        .ephemeral(true)
                })
                .await?;

            let mut score_ids = Vec::<String>::new();
            let mut replay_posted = false;

            while let Some(mci) = serenity::CollectComponentInteraction::new(ctx)
                .author_id(current_user.id)
                .channel_id(ctx.channel_id())
                .timeout(std::time::Duration::from_secs(120))
                .await
            {
                trace!("Interaction response: {:?}", mci.data);

                match mci.data.custom_id.as_str() {
                    "score_id" => {
                        score_ids = mci.data.values.clone();

                        // edit message
                        mci.create_interaction_response(ctx, |ir| {
                            ir.kind(serenity::InteractionResponseType::UpdateMessage)
                                .interaction_response_data(|message| {
                                    message.components(|c| {
                                        add_replay_components(c, &player_scores, &score_ids)
                                    })
                                })
                        })
                        .await?;
                    }
                    "post_btn" => {
                        if !score_ids.is_empty() {
                            post_replays(
                                ctx,
                                &score_ids,
                                &player_scores,
                                &player,
                                &player_score_context,
                                &msg,
                            )
                            .await?;
                        } else {
                            mci.defer(ctx).await?;
                        }

                        replay_posted = true;
                    }
                    _ => {
                        mci.defer(ctx).await?;
                    }
                }
            }

            if !replay_posted {
                msg.edit(ctx, |m| {
                    m.components(|c| c)
                        .content("Interaction timed out. Dismiss this message and try again.")
                })
                .await?;
            }

            Ok(())
        }
        None => {
            say_profile_not_linked(ctx, &selected_user.id).await?;

            Ok(())
        }
    }
}

/// Force refreshing all players scores
#[poise::command(
    slash_command,
    rename = "bl-refresh-scores",
    ephemeral,
    required_permissions = "MANAGE_ROLES",
    default_member_permissions = "MANAGE_ROLES",
    required_bot_permissions = "MANAGE_ROLES",
    guild_only
)]
pub(crate) async fn cmd_refresh_scores(ctx: Context<'_>) -> Result<(), Error> {
    say_without_ping(ctx, "Please wait...", true).await?;

    let players_repository = &ctx.data().players_repository;

    players_repository
        .update_all_players_stats(true, None)
        .await?;

    say_without_ping(ctx, "All players scores refreshed.", true).await?;

    Ok(())
}

fn add_replay_components<'a>(
    c: &'a mut CreateComponents,
    player_scores: &BlList<Score>,
    selected_ids: &Vec<String>,
) -> &'a mut CreateComponents {
    c.create_action_row(|r| {
        r.create_select_menu(|m| {
            m.custom_id("score_id")
                .placeholder("Select replay(s) to post")
                .options(|o| {
                    player_scores.data.iter().fold(o, |acc, s| {
                        acc.create_option(|o| {
                            let label = format!(
                                "{} {} ({})",
                                s.song_name.clone(),
                                s.song_sub_name.clone(),
                                s.difficulty_name.clone(),
                            );
                            o.label(if label.len() > 100 {
                                &label[..100]
                            } else {
                                label.as_str()
                            })
                            .value(s.id.to_string())
                            .description(format!("{:.2}% {:.2}pp", s.accuracy, s.pp))
                            .default_selection(selected_ids.contains(&s.id.to_string()))
                        })
                    })
                })
                .min_values(1)
                .max_values(3)
        })
    })
    .create_action_row(|r| {
        r.create_button(|b| {
            b.custom_id("post_btn")
                .label("Post replay")
                .style(serenity::ButtonStyle::Primary)
                .disabled(selected_ids.is_empty())
        })
    })
}

async fn post_replays(
    ctx: Context<'_>,
    score_ids: &Vec<String>,
    player_scores: &BlList<Score>,
    player: &BotPlayer,
    bl_context: &BlContext,
    msg: &ReplyHandle<'_>,
) -> Result<(), Error> {
    let mut msg_contents = "Loading player avatar...".to_owned();

    let msg_contents_clone = msg_contents.clone();
    msg.edit(ctx, |m| m.components(|c| c).content(msg_contents_clone))
        .await?;

    let player_avatar = get_binary_file(&player.avatar)
        .await
        .unwrap_or(Bytes::new());

    if player_avatar.is_empty() {
        msg_contents.push_str("FAILED\n");
    } else {
        msg_contents.push_str("OK\n");
    }

    let msg_contents_clone = msg_contents.clone();

    msg.edit(ctx, |m| m.components(|c| c).content(msg_contents_clone))
        .await?;

    for score_id in score_ids {
        let Some(score) = player_scores
            .data
            .iter()
            .find(|s| &s.id.to_string() == score_id)
        else {
            continue;
        };

        let mut score = score.clone();

        if !score.difficulty_rating.has_individual_rating() {
            info!("Fetching ratings for {}", &score.song_name);

            msg_contents.push_str(&format!("Fetching ratings for {}...", score.song_name));

            let msg_contents_clone = msg_contents.clone();
            msg.edit(ctx, |m| m.components(|c| c).content(msg_contents_clone))
                .await?;

            let ratings = fetch_rating(
                &score.song_hash,
                &score.difficulty_mode_name,
                score.difficulty_value,
            )
            .await;

            match ratings {
                Ok(ratings) => {
                    score.difficulty_original_rating =
                        MapRating::from_ratings_and_modifier(&ratings, MapRatingModifier::None);

                    score.difficulty_rating = MapRating::from_ratings_and_modifier(
                        &ratings,
                        score.modifiers.as_str().into(),
                    );

                    msg_contents.push_str("OK\n");
                }
                Err(err) => {
                    error!(
                        "Fetching rating for song {} ({}/{}/{}) failed: {}",
                        &score.song_name,
                        &score.song_hash,
                        &score.difficulty_mode_name,
                        &score.difficulty_value,
                        err
                    );
                    msg_contents.push_str("FAILED\n");
                }
            }
        }

        info!("Posting replay for scoreId: {}", score_id);

        msg_contents.push_str(&format!("Generating embed for {}...", score.song_name));

        let msg_contents_clone = msg_contents.clone();
        msg.edit(ctx, |m| m.components(|c| c).content(msg_contents_clone))
            .await?;

        let embed_image = if !player_avatar.is_empty() {
            embed_score(&score, player, player_avatar.as_ref()).await
        } else {
            None
        };

        if embed_image.is_some() {
            msg_contents.push_str("OK\n");
        } else {
            msg_contents.push_str("FAILED\n");
        }

        let send_message_result = ctx
            .channel_id()
            .send_message(ctx, |m| {
                score.add_embed_to_message(m, player, bl_context, embed_image.as_ref());

                m.allowed_mentions(|am| {
                    am.parse(serenity::builder::ParseValue::Users)
                        .parse(serenity::builder::ParseValue::Roles)
                })
            })
            .await;

        if send_message_result.is_err() {
            warn!(
                "An error occurred while trying to post a replay as a new message: {:?}",
                send_message_result.err()
            );

            let reply_result = ctx
                .send(|m| {
                    score.add_embed_to_reply(m, player, bl_context, embed_image.as_ref());

                    m.allowed_mentions(|am| {
                        am.parse(serenity::builder::ParseValue::Users)
                            .parse(serenity::builder::ParseValue::Roles)
                    })
                    .reply(false)
                    .ephemeral(false)
                })
                .await;

            if reply_result.is_err() {
                error!(
                    "An error occurred while trying to post a replay as a reply to the command: {:?}",
                    reply_result.err()
                );

                msg_contents
                    .push_str("An error has occurred. No permissions to post to the channel?");

                msg.edit(ctx, |m| m.components(|c| c).content(msg_contents))
                    .await?;

                return Ok(());
            }
        }
    }

    msg_contents.push_str("Replay(s) posted. You can dismiss this message.");

    msg.edit(ctx, |m| m.components(|c| c).content(msg_contents))
        .await?;

    Ok(())
}

fn add_profile_card(reply: &mut CreateReply, player: BotPlayer) {
    reply.embed(|f| {
        let mut clans = player.clans.join(", ");
        if clans.is_empty() {
            clans = "None".to_string()
        }

        f.title(player.name)
            .url(format!("https://www.beatleader.xyz/u/{}", player.id))
            .thumbnail(player.avatar)
            .field("Rank", player.rank, true)
            .field("PP", format!("{:.2}", player.pp), true)
            .field("Country", player.country, true)
            .field("Top PP", format!("{:.2}", player.top_pp), true)
            .field("Top Acc", format!("{:.2}%", player.top_accuracy), true)
            .field(
                "Top Stars",
                if player.last_scores_fetch.is_some() {
                    format!("{:.2}⭐", player.top_stars)
                } else {
                    "-".to_owned()
                },
                true,
            )
            .field(
                "+1pp",
                if player.last_scores_fetch.is_some() {
                    format!("{:.2}pp", player.plus_1pp)
                } else {
                    "-".to_owned()
                },
                true,
            )
            .field(
                "Last pause",
                if player.last_scores_fetch.is_some() {
                    if player.last_ranked_paused_at.is_some() {
                        format!(
                            "<t:{}:R>",
                            player.last_ranked_paused_at.unwrap().timestamp()
                        )
                    } else {
                        "Never".to_owned()
                    }
                } else {
                    "-".to_owned()
                },
                true,
            )
            .field("Clans", clans, true);

        let footer_text = if !player.is_verified {
            "Profile is NOT VERIFIED\n\n"
        } else {
            ""
        };

        if let Some(last_fetch) = player.last_fetch {
            f.footer(|footer| footer.text(format!("{}Last updated", footer_text)))
                .timestamp(last_fetch);
        } else {
            f.footer(|footer| footer.text(footer_text));
        }

        f
    });
}

pub(crate) async fn say_profile_not_linked(
    ctx: Context<'_>,
    user_id: &UserId,
) -> Result<(), Error> {
    say_without_ping(
        ctx,
        format!(
            "<@{}> is not linked by a bot nor is the Discord account linked on the BL site. Use the ``/bl-link`` command first or link the account on the BL site.",
            user_id
        )
        .as_str(),
        false,
    )
    .await?;

    Ok(())
}

pub(crate) async fn say_without_ping(
    ctx: Context<'_>,
    message: &str,
    ephemeral: bool,
) -> Result<(), Error> {
    ctx.send(|f| {
        f.content(message)
            .allowed_mentions(|am| am.empty_parse())
            .ephemeral(ephemeral)
    })
    .await?;

    Ok(())
}

pub(crate) async fn get_player_embed(player: &BotPlayer) -> Option<Vec<u8>> {
    let player_avatar = get_binary_file(&player.avatar)
        .await
        .unwrap_or(Bytes::new());

    let player_cover = if player.profile_cover.is_some() {
        get_binary_file(player.profile_cover.as_ref().unwrap())
            .await
            .unwrap_or(Bytes::new())
    } else {
        Bytes::new()
    };

    if !player_avatar.is_empty() {
        embed_profile(
            player,
            player_avatar.as_ref(),
            if player_cover.is_empty() {
                player_avatar.as_ref()
            } else {
                player_cover.as_ref()
            },
        )
        .await
    } else {
        None
    }
}
