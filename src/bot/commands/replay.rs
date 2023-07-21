use crate::beatleader::player::PlayerScoreSort;
use crate::beatleader::SortOrder;
use crate::bot::beatleader::fetch_scores;
use crate::bot::db::get_player_id;
use crate::{Context, Error};
use log::{debug, info};
use poise::serenity_prelude as serenity;

/// Post link to a replay, yours or another server user who has linked they BL account.
///
/// Enter any user of this server as a parameter. If you omit it then your replay will be searched for.
#[poise::command(slash_command, rename = "bl-replay", guild_only)]
pub(crate) async fn bl_replay(
    ctx: Context<'_>,
    #[description = "Discord user (YOU if not specified)"] dsc_user: Option<serenity::User>,
) -> Result<(), Error> {
    let current_user = ctx.author();
    let selected_user = dsc_user.as_ref().unwrap_or(current_user);

    let persist = &ctx.data().persist;
    let Ok(player_id) = get_player_id(persist, selected_user.id.into()).await else {
        ctx.say("BL profile is not linked. Use ``/bl-link`` command first.").await?;
        return Ok(());
    };

    let bl_client = &ctx.data().bl_client;
    let player_scores_result = fetch_scores(bl_client, player_id, 25, PlayerScoreSort::Date).await;
    if let Err(e) = player_scores_result {
        ctx.say(format!("Error fetching scores: {}", e)).await?;
        return Ok(());
    }

    let player_scores = player_scores_result.unwrap();

    let msg = ctx
        .send(|m| {
            m.components(|c| {
                c.create_action_row(|r| {
                    r.create_select_menu(|m| {
                        m.custom_id("score_id")
                            .placeholder("Select replay to post")
                            .options(|o| {
                                player_scores.scores.iter().fold(o, |acc, s| {
                                    acc.create_option(|o| {
                                        o.label(format!(
                                            "{} {}",
                                            s.song_name.clone(),
                                            s.song_sub_name.clone()
                                        ))
                                        .value(s.id.to_string())
                                        .description(format!("{:.2}% {:.2}pp", s.accuracy, s.pp))
                                    })
                                })
                            })
                            .min_values(1)
                            .max_values(1)
                    })
                })
                .create_action_row(|r| {
                    r.create_button(|b| {
                        b.custom_id("post_btn")
                            .label("Post replay")
                            .style(serenity::ButtonStyle::Primary)
                    })
                })
            })
            .ephemeral(true)
        })
        .await?;

    let mut score_ids = Vec::<String>::new();

    // https://github.com/serenity-rs/serenity/blob/current/examples/e17_message_components/src/main.rs
    // https://docs.rs/serenity/latest/serenity/model/prelude/prelude/interaction/message_component/struct.MessageComponentInteraction.html
    while let Some(mci) = serenity::CollectComponentInteraction::new(ctx)
        .author_id(current_user.id)
        .channel_id(ctx.channel_id())
        .timeout(std::time::Duration::from_secs(120))
        .await
    {
        debug!("Interaction response: {:?}", mci.data);

        // https://docs.rs/serenity/latest/serenity/model/application/interaction/message_component/struct.MessageComponentInteraction.html
        match mci.data.custom_id.as_str() {
            "score_id" => {
                score_ids = mci.data.values.clone();
                mci.defer(ctx).await?;
            }
            "post_btn" => {
                if score_ids.is_empty() {
                    // EDITS message, works for both ephemeral and normal messages
                    mci.create_interaction_response(ctx, |ir| {
                        ir.kind(serenity::InteractionResponseType::UpdateMessage)
                            .interaction_response_data(|message| {
                                message.content("**Please choose replay to post!**")
                            })
                    })
                    .await?;
                } else {
                    for score_id in &score_ids {
                        info!("Posting replay for scoreId: {}", score_id);

                        ctx.send(|m| {
                            m.content(format!(
                                "<@{}> used ``/bl-replay`` command to show you the replay: https://replay.beatleader.xyz/?scoreId={}", current_user.id, score_id
                            ))
                                .allowed_mentions(|am| {
                                    am.parse(serenity::builder::ParseValue::Users)
                                        .parse(serenity::builder::ParseValue::Roles)
                                })
                                .reply(false)
                                .ephemeral(false)
                        })
                            .await?;
                    }

                    // EDITS message, works for both ephemeral and normal messages
                    mci.create_interaction_response(ctx, |ir| {
                        ir.kind(serenity::InteractionResponseType::UpdateMessage)
                            .interaction_response_data(|message| {
                                message
                                    .content("Replay posted. You can dismiss this message.")
                                    .components(|c| c)
                            })
                    })
                    .await?;
                }
            }
            _ => {
                mci.defer(ctx).await?;
            }
        }

        // let msg = mci.get_interaction_response(ctx).await?;
        // println!("{:#?}", msg);

        // does not work
        // mci.delete_original_interaction_response(ctx).await?;
        // mci.edit_original_interaction_response(ctx, |m| m.content(format!("Test count: {}", 1)))
        //     .await?;

        // works for NON ephemeral messages only
        // let mut msg = mci.message.clone();
        // msg.edit(ctx, |m| m.content(format!("Test count: {}", 1)))
        //     .await?;
        // mci.create_interaction_response(ctx, |ir| {
        //     ir.kind(serenity::InteractionResponseType::DeferredUpdateMessage)
        // })
        // .await?;

        // EDITS message, works for both ephemeral and normal messages !!!
        // mci.create_interaction_response(ctx, |ir| {
        //     ir.kind(serenity::InteractionResponseType::UpdateMessage)
        //         .interaction_response_data(|message| {
        //             message
        //                 .content("Replay posted. You can dismiss this message.")
        //                 .set_components(serenity::builder::CreateComponents(Vec::new()))
        //                 OR: .components(|c| c)
        //         })
        // })
        // .await?;

        // CREATES follow up message, works for both ephemeral and normal messages
        // mci.create_interaction_response(ctx, |ir| {
        //     ir.kind(serenity::InteractionResponseType::ChannelMessageWithSource)
        // https://docs.rs/serenity/latest/serenity/builder/struct.CreateInteractionResponseData.html
        //         .interaction_response_data(|message| message.content("Test content"))
        // })
        // .await?;

        // DOES NOT WORK -> Value must be one of {4, 5, 6, 7, 9, 10, 11}
        // https://docs.rs/serenity/latest/serenity/model/application/interaction/enum.InteractionResponseType.html
        // https://discord.com/developers/docs/interactions/receiving-and-responding#interaction-response-object-interaction-callback-type
        // mci.create_interaction_response(ctx, |ir| ir.kind(serenity::InteractionResponseType::Pong))
        //     .await?;
    }

    msg.edit(ctx, |m| {
        m.components(|c| c)
            .content("Interaction timed out. Dismiss this message and try again.")
    })
    .await?;

    // ctx.say(format!("Data: {:#?}", selected_user)).await?;
    Ok(())
}
