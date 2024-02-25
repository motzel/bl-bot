use chrono::serde::ts_seconds;
use chrono::{DateTime, Utc};
use reqwest::Method;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_with::{serde_as, DefaultOnNull, TimestampSeconds};

use crate::beatleader::oauth::{ClientWithOAuth, OAuthTokenRepository};
use crate::beatleader::player::{Leaderboard, Player, PlayerId};
use crate::beatleader::{
    BlApiListResponse, BlApiResponse, BlContext, Client, List, MetaData, QueryParam, Result,
    SortOrder,
};

pub struct ClanResource<'a> {
    client: &'a Client,
}

#[allow(dead_code)]
#[derive(Serialize, Deserialize, Default, Debug, Clone)]
pub enum ClanMapsSort {
    Date,
    Pp,
    Acc,
    Rank,
    ToHold,
    #[default]
    ToConquer,
}

#[allow(dead_code)]
#[derive(Clone)]
pub enum ClanMapsParam {
    Page(u32),
    Count(u32),
    Sort(ClanMapsSort),
    Order(SortOrder),
    Context(BlContext),
}

impl QueryParam for ClanMapsParam {
    fn as_query_param(&self) -> (String, String) {
        match self {
            ClanMapsParam::Page(page) => ("page".to_owned(), page.to_string()),
            ClanMapsParam::Count(count) => ("count".to_owned(), count.to_string()),
            ClanMapsParam::Sort(field) => (
                "sortBy".to_owned(),
                match field {
                    ClanMapsSort::Date => "date".to_owned(),
                    ClanMapsSort::Pp => "pp".to_owned(),
                    ClanMapsSort::Acc => "acc".to_owned(),
                    ClanMapsSort::Rank => "rank".to_owned(),
                    ClanMapsSort::ToHold => "tohold".to_owned(),
                    ClanMapsSort::ToConquer => "toconquer".to_owned(),
                },
            ),
            ClanMapsParam::Order(order) => ("order".to_owned(), order.to_string()),
            ClanMapsParam::Context(context) => {
                ("leaderboardContext".to_owned(), context.to_string())
            }
        }
    }
}

#[serde_as]
#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ClanMap {
    #[serde(rename = "id")]
    pub clan_map_id: u32,
    #[serde_as(deserialize_as = "DefaultOnNull")]
    pub pp: f64,
    #[serde_as(deserialize_as = "DefaultOnNull")]
    pub rank: u32,
    #[serde_as(deserialize_as = "DefaultOnNull")]
    pub total_score: u32,
    #[serde_as(deserialize_as = "DefaultOnNull")]
    pub average_rank: f64,
    #[serde_as(deserialize_as = "DefaultOnNull")]
    pub average_accuracy: f64,
    #[serde(with = "ts_seconds")]
    pub last_update_time: DateTime<Utc>,
    #[serde_as(deserialize_as = "DefaultOnNull")]
    pub leaderboard: Leaderboard,
}

impl BlApiResponse for ClanMap {}

#[allow(dead_code)]
#[derive(Clone)]
pub enum ClanMapParam {
    Page(u32),
    Count(u32),
}

