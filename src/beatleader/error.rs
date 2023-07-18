#[derive(Debug)]
pub enum BlError {
    RequestError(reqwest::Error),
    NetworkError(reqwest::Error),
    NotFound,
    ClientError,
    ServerError,
    JsonDecodeError,
    UnknownError,
}
