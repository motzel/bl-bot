#![allow(clippy::too_many_arguments)]
use std::borrow::Cow;
use std::cmp::Ordering;
use std::sync::Arc;

use magic_crypt::{new_magic_crypt, MagicCryptTrait};
use poise::serenity_prelude::{
    ChannelId, CreateAttachment, Message, Permissions, Role, User, UserId,
};
use poise::CreateReply;

use crate::beatleader::clan::ClanMapParam;
use crate::beatleader::clan::ClanRankingParam;
use crate::beatleader::clan::{Clan, ClanTag};
use crate::beatleader::oauth::{OAuthScope, OAuthTokenRepository};
use crate::beatleader::player::DifficultyStatus;
use crate::beatleader::pp::calculate_total_pp_from_sorted;
use crate::beatleader::pp::CLAN_WEIGHT_COEFFICIENT;
use crate::beatleader::DataWithMeta;
use crate::discord::bot::beatleader::clan::{
    fetch_clan, AccBoundary, ClanMapWithScores, ClanWarsFc, ClanWarsPlayDate, ClanWarsSort,
    Playlist,
};
use crate::discord::bot::beatleader::player::fetch_player_from_bl;
use crate::discord::bot::commands::guild::{get_guild_id, get_guild_settings};
use crate::discord::bot::commands::player::{
    link_user_if_needed, say_profile_not_linked, say_without_ping,
};
use crate::discord::bot::commands::{
    get_leaderboard_ids_from_message, get_user_id_with_required_permission,
};
use crate::discord::bot::{ClanSettings, GuildOAuthTokenRepository};
use crate::discord::Context;
use crate::storage::bsmaps::{BsMap, BsMapType, BsMapsRepository};
use crate::{Error, BL_CLIENT};

/// Set up sending of clan invitations
#[tracing::instrument(skip(ctx), level=tracing::Level::INFO, name="bot_command:bl-set-clan-invitation")]
#[poise::command(
    slash_command,
    rename = "bl-set-clan-invitation",
    ephemeral,
    required_permissions = "MANAGE_ROLES",
    default_member_permissions = "MANAGE_ROLES",
    required_bot_permissions = "MANAGE_ROLES",
    guild_only,
    hide_in_help
)]
pub(crate) async fn cmd_set_clan_invitation(
    ctx: Context<'_>,
    // #[description = "Allow users to self-invite. Default: true"] self_invite: Option<bool>,
) -> Result<(), Error> {
    let Some(oauth_credentials) = ctx.data().oauth_credentials() else {
        say_without_ping(ctx, "The bot is not properly configured to send invitations to the clan. Contact the bot owner to have it configured.", true).await?;
        return Ok(());
    };

    let guild_settings = get_guild_settings(ctx, true).await?;

    // let self_invite = self_invite.unwrap_or(true);
    let self_invite = true;

    let current_user = ctx.author();

    let player = ctx.data().players_repository.get(&current_user.id).await;

    if player.is_none() {
        say_profile_not_linked(
            ctx,
            &current_user.id,
            guild_settings.requires_verified_profile,
        )
        .await?;
        return Ok(());
    }

    let player = player.unwrap();
    if !player.is_verified {
        say_without_ping(ctx, "The profile must be verified. Go to <https://www.beatleader.xyz/settings#account> and link your discord account with your BL profile.", true).await?;
        return Ok(());
    }

    if player.clans.is_empty() {
        say_without_ping(ctx, "You are not a member of any clan.", true).await?;
        return Ok(());
    }

    let mut msg_contents = "Checking your clans...\n".to_owned();
    let msg = ctx.say(&msg_contents).await?;

    let mut player_clan: Option<Clan> = None;
    for tag in player.clans {
        msg_contents.push_str(format!("Fetching {} clan data...", tag).as_str());

        let msg_contents_clone = msg_contents.clone();
        msg.edit(ctx, CreateReply::default().content(&msg_contents_clone))
            .await?;

        let clan_result = fetch_clan(&tag).await;
        if clan_result.is_err() {
            msg_contents.push_str("FAILED\n");

            let msg_contents_clone = msg_contents.clone();
            msg.edit(ctx, CreateReply::default().content(&msg_contents_clone))
                .await?;
            return Ok(());
        }

        let clan = clan_result.unwrap();
        if player.id == clan.leader_id {
            msg_contents.push_str("OK, you are the owner!\n");

            let msg_contents_clone = msg_contents.clone();
            msg.edit(ctx, CreateReply::default().content(&msg_contents_clone))
                .await?;

            player_clan = Some(clan);
            break;
        } else {
            msg_contents.push_str("OK, not the owner\n");

            let msg_contents_clone = msg_contents.clone();
            msg.edit(ctx, CreateReply::default().content(&msg_contents_clone))
                .await?;
        }
    }

    if player_clan.is_none() {
        msg_contents.push_str("You are not the owner of any clan!\n");

        let msg_contents_clone = msg_contents.clone();
        msg.edit(ctx, CreateReply::default().content(&msg_contents_clone))
            .await?;

        return Ok(());
    }

    let player_clan = player_clan.unwrap();

    let mut clan_settings = guild_settings
        .clan_settings
        .clone()
        .unwrap_or(ClanSettings::new(
            current_user.id,
            player_clan.leader_id.clone(),
            player_clan.id,
            player_clan.tag.clone(),
            self_invite,
        ));
    clan_settings.set_oauth_token(false);

    if ctx
        .data()
        .guild_settings_repository
        .set_clan_settings(&guild_settings.guild_id, Some(clan_settings))
        .await
        .is_err()
    {
        msg_contents.push_str("An error occurred while saving clan settings\n");

        let msg_contents_clone = msg_contents.clone();
        msg.edit(ctx, CreateReply::default().content(&msg_contents_clone))
            .await?;

        return Ok(());
    }

    let guild_oauth_token_repository = GuildOAuthTokenRepository::new(
        player_clan.leader_id,
        Arc::clone(&ctx.data().player_oauth_token_repository),
    );

    let mc = new_magic_crypt!(oauth_credentials.client_secret.clone(), 256);

    let oauth_client = BL_CLIENT.with_oauth(oauth_credentials, guild_oauth_token_repository);

    msg_contents.push_str(format!("\nGreat, you are the owner of the {} clan. Now click this link and authorize the bot to send invitations to the clan on your behalf. {}", &player_clan.tag, oauth_client.oauth().authorize_url(vec![OAuthScope::Profile, OAuthScope::OfflineAccess, OAuthScope::Clan], mc.encrypt_str_to_base64(guild_settings.guild_id.to_string())).unwrap_or("Error when generating authorization link".to_owned())).as_str());

    let msg_contents_clone = msg_contents.clone();
    msg.edit(ctx, CreateReply::default().content(&msg_contents_clone))
        .await?;

    Ok(())
}

