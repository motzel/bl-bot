use std::num::NonZeroU32;
use std::time::Duration;

use governor::clock::DefaultClock;
use governor::middleware::NoOpMiddleware;
use governor::state::{InMemoryState, NotKeyed};
use governor::{Jitter, Quota, RateLimiter};
use log::{debug, error, info};
use reqwest::{Client as HttpClient, IntoUrl, Method, Request, RequestBuilder, Response, Url};

use player::PlayerRequest;

use crate::beatleader::error::Error;
use crate::beatleader::error::Error::{
    Client as ClientError, Network, NotFound, Request as RequestError, Server, Unknown,
};

pub mod error;
pub mod player;
pub mod pp;

pub type Result<T> = std::result::Result<T, Error>;

const DEFAULT_API_URL: &str = "https://api.beatleader.xyz";

static APP_USER_AGENT: &str = concat!(
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

    pub async fn get<U: IntoUrl>(&self, endpoint: U) -> Result<Response> {
        let request = self
            .request_builder(Method::GET, endpoint, self.timeout)
            .build();

        if let Err(err) = request {
            return Err(RequestError(err));
        }

        self.send_request(request.unwrap()).await
    }

    pub async fn send_request(&self, request: Request) -> Result<Response> {
        debug!("Waiting for rate limiter...");

        self.rate_limiter
            .until_ready_with_jitter(Jitter::up_to(Duration::from_millis(100)))
            .await;

        debug!("Got permit from rate limiter.");

        let base = Url::parse(self.base_url.as_str()).unwrap();

        debug!(
            "Sending request to {}",
            base.make_relative(request.url()).unwrap()
        );

        let response = self.http_client.execute(request).await;

        match response {
            Err(err) => {
                error!("Response error: {:#?}", err);

                Err(Network(err))
            }
            Ok(response) => {
                let base = Url::parse(self.base_url.as_str()).unwrap();

                debug!(
                    "Endpoint {} responded with status: {}",
                    base.make_relative(response.url()).unwrap(),
                    response.status().as_u16()
                );

                match response.status().as_u16() {
                    200..=299 => Ok(response),
                    404 => Err(NotFound),
                    400..=499 => Err(ClientError),
                    500..=599 => Err(Server),
                    _ => Err(Unknown),
                }
            }
        }
    }

    pub fn player(&self) -> PlayerRequest {
        PlayerRequest::new(self)
    }

    pub(crate) fn request_builder<U: IntoUrl>(
        &self,
        method: Method,
        endpoint: U,
        timeout: u64,
    ) -> RequestBuilder {
        let full_url = self.base_url.to_owned() + endpoint.as_str();

        self.http_client
            .request(method, full_url)
            .timeout(Duration::from_secs(timeout))
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
