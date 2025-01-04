use std::fmt::Display;
use std::future::Future;
use std::num::NonZeroU32;
use std::result;
use std::time::Duration;

use governor::clock::DefaultClock;
use governor::middleware::NoOpMiddleware;
use governor::state::{InMemoryState, NotKeyed};
use governor::{Jitter, Quota, RateLimiter};
use reqwest::{
    Client as HttpClient, IntoUrl, Method, Request, RequestBuilder, Response as ReqwestResponse,
    Url,
};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use tracing::{debug, error, info, trace};

use player::PlayerResource;

use crate::beatleader::clan::ClanResource;
use crate::beatleader::error::Error;
use crate::beatleader::oauth::{ClientWithOAuth, OAuthAppCredentials, OAuthTokenRepository};
use crate::beatleader::rating::AiRatingsResource;

pub mod clan;
pub mod error;
pub mod oauth;
pub mod player;
pub mod pp;
pub mod rating;

pub type Result<T> = std::result::Result<T, Error>;

const DEFAULT_API_URL: &str = "https://api.beatleader.com";

pub static APP_USER_AGENT: &str = concat!(
    env!("CARGO_PKG_NAME"),
    "/",
    env!("CARGO_PKG_VERSION"),
    " <https://github.com/motzel/bl-bot>"
);

pub struct Client {
    base_url: String,
    http_client: HttpClient,
    timeout: u64,
    rate_limiter: RateLimiter<NotKeyed, InMemoryState, DefaultClock, NoOpMiddleware>,
}

impl Client {
    pub fn new(base_url: String, timeout: u64) -> Self {
        info!(
            "Initialize client with URL {} and timeout {}s. Identify myself as {}",
            base_url, timeout, APP_USER_AGENT
        );

        Self {
            base_url,
            http_client: reqwest::Client::builder()
                .https_only(true)
                .gzip(true)
                .brotli(true)
                .user_agent(APP_USER_AGENT)
                .build()
                .unwrap(),
            timeout,
            rate_limiter: RateLimiter::direct(Quota::per_second(NonZeroU32::new(10u32).unwrap())),
        }
    }

    pub fn player(&self) -> PlayerResource {
        PlayerResource::new(self)
    }

    pub fn clan(&self) -> ClanResource {
        ClanResource::new(self)
    }

    pub fn ai_ratings(&self) -> AiRatingsResource {
        AiRatingsResource::new(self)
    }

    pub fn with_timeout(&self, timeout: u64) -> Client {
        Client::new(self.base_url.clone(), timeout)
    }

    pub fn with_oauth<T: OAuthTokenRepository>(
        &self,
        oauth_credentials: OAuthAppCredentials,
        oauth_token_repository: T,
    ) -> ClientWithOAuth<T> {
        ClientWithOAuth::new(self, oauth_credentials, oauth_token_repository)
    }

    pub async fn get<U: IntoUrl>(&self, endpoint: U) -> Result<ReqwestResponse> {
        let request = self.request_builder(Method::GET, endpoint).build();

        if let Err(err) = request {
            return Err(Error::Request(err));
        }

        self.send_request(request.unwrap()).await
    }

    async fn get_json<
        In: BlApiResponse + Sized + DeserializeOwned,
        Out: From<In> + Sized,
        Param: QueryParam,
    >(
        &self,
        method: Method,
        endpoint: &str,
        params: &[Param],
    ) -> Result<Out> {
        let request = self
            .request_builder(method, endpoint)
            .query(
                &(params
                    .iter()
                    .map(|param| param.as_query_param())
                    .collect::<Vec<(String, String)>>()),
            )
            .build();

        if let Err(err) = request {
            return Err(Error::Request(err));
        }

        match self.send_request(request.unwrap()).await {
            Ok(response) => match response.json::<In>().await {
                Ok(clans) => Ok(clans.into()),
                Err(e) => Err(Error::JsonDecode(e)),
            },
            Err(e) => Err(e),
        }
    }

    async fn build_and_send_request(&self, builder: RequestBuilder) -> Result<ReqwestResponse> {
        match builder.build() {
            Ok(request) => self.send_request(request).await,
            Err(err) => Err(Error::Request(err)),
        }
    }

    pub async fn send_request(&self, request: Request) -> Result<ReqwestResponse> {
        trace!("Waiting for rate limiter...");

        self.rate_limiter
            .until_ready_with_jitter(Jitter::up_to(Duration::from_millis(100)))
            .await;

        trace!("Got permit from rate limiter.");

        let base = Url::parse(self.base_url.as_str()).unwrap();

        let request_url = request.url();
        let relative_url = if request_url.as_str().starts_with(&self.base_url) {
            base.make_relative(request_url)
                .unwrap_or("Unknown URL".to_owned())
        } else {
            request_url.to_string()
        };

        trace!("Sending request to {}", &relative_url);

        let response = self.http_client.execute(request).await;

        match response {
            Err(err) => {
                error!("Response error: {:#?}", err);

                Err(Error::Network(err))
            }
            Ok(response) => {
                debug!(
                    "Endpoint {} responded with status: {}",
                    &relative_url,
                    response.status().as_u16()
                );

                match response.status().as_u16() {
                    204 => Err(Error::NoContent),
                    200..=299 => Ok(response),
                    401 | 403 => Err(Error::Unauthorized),
                    404 => Err(Error::NotFound),
                    400..=499 => Err(Error::Client(
                        response.text_with_charset("utf-8").await.ok(),
                    )),
                    500..=599 => Err(Error::Server),
                    _ => Err(Error::Unknown),
                }
            }
        }
    }

