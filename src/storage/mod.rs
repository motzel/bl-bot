use crate::storage::persist::PersistError;

pub(crate) mod persist;
pub(crate) mod player;
pub(crate) mod settings;

type Result<T> = std::result::Result<T, PersistError>;
