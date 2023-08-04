use log::{debug, error};
use serde::{Deserialize, Serialize};
use shuttle_persist::{PersistError as ShuttlePersistError, PersistInstance};
use std::collections::HashMap;
use std::hash::Hash;
use std::marker::PhantomData;
use std::{error, fmt};
use tokio::sync::{Mutex, RwLock};

#[derive(Debug)]
pub enum PersistError {
    Storage(ShuttlePersistError),
    JsonDeserialize(serde_json::Error),
    JsonSerialize(serde_json::Error),
    Unknown,
}

impl error::Error for PersistError {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        match &self {
            PersistError::Storage(e) => Some(e),
            PersistError::JsonDeserialize(e) => Some(e),
            PersistError::JsonSerialize(e) => Some(e),
            PersistError::Unknown => None,
        }
    }
}

impl fmt::Display for PersistError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PersistError::Storage(e) => write!(f, "storage error: {}", e),
            PersistError::JsonDeserialize(e) => write!(f, "deserialization error: {}", e),
            PersistError::JsonSerialize(e) => write!(f, "serialization error: {}", e),
            PersistError::Unknown => write!(f, "unknown error"),
        }
    }
}

pub(super) struct CachedStorage<'a, K, V>
where
    K: Serialize + for<'b> Deserialize<'b> + Hash + Eq + ToString + Send + Sync + Clone,
    V: Serialize + for<'b> Deserialize<'b> + Send + Sync + Clone,
{
    state: RwLock<HashMap<K, Mutex<V>>>,
    storage: ShuttleStorage<'a, K, V>,
}

impl<'a, K, V> CachedStorage<'a, K, V>
where
    K: Serialize + for<'b> Deserialize<'b> + Hash + Eq + ToString + Send + Sync + Clone,
    V: Serialize + for<'b> Deserialize<'b> + Send + Sync + Clone,
{
    pub async fn new(
        storage: ShuttleStorage<'a, K, V>,
    ) -> Result<CachedStorage<'a, K, V>, PersistError> {
        let keys = storage.load_index().await?;
        let mut hm = HashMap::new();
        for key in keys.into_iter() {
            let value = storage.load(&key).await?;
            hm.insert(key, Mutex::new(value));
        }

        Ok(Self {
            state: RwLock::new(hm),
            storage,
        })
    }

    pub async fn get(&self, key: &K) -> Result<Option<V>, PersistError> {
        let hm_lock = self.state.read().await;

        match hm_lock.get(key) {
            Some(value) => Ok(Some((*value.lock().await).clone())),
            None => Ok(None),
        }
    }

    pub async fn upsert(&self, key: K, value: V) -> Result<Option<()>, PersistError> {
        let read_lock = self.state.read().await;
        let mut key_added: Option<K> = None;

        if let Some(item_mutex) = read_lock.get(&key) {
            let mut item = item_mutex.lock().await;

            *item = value.clone();
        }

        if !read_lock.contains_key(&key) {
            // drop read lock, as we need a write lock to whole hash map
            drop(read_lock);

            let mut write_lock = self.state.write().await;

            write_lock.insert(key.clone(), Mutex::new(value.clone()));

            key_added = Some(key.clone());

            // write lock is dropped here before saving (optimistic locking)
        } else {
            // drop read lock (optimistic locking)
            drop(read_lock);
        }

        // if the write fails then the cache will contain unsaved data
        self.storage.save(key, value).await?;

        if let Some(_key) = key_added {
            self.update_index().await?;

            return Ok(None);
        }

        Ok(Some(()))
    }

    pub async fn remove(&self, key: &K) -> Result<Option<()>, PersistError> {
        let mut write_lock = self.state.write().await;
        let previous = write_lock.remove(key);

        self.update_index().await?;

        Ok(previous.map(|_| ()))
    }

    pub async fn update_index(&self) -> Result<(), PersistError> {
        let read_lock = self.state.read().await;

        let keys = read_lock.keys().cloned().collect::<Vec<K>>();

        // drop read lock (optimistic locking)
        drop(read_lock);

        self.storage.save_index(keys).await
    }
}

