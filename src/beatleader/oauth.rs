use crate::beatleader;
use crate::beatleader::error::Error;
use crate::beatleader::{
    ClientWithOAuth, OAuthErrorResponse, OAuthGrant, OAuthScope, OAuthToken, OAuthTokenResponse,
};

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
