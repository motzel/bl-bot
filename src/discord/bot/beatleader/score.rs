use std::borrow::Cow;
use std::fmt;
use std::fmt::Display;
use std::sync::Arc;

use chrono::serde::ts_seconds;
use chrono::{DateTime, Utc};
use poise::serenity_prelude::{AttachmentType, CreateEmbed, CreateMessage};
use poise::CreateReply;
use serde::{Deserialize, Serialize};
use serde_with::serde_as;
use serde_with::TimestampSeconds;
use serde_with::{DefaultOnError, DefaultOnNull};
use tracing::info;

use crate::beatleader::error::Error as BlError;
use crate::beatleader::player::{
    DifficultyStatus, Duration, LeaderboardId, PlayerId, PlayerScoreParam, Score as BlScore,
};
use crate::beatleader::pp::{calculate_pp_boundary, WEIGHT_COEFFICIENT};
use crate::beatleader::rating::Ratings;
use crate::beatleader::{BlContext, List as BlList};
use crate::discord::bot::beatleader::player::Player;
use crate::other::string_utils;
use crate::other::string_utils::capitalize;
use crate::storage::player_scores::PlayerScoresRepository;
use crate::BL_CLIENT;

const DEFAULT_MAX_RATING: f64 = 15.0;

#[serde_as]
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Score {
    pub id: u32,
    pub player_id: String,
    pub accuracy: f64,
    pub fc_accuracy: f64,
    pub acc_left: f64,
    pub acc_right: f64,
    pub pp: f64,
    pub fc_pp: f64,
    pub rank: u32,
    pub mistakes: u32,
    pub max_streak: u32,
    pub max_combo: u32,
    pub pauses: u32,
    pub full_combo: bool,
    pub modifiers: String,
    pub leaderboard_id: LeaderboardId,
    pub song_name: String,
    pub song_sub_name: String,
    pub song_mapper: String,
    pub song_author: String,
    pub song_cover: String,
    pub song_bpm: f32,
    pub song_duration: Duration,
    pub song_hash: String,
    pub difficulty_name: String,
    pub difficulty_nps: f64,
    #[serde_as(deserialize_as = "DefaultOnNull")]
    pub difficulty_original_rating: MapRating,
    #[serde_as(deserialize_as = "DefaultOnNull")]
    pub difficulty_rating: MapRating,
    #[serde_as(deserialize_as = "DefaultOnError")]
    #[serde(default)]
    pub difficulty_status: DifficultyStatus,
    pub difficulty_mode_name: String,
    pub difficulty_value: u32,
    #[serde_as(as = "TimestampSeconds<String>")]
    pub timeset: DateTime<Utc>,
    #[serde(with = "ts_seconds")]
    pub timepost: DateTime<Utc>,
}

impl From<BlScore> for Score {
    fn from(bl_score: BlScore) -> Self {
        let map_rating = (&bl_score).into();

        Score {
            id: bl_score.id,
            player_id: bl_score.player_id,
            accuracy: bl_score.accuracy * 100.0,
            fc_accuracy: bl_score.fc_accuracy * 100.0,
            acc_left: bl_score.acc_left,
            acc_right: bl_score.acc_right,
            pp: bl_score.pp,
            fc_pp: bl_score.fc_pp,
            rank: bl_score.rank,
            mistakes: bl_score.bad_cuts
                + bl_score.missed_notes
                + bl_score.bomb_cuts
                + bl_score.walls_hit,
            max_streak: bl_score.max_streak,
            max_combo: bl_score.max_combo,
            pauses: bl_score.pauses,
            full_combo: bl_score.full_combo,
            modifiers: bl_score.modifiers,
            leaderboard_id: bl_score.leaderboard.id,
            song_name: bl_score.leaderboard.song.name,
            song_sub_name: bl_score.leaderboard.song.sub_name,
            song_mapper: bl_score.leaderboard.song.mapper,
            song_author: bl_score.leaderboard.song.author,
            song_cover: bl_score.leaderboard.song.cover_image,
            song_bpm: bl_score.leaderboard.song.bpm,
            song_duration: bl_score.leaderboard.song.duration,
            song_hash: bl_score.leaderboard.song.hash,
            difficulty_name: bl_score.leaderboard.difficulty.difficulty_name,
            difficulty_nps: bl_score.leaderboard.difficulty.nps,
            difficulty_original_rating: MapRating::new(
                MapRatingModifier::None,
                bl_score.leaderboard.difficulty.stars,
                bl_score.leaderboard.difficulty.tech_rating,
                bl_score.leaderboard.difficulty.acc_rating,
                bl_score.leaderboard.difficulty.pass_rating,
            ),
            difficulty_rating: map_rating,
            difficulty_status: bl_score.leaderboard.difficulty.status,
            difficulty_mode_name: bl_score.leaderboard.difficulty.mode_name,
            difficulty_value: bl_score.leaderboard.difficulty.value,
            timeset: bl_score.timeset,
            timepost: bl_score.timepost,
        }
    }
}