impl QueryParam for ClanMapParam {
    fn as_query_param(&self) -> (String, String) {
        match self {
            ClanMapParam::Page(page) => ("page".to_owned(), page.to_string()),
            ClanMapParam::Count(count) => ("count".to_owned(), count.to_string()),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClanPlayer {
    pub id: PlayerId,
    pub name: String,
    pub avatar: String,
    pub country: String,
    pub rank: u32,
    pub country_rank: u32,
    pub pp: f64,
}

#[serde_as]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClanMapScore {
    pub id: u32,
    pub player_id: String,
    pub player: ClanPlayer,
    #[serde_as(deserialize_as = "DefaultOnNull")]
    pub accuracy: f64,
    #[serde_as(deserialize_as = "DefaultOnNull")]
    pub pp: f64,
    #[serde_as(deserialize_as = "DefaultOnNull")]
    pub rank: u32,
    #[serde_as(deserialize_as = "DefaultOnNull")]
    pub bad_cuts: u32,
    #[serde_as(deserialize_as = "DefaultOnNull")]
    pub bomb_cuts: u32,
    #[serde_as(deserialize_as = "DefaultOnNull")]
    pub missed_notes: u32,
    #[serde_as(deserialize_as = "DefaultOnNull")]
    pub walls_hit: u32,
    #[serde_as(deserialize_as = "DefaultOnNull")]
    pub full_combo: bool,
    #[serde_as(deserialize_as = "DefaultOnNull")]
    pub modifiers: String,
    #[serde_as(as = "TimestampSeconds<String>")]
    pub timeset: DateTime<Utc>,
    #[serde(with = "ts_seconds")]
    pub timepost: DateTime<Utc>,
}

#[serde_as]
#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct BlApiClanRankingResponse {
    pub id: u32,
    #[serde_as(deserialize_as = "DefaultOnNull")]
    pub pp: f64,
    #[serde_as(deserialize_as = "DefaultOnNull")]
    pub total_score: f64,
    #[serde_as(deserialize_as = "DefaultOnNull")]
    pub clan: Clan,
    #[serde(with = "ts_seconds")]
    pub last_update_time: DateTime<Utc>,
    #[serde_as(deserialize_as = "DefaultOnNull")]
    pub leaderboard: Leaderboard,
    #[serde_as(deserialize_as = "DefaultOnNull")]
    pub associated_scores_count: u32,
    #[serde_as(deserialize_as = "DefaultOnNull")]
    pub associated_scores: Vec<ClanMapScore>,
}

impl BlApiResponse for BlApiClanRankingResponse {}

impl From<BlApiClanRankingResponse> for List<ClanMapScore> {
    fn from(value: BlApiClanRankingResponse) -> Self {
        Self {
            data: value.associated_scores,
            page: 0,
            items_per_page: 0,
            total: value.associated_scores_count,
        }
    }
}

impl From<BlApiClanRankingResponse> for ClanWithList<ClanMapScore> {
    fn from(value: BlApiClanRankingResponse) -> Self {
        Self {
            clan: value.clan,
            list: List {
                data: value.associated_scores,
                page: 0,
                items_per_page: 0,
                total: value.associated_scores_count,
            },
        }
        // Self {
        //     data: value.associated_scores,
        //     page: 0,
        //     items_per_page: 0,
        //     total: value.associated_scores_count,
        // }
    }
}

impl<'a> ClanResource<'a> {
    pub fn new(client: &'a Client) -> Self {
        Self { client }
    }

    pub async fn clans(&self, params: &[ClanParam]) -> Result<List<Clan>> {
        self.client
            .get_json::<BlApiListResponse<Clan>, List<Clan>, ClanParam>(
                Method::GET,
                "/clans",
                params,
            )
            .await
    }

    pub async fn by_tag(&self, tag: &str) -> Result<Clan> {
        self.client
            .get_json::<BlApiClanContainer<Player>, Clan, ClanPlayersParam>(
                Method::GET,
                format!("/clan/{}", tag).as_str(),
                &[ClanPlayersParam::Count(0)],
            )
            .await
    }

    pub async fn by_id(&self, clan_id: ClanId) -> Result<Clan> {
        self.client
            .get_json::<BlApiClanContainer<Player>, Clan, ClanPlayersParam>(
                Method::GET,
                format!("/clan/id/{}", clan_id).as_str(),
                &[ClanPlayersParam::Count(0)],
            )
            .await
    }

    pub async fn players_by_clan_tag(
        &self,
        tag: &str,
        params: &[ClanPlayersParam],
    ) -> Result<ClanWithList<Player>> {
        self.client
            .get_json::<BlApiClanContainer<Player>, ClanWithList<Player>, ClanPlayersParam>(
                Method::GET,
                format!("/clan/{}", tag).as_str(),
                params,
            )
            .await
    }