/// Send yourself an invitation to join the clan
#[tracing::instrument(skip(ctx), level=tracing::Level::INFO, name="bot_command:bl-clan-invitation")]
#[poise::command(slash_command, rename = "bl-clan-invitation", guild_only)]
pub(crate) async fn cmd_clan_invitation(ctx: Context<'_>) -> Result<(), Error> {
    ctx.defer().await?;

    if ctx.data().oauth_credentials().is_none() {
        say_without_ping(ctx, "The bot is not properly configured to send invitations to the clan. Contact the bot owner to have it configured.", true).await?;
        return Ok(());
    }

    let guild_settings = get_guild_settings(ctx, true).await?;

    if guild_settings.clan_settings.is_none() {
        say_without_ping(
            ctx,
            "Invitations to the clan were not set up. Ask the clan owner.",
            true,
        )
        .await?;

        return Ok(());
    }

    let clan_settings = guild_settings.clan_settings.clone().unwrap();

    if !clan_settings.self_invite {
        say_without_ping(
            ctx,
            "Self-sending yourself invitations to the clan is disabled. Ask the clan owner for an invitation.",
            true,
        )
        .await?;

        return Ok(());
    }

    if !clan_settings.oauth_token_is_set {
        say_without_ping(
            ctx,
            "The configuration of clan invitations was not completed by the clan owner. Ask them to complete it.",
            true,
        )
            .await?;

        return Ok(());
    }

    let current_user = ctx.author();

    match link_user_if_needed(
        ctx,
        &guild_settings.guild_id,
        current_user,
        guild_settings.requires_verified_profile,
    )
    .await
    {
        Some(player) => {
            if !player.is_linked_to_guild(&guild_settings.guild_id) {
                say_profile_not_linked(
                    ctx,
                    &current_user.id,
                    guild_settings.requires_verified_profile,
                )
                .await?;

                return Ok(());
            }

            let bl_player = fetch_player_from_bl(&player.id).await;
            if bl_player.is_err() {
                say_without_ping(
                    ctx,
                    format!(
                        "Error: can not fetch player data from BL: {}",
                        bl_player.err().unwrap()
                    )
                    .as_str(),
                    true,
                )
                .await?;

                return Ok(());
            }

            let bl_player = bl_player.unwrap();

            let clan_tag = clan_settings.get_clan();

            if bl_player.clans.iter().any(|clan| clan.tag == clan_tag) {
                say_without_ping(ctx, "You are already a clan member.", true).await?;

                return Ok(());
            }

            if bl_player.clans.len() >= 5 {
                say_without_ping(ctx, "You are already a member of 5 clans. You must leave some clan if you want to join another.", true).await?;

                return Ok(());
            }

            if !bl_player.socials.iter().any(|social| {
                social.service == "Discord" && social.user_id == current_user.id.to_string()
            }) {
                say_without_ping(
                    ctx,
                    "The profile must be verified. Go to <https://www.beatleader.xyz/settings#account> and link your discord account with your BL profile.",
                    true,
                )
                    .await?;

                return Ok(());
            }

            let guild_oauth_token_repository = GuildOAuthTokenRepository::new(
                clan_settings.owner_id.clone(),
                Arc::clone(&ctx.data().player_oauth_token_repository),
            );
            let oauth_client = BL_CLIENT.with_oauth(
                ctx.data().oauth_credentials().unwrap().clone(),
                guild_oauth_token_repository,
            );

            let invitation_result = oauth_client.clan_auth().invite(bl_player.id).await;
            if invitation_result.is_err() {
                say_without_ping(
                    ctx,
                    format!(
                        "Error: sending clan invitation failed: {}",
                        invitation_result.err().unwrap()
                    )
                    .as_str(),
                    true,
                )
                .await?;

                return Ok(());
            }

            say_without_ping(
                ctx,
                "Invitation has been sent! Go to <https://www.beatleader.xyz/clans> and accept it.",
                false,
            )
            .await?;

            Ok(())
        }
        None => {
            say_profile_not_linked(
                ctx,
                &current_user.id,
                guild_settings.requires_verified_profile,
            )
            .await?;

            Ok(())
        }
    }
}