    pub(crate) fn request_builder<U: IntoUrl>(
        &self,
        method: Method,
        endpoint: U,
    ) -> RequestBuilder {
        let full_url = self.base_url.to_owned() + endpoint.as_str();

        self.http_client
            .request(method, full_url)
            .timeout(Duration::from_secs(self.timeout))
    }

    pub(crate) fn get_timeout(&self) -> u64 {
        self.timeout
    }

    pub(crate) fn get_http_client(&self) -> &HttpClient {
        &self.http_client
    }
}

impl Default for Client {
    fn default() -> Self {
        Self::new(DEFAULT_API_URL.to_string(), 30)
    }
}

pub trait BlApiResponse: Sized {}

#[allow(dead_code)]
#[derive(Clone)]
pub enum SortOrder {
    Ascending,
    Descending,
}

impl Display for SortOrder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                SortOrder::Ascending => "asc",
                SortOrder::Descending => "desc",
            }
        )
    }
}

#[allow(dead_code)]
#[derive(Serialize, Deserialize, Default, Debug, Clone)]
pub enum BlContext {
    #[default]
    #[serde(rename = "general")]
    General,
    #[serde(rename = "nomods")]
    NoModifiers,
    #[serde(rename = "nopause")]
    NoPauses,
    #[serde(rename = "golf")]
    Golf,
}

impl Display for BlContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                BlContext::General => "general".to_owned(),
                BlContext::NoModifiers => "nomods".to_owned(),
                BlContext::NoPauses => "nopause".to_owned(),
                BlContext::Golf => "golf".to_owned(),
            }
        )
    }
}

pub trait QueryParam {
    fn as_query_param(&self) -> (String, String);
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct MetaData {
    pub items_per_page: u32,
    pub page: u32,
    pub total: u32,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct BlApiListResponse<T> {
    pub data: Vec<T>,
    pub metadata: MetaData,
}

impl<T> BlApiListResponse<T> {
    pub fn get_data(&self) -> &Vec<T> {
        &self.data
    }

    pub fn get_metadata(&self) -> &MetaData {
        &self.metadata
    }
}

impl<T> BlApiResponse for BlApiListResponse<T> {}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct List<T> {
    pub data: Vec<T>,
    pub page: u32,
    pub items_per_page: u32,
    pub total: u32,
}

impl<In, Out> From<BlApiListResponse<In>> for List<Out>
where
    In: BlApiResponse + Sized + DeserializeOwned,
    Out: From<In> + Sized,
{
    fn from(value: BlApiListResponse<In>) -> Self {
        Self {
            data: value.data.into_iter().map(|v| v.into()).collect(),
            page: value.metadata.page,
            items_per_page: value.metadata.items_per_page,
            total: value.metadata.total,
        }
    }
}

#[derive(Debug, Clone)]
pub struct PageDef {
    pub page: u32,
    pub items_per_page: u32,
}

#[derive(Debug)]
pub struct DataWithMeta<T: Sized, O: Sized> {
    pub data: Vec<T>,
    pub items_per_page: Option<u32>,
    pub total: Option<u32>,
    pub other_data: Option<O>,
}

pub async fn fetch_paged_items<T, O, F, Fut>(
    requested_items_per_page: u32,
    items_count: Option<u32>,
    func: F,
) -> result::Result<DataWithMeta<T, O>, Error>
where
    T: Sized + std::fmt::Debug,
    O: Sized + std::fmt::Debug,
    F: Fn(PageDef) -> Fut,
    Fut: Future<Output = result::Result<DataWithMeta<T, O>, Error>>,
{
    if items_count.is_some() && items_count.unwrap() == 0 {
        return Ok(DataWithMeta {
            data: vec![],
            items_per_page: None,
            total: None,
            other_data: None,
        });
    }

    let mut data = DataWithMeta {
        data: Vec::with_capacity(items_count.unwrap_or(10.max(requested_items_per_page)) as usize),
        items_per_page: Some(requested_items_per_page),
        total: None,
        other_data: None,
    };

    let mut page_def = PageDef {
        page: 1,
        items_per_page: requested_items_per_page,
    };
    let mut total = u32::MAX;

    loop {
        let page_data = func(page_def.clone()).await?;

        let page_is_empty = page_data.data.is_empty();

        data.data.extend(page_data.data);

        page_def.page += 1;

        if let Some(actual_items_per_page) = page_data.items_per_page {
            page_def.items_per_page = actual_items_per_page;
            data.items_per_page = Some(actual_items_per_page);
        }

        if let Some(actual_total) = page_data.total {
            total = actual_total;
            data.total = Some(actual_total);
        }

        data.other_data = page_data.other_data;

        let total_pages = if page_def.items_per_page > 0 {
            total.div_ceil(page_def.items_per_page)
        } else {
            0
        };

        if page_is_empty
            || page_def.page > total_pages
            || data.data.len() as u32 >= items_count.unwrap_or(u32::MAX)
        {
            return Ok(if let Some(items_count) = items_count {
                DataWithMeta {
                    data: data
                        .data
                        .into_iter()
                        .take(items_count as usize)
                        .collect::<Vec<_>>(),
                    ..data
                }
            } else {
                data
            });
        }
    }
}