impl Score {
    pub(crate) fn add_embed_to_message<'a>(
        &self,
        message: &mut CreateMessage<'a>,
        player: &Player,
        bl_context: &BlContext,
        embed_image: Option<&'a Vec<u8>>,
    ) {
        let with_embed_image = embed_image.is_some();

        if let Some(embed_buffer) = embed_image {
            message.add_file(AttachmentType::Bytes {
                data: Cow::<[u8]>::from(embed_buffer),
                filename: "embed.png".to_string(),
            });
        }

        message.embed(|f| {
            self.add_embed(player, bl_context, with_embed_image, f);

            f
        });
    }

    pub(crate) fn add_embed_to_reply<'a>(
        &self,
        message: &mut CreateReply<'a>,
        player: &Player,
        bl_context: &BlContext,
        embed_image: Option<&'a Vec<u8>>,
    ) {
        let with_embed_image = embed_image.is_some();

        if let Some(embed_buffer) = embed_image {
            message.attachment(AttachmentType::Bytes {
                data: Cow::<[u8]>::from(embed_buffer),
                filename: "embed.png".to_string(),
            });
        }

        message.embed(|f| {
            self.add_embed(player, bl_context, with_embed_image, f);

            f
        });
    }

    pub(crate) fn add_embed<'a>(
        &'a self,
        player: &Player,
        bl_context: &BlContext,
        with_embed_image: bool,
        f: &'a mut CreateEmbed,
    ) {
        let mut desc = "".to_owned();

        desc.push_str(&format!(
            "**{} / {} / {}",
            capitalize(&bl_context.to_string()),
            self.difficulty_name,
            self.difficulty_status
        ));

        if self.difficulty_rating.stars > 0.0 {
            desc.push_str(&format!(" / {:.2}⭐", self.difficulty_rating.stars));
        }

        if !self.modifiers.is_empty() {
            desc.push_str(&(" / ".to_owned() + &self.modifiers.clone()));
        }

        desc.push_str("**");

        desc.push_str(&format!("\n### **[BL Replay](https://replay.beatleader.xyz/?scoreId={}) | [ArcViewer](https://allpoland.github.io/ArcViewer/?scoreID={})**\n", self.id, self.id));

        f.author(|a| {
            a.name(player.name.clone())
                .icon_url(player.avatar.clone())
                .url(format!("https://www.beatleader.xyz/u/{}", player.id))
        })
        .title(format!("{} {}", self.song_name, self.song_sub_name,))
        .description(desc)
        .url(format!(
            "https://www.beatleader.xyz/leaderboard/global/{}/1",
            self.leaderboard_id
        ))
        .timestamp(self.timeset);

        if !with_embed_image {
            f.thumbnail(self.song_cover.clone());

            if self.pp > 0.00 {
                if self.full_combo {
                    f.field("PP", format!("{:.2}", self.pp), true);
                } else {
                    f.field("PP", format!("{:.2} ({:.2} FC)", self.pp, self.fc_pp), true);
                }
            }

            if self.full_combo {
                f.field("Acc", format!("{:.2}%", self.accuracy), true);
            } else {
                f.field(
                    "Acc",
                    format!("{:.2}% ({:.2}% FC)", self.accuracy, self.fc_accuracy),
                    true,
                );
            }

            f.field("Rank", format!("#{}", self.rank), true)
                .field(
                    "Mistakes",
                    if self.mistakes == 0 {
                        "FC".to_string()
                    } else {
                        self.mistakes.to_string()
                    },
                    true,
                )
                .field("Acc Left", format!("{:.2}", self.acc_left), true)
                .field("Acc Right", format!("{:.2}", self.acc_right), true)
                .field("Pauses", self.pauses, true)
                .field("Max combo", self.max_combo, true)
                .field("Max streak", self.max_streak, true);
        }
    }
}

