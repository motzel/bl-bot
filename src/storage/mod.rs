pub(crate) use crate::storage::persist::PersistError;

pub(crate) mod guild;
mod persist;
pub(crate) mod player;

type Result<T> = std::result::Result<T, PersistError>;