/// Generate clan wars playlist (clan members only)
#[tracing::instrument(skip(ctx), level=tracing::Level::INFO, name="bot_command:bl-clan-wars-playlist")]
#[poise::command(slash_command, rename = "bl-clan-wars-playlist", guild_only)]
pub(crate) async fn cmd_clan_wars_playlist(
    ctx: Context<'_>,
    #[description = "Playlist type (default: To Conquer)"] playlist_type: Option<ClanWarsSort>,
    #[description = "Last played (default: Never)"] played: Option<ClanWarsPlayDate>,
    #[description = "Maps count (max: 300, default: 100)"] count: Option<u32>,
    #[description = "Maps map stars (default: player's top stars)"] max_stars: Option<f64>,
    #[description = "Maps clan pp difference (default: player's top pp)"] max_clan_pp_diff: Option<
        f64,
    >,
    #[description = "FC status"] fc: Option<ClanWarsFc>,
    #[description = "Skip the commander's orders (default: false)"] skip_commander_order: Option<
        bool,
    >,
    #[description = "Your custom playlist name (default: autogenerated"] playlist_name: Option<
        String,
    >,
) -> Result<(), Error> {
    ctx.defer().await?;

    let playlist_type_filter = playlist_type.unwrap_or(ClanWarsSort::ToConquer);
    let played_filter = played.unwrap_or(ClanWarsPlayDate::Never);
    let count = match count {
        None => 100,
        Some(v) if v > 0 && v <= 300 => v,
        Some(_others) => 100,
    };
    let fc_status: Option<bool> = fc.unwrap_or(ClanWarsFc::NoMatter).into();

    let guild_settings = get_guild_settings(ctx, true).await?;
    if guild_settings.clan_settings.is_none() {
        say_without_ping(ctx, "Clan is not set up in this guild.", true).await?;

        return Ok(());
    }

    let clan_settings = guild_settings.clan_settings.clone().unwrap();

    let current_user = ctx.author();

    match link_user_if_needed(
        ctx,
        &guild_settings.guild_id,
        current_user,
        guild_settings.requires_verified_profile,
    )
    .await
    {
        Some(player) => {
            if !player.is_linked_to_guild(&guild_settings.guild_id) {
                say_profile_not_linked(
                    ctx,
                    &current_user.id,
                    guild_settings.requires_verified_profile,
                )
                .await?;

                return Ok(());
            }

            let bl_player = fetch_player_from_bl(&player.id).await;
            if bl_player.is_err() {
                say_without_ping(
                    ctx,
                    format!(
                        "Error: can not fetch player data from BL: {}",
                        bl_player.err().unwrap()
                    )
                    .as_str(),
                    true,
                )
                .await?;

                return Ok(());
            }

            let bl_player = bl_player.unwrap();

            let clan_tag = clan_settings.get_clan();

            if !bl_player.clans.iter().any(|clan| clan.tag == clan_tag) {
                say_without_ping(
                    ctx,
                    format!("You are not a member of the {} clan.", &clan_tag).as_str(),
                    false,
                )
                .await?;

                return Ok(());
            }

            if bl_player.clans.first().unwrap().tag != clan_tag {
                say_without_ping(
                    ctx,
                    format!("You did not set clan {} as primary. Go to your profile and move the clan to the first position on the list.", &clan_tag).as_str(),
                    true,
                )
                    .await?;

                return Ok(());
            }

            match Playlist::for_clan_player(
                &ctx.data().player_scores_repository.clone(),
                &ctx.data().maps_repository.clone(),
                &ctx.data().settings.server.url.clone(),
                clan_tag,
                player,
                playlist_type_filter,
                played_filter,
                count,
                max_stars,
                max_clan_pp_diff,
                fc_status,
                skip_commander_order,
                playlist_name,
                false,
            )
            .await
            {
                Ok(playlist) => match &ctx.data().playlists_repository.save(playlist.clone()).await
                {
                    Ok(_) => {
                        match serde_json::to_string::<Playlist>(&playlist) {
                            Ok(data_json) => {
                                ctx.send(
                                    CreateReply::default()
                                        .content("Here's your personalized playlist:")
                                        .attachment(CreateAttachment::bytes(
                                            Cow::from(data_json.into_bytes()),
                                            format!(
                                                "{}.json",
                                                playlist
                                                    .get_title()
                                                    .replace([' ', '-', '/', '\\', ':'], "_")
                                            ),
                                        ))
                                        .ephemeral(true),
                                )
                                .await?;
                            }
                            Err(err) => {
                                ctx.say(format!("An error occurred: {}", err)).await?;
                            }
                        };

                        Ok(())
                    }
                    Err(err) => {
                        ctx.say(format!("An error occurred: {}", err)).await?;

                        Ok(())
                    }
                },
                Err(err) => {
                    say_without_ping(ctx, err.as_str(), false).await?;

                    Ok(())
                }
            }
        }
        None => {
            say_profile_not_linked(
                ctx,
                &current_user.id,
                guild_settings.requires_verified_profile,
            )
            .await?;

            Ok(())
        }
    }
}

