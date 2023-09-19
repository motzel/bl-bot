use std::collections::HashMap;

use chrono::{DateTime, Utc};
use reqwest::{Method, RequestBuilder};
use serde::{Deserialize, Serialize};

use crate::beatleader;
use crate::beatleader::error::Error;
use crate::beatleader::Client;

pub struct OauthResource<'a> {
    client: &'a ClientWithOAuth<'a>,
}

impl<'a> OauthResource<'a> {
    pub fn new(client: &'a ClientWithOAuth) -> Self {
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
        self.send_oauth_request(&OAuthGrant::AuthorizationCode(code.to_owned()))
            .await
    }

    pub async fn refresh_token(&self, refresh_token: &str) -> beatleader::Result<OAuthToken> {
        self.send_oauth_request(&OAuthGrant::RefreshToken(refresh_token.to_owned()))
            .await
    }

    async fn send_oauth_request(&self, oauth_grant: &OAuthGrant) -> beatleader::Result<OAuthToken> {
        let request = oauth_grant.get_request_builder(self.client).build();

        if let Err(err) = request {
            return Err(Error::Request(err));
        }

        match self.client.client.send_request(request.unwrap()).await {
            Ok(response) => match response.json::<OAuthTokenResponse>().await {
                Ok(oauth_token_response) => Ok(oauth_token_response.into()),
                Err(e) => Err(Error::JsonDecode(e)),
            },
            Err(e) => match e {
                Error::Client(error_text) => match error_text {
                    Some(error_text) => Err(Error::OAuth(
                        serde_json::from_str::<OAuthErrorResponse>(error_text.as_str()).ok(),
                    )),
                    None => Err(Error::OAuth(None)),
                },
                _ => Err(e),
            },
        }
    }
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
    pub error: String,
    pub error_description: String,
    pub error_uri: String,
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
    pub fn new(client: &Client, oauth_credentials: OAuthCredentials) -> ClientWithOAuth<'_> {
        ClientWithOAuth {
            client,
            oauth_credentials,
        }
    }
    pub fn oauth(&self) -> OauthResource {
        OauthResource::new(self)
    }
}
