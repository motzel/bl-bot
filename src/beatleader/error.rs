#[derive(Debug)]
pub enum BlError {
    NetworkError,
    NotFound,
    ClientError,
    ServerError,
    JsonDecodeError,
    UnknownError,
}
