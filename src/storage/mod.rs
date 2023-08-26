pub(crate) use crate::storage::persist::{PersistError, StorageKey, StorageValue};

pub(crate) mod guild;
pub(crate) mod persist;
pub(crate) mod player;

type Result<T> = std::result::Result<T, PersistError>;
