use crate::bot::db::link_player;
use crate::{Context, Error};
use log::info;
use poise::serenity_prelude as serenity;

/// Link your account to your Beat Leader profile.
#[poise::command(slash_command, rename = "bl-link", guild_only)]
pub(crate) async fn bl_link(
    ctx: Context<'_>,
    #[description = "Beat Leader PlayerID"] bl_player_id: String,
    #[description = "Discord user (YOU if not specified)"] dsc_user: Option<serenity::User>,
) -> Result<(), Error> {
    let selected_user = dsc_user.as_ref().unwrap_or_else(|| ctx.author());
    // let selected_user_name = &selected_user.name;
    //
    // let member_name = match ctx
    //     .serenity_context()
    //     .http
    //     .get_member(ctx.data().guild_id.into(), selected_user.id.into())
    //     .await
    // {
    //     Ok(member) => match member.nick {
    //         Some(nick) => nick,
    //         None => selected_user_name.to_string(),
    //     },
    //     Err(_) => selected_user_name.to_string(),
    // };

    let bl_client = &ctx.data().bl_client;
    let persist = &ctx.data().persist;

    let player_result = link_player(
        bl_client,
        persist,
        selected_user.id.into(),
        bl_player_id.to_owned(),
    )
    .await;

    match player_result {
        Ok(player) => {
            ctx.send(|m| {
                m.content(format!(
                    "<@{}> has been linked to the BL profile",
                    selected_user.id
                ))
                .embed(|f| {
                    f.title(player.name)
                        .url(format!("https://www.beatleader.xyz/u/{}", player.id))
                        .thumbnail(player.avatar)
                        .field("Rank", player.rank, true)
                        .field("PP", format!("{:.2}", player.pp), true)
                        .field("Country", player.country, true)
                })
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
                f.content(format!("An error occurred: {}", e))
                    .ephemeral(true)
            })
            .await?;

            Ok(())
        }
    }
}
