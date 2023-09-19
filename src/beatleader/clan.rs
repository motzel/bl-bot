use reqwest::Method;
use serde::Deserialize;

use crate::beatleader::player::PlayerId;
use crate::beatleader::{BlApiListResponse, BlApiResponse, Client, QueryParam, Result, SortOrder};

pub struct ClanResource<'a> {
    client: &'a Client,
}

impl<'a> ClanResource<'a> {
    pub fn new(client: &'a Client) -> Self {
        Self { client }
    }

    pub async fn search(&self, params: &[ClanParam]) -> Result<BlApiListResponse<Clan>> {
        self.client
            .get_json::<BlApiListResponse<BlApiClan>, BlApiListResponse<Clan>, ClanParam>(
                Method::GET,
                "/clans",
                params,
            )
            .await
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

impl From<BlApiListResponse<BlApiClan>> for BlApiListResponse<Clan> {
    fn from(value: BlApiListResponse<BlApiClan>) -> Self {
        Self {
            data: value.data.into_iter().map(|v| v.into()).collect(),
            metadata: value.metadata,
        }
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
