use crate::beatleader::clan::Clan;
use crate::beatleader::oauth::{OAuthScope, OAuthTokenRepository};
use crate::bot::beatleader::fetch_clan;
use crate::bot::commands::guild::get_guild_settings;
use crate::bot::commands::player::{say_profile_not_linked, say_without_ping};
use crate::bot::{ClanSettings, GuildOAuthTokenRepository};
use crate::storage::player::PlayerRepository;
use crate::{Context, Error, BL_CLIENT};
use poise::serenity_prelude::User;
use std::sync::Arc;

/// Set up sending of clan invitations
#[poise::command(
    slash_command,
    rename = "bl-set-clan-invitation",
    ephemeral,
    required_permissions = "MANAGE_ROLES",
    default_member_permissions = "MANAGE_ROLES",
    required_bot_permissions = "MANAGE_ROLES",
    guild_only
)]
pub(crate) async fn cmd_set_clan_invitation(
    ctx: Context<'_>,
    // #[description = "Allow users to self-invite. Default: true"] self_invite: Option<bool>,
) -> Result<(), Error> {
    if ctx.data().oauth_credentials.is_none() {
        say_without_ping(ctx, "The bot is not properly configured to send invitations to the clan. Contact the bot owner to have it configured.", true).await?;
        return Ok(());
    }

    let guild_settings = get_guild_settings(ctx, true).await?;

    // let self_invite = self_invite.unwrap_or(true);
    let self_invite = true;

    let current_user = ctx.author();

    let player = ctx.data().players_repository.get(&current_user.id).await;

    if player.is_none() {
        say_profile_not_linked(ctx, &current_user.id).await?;
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
        msg.edit(ctx, |m| m.components(|c| c).content(&msg_contents_clone))
            .await?;

        let clan_result = fetch_clan(tag.as_str()).await;
        if clan_result.is_err() {
            msg_contents.push_str("FAILED\n");

            let msg_contents_clone = msg_contents.clone();
            msg.edit(ctx, |m| m.components(|c| c).content(&msg_contents_clone))
                .await?;
            return Ok(());
        }

        let clan = clan_result.unwrap();
        if player.id == clan.leader_id {
            msg_contents.push_str("OK, you are the owner!\n");

            let msg_contents_clone = msg_contents.clone();
            msg.edit(ctx, |m| m.components(|c| c).content(&msg_contents_clone))
                .await?;

            player_clan = Some(clan);
            break;
        } else {
            msg_contents.push_str("OK, not the owner\n");

            let msg_contents_clone = msg_contents.clone();
            msg.edit(ctx, |m| m.components(|c| c).content(&msg_contents_clone))
                .await?;
        }
    }

    if player_clan.is_none() {
        msg_contents.push_str("You are not the owner of any clan!\n");

        let msg_contents_clone = msg_contents.clone();
        msg.edit(ctx, |m| m.components(|c| c).content(&msg_contents_clone))
            .await?;

        return Ok(());
    }

    let player_clan = player_clan.unwrap();

    let clan_settings = ClanSettings::new(
        current_user.id,
        player_clan.leader_id.clone(),
        player_clan.tag.clone(),
        self_invite,
    );

    if ctx
        .data()
        .guild_settings_repository
        .set_clan_settings(&guild_settings.guild_id, Some(clan_settings))
        .await
        .is_err()
    {
        msg_contents.push_str("An error occurred while saving clan settings\n");

        let msg_contents_clone = msg_contents.clone();
        msg.edit(ctx, |m| m.components(|c| c).content(&msg_contents_clone))
            .await?;

        return Ok(());
    }

    let guild_oauth_token_repository = GuildOAuthTokenRepository::new(
        player_clan.leader_id,
        Arc::clone(&ctx.data().player_oauth_token_repository),
    );

    let oauth_client = BL_CLIENT.with_oauth(
        ctx.data().oauth_credentials.as_ref().unwrap().clone(),
        guild_oauth_token_repository,
    );

    msg_contents.push_str(format!("\nGreat, you are the owner of the {} clan. Now click this link and authorize the bot to send invitations to the clan on your behalf. {}", &player_clan.tag, oauth_client.oauth().authorize_url(vec![OAuthScope::Profile, OAuthScope::OfflineAccess, OAuthScope::Clan]).unwrap_or("Error when generating authorization link".to_owned())).as_str());
    let msg_contents_clone = msg_contents.clone();
    msg.edit(ctx, |m| m.components(|c| c).content(&msg_contents_clone))
        .await?;

    Ok(())
}