impl From<BlList<BlScore>> for BlList<Score> {
    fn from(value: BlList<BlScore>) -> Self {
        Self {
            data: value.data.into_iter().map(|v| v.into()).collect(),
            page: value.page,
            items_per_page: value.items_per_page,
            total: value.total,
        }
    }
}

pub(crate) async fn fetch_scores(
    player_id: &PlayerId,
    params: &[PlayerScoreParam],
) -> Result<BlList<Score>, BlError> {
    Ok(BL_CLIENT.player().scores(player_id, params).await?.into())
}

pub(crate) async fn fetch_rating(
    hash: &str,
    mode_name: &str,
    value: u32,
) -> Result<Ratings, BlError> {
    BL_CLIENT.ratings().get(hash, mode_name, value).await
}

#[derive(Debug, Default)]
pub(crate) struct ScoreStats {
    pub last_scores_fetch: DateTime<Utc>,
    pub last_ranked_paused_at: Option<DateTime<Utc>>,
    pub top_stars: f64,
    pub plus_1pp: f64,
}

pub(crate) async fn fetch_ranked_scores_stats(
    player_scores_repository: &Arc<PlayerScoresRepository>,
    player: &Player,
    force: bool,
) -> Result<Option<ScoreStats>, BlError> {
    info!("Updating ranked scores stats of {}...", player.name);

    let player_scores = player_scores_repository
        .update_player_scores(player, force)
        .await;
    if let Err(err) = player_scores {
        return Err(BlError::Db(err.to_string()));
    }

    let player_scores = player_scores.unwrap();
    if player_scores.is_none() {
        info!("No scores, skipping.",);

        return Ok(None);
    }

    let player_scores = player_scores.unwrap();

    let mut pps = player_scores
        .scores
        .iter()
        .map(|score| score.pp)
        .collect::<Vec<f64>>();

    let top_stars = player_scores.scores.iter().fold(0.0, |acc, score| {
        if acc < score.difficulty_rating.stars {
            score.difficulty_rating.stars
        } else {
            acc
        }
    });

    let last_ranked_paused_at = player_scores.scores.iter().fold(None, |acc, score| {
        if score.pauses > 0 && (acc.is_none() || acc.unwrap() < score.timepost) {
            Some(score.timepost)
        } else {
            acc
        }
    });

    let plus_1pp = calculate_pp_boundary(WEIGHT_COEFFICIENT, &mut pps, 1.0);

    info!("Ranked scores stats of {} updated.", player.name);

    Ok(Some(ScoreStats {
        last_scores_fetch: Utc::now(),
        top_stars,
        last_ranked_paused_at,
        plus_1pp,
    }))
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub enum MapRatingModifier {
    #[default]
    None,
    SlowerSong,
    FasterSong,
    SuperFastSong,
}

impl MapRatingModifier {
    pub fn speed_multiplier(&self) -> f64 {
        match self {
            MapRatingModifier::None => 1.0,
            MapRatingModifier::SlowerSong => 0.85,
            MapRatingModifier::FasterSong => 1.2,
            MapRatingModifier::SuperFastSong => 1.5,
        }
    }
}

impl From<&str> for MapRatingModifier {
    fn from(value: &str) -> Self {
        if value.contains("SS") {
            return MapRatingModifier::SlowerSong;
        } else if value.contains("FS") {
            return MapRatingModifier::FasterSong;
        } else if value.contains("SF") {
            return MapRatingModifier::SuperFastSong;
        }

        MapRatingModifier::None
    }
}

impl Display for MapRatingModifier {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MapRatingModifier::None => write!(f, ""),
            MapRatingModifier::SlowerSong => write!(f, "SS"),
            MapRatingModifier::FasterSong => write!(f, "FS"),
            MapRatingModifier::SuperFastSong => write!(f, "SF"),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MapRating {
    pub modifier: MapRatingModifier,
    pub stars: f64,
    pub tech: f64,
    pub acc: f64,
    pub pass: f64,
}

impl MapRating {
    pub fn new(modifier: MapRatingModifier, stars: f64, tech: f64, acc: f64, pass: f64) -> Self {
        Self {
            modifier,
            stars,
            tech,
            acc,
            pass,
        }
    }

    pub fn from_ratings_and_modifier(ratings: &Ratings, modifier: MapRatingModifier) -> Self {
        match modifier {
            MapRatingModifier::None => Self {
                modifier,
                stars: ratings.none.star_rating,
                tech: ratings.none.lack_map_calculation.balanced_tech,
                acc: ratings.none.acc_rating,
                pass: ratings.none.lack_map_calculation.balanced_pass_diff,
            },
            MapRatingModifier::SlowerSong => Self {
                modifier,
                stars: ratings.ss.star_rating,
                tech: ratings.ss.lack_map_calculation.balanced_tech,
                acc: ratings.ss.acc_rating,
                pass: ratings.ss.lack_map_calculation.balanced_pass_diff,
            },
            MapRatingModifier::FasterSong => Self {
                modifier,
                stars: ratings.fs.star_rating,
                tech: ratings.fs.lack_map_calculation.balanced_tech,
                acc: ratings.fs.acc_rating,
                pass: ratings.fs.lack_map_calculation.balanced_pass_diff,
            },
            MapRatingModifier::SuperFastSong => Self {
                modifier,
                stars: ratings.sf.star_rating,
                tech: ratings.sf.lack_map_calculation.balanced_tech,
                acc: ratings.sf.acc_rating,
                pass: ratings.sf.lack_map_calculation.balanced_pass_diff,
            },
        }
    }

    pub fn has_individual_rating(&self) -> bool {
        self.tech > 0.0 || self.acc > 0.0 || self.pass > 0.0
    }

    pub fn get_max_rating(&self) -> f64 {
        f64::max(
            f64::max(f64::max(self.tech, self.acc), self.pass).ceil(),
            DEFAULT_MAX_RATING,
        )
        .ceil()
    }

    pub fn get_tech_relative(&self) -> f64 {
        f64::min(self.tech / self.get_max_rating(), 1.0)
    }

    pub fn get_acc_relative(&self) -> f64 {
        f64::min(self.acc / self.get_max_rating(), 1.0)
    }

    pub fn get_pass_relative(&self) -> f64 {
        f64::min(self.pass / self.get_max_rating(), 1.0)
    }
}

impl From<&BlScore> for MapRating {
    fn from(bl_score: &BlScore) -> Self {
        let mut map_rating = MapRating::new(
            MapRatingModifier::None,
            bl_score.leaderboard.difficulty.stars,
            bl_score.leaderboard.difficulty.tech_rating,
            bl_score.leaderboard.difficulty.acc_rating,
            bl_score.leaderboard.difficulty.pass_rating,
        );

        if let Some(ref ratings) = bl_score.leaderboard.difficulty.modifiers_rating {
            if bl_score.modifiers.contains("SS") && ratings.ss_stars > 0.00 {
                map_rating.modifier = MapRatingModifier::SlowerSong;
                map_rating.stars = ratings.ss_stars;
                map_rating.tech = ratings.ss_tech_rating;
                map_rating.acc = ratings.ss_acc_rating;
                map_rating.pass = ratings.ss_pass_rating;
            } else if bl_score.modifiers.contains("FS") && ratings.fs_stars > 0.00 {
                map_rating.modifier = MapRatingModifier::FasterSong;
                map_rating.stars = ratings.fs_stars;
                map_rating.tech = ratings.fs_tech_rating;
                map_rating.acc = ratings.fs_acc_rating;
                map_rating.pass = ratings.fs_pass_rating;
            } else if bl_score.modifiers.contains("SF") && ratings.sf_stars > 0.00 {
                map_rating.modifier = MapRatingModifier::SuperFastSong;
                map_rating.stars = ratings.sf_stars;
                map_rating.tech = ratings.sf_tech_rating;
                map_rating.acc = ratings.sf_acc_rating;
                map_rating.pass = ratings.sf_pass_rating;
            }
        }

        map_rating
    }
}

impl Default for MapRating {
    fn default() -> Self {
        Self {
            modifier: MapRatingModifier::default(),
            stars: 0.0,
            tech: 0.0,
            acc: 0.0,
            pass: 0.0,
        }
    }
}
