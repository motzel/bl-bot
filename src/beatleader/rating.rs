use crate::beatleader;
use crate::beatleader::error::Error;
use crate::beatleader::{BlApiResponse, Client};
use reqwest::Method;
use serde::Deserialize;
use std::time::Duration;

pub struct AiRatingsResource<'a> {
    client: &'a Client,
}

impl<'a> AiRatingsResource<'a> {
    pub fn new(client: &'a Client) -> Self {
        Self { client }
    }

    pub async fn get(
        &self,
        hash: &str,
        mode_name: &str,
        value: u32,
    ) -> beatleader::Result<AiRatings> {
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
            Ok(response) => match response.json::<AiRatings>().await {
                Ok(ratings) => Ok(ratings),
                Err(e) => Err(Error::JsonDecode(e)),
            },
            Err(e) => Err(e),
        }
    }
}

#[derive(Deserialize, Debug, Clone)]
pub struct AiRatings {
    pub none: AiModifierRating,
    #[serde(rename = "SS")]
    pub ss: AiModifierRating,
    #[serde(rename = "FS")]
    pub fs: AiModifierRating,
    #[serde(rename = "SFS")]
    pub sf: AiModifierRating,
}

#[derive(Deserialize, Debug, Clone)]
pub struct AiModifierRating {
    pub predicted_acc: f64,
    pub acc_rating: f64,
    pub star_rating: f64,
    pub lack_map_calculation: AiRatingMapCalculation,
}
#[derive(Deserialize, Debug, Clone)]
pub struct AiRatingMapCalculation {
    pub multi_rating: f64,
    pub balanced_pass_diff: f64,
    pub linear_rating: f64,
    pub balanced_tech: f64,
    pub low_note_nerf: f64,
}

impl BlApiResponse for AiRatings {}
