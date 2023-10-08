pub(crate) use crate::storage::persist::{PersistError, StorageKey, StorageValue};

pub(crate) mod guild;
pub(crate) mod persist;
pub(crate) mod player;
pub(crate) mod player_oauth_token;

type Result<T> = std::result::Result<T, PersistError>;
