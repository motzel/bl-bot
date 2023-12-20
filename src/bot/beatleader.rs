use crate::beatleader::clan::Clan;
use crate::beatleader::player::{DifficultyStatus, Duration, MapType, PlayerId};
use crate::beatleader::player::{
    Player as BlPlayer, PlayerScoreParam, PlayerScoreSort, Score as BlScore,
};
use crate::beatleader::pp::calculate_pp_boundary;
use crate::beatleader::{error::Error as BlError, BlContext, List as BlList, MetaData, SortOrder};
use crate::bot::{Metric, PlayerMetricValue};
use crate::storage::{StorageKey, StorageValue};
use crate::BL_CLIENT;
use chrono::serde::{ts_seconds, ts_seconds_option};
use chrono::{DateTime, Utc};
use log::{debug, error, info, trace};
use poise::serenity_prelude::{AttachmentType, CreateEmbed, CreateMessage, GuildId, UserId};
use poise::CreateReply;
use serde::{Deserialize, Serialize};
use serde_with::{serde_as, DefaultOnError, TimestampSeconds};
use std::borrow::Cow;

#[derive(Serialize, Deserialize, Default, Debug, Clone)]
#[serde(rename_all = "camelCase")]
#[serde(default)]
#[non_exhaustive]
pub struct Player {
    pub id: PlayerId,
    pub user_id: UserId,
    pub linked_guilds: Vec<GuildId>,
    pub name: String,
    pub active: bool,
    pub avatar: String,
    pub profile_cover: Option<String>,
    pub country: String,
    pub rank: u32,
    pub country_rank: u32,
    pub pp: f64,
    pub acc_pp: f64,
    pub tech_pp: f64,
    pub pass_pp: f64,
    pub max_streak: u32,
    pub ranked_max_streak: u32,
    pub unranked_max_streak: u32,
    pub avg_accuracy: f64,
    pub avg_ranked_accuracy: f64,
    pub avg_unranked_accuracy: f64,
    pub top_accuracy: f64,
    pub top_ranked_accuracy: f64,
    pub top_unranked_accuracy: f64,
    pub top_acc_pp: f64,
    pub top_tech_pp: f64,
    pub top_pass_pp: f64,
    pub top_pp: f64,
    pub top_stars: f64,
    pub plus_1pp: f64,
    pub total_play_count: u32,
    pub ranked_play_count: u32,
    pub unranked_play_count: u32,
    pub peak_rank: u32,
    pub top1_count: u32,
    pub anonymous_replay_watched: u32,
    pub authorized_replay_watched: u32,
    pub total_replay_watched: u32,
    pub watched_replays: u32,
    pub clans: Vec<String>,
    pub is_verified: bool,
    #[serde(with = "ts_seconds")]
    pub last_ranked_score_time: DateTime<Utc>,
    #[serde(with = "ts_seconds")]
    pub last_unranked_score_time: DateTime<Utc>,
    #[serde(with = "ts_seconds")]
    pub last_score_time: DateTime<Utc>,
    #[serde(with = "ts_seconds_option")]
    pub last_fetch: Option<DateTime<Utc>>,
    #[serde(with = "ts_seconds_option")]
    pub last_scores_fetch: Option<DateTime<Utc>>,
    #[serde(with = "ts_seconds_option")]
    pub last_ranked_paused_at: Option<DateTime<Utc>>,
}

impl StorageKey for UserId {}
impl StorageValue<UserId> for Player {
    fn get_key(&self) -> UserId {
        self.user_id
    }
}

impl Player {
    pub fn get_key(&self) -> UserId {
        self.user_id
    }