/// Send the player an invitation to join the clan
#[poise::command(
    slash_command,
    rename = "bl-invite-player",
    required_permissions = "MANAGE_ROLES",
    default_member_permissions = "MANAGE_ROLES",
    required_bot_permissions = "MANAGE_ROLES",
    guild_only
)]
pub(crate) async fn cmd_invite_player(
    _ctx: Context<'_>,
    #[description = "Discord user"] _user: User,
) -> Result<(), Error> {
    todo!()
}

#[tracing::instrument(skip(ctx, message), level=tracing::Level::INFO, name="bot_command:capture-map")]
#[poise::command(
    context_menu_command = "Capture the map",
    guild_only,
    member_cooldown = 5
)]
pub(crate) async fn cmd_capture(
    ctx: Context<'_>,
    #[description = "Message to analyze"] message: Message,
) -> Result<(), Error> {
    ctx.defer().await?;

    let guild_settings = get_guild_settings(ctx, true).await?;
    if guild_settings.clan_settings.is_none() {
        say_without_ping(ctx, "Clan is not set up in this guild.", false).await?;

        return Ok(());
    }

    let leaderboard_ids = get_leaderboard_ids_from_message(message);

    if leaderboard_ids.is_empty() {
        say_without_ping(ctx, "Are you sure you wanted to capture map from this message? I can't find any link to the leaderboard here.", false).await?;

        return Ok(());
    }

    let clan_settings = guild_settings.clan_settings.clone().unwrap();

    let current_user = ctx.author();

    match link_user_if_needed(
        ctx,
        &guild_settings.guild_id,
        current_user,
        guild_settings.requires_verified_profile,
    )
    .await
    {
        Some(player) => {
            if !player.is_linked_to_guild(&guild_settings.guild_id) {
                say_profile_not_linked(
                    ctx,
                    &current_user.id,
                    guild_settings.requires_verified_profile,
                )
                .await?;

                return Ok(());
            }

            let clan_tag = clan_settings.get_clan();

            if !player.clans.iter().any(|clan| clan == &clan_tag) {
                say_without_ping(
                    ctx,
                    format!("You are not a member of the {} clan.", &clan_tag).as_str(),
                    false,
                )
                .await?;

                return Ok(());
            }

            if player.clans.first().unwrap() != &clan_tag {
                say_without_ping(
                    ctx,
                    format!("You did not set clan {} as primary. Go to your profile and move the clan to the first position on the list.", &clan_tag).as_str(),
                    false,
                )
                    .await?;

                return Ok(());
            }

            let msg = ctx
                .say("Sure, lemme check! Oil your gun properly while I check this map for you.")
                .await?;

            let leaderboard_id = leaderboard_ids.first().unwrap();

            let (map, clan_id, leading_clan_tag) = match BL_CLIENT
                .clan()
                .clan_ranking(leaderboard_id, &[ClanRankingParam::Count(1)])
                .await
            {
                Ok(mut clan_maps) => {
                    if clan_maps.list.data.is_empty() {
                        msg.edit(ctx, CreateReply::default().content("Oh snap! It seems that there is no clan wars over this leaderboard.")
                        )
                        .await?;

                        return Ok(());
                    }

                    (
                        clan_maps.list.data.swap_remove(0),
                        clan_maps.clan.id,
                        clan_maps.clan.tag,
                    )
                }
                Err(err) => {
                    msg.edit(
                        ctx,
                        CreateReply::default()
                            .content(format!("Oh snap! An error occurred: {}", err)),
                    )
                    .await?;

                    return Ok(());
                }
            };

            match crate::beatleader::fetch_paged_items(50, None, move |page_def| async move {
                let scores = BL_CLIENT
                    .clan()
                    .scores_response(
                        leaderboard_id,
                        clan_settings.clan_id,
                        &[
                            ClanMapParam::Count(page_def.items_per_page),
                            ClanMapParam::Page(page_def.page),
                        ],
                    )
                    .await?;

                Ok(DataWithMeta {
                    data: scores.associated_scores,
                    items_per_page: None,
                    total: Some(scores.associated_scores_count),
                    other_data: Some((scores.clan, scores.pp, scores.leaderboard)),
                })
            })
            .await
            {
                Ok(data) => {
                    let clan_pp = match data.other_data {
                        Some(ref data) => data.1,
                        None => 0.0,
                    };

                    let mut is_captured = false;
                    if let Some((clan, ..)) = data.other_data {
                        if clan.id == clan_id {
                            is_captured = true;
                        }
                    }

                    let leading_clan_pp = map.pp;
                    let real_pp_loss = clan_pp - leading_clan_pp;

                    let player_id = player.id.clone();
                    let mut pps_without_player = data
                        .data
                        .iter()
                        .filter(|score| score.player_id != player_id)
                        .map(|score| score.pp)
                        .collect::<Vec<_>>();
                    pps_without_player
                        .sort_unstable_by(|a, b| b.partial_cmp(a).unwrap_or(Ordering::Equal));

                    let mut clan_map_with_scores = ClanMapWithScores {
                        map,
                        scores: data.data,
                        pp_boundary: 0.0,
                        acc_boundary: AccBoundary::default(),
                    };

                    // calculate clan pp without player and pp_boundary
                    let clan_pp_without_player = calculate_total_pp_from_sorted(
                        CLAN_WEIGHT_COEFFICIENT,
                        &pps_without_player,
                        0,
                    );
                    clan_map_with_scores.map.pp = clan_pp_without_player - leading_clan_pp;
                    clan_map_with_scores.calc_pp_boundary(Some(player.id.clone()));

                    // set real pp loss
                    clan_map_with_scores.map.pp = real_pp_loss;

                    msg.edit(
                        ctx,
                        CreateReply::default().content(clan_map_with_scores.to_player_string(
                            clan_tag,
                            leading_clan_tag,
                            player.id,
                            is_captured,
                        )),
                    )
                    .await?;
                }
                Err(err) => {
                    msg.edit(
                        ctx,
                        CreateReply::default()
                            .content(format!("Oh snap! An error occurred: {}", err)),
                    )
                    .await?;
                }
            }

            Ok(())
        }
        None => {
            say_profile_not_linked(
                ctx,
                &current_user.id,
                guild_settings.requires_verified_profile,
            )
            .await?;

            Ok(())
        }
    }
}