/// Authorize sending of clan invitations
#[poise::command(
    slash_command,
    rename = "bl-set-clan-invitation-code",
    ephemeral,
    required_permissions = "MANAGE_ROLES",
    default_member_permissions = "MANAGE_ROLES",
    required_bot_permissions = "MANAGE_ROLES",
    guild_only
)]
pub(crate) async fn cmd_set_clan_invitation_code(
    ctx: Context<'_>,
    #[description = "BL authorization code"] auth_code: String,
) -> Result<(), Error> {
    if ctx.data().oauth_credentials.is_none() {
        say_without_ping(ctx, "The bot is not properly configured to send invitations to the clan. Contact the bot owner to have it configured.", true).await?;
        return Ok(());
    }

    let guild_settings = get_guild_settings(ctx, true).await?;

    if guild_settings.clan_settings.is_none() {
        say_without_ping(
            ctx,
            "Error: clan settings not found, use ``/bl-set-clan-invitation`` command first",
            true,
        )
        .await?;

        return Ok(());
    }

    let owner_id = guild_settings
        .clan_settings
        .as_ref()
        .unwrap()
        .owner_id
        .clone();
    let self_invite = guild_settings.clan_settings.as_ref().unwrap().self_invite;

    let mut msg_contents = "Fetching access token...".to_owned();
    let msg = ctx.say(&msg_contents).await?;

    let guild_oauth_token_repository = GuildOAuthTokenRepository::new(
        owner_id,
        Arc::clone(&ctx.data().player_oauth_token_repository),
    );
    let oauth_client = BL_CLIENT.with_oauth(
        ctx.data().oauth_credentials.as_ref().unwrap().clone(),
        guild_oauth_token_repository,
    );

    let access_token = oauth_client
        .oauth()
        .access_token_and_store(auth_code.as_str())
        .await;
    if access_token.is_err() {
        msg_contents.push_str(
            format!(
                "An error has occurred: {} Use the ``/bl-set-clan-invitation`` command again.",
                access_token.unwrap_err()
            )
            .as_str(),
        );

        let msg_contents_clone = msg_contents.clone();
        msg.edit(ctx, |m| m.components(|c| c).content(&msg_contents_clone))
            .await?;

        return Ok(());
    }

    msg_contents.push_str("OK\nSaving clan settings...");

    let msg_contents_clone = msg_contents.clone();
    msg.edit(ctx, |m| m.components(|c| c).content(&msg_contents_clone))
        .await?;

    let mut clan_settings = guild_settings.clan_settings.unwrap();
    clan_settings.oauth_token_is_set = true;

    if ctx
        .data()
        .guild_settings_repository
        .set_clan_settings(&guild_settings.guild_id, Some(clan_settings))
        .await
        .is_err()
    {
        msg_contents.push_str("An error occurred while saving clan settings\n");

        let msg_contents_clone = msg_contents.clone();
        msg.edit(ctx, |m| m.components(|c| c).content(&msg_contents_clone))
            .await?;

        return Ok(());
    }

    msg_contents.push_str(format!("OK\nClan invitation service has been set up. {}", if self_invite {"Players can use the ``/bl-clan-invitation`` command to send themselves an invitation to join the clan."} else {"You can use the ``/bl-invite-player`` command to send a player an invitation to join the clan."}).as_str());

    let msg_contents_clone = msg_contents.clone();
    msg.edit(ctx, |m| m.components(|c| c).content(&msg_contents_clone))
        .await?;

    Ok(())
}

/// Send yourself an invitation to join the clan
#[poise::command(slash_command, rename = "bl-clan-invitation", guild_only)]
pub(crate) async fn cmd_clan_invitation(ctx: Context<'_>) -> Result<(), Error> {
    if ctx.data().oauth_credentials.is_none() {
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

    let user_id = ctx.author().id;

    match ctx.data().players_repository.get(&user_id).await {
        Some(player) => {
            if !player.is_linked_to_guild(&guild_settings.guild_id) {
                say_profile_not_linked(ctx, &user_id).await?;

                return Ok(());
            }

            let bl_player = PlayerRepository::fetch_player_from_bl(&player.id).await;
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

            if bl_player.clans.len() >= 3 {
                say_without_ping(ctx, "You are already a member of 3 clans. You must leave some clan if you want to join another.", true).await?;

                return Ok(());
            }

            if !bl_player
                .socials
                .iter()
                .any(|social| social.service == "Discord" && social.user_id == user_id.to_string())
            {
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
                ctx.data().oauth_credentials.as_ref().unwrap().clone(),
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
            say_profile_not_linked(ctx, &user_id).await?;

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