pub(super) struct ShuttleStorage<'a, K, V>
where
    K: Serialize + for<'b> Deserialize<'b> + Hash + Eq + ToString + Send + Sync,
    V: Serialize + for<'b> Deserialize<'b> + Send + Sync,
{
    persist: &'a PersistInstance,
    name: String,
    _phantom_key: PhantomData<K>,
    _phantom_value: PhantomData<V>,
}

impl<'a, K, V> ShuttleStorage<'a, K, V>
where
    K: Serialize + for<'b> Deserialize<'b> + Hash + Eq + ToString + Send + Sync,
    V: Serialize + for<'b> Deserialize<'b> + Send + Sync,
{
    pub fn new(name: &str, persist: &'a PersistInstance) -> ShuttleStorage<'a, K, V> {
        Self {
            persist,
            name: name.to_owned(),
            _phantom_key: Default::default(),
            _phantom_value: Default::default(),
        }
    }

    async fn load_index(&self) -> Result<Vec<K>, PersistError> {
        let storage_name = self.get_storage_index_name();

        debug!(
            "Loading {} storage index with name {}...",
            self.name, storage_name
        );

        match self.persist.load::<String>(storage_name.as_str()) {
            Ok(json) => match serde_json::from_str::<Vec<K>>(json.as_str()) {
                Ok(keys) => Ok(keys),
                Err(e) => {
                    error!(
                        "Can not deserialize {} storage index to JSON: {}",
                        self.name, e
                    );

                    Err(PersistError::JsonDeserialize(e))
                }
            },
            Err(e) => {
                error!("Can not load {} storage index: {}", self.name, e);

                Err(PersistError::Storage(e))
            }
        }
    }

    async fn save_index(&self, keys: Vec<K>) -> Result<(), PersistError> {
        let storage_name = self.get_storage_index_name();

        debug!(
            "Saving {} storage index with name {}...",
            self.name, storage_name
        );

        match serde_json::to_string::<Vec<K>>(&keys) {
            Ok(json) => match self.persist.save::<String>(storage_name.as_str(), json) {
                Ok(_) => Ok(()),
                Err(e) => {
                    error!("Can not save {} storage index: {}", self.name, e);

                    Err(PersistError::Storage(e))
                }
            },
            Err(e) => {
                error!(
                    "Can not serialize {} storage index to JSON: {}",
                    self.name, e
                );

                Err(PersistError::JsonSerialize(e))
            }
        }
    }

    async fn load(&self, key: &K) -> Result<V, PersistError> {
        let storage_name = self.get_storage_item_name(key);

        debug!(
            "Loading item {} from {} storage with name {}...",
            key.to_string(),
            self.name,
            storage_name
        );

        match self.persist.load::<String>(storage_name.as_str()) {
            Ok(json) => match serde_json::from_str::<V>(json.as_str()) {
                Ok(value) => Ok(value),
                Err(e) => {
                    error!(
                        "Can not deserialize {} from {} storage to JSON: {}",
                        key.to_string(),
                        self.name,
                        e
                    );

                    Err(PersistError::JsonDeserialize(e))
                }
            },
            Err(e) => {
                error!(
                    "Can not load {} from {} storage: {}",
                    key.to_string(),
                    self.name,
                    e
                );

                Err(PersistError::Storage(e))
            }
        }
    }

    async fn save(&self, key: K, value: V) -> Result<(), PersistError> {
        let storage_name = self.get_storage_item_name(&key);

        debug!(
            "Saving {} to {} storage with name {}...",
            key.to_string(),
            self.name,
            storage_name
        );

        match serde_json::to_string::<V>(&value) {
            Ok(json) => match self.persist.save::<String>(storage_name.as_str(), json) {
                Ok(_) => Ok(()),
                Err(e) => {
                    error!(
                        "Can not save {} to {} storage: {}",
                        key.to_string(),
                        self.name,
                        e
                    );

                    Err(PersistError::Storage(e))
                }
            },
            Err(e) => {
                error!(
                    "Can not serialize {} from {} storage to JSON: {}",
                    key.to_string(),
                    self.name,
                    e
                );

                Err(PersistError::JsonSerialize(e))
            }
        }
    }

    fn get_storage_index_name(&self) -> String {
        format!("{}-index", self.name)
    }

    fn get_storage_item_name(&self, key: &K) -> String {
        format!("{}-{}", self.name, key.to_string())
    }
}