    pub async fn players_by_clan_id(
        &self,
        clan_id: ClanId,
        params: &[ClanPlayersParam],
    ) -> Result<ClanWithList<Player>> {
        self.client
            .get_json::<BlApiClanContainer<Player>, ClanWithList<Player>, ClanPlayersParam>(
                Method::GET,
                format!("/clan/id/{}", clan_id).as_str(),
                params,
            )
            .await
    }

    pub async fn maps_by_clan_tag(
        &self,
        tag: &str,
        params: &[ClanMapsParam],
    ) -> Result<ClanWithList<ClanMap>> {
        self.client
            .get_json::<BlApiClanContainer<ClanMap>, ClanWithList<ClanMap>, ClanMapsParam>(
                Method::GET,
                &format!("/clan/{}/maps", tag),
                params,
            )
            .await
    }

    pub async fn maps_by_clan_id(
        &self,
        clan_id: ClanId,
        params: &[ClanMapsParam],
    ) -> Result<ClanWithList<ClanMap>> {
        self.client
            .get_json::<BlApiClanContainer<ClanMap>, ClanWithList<ClanMap>, ClanMapsParam>(
                Method::GET,
                &format!("/clan/id/{}/maps", clan_id),
                params,
            )
            .await
    }

    pub async fn scores_by_clan_map_id(
        &self,
        leaderboard_id: &str,
        clan_map_id: u32,
        params: &[ClanMapParam],
    ) -> Result<ClanWithList<ClanMapScore>> {
        self.client
            .get_json::<BlApiClanRankingResponse, ClanWithList<ClanMapScore>, ClanMapParam>(
                Method::GET,
                &format!(
                    "/leaderboard/clanRankings/{}/{}",
                    leaderboard_id, clan_map_id
                ),
                params,
            )
            .await
    }

    pub async fn scores_by_clan_id(
        &self,
        leaderboard_id: &str,
        clan_id: ClanId,
        params: &[ClanMapParam],
    ) -> Result<ClanWithList<ClanMapScore>> {
        self.client
            .get_json::<BlApiClanRankingResponse, ClanWithList<ClanMapScore>, ClanMapParam>(
                Method::GET,
                &format!(
                    "/leaderboard/clanRankings/{}/clan/{}",
                    leaderboard_id, clan_id
                ),
                params,
            )
            .await
    }
}

pub struct ClanAuthResource<'a, T: OAuthTokenRepository> {
    client: &'a ClientWithOAuth<'a, T>,
}

impl<'a, T: OAuthTokenRepository> ClanAuthResource<'a, T> {
    pub fn new(client: &'a ClientWithOAuth<T>) -> Self {
        Self { client }
    }

    pub async fn invite(&self, player_id: PlayerId) -> Result<()> {
        let builder = self
            .client
            .request_builder(Method::POST, "/clan/invite")
            .query(&[("player", player_id)]);

        self.client.send_authorized_request(builder).await?;

        Ok(())
    }
}

pub(crate) type ClanTag = String;

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct BlApiClanContainer<T> {
    pub data: Vec<T>,
    pub metadata: MetaData,
    pub container: Clan,
}

impl<T> BlApiResponse for BlApiClanContainer<T> {}