/// Set or unset clan wars maps channel
#[tracing::instrument(skip(ctx), level=tracing::Level::INFO, name="bot_command:bl-set-clan-wars-maps-channel")]
#[poise::command(
    slash_command,
    rename = "bl-set-clan-wars-maps-channel",
    ephemeral,
    required_permissions = "MANAGE_ROLES",
    default_member_permissions = "MANAGE_ROLES",
    required_bot_permissions = "MANAGE_ROLES",
    guild_only,
    hide_in_help
)]
pub(crate) async fn cmd_set_clan_wars_maps_channel(
    ctx: Context<'_>,
    #[description = "The channel where the bot will post maps to play within clan wars. Leave empty to disable."]
    #[channel_types("Text")]
    channel_id: Option<ChannelId>,
) -> Result<(), Error> {
    let guild_id = get_guild_id(ctx, true).await?;

    match ctx
        .data()
        .guild_settings_repository
        .set_clan_wars_maps_channel(&guild_id, channel_id)
        .await
    {
        Ok(guild_settings) => {
            ctx.say(format!("{}", guild_settings)).await?;

            Ok(())
        }
        Err(e) => {
            ctx.say(format!("An error occurred: {}", e)).await?;

            Ok(())
        }
    }
}

/// Set or unset clan wars contribution channel
#[tracing::instrument(skip(ctx), level=tracing::Level::INFO, name="bot_command:bl-set-clan-wars-contribution-channel")]
#[poise::command(
    slash_command,
    rename = "bl-set-clan-wars-contrib-channel",
    ephemeral,
    required_permissions = "MANAGE_ROLES",
    default_member_permissions = "MANAGE_ROLES",
    required_bot_permissions = "MANAGE_ROLES",
    guild_only,
    hide_in_help
)]
pub(crate) async fn cmd_set_clan_wars_contribution_channel(
    ctx: Context<'_>,
    #[description = "The channel where the bot will post clan wars contributions. Leave empty to disable."]
    #[channel_types("Text")]
    channel_id: Option<ChannelId>,
) -> Result<(), Error> {
    let guild_id = get_guild_id(ctx, true).await?;

    match ctx
        .data()
        .guild_settings_repository
        .set_clan_wars_maps_contribution_channel(&guild_id, channel_id)
        .await
    {
        Ok(guild_settings) => {
            ctx.say(format!("{}", guild_settings)).await?;

            Ok(())
        }
        Err(e) => {
            ctx.say(format!("An error occurred: {}", e)).await?;

            Ok(())
        }
    }
}

/// Enlist for clan wars
#[tracing::instrument(skip(ctx), level=tracing::Level::INFO, name="bot_command:bl-clan-wars-enlist")]
#[poise::command(slash_command, rename = "bl-clan-wars-enlist", guild_only)]
pub(crate) async fn cmd_clan_wars_enlist(
    ctx: Context<'_>,
    #[description = "Discord user (admin only, YOU if not specified)"] user: Option<User>,
) -> Result<(), Error> {
    let ephemeral = user.is_some();

    let selected_user_id =
        match get_user_id_with_required_permission(ctx, user, Permissions::MANAGE_ROLES).await {
            Ok(user_id) => user_id,
            Err(err) => {
                say_without_ping(ctx, err.as_str(), true).await?;

                return Ok(());
            }
        };

    let guild_settings = get_guild_settings(ctx, true).await?;
    if guild_settings.clan_settings.is_none() {
        say_without_ping(ctx, "Clan is not set up in this guild.", true).await?;

        return Ok(());
    }

    match ctx
        .data()
        .guild_settings_repository
        .add_clan_wars_soldier(&guild_settings.guild_id, selected_user_id)
        .await
    {
        Ok(_) => {
            let message = format!("<@{}> has been enlisted.", selected_user_id);
            say_without_ping(ctx, message.as_str(), ephemeral).await?;

            Ok(())
        }
        Err(e) => {
            let message = format!("An error occurred: {}", e);
            say_without_ping(ctx, message.as_str(), true).await?;

            Ok(())
        }
    }
}

