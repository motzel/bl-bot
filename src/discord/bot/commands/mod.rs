use crate::discord::bot::commands::clan::{
    cmd_capture, cmd_clan_wars_enlist, cmd_clan_wars_playlist, cmd_clan_wars_release,
    cmd_set_clan_wars_contribution_channel, cmd_set_clan_wars_maps_channel,
    cmd_set_clan_wars_soldier_role,
};
use crate::discord::{BotData, Context};
pub(crate) use backup::{cmd_export, cmd_import};
pub(crate) use clan::{cmd_clan_invitation, cmd_set_clan_invitation};
pub(crate) use guild::{
    cmd_add_auto_role, cmd_remove_auto_role, cmd_set_log_channel, cmd_set_profile_verification,
    cmd_show_settings,
};
pub(crate) use player::{cmd_link, cmd_profile, cmd_refresh_scores, cmd_replay, cmd_unlink};
use poise::serenity_prelude::{Permissions, User, UserId};
pub(crate) use register::cmd_register;

pub(crate) mod backup;
pub(crate) mod clan;
pub(crate) mod guild;
pub(crate) mod player;
pub(crate) mod register;

pub(crate) fn commands() -> Vec<poise::Command<BotData, crate::Error>> {
    vec![
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
        cmd_clan_invitation(),
        cmd_clan_wars_playlist(),
        cmd_set_clan_wars_maps_channel(),
        cmd_set_clan_wars_contribution_channel(),
        cmd_clan_wars_enlist(),
        cmd_clan_wars_release(),
        cmd_set_clan_wars_soldier_role(),
        cmd_capture(),
        // cmd_invite_player(),
        cmd_register(),
        cmd_export(),
        cmd_import(),
        cmd_refresh_scores(),
        cmd_help(),
    ]
}

/// Shows help
#[tracing::instrument(skip(ctx), level=tracing::Level::INFO, name="bot_command:bl-help")]
#[poise::command(track_edits, slash_command, rename = "bl-help")]
pub async fn cmd_help(
    ctx: Context<'_>,
    #[description = "Specific command to show help about"] command: Option<String>,
) -> Result<(), crate::Error> {
    let version_string = format!(
        "{} v{} {}",
        env!("CARGO_PKG_NAME"),
        env!("CARGO_PKG_VERSION"),
        "<https://github.com/motzel/bl-bot>"
    );
    let config = poise::builtins::HelpConfiguration {
        extra_text_at_bottom: version_string.as_str(),
        ..Default::default()
    };

    poise::builtins::help(ctx, command.as_deref(), config).await?;
    Ok(())
}

pub(crate) async fn get_user_id_with_required_permission(
    ctx: Context<'_>,
    user: Option<User>,
    required_permissions: Permissions,
) -> Result<UserId, String> {
    match user {
        Some(user) => match ctx.author_member().await {
            Some(member) => match member.permissions {
                Some(member_permissions) => {
                    if !member_permissions.contains(required_permissions) {
                        return Err("Error: requires administrator privilege".to_owned());
                    }

                    Ok(user.id)
                }
                None => Err("Error: can not get user permissions".to_owned()),
            },
            None => Err("Error: can not get user permissions".to_owned()),
        },
        None => Ok(ctx.author().id),
    }
}
