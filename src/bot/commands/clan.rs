use crate::beatleader::clan::Clan;
use crate::beatleader::oauth::OAuthScope;
use crate::bot::beatleader::fetch_clan;
use crate::bot::commands::player::{say_profile_not_linked, say_without_ping};
use crate::{Context, Error, BL_CLIENT};
use futures::Stream;
use poise::serenity_prelude;

/// Set up automatic sending of clan invitations
#[poise::command(
    slash_command,
    rename = "bl-set-clan-invite",
    ephemeral,
    required_permissions = "MANAGE_ROLES",
    default_member_permissions = "MANAGE_ROLES",
    required_bot_permissions = "MANAGE_ROLES",
    guild_only
)]
pub(crate) async fn cmd_set_clan_invite(ctx: Context<'_>) -> Result<(), Error> {
    if ctx.data().oauth_credentials.is_none() {
        say_without_ping(ctx, "The bot is not properly configured to send invitations to the clan. Contact the bot owner to have it configured.", true).await?;
        return Ok(());
    }

    let Some(guild_id) = ctx.guild_id() else {
        say_without_ping(ctx, "Can not get guild data", true).await?;
        return Ok(());
    };

    let Ok(_guild) = ctx.data().guild_settings_repository.get(&guild_id).await else {
        say_without_ping(ctx, "Error: can not get guild settings", true).await?;

        return Ok(());
    };

    let current_user = ctx.author();

    let player = ctx.data().players_repository.get(&current_user.id).await;

    if player.is_none() {
        say_profile_not_linked(ctx, &current_user.id).await?;
        return Ok(());
    }

    let player = player.unwrap();
    if !player.is_verified {
        say_without_ping(ctx, "The profile must be verified. Go to https://www.beatleader.xyz and link your discord account with your BL profile.", true).await?;
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

    let oauth_client = BL_CLIENT.with_oauth(ctx.data().oauth_credentials.as_ref().unwrap().clone());

    msg_contents.push_str(format!("\nGreat, you are the owner of the {} clan. Now click this link and authorize the bot to send invitations to the clan on your behalf. {}", &player_clan.tag, oauth_client.oauth().authorize_url(vec![OAuthScope::Profile, OAuthScope::OfflineAccess, OAuthScope::Clan]).unwrap_or("Error when generating authorization link".to_owned())).as_str());
    let msg_contents_clone = msg_contents.clone();
    msg.edit(ctx, |m| m.components(|c| c).content(&msg_contents_clone))
        .await?;

    Ok(())
}
