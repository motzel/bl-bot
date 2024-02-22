use std::collections::HashMap;

use chrono::{DateTime, Duration, Utc};
use futures::future::BoxFuture;
use poise::async_trait;
use reqwest::{IntoUrl, Method, RequestBuilder, Response as ReqwestResponse, Url};
use serde::{Deserialize, Serialize};
use tracing::{debug, error, trace};

use crate::beatleader;
use crate::beatleader::clan::{ClanAuthResource, ClanResource};
use crate::beatleader::error::Error;
use crate::beatleader::player::PlayerResource;
use crate::beatleader::Client;

pub struct OauthResource<'a, T: OAuthTokenRepository> {
    client: &'a ClientWithOAuth<'a, T>,
}

impl<'a, T: OAuthTokenRepository> OauthResource<'a, T> {
    pub fn new(client: &'a ClientWithOAuth<T>) -> Self {
        Self { client }
    }

    pub fn authorize_url(&self, scopes: Vec<OAuthScope>) -> Option<String> {
        let request = OAuthGrant::Authorize(scopes)
            .get_request_builder(self.client)
            .build()
            .ok()?;

        Some(request.url().to_string())
    }

    pub async fn access_token(&self, code: &str) -> beatleader::Result<OAuthToken> {
        let access_token = self
            .send_oauth_request(&OAuthGrant::AuthorizationCode(code.to_owned()))
            .await?;

        Ok(access_token)
    }

    pub async fn access_token_and_store(&self, code: &str) -> beatleader::Result<OAuthToken> {
        let access_token = self.access_token(code).await?;

        self.client.store_token(access_token.clone()).await
    }

    pub async fn refresh_token(&self, refresh_token: &str) -> beatleader::Result<OAuthToken> {
        self.send_oauth_request(&OAuthGrant::RefreshToken(refresh_token.to_owned()))
            .await
    }

    pub async fn refresh_token_and_store(
        &self,
        refresh_token: &str,
    ) -> beatleader::Result<OAuthToken> {
        let access_token = self.refresh_token(refresh_token).await?;

        self.client.store_token(access_token.clone()).await?;

        Ok(access_token)
    }

