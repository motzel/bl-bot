use chrono::serde::ts_seconds;
use chrono::{DateTime, Utc};
use reqwest::Method;
use serde::Deserialize;
use serde_with::{serde_as, DefaultOnNull};

use crate::beatleader::oauth::{ClientWithOAuth, OAuthTokenRepository};
use crate::beatleader::player::{Leaderboard, Player as BlPlayer, PlayerId};
use crate::beatleader::{
    BlApiListResponse, BlApiResponse, BlContext, Client, List, MetaData, QueryParam, Result,
    SortOrder,
};

pub struct ClanResource<'a> {
    client: &'a Client,
}

#[allow(dead_code)]
#[derive(Clone)]
pub enum ClanScoreSort {
    Date,
    Pp,
    Acc,
    Rank,
    ToHold,
    ToConquer,
}

#[allow(dead_code)]
#[derive(Clone)]
pub enum ClanScoreParam {
    Page(u32),
    Count(u32),
    Sort(ClanScoreSort),
    Order(SortOrder),
    Context(BlContext),
}

impl QueryParam for ClanScoreParam {
    fn as_query_param(&self) -> (String, String) {
        match self {
            ClanScoreParam::Page(page) => ("page".to_owned(), page.to_string()),
            ClanScoreParam::Count(count) => ("count".to_owned(), count.to_string()),
            ClanScoreParam::Sort(field) => (
                "sortBy".to_owned(),
                match field {
                    ClanScoreSort::Date => "date".to_owned(),
                    ClanScoreSort::Pp => "pp".to_owned(),
                    ClanScoreSort::Acc => "acc".to_owned(),
                    ClanScoreSort::Rank => "rank".to_owned(),
                    ClanScoreSort::ToHold => "tohold".to_owned(),
                    ClanScoreSort::ToConquer => "toconquer".to_owned(),
                },
            ),
            ClanScoreParam::Order(order) => ("order".to_owned(), order.to_string()),
            ClanScoreParam::Context(context) => {
                ("leaderboardContext".to_owned(), context.to_string())
            }
        }
    }
}

#[serde_as]
#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Score {
    pub id: u32,
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

impl BlApiResponse for Score {}

impl<'a> ClanResource<'a> {
    pub fn new(client: &'a Client) -> Self {
        Self { client }
    }

    pub async fn search(&self, params: &[ClanParam]) -> Result<List<Clan>> {
        self.client
            .get_json::<BlApiListResponse<BlApiClan>, List<Clan>, ClanParam>(
                Method::GET,
                "/clans",
                params,
            )
            .await
    }

    pub async fn by_tag(&self, tag: &str) -> Result<Clan> {
        self.client
            .get_json::<BlApiClanContainer, Clan, ClanParam>(
                Method::GET,
                format!("/clan/{}", tag).as_str(),
                &[ClanParam::Count(0)],
            )
            .await
    }

    pub async fn maps(&self, tag: &str, params: &[ClanScoreParam]) -> Result<List<Score>> {
        self.client
            .get_json::<BlApiListResponse<Score>, List<Score>, ClanScoreParam>(
                Method::GET,
                &format!("/clan/{}/maps", tag),
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
pub struct BlApiClan {
    pub id: u32,
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

impl BlApiResponse for BlApiClan {}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct BlApiClanContainer {
    pub data: Vec<BlPlayer>,
    pub metadata: MetaData,
    pub container: BlApiClan,
}

impl BlApiResponse for BlApiClanContainer {}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Clan {
    pub id: u32,
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

impl From<BlApiClan> for Clan {
    fn from(value: BlApiClan) -> Self {
        Clan {
            id: value.id,
            tag: value.tag,
            leader_id: value.leader_id,
            name: value.name,
            description: value.description,
            pp: value.pp,
            average_rank: value.average_rank,
            average_accuracy: value.average_accuracy * 100.0,
            players_count: value.players_count,
            icon: value.icon,
        }
    }
}

impl From<BlApiClanContainer> for Clan {
    fn from(value: BlApiClanContainer) -> Self {
        value.container.into()
    }
}

#[allow(dead_code)]
#[derive(Clone)]
pub enum ClanSort {
    Pp,
    Acc,
    Rank,
    Players,
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
