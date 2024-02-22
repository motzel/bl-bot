use std::borrow::Cow;
use std::collections::HashMap;
use std::sync::Arc;

use chrono::{DateTime, Duration, Utc};
use poise::serenity_prelude::{AttachmentType, UserId};
use serde::{Deserialize, Serialize};

use crate::beatleader::clan::{ClanMap, ClanMapsParam, ClanMapsSort, ClanTag};
use crate::beatleader::player::PlayerId;
use crate::beatleader::{BlContext, SortOrder};
use crate::discord::bot::beatleader::fetch_player_from_bl;
use crate::discord::bot::commands::guild::get_guild_settings;
use crate::discord::bot::commands::player::{
    link_user_if_needed, say_profile_not_linked, say_without_ping,
};
use crate::discord::Context;
use crate::storage::player_scores::PlayerScoresRepository;
use crate::{Error, BL_CLIENT};

#[derive(Debug, poise::ChoiceParameter, Default, Clone, Serialize, Deserialize)]
pub(crate) enum BlCommandPlayDate {
    #[name = "Never"]
    #[default]
    Never,
    #[name = "Never or more than a month ago"]
    Month,
    #[name = "Never or more than 3 months ago"]
    ThreeMonths,
    #[name = "Never or more than 6 months ago"]
    SixMonths,
    #[name = "Never or more than a year ago"]
    Year,
    #[name = "No matter if played"]
    NoMatter,
}

impl From<BlCommandPlayDate> for Option<DateTime<Utc>> {
    fn from(value: BlCommandPlayDate) -> Self {
        match value {
            BlCommandPlayDate::Never => None,
            BlCommandPlayDate::Month => Some(Utc::now() - Duration::days(30)),
            BlCommandPlayDate::ThreeMonths => Some(Utc::now() - Duration::days(90)),
            BlCommandPlayDate::SixMonths => Some(Utc::now() - Duration::days(180)),
            BlCommandPlayDate::Year => Some(Utc::now() - Duration::days(365)),
            BlCommandPlayDate::NoMatter => Some(Utc::now()),
        }
    }
}

#[derive(Debug, poise::ChoiceParameter, Serialize, Deserialize, Clone, Default)]
pub(crate) enum BlCommandClanMapSortParam {
    #[default]
    #[name = "To Conquer"]
    ToConquer,
    #[name = "To Hold"]
    ToHold,
}

impl From<BlCommandClanMapSortParam> for ClanMapsParam {
    fn from(value: BlCommandClanMapSortParam) -> Self {
        match value {
            BlCommandClanMapSortParam::ToConquer => ClanMapsParam::Sort(ClanMapsSort::ToConquer),
            BlCommandClanMapSortParam::ToHold => ClanMapsParam::Sort(ClanMapsSort::ToHold),
        }
    }
}

impl BlCommandClanMapSortParam {
    fn to_playlist_type_name(&self) -> String {
        match self {
            BlCommandClanMapSortParam::ToConquer => "to conquer".to_owned(),
            BlCommandClanMapSortParam::ToHold => "to hold".to_owned(),
        }
    }
}

#[derive(Serialize, Deserialize, Default, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PlaylistDifficulty {
    characteristic: String,
    name: String,
}

#[derive(Serialize, Deserialize, Default, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PlaylistItem {
    hash: String,
    difficulties: Vec<PlaylistDifficulty>,
}

#[derive(Default, Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PlaylistCustomData {
    #[serde(rename = "syncURL")]
    sync_url: String,
    owner: String,
    hash: String,
    shared: bool,
    pub clan_tag: ClanTag,
    pub player_id: PlayerId,
    pub playlist_type: BlCommandClanMapSortParam,
    pub last_played: BlCommandPlayDate,
    pub count: u32,
}

pub(crate) type PlaylistId = String;

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Playlist {
    id: PlaylistId,
    allow_duplicates: bool,
    playlist_title: String,
    playlist_author: String,
    playlist_description: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub custom_data: Option<PlaylistCustomData>,
    songs: Vec<PlaylistItem>,
    image: String,
}

