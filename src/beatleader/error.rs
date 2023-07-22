use std::{error, fmt};

#[derive(Debug)]
pub enum Error {
    Request(reqwest::Error),
    Network(reqwest::Error),
    NotFound,
    Client,
    Server,
    JsonDecode,
    DbError(String),
    Unknown,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Request(e) => write!(f, "BL request error: {}", e),
            Error::Network(e) => write!(f, "network error ({})", e),
            Error::NotFound => write!(f, "BL player not found"),
            Error::Client => write!(f, "BL client error"),
            Error::Server => write!(f, "BL server error"),
            Error::JsonDecode => write!(f, "invalid BL response"),
            Error::DbError(e) => write!(f, "db error: {}", e),
            Error::Unknown => write!(f, "unknown error"),
        }
    }
}

impl error::Error for Error {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        match &self {
            Error::Request(e) | Error::Network(e) => Some(e),
            Error::NotFound
            | Error::Client
            | Error::Server
            | Error::JsonDecode
            | Error::DbError(_)
            | Error::Unknown => None,
        }
    }
}