    pub fn from_user_id_and_bl_player(
        user_id: UserId,
        guild_ids: Vec<GuildId>,
        bl_player: BlPlayer,
        previous: Option<&Player>,
    ) -> Self {
        Player {
            id: bl_player.id,
            user_id,
            linked_guilds: guild_ids,
            name: bl_player.name,
            active: !bl_player.inactive && !bl_player.banned && !bl_player.bot,
            avatar: bl_player.avatar,
            profile_cover: bl_player.profile_settings.profile_cover,
            country: bl_player.country,
            clans: bl_player
                .clans
                .iter()
                .map(|clan| clan.tag.clone())
                .collect::<Vec<String>>(),
            rank: bl_player.rank,
            country_rank: bl_player.country_rank,
            pp: bl_player.pp,
            acc_pp: bl_player.acc_pp,
            tech_pp: bl_player.tech_pp,
            pass_pp: bl_player.pass_pp,
            max_streak: bl_player.score_stats.max_streak,
            ranked_max_streak: bl_player.score_stats.ranked_max_streak,
            unranked_max_streak: bl_player.score_stats.unranked_max_streak,
            avg_accuracy: bl_player.score_stats.average_accuracy * 100.0,
            avg_ranked_accuracy: bl_player.score_stats.average_ranked_accuracy * 100.0,
            avg_unranked_accuracy: bl_player.score_stats.average_unranked_accuracy * 100.0,
            top_accuracy: bl_player.score_stats.top_accuracy * 100.0,
            top_ranked_accuracy: bl_player.score_stats.top_ranked_accuracy * 100.0,
            top_unranked_accuracy: bl_player.score_stats.top_unranked_accuracy * 100.0,
            top_acc_pp: bl_player.score_stats.top_acc_pp,
            top_tech_pp: bl_player.score_stats.top_tech_pp,
            top_pass_pp: bl_player.score_stats.top_pass_pp,
            top_pp: bl_player.score_stats.top_pp,
            top_stars: if let Some(old_player) = previous {
                old_player.top_stars
            } else {
                0.0
            },
            plus_1pp: if let Some(old_player) = previous {
                old_player.plus_1pp
            } else {
                0.0
            },
            total_play_count: bl_player.score_stats.total_play_count,
            ranked_play_count: bl_player.score_stats.ranked_play_count,
            unranked_play_count: bl_player.score_stats.unranked_play_count,
            peak_rank: bl_player.score_stats.peak_rank,
            top1_count: bl_player.score_stats.top1_count,
            anonymous_replay_watched: bl_player.score_stats.anonymous_replay_watched,
            authorized_replay_watched: bl_player.score_stats.authorized_replay_watched,
            total_replay_watched: bl_player.score_stats.anonymous_replay_watched
                + bl_player.score_stats.authorized_replay_watched,
            watched_replays: bl_player.score_stats.watched_replays,
            is_verified: bl_player
                .socials
                .iter()
                .any(|social| social.service == "Discord" && social.user_id == user_id.to_string()),
            last_ranked_score_time: bl_player.score_stats.last_ranked_score_time,
            last_unranked_score_time: bl_player.score_stats.last_unranked_score_time,
            last_score_time: bl_player.score_stats.last_score_time,
            last_fetch: if let Some(player) = previous {
                player.last_fetch
            } else {
                None
            },
            last_scores_fetch: if let Some(player) = previous {
                player.last_scores_fetch
            } else {
                None
            },
            last_ranked_paused_at: if let Some(player) = previous {
                player.last_ranked_paused_at
            } else {
                None
            },
        }
    }

    pub(crate) fn is_linked_to_any_guild(&self) -> bool {
        !self.linked_guilds.is_empty()
    }

    pub(crate) fn is_linked_to_guild(&self, guild_id: &GuildId) -> bool {
        self.linked_guilds.contains(guild_id)
    }

    pub(crate) fn get_metric_with_value(&self, metric: Metric) -> PlayerMetricValue {
        match metric {
            Metric::TopPp => PlayerMetricValue::TopPp(self.top_pp),
            Metric::TopAcc => PlayerMetricValue::TopAcc(self.top_accuracy),
            Metric::TotalPp => PlayerMetricValue::TotalPp(self.pp),
            Metric::Rank => PlayerMetricValue::Rank(self.rank),
            Metric::CountryRank => PlayerMetricValue::CountryRank(self.country_rank),

            Metric::MaxStreak => PlayerMetricValue::MaxStreak(self.max_streak),
            Metric::Top1Count => PlayerMetricValue::Top1Count(self.top1_count),
            Metric::MyReplaysWatched => {
                PlayerMetricValue::MyReplaysWatched(self.total_replay_watched)
            }
            Metric::ReplaysIWatched => PlayerMetricValue::ReplaysIWatched(self.watched_replays),
            Metric::Clan => PlayerMetricValue::Clan(self.clans.clone()),
            Metric::TopStars => PlayerMetricValue::TopStars(self.top_stars),
            Metric::LastPause => PlayerMetricValue::LastPause(self.last_ranked_paused_at),
        }
    }
}

