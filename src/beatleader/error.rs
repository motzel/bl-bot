use crate::beatleader::oauth::OAuthErrorResponse;
use std::{error, fmt};

#[derive(Debug)]
pub enum Error {
    Request(reqwest::Error),
    Network(reqwest::Error),
    NotFound,
    Unauthorized,
    Client(Option<String>),
    OAuth(Option<OAuthErrorResponse>),
    Server,
    JsonDecode(reqwest::Error),
    DbError(String),
    Unknown,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Request(e) => write!(f, "BL request building error: {}", e),
            Error::Network(e) => write!(f, "network error ({})", e),
            Error::NotFound => write!(f, "BL player not found"),
            Error::Unauthorized => write!(f, "BL unauthorized error"),
            Error::Client(_) => write!(f, "BL client error"),
            Error::Server => write!(f, "BL server error"),
            Error::JsonDecode(e) => write!(f, "invalid BL response: {}", e),
            Error::DbError(e) => write!(f, "db error: {}", e),
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
        }
    }
}

impl error::Error for Error {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        match &self {
            Error::Request(e) | Error::Network(e) => Some(e),
            Error::JsonDecode(e) => Some(e),
            Error::NotFound
            | Error::Unauthorized
            | Error::Client(_)
            | Error::Server
            | Error::DbError(_)
            | Error::OAuth(_)
            | Error::Unknown => None,
        }
    }
}
