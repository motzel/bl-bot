use crate::beatleader::player::PlayerScoreSort;
use crate::beatleader::SortOrder;
use crate::bot::beatleader::fetch_scores;
use crate::bot::db::get_player_id;
use crate::{Context, Error};
use log::{debug, info};
use poise::serenity_prelude as serenity;
use std::convert::From;

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

/// Post link to a replay, yours or another server user who has linked they BL account.
///
/// Enter any user of this server as a parameter. If you omit it then your replay will be searched for.
#[poise::command(slash_command, rename = "bl-replay", guild_only)]
pub(crate) async fn bl_replay(
    ctx: Context<'_>,
    #[description = "Sort by (latest if not specified)"] sort: Option<Sort>,
    #[description = "Discord user (YOU if not specified)"] dsc_user: Option<serenity::User>,
) -> Result<(), Error> {
    let current_user = ctx.author();
    let selected_user = dsc_user.as_ref().unwrap_or(current_user);

    let player_score_sort = (sort.unwrap_or(Sort::default())).to_player_score_sort();

    let persist = &ctx.data().persist;
    let Ok(player_id) = get_player_id(persist, selected_user.id.into()).await else {
        ctx.say("BL profile is not linked. Use ``/bl-link`` command first.").await?;
        return Ok(());
    };

    let player_scores_result = fetch_scores(player_id, 25, player_score_sort).await;
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
    let mut replay_posted = false;

    while let Some(mci) = serenity::CollectComponentInteraction::new(ctx)
        .author_id(current_user.id)
        .channel_id(ctx.channel_id())
        .timeout(std::time::Duration::from_secs(10))
        .await
    {
        debug!("Interaction response: {:?}", mci.data);

        match mci.data.custom_id.as_str() {
            "score_id" => {
                score_ids = mci.data.values.clone();
                mci.defer(ctx).await?;
            }
            "post_btn" => {
                if score_ids.is_empty() {
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

                    replay_posted = true;
                }
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