/// Release from service in clan wars
#[tracing::instrument(skip(ctx), level=tracing::Level::INFO, name="bot_command:bl-clan-wars-release")]
#[poise::command(slash_command, rename = "bl-clan-wars-release", guild_only)]
pub(crate) async fn cmd_clan_wars_release(
    ctx: Context<'_>,
    #[description = "Discord user (admin only, YOU if not specified)"] user: Option<User>,
) -> Result<(), Error> {
    let ephemeral = user.is_some();

    let selected_user_id =
        match get_user_id_with_required_permission(ctx, user, Permissions::MANAGE_ROLES).await {
            Ok(user_id) => user_id,
            Err(err) => {
                say_without_ping(ctx, err.as_str(), true).await?;

                return Ok(());
            }
        };

    let guild_settings = get_guild_settings(ctx, true).await?;
    if guild_settings.clan_settings.is_none() {
        say_without_ping(ctx, "Clan is not set up in this guild.", true).await?;

        return Ok(());
    }

    match ctx
        .data()
        .guild_settings_repository
        .remove_clan_wars_soldier(&guild_settings.guild_id, selected_user_id)
        .await
    {
        Ok(_) => {
            let message = format!("<@{}> has been released from service.", selected_user_id);
            say_without_ping(ctx, message.as_str(), ephemeral).await?;

            Ok(())
        }
        Err(e) => {
            let message = format!("An error occurred: {}", e);
            say_without_ping(ctx, message.as_str(), true).await?;

            Ok(())
        }
    }
}

/// Set soldier role for clan wars
#[tracing::instrument(skip(ctx), level=tracing::Level::INFO, name="bot_command:bl-set-clan-wars-soldier-role")]
#[poise::command(
    slash_command,
    rename = "bl-set-clan-wars-soldier-role",
    ephemeral,
    required_permissions = "MANAGE_ROLES",
    default_member_permissions = "MANAGE_ROLES",
    required_bot_permissions = "MANAGE_ROLES",
    guild_only,
    hide_in_help
)]
pub(crate) async fn cmd_set_clan_wars_soldier_role(
    ctx: Context<'_>,
    #[description = "Role to assign. Leve empty to remove."] role: Option<Role>,
) -> Result<(), Error> {
    let guild_settings = get_guild_settings(ctx, true).await?;
    if guild_settings.clan_settings.is_none() {
        say_without_ping(ctx, "Clan is not set up in this guild.", true).await?;

        return Ok(());
    }

    match ctx
        .data()
        .guild_settings_repository
        .set_clan_wars_soldier_role(&guild_settings.guild_id, role.map(|r| r.id))
        .await
    {
        Ok(guild_settings) => {
            say_without_ping(ctx, format!("{}", guild_settings).as_str(), true).await?;

            Ok(())
        }
        Err(e) => {
            let message = format!("An error occurred: {}", e);
            say_without_ping(ctx, message.as_str(), true).await?;

            Ok(())
        }
    }
}

#[tracing::instrument(skip(ctx), level=tracing::Level::INFO, name="bot_command:bl-set-clan-commander-role")]
#[poise::command(
    slash_command,
    rename = "bl-set-clan-commander-role",
    ephemeral,
    required_permissions = "MANAGE_ROLES",
    default_member_permissions = "MANAGE_ROLES",
    required_bot_permissions = "MANAGE_ROLES",
    guild_only,
    hide_in_help
)]
pub(crate) async fn cmd_set_clan_commander_role(
    ctx: Context<'_>,
    #[description = "The role of a user who will be able to manage the clan's maps. Leave empty to disable."]
    role: Option<Role>,
) -> Result<(), Error> {
    let guild_settings = get_guild_settings(ctx, true).await?;
    if guild_settings.clan_settings.is_none() {
        say_without_ping(ctx, "Clan is not set up in this guild.", true).await?;

        return Ok(());
    }

    match ctx
        .data()
        .guild_settings_repository
        .set_clan_commander_role(&guild_settings.guild_id, role.map(|r| r.id))
        .await
    {
        Ok(guild_settings) => {
            ctx.say(format!("{}", guild_settings)).await?;

            Ok(())
        }
        Err(e) => {
            ctx.say(format!("An error occurred: {}", e)).await?;

            Ok(())
        }
    }
}

