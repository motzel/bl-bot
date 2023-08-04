use crate::storage::persist::PersistError;

mod persist;
pub(crate) mod player;
pub(crate) mod settings;

type Result<T> = std::result::Result<T, PersistError>;
