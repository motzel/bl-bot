use crate::bot::db::unlink_player;
use crate::{Context, Error};
use log::info;
use poise::serenity_prelude as serenity;

/// Unlink your account from your Beat Leader profile.
#[poise::command(slash_command, rename = "bl-unlink", guild_only)]
pub(crate) async fn cmd_unlink(ctx: Context<'_>) -> Result<(), Error> {
    let selected_user = ctx.author();

    let persist = &ctx.data().persist;

    let player_result = unlink_player(persist, selected_user.id).await;

    match player_result {
        Ok(_) => {
            ctx.send(|m| {
                m.content(format!(
                    "<@{}> has been unlinked from BL profile",
                    selected_user.id
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
        Err(e) => {
            ctx.send(|f| {
                f.content(format!("An error has occurred: {}", e))
                    .ephemeral(false)
            })
            .await?;

            Ok(())
        }
    }
}