#[tracing::instrument(skip(ctx, message), level=tracing::Level::INFO, name="bot_command:commanders-order")]
#[poise::command(
    context_menu_command = "Commander's order",
    guild_only,
    member_cooldown = 5
)]
pub(crate) async fn cmd_commanders_order(
    ctx: Context<'_>,
    #[description = "Message to analyze"] message: Message,
) -> Result<(), Error> {
    let (leaderboard_ids, clan_tag) = match get_leaderboard_id_for_commander(ctx, message).await {
        Ok(leaderboard_ids) => leaderboard_ids,
        Err(e) => {
            say_without_ping(ctx, format!("{}", e).as_str(), false).await?;

            return Ok(());
        }
    };

    let msg = ctx.say("Sure, give me a moment to check this map.").await?;

    let leaderboard_id = leaderboard_ids.first().unwrap();

    match BL_CLIENT.clan().leaderboard(leaderboard_id, &[]).await {
        Ok(leaderboard) => {
            if leaderboard.difficulty.status != DifficultyStatus::Ranked
                && leaderboard.difficulty.status != DifficultyStatus::Qualified
                && leaderboard.difficulty.status != DifficultyStatus::Nominated
            {
                msg.edit(
                    ctx,
                    CreateReply::default()
                        .content("Leaderboard must have nominated, qualified or ranked status."),
                )
                .await?;
                return Ok(());
            }

            match ctx
                .data()
                .maps_repository
                .get_commander_order(&leaderboard.id, &clan_tag)
                .await
            {
                Ok(commander_order) => {
                    let map_link = format!(
                        "[{} / {}](<https://www.beatleader.xyz/leaderboard/clanranking/{}/1>)",
                        &leaderboard.song.name,
                        &leaderboard.difficulty.difficulty_name,
                        &leaderboard.id,
                    );

                    if commander_order.is_some() {
                        msg.edit(
                            ctx,
                            CreateReply::default().content(format!(
                                "{} is already added to commander's order.",
                                &map_link
                            )),
                        )
                        .await?;
                        return Ok(());
                    }

                    match ctx
                        .data()
                        .maps_repository
                        .save(BsMap::new(
                            ctx.author().id,
                            leaderboard,
                            BsMapType::CommanderOrder,
                            None,
                            Some(clan_tag),
                        ))
                        .await
                    {
                        Ok(_) => {
                            msg.edit(
                                ctx,
                                CreateReply::default()
                                    .content(format!("{} added to commander's order.", &map_link)),
                            )
                            .await?;
                        }
                        Err(err) => {
                            msg.edit(
                                ctx,
                                CreateReply::default()
                                    .content(format!("Oh snap! An error occurred: {}", err)),
                            )
                            .await?;
                        }
                    }

                    Ok(())
                }
                Err(err) => {
                    msg.edit(
                        ctx,
                        CreateReply::default()
                            .content(format!("Oh snap! An error occurred: {}", err)),
                    )
                    .await?;
                    return Ok(());
                }
            }
        }
        Err(err) => {
            msg.edit(
                ctx,
                CreateReply::default().content(format!("Oh snap! An error occurred: {}", err)),
            )
            .await?;

            Ok(())
        }
    }
}

#[tracing::instrument(skip(ctx, message), level=tracing::Level::INFO, name="bot_command:revoke-commanders-order")]
#[poise::command(
    context_menu_command = "Revoke commander's order",
    guild_only,
    member_cooldown = 5
)]
pub(crate) async fn cmd_revoke_commanders_order(
    ctx: Context<'_>,
    #[description = "Message to analyze"] message: Message,
) -> Result<(), Error> {
    let (leaderboard_ids, clan_tag) = match get_leaderboard_id_for_commander(ctx, message).await {
        Ok((leaderboard_ids, clan_tag)) => (leaderboard_ids, clan_tag),
        Err(e) => {
            say_without_ping(ctx, format!("{}", e).as_str(), false).await?;

            return Ok(());
        }
    };

    let msg = ctx.say("Sure, give me a moment to check this map.").await?;

    let leaderboard_id = leaderboard_ids.first().unwrap();

    match ctx
        .data()
        .maps_repository
        .get_commander_order(leaderboard_id, &clan_tag)
        .await
    {
        Ok(commander_order) => {
            if commander_order.is_none() {
                msg.edit(
                    ctx,
                    CreateReply::default().content("This map is not among the commander's orders."),
                )
                .await?;
                return Ok(());
            }

            let commander_order = commander_order.unwrap();

            match ctx
                .data()
                .maps_repository
                .remove(commander_order.get_id())
                .await
            {
                Ok(_) => {
                    msg.edit(
                        ctx,
                        CreateReply::default().content(format!(
                            "{} removed from commander's order.",
                            &commander_order.to_string()
                        )),
                    )
                    .await?;
                }
                Err(err) => {
                    msg.edit(
                        ctx,
                        CreateReply::default()
                            .content(format!("Oh snap! An error occurred: {}", err)),
                    )
                    .await?;
                }
            }

            Ok(())
        }
        Err(err) => {
            msg.edit(
                ctx,
                CreateReply::default().content(format!("Oh snap! An error occurred: {}", err)),
            )
            .await?;
            return Ok(());
        }
    }
}

