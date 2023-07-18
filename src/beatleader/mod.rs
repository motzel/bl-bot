use log::{debug, error, info};
use reqwest::{Client as HttpClient, Response};

use player::PlayerRequest;

use crate::beatleader::error::BlError;
use crate::beatleader::error::BlError::{
    ClientError, NetworkError, NotFound, ServerError, UnknownError,
};

pub mod error;
pub mod player;

pub type Result<T> = std::result::Result<T, BlError>;

const DEFAULT_API_URL: &str = "https://api.beatleader.xyz";

pub struct Client {
    base_url: String,
    http_client: HttpClient,
}

impl Client {
    pub fn new(base_url: String) -> Self {
        info!("Initialize client with URL {}", base_url);

        Self {
            base_url,
            http_client: reqwest::Client::new(),
        }
    }

    pub async fn send_get_request(&self, url: &str) -> Result<Response> {
        debug!("Sending request to {}", url);

        let response = self
            .http_client
            .get(self.base_url.to_owned() + url)
            .send()
            .await;

        match response {
            Err(err) => {
                error!("Response error: {:#?}", err);

                Err(NetworkError)
            },
            Ok(response) => {
                debug!("Response status: {}", response.status().as_u16());

                match response.status().as_u16() {
                    200..=299 => Ok(response),
                    404 => Err(NotFound),
                    400..=499 => Err(ClientError),
                    500..=599 => Err(ServerError),
                    _ => Err(UnknownError),
                }
            },
        }
    }

    pub fn player(&self) -> PlayerRequest {
        PlayerRequest::new(self)
    }
}

impl Default for Client {
    fn default() -> Self {
        Self::new(DEFAULT_API_URL.to_string())
    }
}