impl Playlist {
    pub async fn for_clan_player(
        player_scores_repository: &Arc<PlayerScoresRepository>,
        server_url: &str,
        clan_tag: ClanTag,
        player_id: PlayerId,
        playlist_type: BlCommandClanMapSortParam,
        last_played: BlCommandPlayDate,
        count: u32,
    ) -> Result<Self, String> {
        let maps_list = BL_CLIENT
            .clan()
            .maps(
                clan_tag.as_str(),
                &[
                    ClanMapsParam::Count(100),
                    ClanMapsParam::Page(1),
                    ClanMapsParam::Order(SortOrder::Descending),
                    ClanMapsParam::Context(BlContext::General),
                    playlist_type.clone().into(),
                ],
            )
            .await;

        if let Err(err) = maps_list {
            return Err(format!("Map list download error: {}", err));
        }

        let maps_list = maps_list.unwrap();

        if maps_list.data.is_empty() {
            return Err("No maps of the selected type".to_string());
        }

        let player_leaderboard_ids = player_scores_repository
            .get(&player_id)
            .await
            .unwrap_or_default()
            .scores
            .into_iter()
            .map(|score| (score.leaderboard_id, score.timepost))
            .collect::<HashMap<String, DateTime<Utc>>>();

        let played_filter: Option<DateTime<Utc>> = last_played.clone().into();

        let playlist_maps = maps_list
            .data
            .into_iter()
            .filter(|score| {
                let score_timepost = player_leaderboard_ids.get(&score.leaderboard.id);

                score_timepost.is_none()
                    || (played_filter.is_some()
                        && played_filter.unwrap() > *score_timepost.unwrap())
            })
            .take(count as usize)
            .collect::<Vec<_>>();

        if playlist_maps.is_empty() {
            return Err("No maps meeting the criteria".to_string());
        }

        let playlist_title = format!(
            "{}-clan wars-{}",
            clan_tag,
            playlist_type.to_playlist_type_name()
        );
        let id = Playlist::generate_id();

        Ok(Playlist {
            id: id.clone(),
            playlist_title: playlist_title.clone(),
            songs: Playlist::songs_from_scores(
                playlist_maps.into_iter().take(count as usize).collect(),
            ),
            custom_data: Some(PlaylistCustomData {
                sync_url: format!("{}/playlist/{}/{}", server_url, player_id, id.clone()),
                owner: format!("{}/{}", clan_tag, player_id),
                hash: format!("{}-{}", id, Utc::now().timestamp()),
                shared: false,
                clan_tag,
                player_id,
                playlist_type,
                last_played,
                count,
            }),
            ..Playlist::default()
        })
    }

    pub fn generate_id() -> PlaylistId {
        uuid::Uuid::new_v4()
            .hyphenated()
            .encode_lower(&mut uuid::Uuid::encode_buffer())
            .to_owned()
    }

    pub fn default_image() -> String {
        "base64,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".to_string()
    }

    pub fn get_id(&self) -> &PlaylistId {
        &self.id
    }

    pub fn set_id(&mut self, playlist_id: PlaylistId) -> &mut Self {
        let old_id = self.id.clone();

        self.id = playlist_id;

        if let Some(ref mut custom_data) = self.custom_data {
            custom_data.hash = custom_data.hash.replace(old_id.as_str(), self.id.as_str());
            custom_data.sync_url = custom_data
                .sync_url
                .replace(old_id.as_str(), self.id.as_str());
        }

        self
    }

    pub fn get_title(&self) -> &String {
        &self.playlist_title
    }

    pub fn set_image(&mut self, image: String) -> &mut Self {
        self.image = image;

        self
    }

    pub fn songs_from_scores(scores: Vec<ClanMap>) -> Vec<PlaylistItem> {
        scores
            .into_iter()
            .fold(
                HashMap::<String, Vec<PlaylistDifficulty>>::new(),
                |mut acc, score| {
                    let diff = PlaylistDifficulty {
                        characteristic: score.leaderboard.difficulty.mode_name,
                        name: Playlist::lower_fist_char(
                            score.leaderboard.difficulty.difficulty_name.as_str(),
                        ),
                    };

                    acc.entry(score.leaderboard.song.hash)
                        .and_modify(|playlist_difficulty| playlist_difficulty.push(diff.clone()))
                        .or_insert(vec![diff]);

                    acc
                },
            )
            .into_iter()
            .map(|(hash, difficulties)| PlaylistItem { hash, difficulties })
            .collect::<Vec<_>>()
    }

    pub fn lower_fist_char(s: &str) -> String {
        let mut c = s.chars();
        match c.next() {
            None => String::new(),
            Some(f) => f.to_lowercase().collect::<String>() + c.as_str(),
        }
    }
}