#[tracing::instrument(skip(ctx, message), level=tracing::Level::INFO, name="bot_command:remove-from-map-list")]
#[poise::command(
    context_menu_command = "Remove from the map list",
    guild_only,
    member_cooldown = 5
)]
pub(crate) async fn cmd_remove_from_map_list(
    ctx: Context<'_>,
    #[description = "Message to analyze"] message: Message,
) -> Result<(), Error> {
    let (leaderboard_ids, clan_tag) = match get_leaderboard_id_for_commander(ctx, message).await {
        Ok((leaderboard_ids, clan_tag)) => (leaderboard_ids, clan_tag),
        Err(e) => {
            say_without_ping(ctx, format!("{}", e).as_str(), false).await?;

            return Ok(());
        }
    };

    let msg = ctx.say("Sure, give me a moment to check this map.").await?;

    let leaderboard_id = leaderboard_ids.first().unwrap();

    match BL_CLIENT.clan().leaderboard(leaderboard_id, &[]).await {
        Ok(leaderboard) => {
            if leaderboard.difficulty.status != DifficultyStatus::Ranked
                && leaderboard.difficulty.status != DifficultyStatus::Qualified
                && leaderboard.difficulty.status != DifficultyStatus::Nominated
            {
                msg.edit(
                    ctx,
                    CreateReply::default()
                        .content("Leaderboard must have nominated, qualified or ranked status."),
                )
                .await?;
                return Ok(());
            }

            match ctx
                .data()
                .maps_repository
                .get_map_list_ban(&leaderboard.id, &clan_tag)
                .await
            {
                Ok(map) => {
                    let map_link = format!(
                        "[{} / {}](<https://www.beatleader.xyz/leaderboard/clanranking/{}/1>)",
                        &leaderboard.song.name,
                        &leaderboard.difficulty.difficulty_name,
                        &leaderboard.id,
                    );

                    if map.is_some() {
                        msg.edit(
                            ctx,
                            CreateReply::default().content(format!(
                                "{} is already removed from the map list.",
                                &map_link
                            )),
                        )
                        .await?;
                        return Ok(());
                    }

                    match ctx
                        .data()
                        .maps_repository
                        .save(BsMap::new(
                            ctx.author().id,
                            leaderboard,
                            BsMapType::MapListSkip,
                            None,
                            Some(clan_tag),
                        ))
                        .await
                    {
                        Ok(_) => {
                            msg.edit(
                                ctx,
                                CreateReply::default()
                                    .content(format!("{} removed from the map list.", &map_link)),
                            )
                            .await?;
                        }
                        Err(err) => {
                            msg.edit(
                                ctx,
                                CreateReply::default()
                                    .content(format!("Oh snap! An error occurred: {}", err)),
                            )
                            .await?;
                        }
                    }

                    Ok(())
                }
                Err(err) => {
                    msg.edit(
                        ctx,
                        CreateReply::default()
                            .content(format!("Oh snap! An error occurred: {}", err)),
                    )
                    .await?;
                    return Ok(());
                }
            }
        }
        Err(err) => {
            msg.edit(
                ctx,
                CreateReply::default().content(format!("Oh snap! An error occurred: {}", err)),
            )
            .await?;

            Ok(())
        }
    }
}

#[tracing::instrument(skip(ctx, message), level=tracing::Level::INFO, name="bot_command:restore-to-map-list")]
#[poise::command(
    context_menu_command = "Restore to the map list",
    guild_only,
    member_cooldown = 5
)]
pub(crate) async fn cmd_restore_to_map_list(
    ctx: Context<'_>,
    #[description = "Message to analyze"] message: Message,
) -> Result<(), Error> {
    let (leaderboard_ids, clan_tag) = match get_leaderboard_id_for_commander(ctx, message).await {
        Ok((leaderboard_ids, clan_tag)) => (leaderboard_ids, clan_tag),
        Err(e) => {
            say_without_ping(ctx, format!("{}", e).as_str(), false).await?;

            return Ok(());
        }
    };

    let msg = ctx.say("Sure, give me a moment to check this map.").await?;

    let leaderboard_id = leaderboard_ids.first().unwrap();

    match ctx
        .data()
        .maps_repository
        .get_map_list_ban(leaderboard_id, &clan_tag)
        .await
    {
        Ok(map) => {
            if map.is_none() {
                msg.edit(
                    ctx,
                    CreateReply::default().content("This map is not removed from the map list."),
                )
                .await?;
                return Ok(());
            }

            let map = map.unwrap();

            match ctx.data().maps_repository.remove(map.get_id()).await {
                Ok(_) => {
                    msg.edit(
                        ctx,
                        CreateReply::default()
                            .content(format!("{} restored to the map list.", &map.to_string())),
                    )
                    .await?;
                }
                Err(err) => {
                    msg.edit(
                        ctx,
                        CreateReply::default()
                            .content(format!("Oh snap! An error occurred: {}", err)),
                    )
                    .await?;
                }
            }

            Ok(())
        }
        Err(err) => {
            msg.edit(
                ctx,
                CreateReply::default().content(format!("Oh snap! An error occurred: {}", err)),
            )
            .await?;
            return Ok(());
        }
    }
}

async fn get_leaderboard_id_for_commander(
    ctx: Context<'_>,
    message: Message,
) -> Result<(Vec<String>, ClanTag), Error> {
    ctx.defer().await?;

    let guild_settings = get_guild_settings(ctx, true).await?;

    if guild_settings.clan_settings.is_none() {
        return Err("Clan is not set up in this guild".to_owned().into());
    }

    let leaderboard_ids = get_leaderboard_ids_from_message(message);

    if leaderboard_ids.is_empty() {
        return Err("I can't find any link to the leaderboard here."
            .to_owned()
            .into());
    }

    let clan_settings = guild_settings.clan_settings.clone().unwrap();

    let is_owner_or_commander = clan_settings.user_id == ctx.author().id
        || if clan_settings.commander_role.is_some() {
            let commander_role = clan_settings.commander_role.unwrap();
            let member = ctx.author_member().await;

            member.is_some() && member.unwrap().roles.contains(&commander_role)
        } else {
            false
        };

    if !is_owner_or_commander {
        return Err("Only clan owner or commander can use this command."
            .to_owned()
            .into());
    }

    Ok((leaderboard_ids, clan_settings.clan))
}
