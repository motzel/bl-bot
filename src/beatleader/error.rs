use std::{error, fmt};

use chrono::{DateTime, Utc};

use crate::beatleader::oauth::OAuthErrorResponse;

#[derive(Debug)]
pub enum Error {
    Request(reqwest::Error),
    Network(reqwest::Error),
    NotFound,
    NoContent,
    Unauthorized,
    Client(Option<String>),
    OAuth(Option<OAuthErrorResponse>),
    OAuthExpired(DateTime<Utc>),
    OAuthStorage,
    Server,
    JsonDecode(reqwest::Error),
    Db(String),
    Unknown,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Request(e) => write!(f, "BL request building error: {e}"),
            Error::Network(e) => write!(f, "network error ({e})"),
            Error::NoContent => write!(f, "no content"),
            Error::NotFound => write!(f, "not found"),
            Error::Unauthorized => write!(f, "BL unauthorized error"),
            Error::Client(_) => write!(f, "BL client error"),
            Error::Server => write!(f, "BL server error"),
            Error::JsonDecode(e) => write!(f, "invalid BL response: {e}"),
            Error::Db(e) => write!(f, "db error: {e}"),
            Error::Unknown => write!(f, "unknown error"),
            Error::OAuth(e) => write!(
                f,
                "invalid BL OAuth response: {}",
                if let Some(resp) = e {
                    resp.error_description.as_str()
                } else {
                    "unknown response"
                }
            ),
            Error::OAuthStorage => write!(f, "OAuth storage error"),
            Error::OAuthExpired(date) => write!(f, "OAuth token has expired on {date}"),
        }
    }
}

impl error::Error for Error {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        match &self {
            Error::Request(e) | Error::Network(e) => Some(e),
            Error::JsonDecode(e) => Some(e),
            Error::NotFound
            | Error::NoContent
            | Error::Unauthorized
            | Error::Client(_)
            | Error::Server
            | Error::Db(_)
            | Error::OAuth(_)
            | Error::OAuthStorage
            | Error::OAuthExpired(_)
            | Error::Unknown => None,
        }
    }
}
