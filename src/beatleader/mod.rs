use std::collections::HashMap;
use std::num::NonZeroU32;
use std::time::Duration;

use chrono::{DateTime, Utc};
use governor::clock::DefaultClock;
use governor::middleware::NoOpMiddleware;
use governor::state::{InMemoryState, NotKeyed};
use governor::{Jitter, Quota, RateLimiter};
use log::{error, info, trace};
use reqwest::{Client as HttpClient, IntoUrl, Method, Request, RequestBuilder, Response, Url};
use serde::{Deserialize, Serialize};

use player::PlayerRequest;

use crate::beatleader::error::Error;
use crate::beatleader::oauth::OauthRequest;

pub mod error;
mod oauth;
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

    pub fn with_oauth(&self, oauth_credentials: OAuthCredentials) -> ClientWithOAuth {
        ClientWithOAuth {
            client: self,
            oauth_credentials,
        }
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

    pub fn player(&self) -> PlayerRequest {
        PlayerRequest::new(self)
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

#[derive(Debug, Clone)]
pub struct OAuthCredentials {
    pub client_id: String,
    pub client_secret: String,
    pub redirect_uri: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum OAuthScope {
    Profile,
    OfflineAccess,
    Clan,
}

impl TryFrom<&str> for OAuthScope {
    type Error = &'static str;

    fn try_from(value: &str) -> std::result::Result<Self, Self::Error> {
        match value {
            "profile" => Ok(OAuthScope::Profile),
            "offline_access" => Ok(OAuthScope::OfflineAccess),
            "clan" => Ok(OAuthScope::Clan),
            _ => Err("invalid scope"),
        }
    }
}

impl From<&OAuthScope> for String {
    fn from(value: &OAuthScope) -> Self {
        match value {
            OAuthScope::Profile => "profile".to_owned(),
            OAuthScope::OfflineAccess => "offline_access".to_owned(),
            OAuthScope::Clan => "clan".to_owned(),
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
pub struct OAuthTokenResponse {
    access_token: String,
    token_type: String,
    expires_in: u32,
    scope: String,
    refresh_token: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct OAuthErrorResponse {
    error: String,
    error_description: String,
    error_uri: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct OAuthToken {
    access_token: String,
    token_type: String,
    expiration_date: DateTime<Utc>,
    scopes: Vec<OAuthScope>,
    refresh_token: Option<String>,
}

impl From<OAuthTokenResponse> for OAuthToken {
    fn from(value: OAuthTokenResponse) -> Self {
        OAuthToken {
            access_token: value.access_token,
            token_type: value.token_type,
            expiration_date: Utc::now()
                .checked_add_signed(chrono::Duration::seconds(value.expires_in.into()))
                .unwrap(),
            scopes: value
                .scope
                .split(' ')
                .filter_map(|v| OAuthScope::try_from(v).ok())
                .collect(),
            refresh_token: value.refresh_token,
        }
    }
}

pub enum OAuthGrant {
    Authorize(Vec<OAuthScope>),
    AuthorizationCode(String),
    RefreshToken(String),
}

impl OAuthGrant {
    pub fn get_request_builder(&self, client: &ClientWithOAuth) -> RequestBuilder {
        let mut params = HashMap::from([
            ("client_id", client.oauth_credentials.client_id.clone()),
            (
                "redirect_uri",
                client.oauth_credentials.redirect_uri.clone(),
            ),
        ]);

        match self {
            OAuthGrant::Authorize(scopes) => {
                params.extend(HashMap::from([
                    ("response_type", "code".to_owned()),
                    (
                        "scope",
                        scopes
                            .iter()
                            .map(String::from)
                            .collect::<Vec<_>>()
                            .join(" "),
                    ),
                ]));

                client
                    .client
                    .request_builder(Method::GET, "/oauth2/authorize")
                    .query(&params)
            }
            OAuthGrant::AuthorizationCode(auth_code) => {
                params.extend(HashMap::from([
                    ("code", auth_code.clone()),
                    ("grant_type", "authorization_code".to_owned()),
                    (
                        "client_secret",
                        client.oauth_credentials.client_secret.clone(),
                    ),
                ]));

                client
                    .client
                    .request_builder(Method::POST, "/oauth2/token")
                    .form(&params)
            }
            OAuthGrant::RefreshToken(refresh_token) => {
                params.extend(HashMap::from([
                    ("refresh_token", refresh_token.clone()),
                    ("grant_type", "refresh_token".to_owned()),
                    (
                        "client_secret",
                        client.oauth_credentials.client_secret.clone(),
                    ),
                ]));

                client
                    .client
                    .request_builder(Method::POST, "/oauth2/token")
                    .form(&params)
            }
        }
    }
}

pub struct ClientWithOAuth<'a> {
    client: &'a Client,
    oauth_credentials: OAuthCredentials,
}

impl ClientWithOAuth<'_> {
    pub fn oauth(&self) -> OauthRequest {
        OauthRequest::new(self)
    }
}
