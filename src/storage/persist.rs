use super::Result;
use log::{debug, error, trace};
use serde::{Deserialize, Serialize};
use shuttle_persist::{PersistError as ShuttlePersistError, PersistInstance};
use std::collections::HashMap;
use std::fmt::Display;
use std::hash::Hash;
use std::marker::PhantomData;
use std::{error, fmt};
use tokio::sync::{Mutex, MutexGuard, RwLock};

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
    K: Serialize + for<'b> Deserialize<'b> + Hash + Eq + ToString + Send + Sync + Clone + Display,
    V: Serialize + for<'b> Deserialize<'b> + Send + Sync + Clone,
{
    pub async fn new(storage: ShuttleStorage<'a, K, V>) -> Result<CachedStorage<'a, K, V>> {
        let storage_name = storage.get_name();

        debug!("Initializing {} storage...", storage_name);

        let mut hm = HashMap::new();

        debug!("Loading {} storage index...", storage_name);
        let keys = storage.load_index().await?;
        debug!("{} storage loaded.", storage_name);

        debug!("Loading {} storage data...", storage_name);
        for key in keys.into_iter() {
            trace!("Loading {} storage data for key {}", storage_name, key);
            let value = storage.load(&key).await?;
            trace!("{} storage data for key {} loaded.", storage_name, key);
            hm.insert(key, Mutex::new(value));
        }
        debug!("{} storage data loaded.", storage_name);

        debug!("{} storage initialized.", storage_name);

        Ok(Self {
            state: RwLock::new(hm),
            storage,
        })
    }

    pub async fn get(&self, key: &K) -> Result<Option<V>> {
        let storage_name = self.storage.get_name();

        debug!("Loading {} storage data for key {}...", storage_name, key);
        let hm_lock = self.state.read().await;

        debug!("{} storage data for key {} loaded", storage_name, key);

        match hm_lock.get(key) {
            Some(value) => Ok(Some((*value.lock().await).clone())),
            None => Ok(None),
        }
    }

    pub async fn contains_key(&self, key: &K) -> bool {
        let read_lock = self.state.read().await;

        read_lock.contains_key(key)
    }

    pub async fn get_and_modify<F>(&self, key: &K, mut func: F) -> Result<Option<V>>
    where
        F: FnMut(MutexGuard<V>) -> Result<V>,
    {
        let storage_name = self.storage.get_name();

        debug!("Modifying {} storage data for key {}...", storage_name, key);
        let read_lock = self.state.read().await;

        if let Some(value_mutex) = read_lock.get(key) {
            let value = value_mutex.lock().await;

            let value = func(value)?;

            debug!("{} storage data for key {} modified", storage_name, key);

            Ok(Some(value))
        } else {
            debug!(
                "{} storage data for key {} does not exists",
                storage_name, key
            );

            Ok(None)
        }
    }

    pub async fn set(&self, key: &K, value: V) -> Result<bool> {
        let storage_name = self.storage.get_name();

        debug!("Setting {} storage data for key {}...", storage_name, key);

        let read_lock = self.state.read().await;
        let mut key_added: Option<K> = None;

        if let Some(item_mutex) = read_lock.get(key) {
            trace!("Key {} in {} storage data exists", key, storage_name);

            let mut item = item_mutex.lock().await;

            *item = value.clone();

            trace!("Value for key {} in {} storage modified", key, storage_name);
        }

        if !read_lock.contains_key(key) {
            trace!("Key {} in {} storage data NOT exists", key, storage_name);

            // drop read lock, as we need a write lock to whole hash map
            drop(read_lock);

            let mut write_lock = self.state.write().await;

            write_lock.insert(key.clone(), Mutex::new(value.clone()));

            trace!("Value for key {} in {} storage inserted", key, storage_name);

            key_added = Some(key.clone());

            // write lock is dropped here before saving (optimistic locking)
        } else {
            // drop read lock (optimistic locking)
            drop(read_lock);
        }

        // if the write fails then the cache will contain unsaved data
        self.storage.save(key.clone(), value).await?;

        if let Some(_key) = key_added {
            self.update_index().await?;

            debug!(
                "{} storage data for key {} set (previously NOT existed)",
                storage_name, key
            );

            return Ok(false);
        }

        debug!(
            "{} storage data for key {} set (previously existed)",
            storage_name, key
        );

        Ok(true)
    }

    pub async fn remove(&self, key: &K) -> Result<bool> {
        let storage_name = self.storage.get_name();

        debug!("Removing key {} from {} storage...", key, storage_name);

        let mut write_lock = self.state.write().await;
        let previous = write_lock.remove(key);

        self.update_index().await?;

        debug!("key {} removed from {} storage...", key, storage_name);

        Ok(previous.is_some())
    }

    async fn update_index(&self) -> Result<()> {
        let storage_name = self.storage.get_name();

        debug!("Updating {} storage index...", storage_name);

        let read_lock = self.state.read().await;

        let keys = read_lock.keys().cloned().collect::<Vec<K>>();

        // drop read lock (optimistic locking)
        drop(read_lock);

        let result = self.storage.save_index(keys).await;

        debug!("{} storage index updated.", storage_name);

        result
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

    pub fn get_name(&self) -> String {
        self.name.clone()
    }

    async fn load_index(&self) -> Result<Vec<K>> {
        let storage_name = self.get_storage_index_name();

        debug!(
            "Loading {} storage index with name {}...",
            self.name, storage_name
        );

        match self.persist.load::<String>(storage_name.as_str()) {
            Ok(json) => match serde_json::from_str::<Vec<K>>(json.as_str()) {
                Ok(keys) => {
                    debug!(
                        "{} storage index with name {} loaded.",
                        self.name, storage_name
                    );

                    Ok(keys)
                }
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

    async fn save_index(&self, keys: Vec<K>) -> Result<()> {
        let storage_name = self.get_storage_index_name();

        debug!(
            "Saving {} storage index with name {}...",
            self.name, storage_name
        );

        match serde_json::to_string::<Vec<K>>(&keys) {
            Ok(json) => match self.persist.save::<String>(storage_name.as_str(), json) {
                Ok(_) => {
                    debug!(
                        "{} storage index with name {} saved.",
                        self.name, storage_name
                    );

                    Ok(())
                }
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

    async fn load(&self, key: &K) -> Result<V> {
        let storage_name = self.get_storage_item_name(key);

        debug!(
            "Loading item {} from {} storage with name {}...",
            key.to_string(),
            self.name,
            storage_name
        );

        match self.persist.load::<String>(storage_name.as_str()) {
            Ok(json) => match serde_json::from_str::<V>(json.as_str()) {
                Ok(value) => {
                    debug!(
                        "item {} from {} storage with name {} loaded",
                        key.to_string(),
                        self.name,
                        storage_name
                    );

                    Ok(value)
                }
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

    async fn save(&self, key: K, value: V) -> Result<()> {
        let storage_name = self.get_storage_item_name(&key);

        debug!(
            "Saving {} to {} storage with name {}...",
            key.to_string(),
            self.name,
            storage_name
        );

        match serde_json::to_string::<V>(&value) {
            Ok(json) => match self.persist.save::<String>(storage_name.as_str(), json) {
                Ok(_) => {
                    debug!(
                        "{} to {} storage with name {} saved.",
                        key.to_string(),
                        self.name,
                        storage_name
                    );

                    Ok(())
                }
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
