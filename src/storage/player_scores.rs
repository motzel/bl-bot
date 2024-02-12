use chrono::Utc;
use log::{debug, info, trace};
use std::sync::Arc;

use poise::serenity_prelude::UserId;
use serde::{Deserialize, Serialize};

use crate::beatleader::player::{MapType, PlayerId, PlayerScoreParam, PlayerScoreSort};
use crate::beatleader::{BlContext, SortOrder};
use crate::bot::beatleader::{fetch_scores, Player, Score};
use crate::file_storage::PersistInstance;
use crate::storage::persist::{CachedStorage, ShuttleStorage};

use super::{PersistError, Result, StorageValue};

#[derive(Serialize, Deserialize, Default, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct PlayerScores {
    pub user_id: UserId,
    pub player_id: PlayerId,
    pub bl_context: BlContext,
    pub scores: Vec<Score>,
}

impl StorageValue<PlayerId> for PlayerScores {
    fn get_key(&self) -> PlayerId {
        self.player_id.clone()
    }
}

pub(crate) struct PlayerScoresRepository {
    storage: CachedStorage<PlayerId, PlayerScores>,
    pub bl_context: BlContext,
}

impl<'a> PlayerScoresRepository {
    pub(crate) async fn new(
        persist: Arc<PersistInstance>,
        bl_context: BlContext,
    ) -> Result<PlayerScoresRepository> {
        Ok(Self {
            storage: CachedStorage::new(ShuttleStorage::new(
                format!("player-scores-{}", bl_context.to_string()).as_str(),
                persist,
            ))
            .await?,
            bl_context,
        })
    }

    pub(crate) async fn all(&self) -> Vec<PlayerScores> {
        self.storage.values().await
    }

    pub(crate) async fn len(&self) -> usize {
        self.storage.len().await
    }

    pub(crate) async fn get(&self, player_id: &PlayerId) -> Option<PlayerScores> {
        self.storage.get(player_id).await
    }

    pub(crate) async fn update_player_scores(
        &self,
        player: &Player,
        force_scores_download: bool,
    ) -> Result<Option<PlayerScores>> {
        trace!(
            "Updating user {} / BL player {} scores...",
            player.user_id,
            player.name
        );

        // do not update if not linked in any guild
        if !player.is_linked_to_any_guild() {
            return Ok(None);
        }

        let mut force_scores_download = force_scores_download;
        if !force_scores_download {
            let current_player_scores = self.get(&player.id).await;
            if current_player_scores.is_none() {
                force_scores_download = true;
            }
        }

        // do not update if fetching is skipped
        let player_scores = PlayerScoresRepository::fetch_player_scores(
            player,
            self.bl_context.clone(),
            force_scores_download,
        )
        .await?;
        if player_scores.is_none() {
            return Ok(None);
        }

        let player_scores = player_scores.unwrap();
        let player_scores_clone = player_scores.clone();

        let bl_context = self.bl_context.clone();

        match self
            .storage
            .get_and_modify_or_insert(
                &player.id,
                move |existing_player_scores| {
                    **existing_player_scores = PlayerScores {
                        user_id: player.user_id,
                        player_id: player.id.clone(),
                        bl_context,
                        scores: player_scores,
                    }
                },
                || {
                    Some(PlayerScores {
                        user_id: player.user_id,
                        player_id: player.id.clone(),
                        bl_context: self.bl_context.clone(),
                        scores: player_scores_clone,
                    })
                },
            )
            .await?
        {
            None => {
                debug!("User {} not found.", player.user_id);

                Err(PersistError::NotFound("player not found".to_owned()))
            }
            Some(scores) => {
                debug!(
                    "User {} / BL player {} scores updated.",
                    player.user_id, player.name
                );

                Ok(Some(scores))
            }
        }
    }

    pub(crate) async fn restore(&self, values: Vec<PlayerScores>) -> Result<()> {
        self.storage.restore(values).await
    }

    pub(crate) async fn fetch_player_scores(
        player: &Player,
        bl_context: BlContext,
        force: bool,
    ) -> std::result::Result<Option<Vec<Score>>, crate::beatleader::error::Error> {
        info!("Fetching all ranked scores of {}...", player.name);

        if !force
            && player.last_scores_fetch.is_some()
            && player.last_scores_fetch.unwrap() > player.last_ranked_score_time
            && player.last_scores_fetch.unwrap() > Utc::now() - chrono::Duration::hours(24)
        {
            info!(
                "No new scores since last fetching ({}), skipping.",
                player.last_scores_fetch.unwrap()
            );

            return Ok(None);
        }

        const ITEMS_PER_PAGE: u32 = 100;

        let time_param: Vec<PlayerScoreParam> = match player.last_scores_fetch {
            Some(last_scores_fetch) => {
                if force {
                    vec![]
                } else {
                    vec![PlayerScoreParam::TimeFrom(last_scores_fetch)]
                }
            }
            None => vec![],
        };

        let mut player_scores = Vec::<Score>::with_capacity(player.ranked_play_count as usize);

        let mut page = 1;
        let mut page_count = 1;
        'outer: loop {
            trace!("Fetching scores page {} / {}...", page, page_count);

            match fetch_scores(
                &player.id,
                &[
                    &[
                        PlayerScoreParam::Page(page),
                        PlayerScoreParam::Count(ITEMS_PER_PAGE),
                        PlayerScoreParam::Sort(PlayerScoreSort::Date),
                        PlayerScoreParam::Order(SortOrder::Ascending),
                        PlayerScoreParam::Type(MapType::Ranked),
                        PlayerScoreParam::Context(bl_context.clone()),
                    ],
                    &time_param[..],
                ]
                .concat(),
            )
            .await
            {
                Ok(scores_page) => {
                    debug!("Scores page #{} fetched.", page);

                    if scores_page.data.is_empty() {
                        break 'outer;
                    }

                    page_count = scores_page.total / ITEMS_PER_PAGE
                        + u32::from(scores_page.total % ITEMS_PER_PAGE != 0);

                    for score in scores_page.data {
                        if score.modifiers.contains("NF")
                            || score.modifiers.contains("NB")
                            || score.modifiers.contains("NO")
                            || score.modifiers.contains("NA")
                            || score.modifiers.contains("OP")
                        {
                            continue;
                        }

                        player_scores.push(score);
                    }
                }
                Err(e) => {
                    return Err(e);
                }
            };

            page += 1;

            if page > page_count {
                break;
            }
        }

        info!("All ranked scores of {} fetched.", player.name);

        Ok(Some(player_scores))
    }
}
