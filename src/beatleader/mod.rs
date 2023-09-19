use std::num::NonZeroU32;
use std::time::Duration;

use governor::clock::DefaultClock;
use governor::middleware::NoOpMiddleware;
use governor::state::{InMemoryState, NotKeyed};
use governor::{Jitter, Quota, RateLimiter};
use log::{error, info, trace};
use reqwest::{Client as HttpClient, IntoUrl, Method, Request, RequestBuilder, Response, Url};

use crate::beatleader::clan::ClanResource;
use player::PlayerResource;

use crate::beatleader::error::Error;
use crate::beatleader::oauth::{ClientWithOAuth, OAuthCredentials};

pub mod clan;
pub mod error;
pub mod oauth;
pub mod player;
pub mod pp;

pub type Result<T> = std::result::Result<T, Error>;

const DEFAULT_API_URL: &str = "https://api.beatleader.xyz";

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

    pub fn with_oauth(&self, oauth_credentials: OAuthCredentials) -> ClientWithOAuth {
        ClientWithOAuth::new(self, oauth_credentials)
    }

    pub async fn get<U: IntoUrl>(&self, endpoint: U) -> Result<Response> {
        let request = self.request_builder(Method::GET, endpoint).build();

        if let Err(err) = request {
            return Err(Error::Request(err));
        }

        self.send_request(request.unwrap()).await
    }

    pub async fn send_request(&self, request: Request) -> Result<Response> {
        trace!("Waiting for rate limiter...");

        self.rate_limiter
            .until_ready_with_jitter(Jitter::up_to(Duration::from_millis(100)))
            .await;

        trace!("Got permit from rate limiter.");

        let base = Url::parse(self.base_url.as_str()).unwrap();

        trace!(
            "Sending request to {}",
            base.make_relative(request.url()).unwrap()
        );

        let response = self.http_client.execute(request).await;

        match response {
            Err(err) => {
                error!("Response error: {:#?}", err);

                Err(Error::Network(err))
            }
            Ok(response) => {
                let base = Url::parse(self.base_url.as_str()).unwrap();

                trace!(
                    "Endpoint {} responded with status: {}",
                    base.make_relative(response.url()).unwrap(),
                    response.status().as_u16()
                );

                match response.status().as_u16() {
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
}

impl Default for Client {
    fn default() -> Self {
        Self::new(DEFAULT_API_URL.to_string(), 30)
    }
}

#[allow(dead_code)]
#[derive(Clone)]
pub enum SortOrder {
    Ascending,
    Descending,
}

impl ToString for SortOrder {
    fn to_string(&self) -> String {
        match self {
            SortOrder::Ascending => "asc".to_owned(),
            SortOrder::Descending => "desc".to_owned(),
        }
    }
}

pub trait QueryParam {
    fn as_query_param(&self) -> (String, String);
}