#[serde_as]
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Score {
    pub id: u32,
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
    pub leaderboard_id: String,
    pub song_name: String,
    pub song_sub_name: String,
    pub song_mapper: String,
    pub song_author: String,
    pub song_cover: String,
    pub song_bpm: f32,
    pub song_duration: Duration,
    pub difficulty_name: String,
    pub difficulty_stars: f64,
    pub difficulty_stars_modified: bool,
    pub difficulty_nps: f64,
    #[serde_as(deserialize_as = "DefaultOnError")]
    #[serde(default)]
    pub difficulty_status: DifficultyStatus,
    pub mode_name: String,
    #[serde_as(as = "TimestampSeconds<String>")]
    pub timeset: DateTime<Utc>,
    #[serde(with = "ts_seconds")]
    pub timepost: DateTime<Utc>,
}

impl From<BlScore> for Score {
    fn from(bl_score: BlScore) -> Self {
        let mut modified_stars = false;
        let mut stars = bl_score.leaderboard.difficulty.stars;

        if let Some(ratings) = bl_score.leaderboard.difficulty.modifiers_rating {
            if bl_score.modifiers.contains("SS") && ratings.ss_stars > 0.00 {
                modified_stars = true;
                stars = ratings.ss_stars;
            } else if bl_score.modifiers.contains("FS") && ratings.fs_stars > 0.00 {
                modified_stars = true;
                stars = ratings.fs_stars;
            } else if bl_score.modifiers.contains("SF") && ratings.sf_stars > 0.00 {
                modified_stars = true;
                stars = ratings.sf_stars;
            }
        }

        Score {
            id: bl_score.id,
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
            difficulty_name: bl_score.leaderboard.difficulty.difficulty_name,
            difficulty_stars: stars,
            difficulty_stars_modified: modified_stars,
            difficulty_nps: bl_score.leaderboard.difficulty.nps,
            difficulty_status: bl_score.leaderboard.difficulty.status,
            mode_name: bl_score.leaderboard.difficulty.mode_name,
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

        if self.difficulty_stars > 0.0 {
            desc.push_str(&format!(
                " / {:.2}⭐{}",
                self.difficulty_stars,
                if self.difficulty_stars_modified {
                    "(M)"
                } else {
                    ""
                }
            ));
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

pub(crate) async fn fetch_clan(tag: &str) -> Result<Clan, BlError> {
    BL_CLIENT.clan().by_tag(tag).await
}

#[derive(Debug, Default)]
pub(crate) struct ScoreStats {
    pub last_scores_fetch: DateTime<Utc>,
    pub last_ranked_paused_at: Option<DateTime<Utc>>,
    pub top_stars: f64,
    pub plus_1pp: f64,
}

pub(crate) async fn fetch_ranked_scores_stats(
    player: &Player,
    force: bool,
) -> Result<Option<ScoreStats>, BlError> {
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

    let mut player_scores = Vec::<f64>::with_capacity(player.ranked_play_count as usize);

    let mut last_scores_fetch;
    let mut top_stars = 0.0;
    let mut last_ranked_paused_at: Option<DateTime<Utc>> = None;

    let mut page = 1;
    let mut page_count = 1;
    'outer: loop {
        trace!("Fetching scores page {} / {}...", page, page_count);

        last_scores_fetch = Utc::now();

        match fetch_scores(
            &player.id,
            &[
                &[
                    PlayerScoreParam::Page(page),
                    PlayerScoreParam::Count(ITEMS_PER_PAGE),
                    PlayerScoreParam::Sort(PlayerScoreSort::Date),
                    PlayerScoreParam::Order(SortOrder::Ascending),
                    PlayerScoreParam::Type(MapType::Ranked),
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
                    player_scores.push(score.pp);

                    if score.modifiers.contains("NF")
                        || score.modifiers.contains("NB")
                        || score.modifiers.contains("NO")
                        || score.modifiers.contains("NA")
                        || score.modifiers.contains("OP")
                    {
                        continue;
                    }

                    if top_stars < score.difficulty_stars {
                        top_stars = score.difficulty_stars;
                    }

                    if score.pauses > 0
                        && (last_ranked_paused_at.is_none()
                            || last_ranked_paused_at.unwrap() < score.timepost)
                    {
                        last_ranked_paused_at = Some(score.timepost);
                    }
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

    let plus_1pp = calculate_pp_boundary(&mut player_scores, 1.0);

    info!("All ranked scores of {} fetched.", player.name);

    Ok(Some(ScoreStats {
        last_scores_fetch,
        top_stars,
        last_ranked_paused_at,
        plus_1pp,
    }))
}

pub fn capitalize(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        None => String::new(),
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
    }
}