impl Default for Playlist {
    fn default() -> Self {
        Playlist {
            id: Playlist::generate_id(),
            allow_duplicates: true,
            playlist_title: "Clan wars".to_string(),
            playlist_author: "xor eax eax".to_string(),
            playlist_description: "".to_string(),
            custom_data: None,
            songs: vec![],
            image: Playlist::default_image(),
        }
    }
}

/// Generate clan wars playlist
#[poise::command(slash_command, rename = "bl-clan-wars-playlist", guild_only)]
pub(crate) async fn cmd_clan_wars_playlist(
    ctx: Context<'_>,
    #[description = "Playlist type (default: To Conquer)"] playlist_type: Option<
        BlCommandClanMapSortParam,
    >,
    #[description = "Last played (default: Never)"] played: Option<BlCommandPlayDate>,
    #[description = "Maps count (max: 100, default: 100)"] count: Option<u8>,
) -> Result<(), Error> {
    ctx.defer().await?;

    let playlist_type_filter = playlist_type.unwrap_or(BlCommandClanMapSortParam::ToConquer);
    let played_filter = played.unwrap_or(BlCommandPlayDate::Never);
    let count = match count {
        None => 100,
        Some(v) if v > 0 && v <= 100 => v,
        Some(_others) => 100,
    };

    let guild_settings = get_guild_settings(ctx, true).await?;
    if guild_settings.clan_settings.is_none() {
        say_without_ping(ctx, "Clan is not set up in this guild.", true).await?;

        return Ok(());
    }

    let clan_settings = guild_settings.clan_settings.clone().unwrap();

    let current_user = ctx.author();

    match link_user_if_needed(
        ctx,
        &guild_settings.guild_id,
        current_user,
        guild_settings.requires_verified_profile,
    )
    .await
    {
        Some(player) => {
            if !player.is_linked_to_guild(&guild_settings.guild_id) {
                say_profile_not_linked(ctx, &current_user.id).await?;

                return Ok(());
            }

            let bl_player = fetch_player_from_bl(&player.id).await;
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

            if !bl_player.socials.iter().any(|social| {
                social.service == "Discord" && social.user_id == current_user.id.to_string()
            }) {
                say_without_ping(
                    ctx,
                    "The profile must be verified. Go to <https://www.beatleader.xyz/settings#account> and link your discord account with your BL profile.",
                    false,
                )
                    .await?;

                return Ok(());
            }

            let clan_tag = clan_settings.get_clan();

            if !bl_player.clans.iter().any(|clan| clan.tag == clan_tag) {
                say_without_ping(
                    ctx,
                    format!("You are not a member of the {} clan.", &clan_tag).as_str(),
                    false,
                )
                .await?;

                return Ok(());
            }

            if bl_player.clans.first().unwrap().tag != clan_tag {
                say_without_ping(
                    ctx,
                    format!("You did not set clan {} as primary. Go to your profile and move the clan to the first position on the list.", &clan_tag).as_str(),
                    true,
                )
                .await?;

                return Ok(());
            }

            match Playlist::for_clan_player(
                &ctx.data().player_scores_repository.clone(),
                &ctx.data().settings.server.url.clone(),
                clan_tag,
                player.id,
                playlist_type_filter,
                played_filter,
                count as u32,
            )
            .await
            {
                Ok(playlist) => match &ctx.data().playlists_repository.save(playlist.clone()).await
                {
                    Ok(_) => {
                        match serde_json::to_string::<Playlist>(&playlist) {
                            Ok(data_json) => {
                                ctx.send(|f| {
                                    f.content("Here's your personalized playlist:")
                                        .attachment(AttachmentType::Bytes {
                                            data: Cow::from(data_json.into_bytes()),
                                            filename: format!(
                                                "{}.json",
                                                playlist.playlist_title.replace([' ', '-'], "_")
                                            ),
                                        })
                                        .ephemeral(true)
                                })
                                .await?;
                            }
                            Err(err) => {
                                ctx.say(format!("An error occurred: {}", err)).await?;
                            }
                        };

                        Ok(())
                    }
                    Err(err) => {
                        ctx.say(format!("An error occurred: {}", err)).await?;

                        Ok(())
                    }
                },
                Err(err) => {
                    say_without_ping(ctx, err.as_str(), false).await?;

                    Ok(())
                }
            }
        }
        None => {
            say_profile_not_linked(ctx, &current_user.id).await?;

            Ok(())
        }
    }
}