    async fn send_oauth_request(&self, oauth_grant: &OAuthGrant) -> beatleader::Result<OAuthToken> {
        let request = oauth_grant.get_request_builder(self.client).build();

        if let Err(err) = request {
            error!("OAuth grant builder error: {}", err);

            return Err(Error::Request(err));
        }

        let request = request.unwrap();

        let base = Url::parse(self.client.client.base_url.as_str()).unwrap();

        debug!(
            "Sending OAuth request to {}",
            base.make_relative(request.url()).unwrap()
        );

        match self.client.client.send_request(request).await {
            Ok(response) => {
                debug!(
                    "Endpoint {} responded with status: {}",
                    base.make_relative(response.url()).unwrap(),
                    response.status().as_u16()
                );

                match response.json::<OAuthTokenResponse>().await {
                    Ok(oauth_token_response) => {
                        trace!("OAuth token retrieved from response");

                        Ok(oauth_token_response.into())
                    }
                    Err(e) => {
                        error!("OAuth token retrieving from response error: {}", e);

                        Err(Error::JsonDecode(e))
                    }
                }
            }
            Err(e) => {
                error!("Response error: {:#?}", e);

                match e {
                    Error::Client(error_text) => match error_text {
                        Some(error_text) => Err(Error::OAuth(
                            serde_json::from_str::<OAuthErrorResponse>(error_text.as_str()).ok(),
                        )),
                        None => Err(Error::OAuth(None)),
                    },
                    _ => Err(e),
                }
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct OAuthAppCredentials {
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
    pub error: String,
    pub error_description: String,
    pub error_uri: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct OAuthToken {
    access_token: String,
    token_type: String,
    expiration_date: DateTime<Utc>,
    scopes: Vec<OAuthScope>,
    refresh_token: Option<String>,
}

impl OAuthToken {
    pub fn get_expiration(&self) -> DateTime<Utc> {
        self.expiration_date
    }

    pub fn is_valid_for(&self, duration: Duration) -> bool {
        self.expiration_date.ge(&(Utc::now() + duration))
    }

    pub fn is_newer_than(&self, other_token: &OAuthToken) -> bool {
        self.expiration_date.gt(&other_token.expiration_date)
    }
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
    pub fn get_request_builder<T: OAuthTokenRepository>(
        &self,
        client: &ClientWithOAuth<T>,
    ) -> RequestBuilder {
        trace!(
            "Getting OAuth grant request builder for client {}...",
            &client.oauth_credentials.client_id
        );

        let mut params = HashMap::from([
            ("client_id", client.oauth_credentials.client_id.clone()),
            (
                "redirect_uri",
                client.oauth_credentials.redirect_uri.clone(),
            ),
        ]);

        match self {
            OAuthGrant::Authorize(scopes) => {
                let scopes_str = scopes
                    .iter()
                    .map(String::from)
                    .collect::<Vec<_>>()
                    .join(" ");

                trace!("Authorize grant selected with scopes {}", &scopes_str);

                params.extend(HashMap::from([
                    ("response_type", "code".to_owned()),
                    ("scope", scopes_str),
                ]));

                client
                    .client
                    .request_builder(Method::GET, "/oauth2/authorize")
                    .query(&params)
            }
            OAuthGrant::AuthorizationCode(auth_code) => {
                trace!("Authorization code grant selected");

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
                trace!("Refresh token grant selected");

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

#[async_trait]
pub trait OAuthTokenRepository: Sync + Send + Clone + 'static {
    async fn get(&self) -> Result<Option<OAuthToken>, Error>;
    async fn store<ModifyFunc>(&self, modify_func: ModifyFunc) -> Result<OAuthToken, Error>
    where
        ModifyFunc: for<'b> FnOnce(&'b mut OAuthToken) -> BoxFuture<'b, ()> + Send + 'static;
}

pub struct ClientWithOAuth<'a, T>
where
    T: OAuthTokenRepository,
{
    client: &'a Client,
    oauth_credentials: OAuthAppCredentials,
    oauth_token_repository: T,
}

impl<'a, T> ClientWithOAuth<'a, T>
where
    T: OAuthTokenRepository,
{
    pub fn new(
        client: &Client,
        oauth_credentials: OAuthAppCredentials,
        oauth_token_repository: T,
    ) -> ClientWithOAuth<'_, T> {
        ClientWithOAuth {
            client,
            oauth_credentials,
            oauth_token_repository,
        }
    }
    pub fn oauth(&self) -> OauthResource<T> {
        OauthResource::new(self)
    }

    pub fn player(&self) -> PlayerResource {
        PlayerResource::new(self.client)
    }

    pub fn clan(&self) -> ClanResource {
        ClanResource::new(self.client)
    }

    pub fn clan_auth(&self) -> ClanAuthResource<T> {
        ClanAuthResource::new(self)
    }

    pub(crate) fn request_builder<U: IntoUrl>(
        &self,
        method: Method,
        endpoint: U,
    ) -> RequestBuilder {
        self.client.request_builder(method, endpoint)
    }

    pub async fn send_authorized_request(
        &self,
        builder: RequestBuilder,
    ) -> super::Result<ReqwestResponse> {
        let oauth_token = self.refresh_token_if_needed().await?;

        trace!("Sending authorized refresh...");

        self.client
            .build_and_send_request(builder.header(
                "Authorization",
                format!("Bearer {}", oauth_token.access_token),
            ))
            .await
    }

    pub async fn get_token(&self) -> Result<Option<OAuthToken>, Error> {
        self.oauth_token_repository.get().await
    }

    pub async fn store_token(&self, oauth_token: OAuthToken) -> Result<OAuthToken, Error> {
        self.oauth_token_repository
            .store(|token| {
                Box::pin(async move {
                    if oauth_token.is_newer_than(token) {
                        *token = oauth_token;
                    }
                })
            })
            .await
    }

    pub async fn refresh_token_if_needed(&self) -> super::Result<OAuthToken> {
        let Some(oauth_token) = self.get_token().await? else {
            return Err(Error::OAuthStorage);
        };

        trace!("OAuth token retrieved from repository");

        if oauth_token.is_valid_for(Duration::seconds(self.client.get_timeout() as i64 + 30)) {
            trace!("OAuth token is valid, skipping refreshing.");

            return Ok(oauth_token);
        }

        if oauth_token.refresh_token.is_none() {
            trace!("No refresh token, skip refreshing.");

            return Err(Error::OAuthStorage);
        }

        let oauth_credentials = self.oauth_credentials.clone();
        let oauth_token_repository = self.oauth_token_repository.clone();

        trace!("Trying to refresh expired OAuth token...");

        let oauth_token = self
            .oauth_token_repository
            .store(move |token| {
                Box::pin(async move {
                    // refresh only if the token has not changed in the meantime
                    if token.access_token == oauth_token.access_token {
                        let new_token_result = Client::default()
                            .with_oauth(oauth_credentials, oauth_token_repository)
                            .oauth()
                            .refresh_token(oauth_token.refresh_token.as_ref().unwrap())
                            .await;

                        if let Ok(new_token) = new_token_result {
                            *token = new_token;
                        }
                    }
                })
            })
            .await?;

        debug!("OAuth token refreshed.");

        Ok(oauth_token)
    }
}