impl<In, Out> From<BlApiClanContainer<In>> for List<Out>
where
    In: BlApiResponse + Sized + DeserializeOwned,
    Out: From<In> + Sized,
{
    fn from(value: BlApiClanContainer<In>) -> Self {
        Self {
            data: value.data.into_iter().map(|v| v.into()).collect(),
            page: value.metadata.page,
            items_per_page: value.metadata.items_per_page,
            total: value.metadata.total,
        }
    }
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ClanWithList<T> {
    pub clan: Clan,
    pub list: List<T>,
}

impl<In, Out> From<BlApiClanContainer<In>> for ClanWithList<Out>
where
    In: BlApiResponse + Sized + DeserializeOwned,
    Out: From<In> + Sized,
{
    fn from(value: BlApiClanContainer<In>) -> Self {
        Self {
            clan: value.container,
            list: List {
                data: value.data.into_iter().map(|v| v.into()).collect(),
                page: value.metadata.page,
                items_per_page: value.metadata.items_per_page,
                total: value.metadata.total,
            },
        }
    }
}

pub(crate) type ClanId = u32;

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct Clan {
    pub id: ClanId,
    pub tag: ClanTag,
    #[serde(rename = "leaderID")]
    pub leader_id: PlayerId,
    pub name: String,
    pub description: String,
    pub pp: f64,
    pub average_rank: f64,
    pub average_accuracy: f64,
    pub players_count: u32,
    pub icon: String,
}

impl<T> From<BlApiClanContainer<T>> for Clan {
    fn from(value: BlApiClanContainer<T>) -> Self {
        value.container
    }
}

impl BlApiResponse for Clan {}

#[allow(dead_code)]
#[derive(Clone)]
pub enum ClanSort {
    Pp,
    Acc,
    Rank,
    Players,
    MapsCaptured,
}

#[allow(dead_code)]
#[derive(Clone)]
pub enum ClanParam {
    Page(u32),
    Sort(ClanSort),
    Order(SortOrder),
    Count(u32),
    Search(String),
}

impl QueryParam for ClanParam {
    fn as_query_param(&self) -> (String, String) {
        match self {
            ClanParam::Page(page) => ("page".to_owned(), page.to_string()),
            ClanParam::Sort(field) => (
                "sortBy".to_owned(),
                match field {
                    ClanSort::Pp => "pp".to_owned(),
                    ClanSort::Acc => "acc".to_owned(),
                    ClanSort::Rank => "rank".to_owned(),
                    ClanSort::Players => "count".to_owned(),
                    ClanSort::MapsCaptured => "captures".to_owned(),
                },
            ),
            ClanParam::Order(order) => (
                "order".to_owned(),
                match order {
                    SortOrder::Ascending => "asc".to_owned(),
                    SortOrder::Descending => "desc".to_owned(),
                },
            ),
            ClanParam::Count(count) => ("count".to_owned(), count.to_string()),
            ClanParam::Search(search) => ("search".to_owned(), search.to_string()),
        }
    }
}

#[allow(dead_code)]
#[derive(Clone)]
pub enum ClanPlayersSort {
    Pp,
    Acc,
    Rank,
}

#[allow(dead_code)]
#[derive(Clone)]
pub enum ClanPlayersParam {
    Page(u32),
    Sort(ClanPlayersSort),
    Order(SortOrder),
    Count(u32),
    Primary(bool),
    Search(String),
    CapturedLeaderboards(String),
}

impl QueryParam for ClanPlayersParam {
    fn as_query_param(&self) -> (String, String) {
        match self {
            ClanPlayersParam::Page(page) => ("page".to_owned(), page.to_string()),
            ClanPlayersParam::Sort(field) => (
                "sortBy".to_owned(),
                match field {
                    ClanPlayersSort::Pp => "pp".to_owned(),
                    ClanPlayersSort::Acc => "acc".to_owned(),
                    ClanPlayersSort::Rank => "rank".to_owned(),
                },
            ),
            ClanPlayersParam::Order(order) => (
                "order".to_owned(),
                match order {
                    SortOrder::Ascending => "asc".to_owned(),
                    SortOrder::Descending => "desc".to_owned(),
                },
            ),
            ClanPlayersParam::Count(count) => ("count".to_owned(), count.to_string()),
            ClanPlayersParam::Search(search) => ("search".to_owned(), search.to_string()),
            ClanPlayersParam::Primary(primary) => ("primary".to_owned(), primary.to_string()),
            ClanPlayersParam::CapturedLeaderboards(captured) => {
                ("capturedLeaderboards".to_owned(), captured.to_string())
            }
        }
    }
}
