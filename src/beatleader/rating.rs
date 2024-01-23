use crate::beatleader;
use crate::beatleader::error::Error;
use crate::beatleader::{BlApiResponse, Client};
use reqwest::Method;
use serde::Deserialize;
use std::time::Duration;

pub struct RatingsResource<'a> {
    client: &'a Client,
}

impl<'a> RatingsResource<'a> {
    pub fn new(client: &'a Client) -> Self {
        Self { client }
    }

    pub async fn get(
        &self,
        hash: &str,
        mode_name: &str,
        value: u32,
    ) -> beatleader::Result<Ratings> {
        let url = format!(
            "https://stage.api.beatleader.net/ppai2/{}/{}/{}",
            hash, mode_name, value
        );
        let request = self
            .client
            .get_http_client()
            .request(Method::GET, url)
            .timeout(Duration::from_secs(self.client.get_timeout()))
            .build();

        if let Err(err) = request {
            return Err(Error::Request(err));
        }

        match self.client.send_request(request.unwrap()).await {
            Ok(response) => match response.json::<Ratings>().await {
                Ok(ratings) => Ok(ratings),
                Err(e) => Err(Error::JsonDecode(e)),
            },
            Err(e) => Err(e),
        }
    }
}

#[derive(Deserialize, Debug, Clone)]
pub struct Ratings {
    pub none: ModifierRating,
    #[serde(rename = "SS")]
    pub ss: ModifierRating,
    #[serde(rename = "FS")]
    pub fs: ModifierRating,
    #[serde(rename = "SFS")]
    pub sf: ModifierRating,
}

#[derive(Deserialize, Debug, Clone)]
pub struct ModifierRating {
    pub predicted_acc: f64,
    pub acc_rating: f64,
    pub star_rating: f64,
    pub lack_map_calculation: RatingMapCalculation,
}
#[derive(Deserialize, Debug, Clone)]
pub struct RatingMapCalculation {
    pub multi_rating: f64,
    pub balanced_pass_diff: f64,
    pub linear_rating: f64,
    pub balanced_tech: f64,
    pub low_note_nerf: f64,
}

impl BlApiResponse for Ratings {}
