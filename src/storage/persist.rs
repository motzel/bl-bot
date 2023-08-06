use std::collections::HashMap;
use std::fmt::Display;
use std::hash::Hash;
use std::marker::PhantomData;
use std::sync::Arc;
use std::{error, fmt};

use log::{debug, error, trace, warn};
use serde::{Deserialize, Serialize};
use shuttle_persist::{PersistError as ShuttlePersistError, PersistInstance};
use tokio::sync::{Mutex, MutexGuard, RwLock};

use super::Result;

#[derive(Debug)]
pub enum PersistError {
    Storage(ShuttlePersistError),
    JsonDeserialize(serde_json::Error),
    JsonSerialize(serde_json::Error),
    BlApi(crate::beatleader::error::Error),
    NotFound(String),
    Unknown,
}

impl error::Error for PersistError {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        match &self {
            PersistError::Storage(e) => Some(e),
            PersistError::JsonDeserialize(e) => Some(e),
            PersistError::JsonSerialize(e) => Some(e),
            PersistError::BlApi(e) => Some(e),
            PersistError::Unknown | PersistError::NotFound(_) => None,
        }
    }
}

impl Display for PersistError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PersistError::Storage(e) => write!(f, "storage error: {}", e),
            PersistError::JsonDeserialize(e) => write!(f, "deserialization error: {}", e),
            PersistError::JsonSerialize(e) => write!(f, "serialization error: {}", e),
            PersistError::BlApi(e) => write!(f, "Beat Leader API error: {}", e),
            PersistError::NotFound(e) => write!(f, "{}", e),
            PersistError::Unknown => write!(f, "unknown error"),
        }
    }
}

pub(super) struct CachedStorage<K, V>
where
    K: Serialize + for<'b> Deserialize<'b> + Hash + Eq + ToString + Send + Sync + Clone,
    V: Serialize + for<'b> Deserialize<'b> + Send + Sync + Clone,
{
    state: RwLock<HashMap<K, Mutex<V>>>,
    storage: ShuttleStorage<K, V>,
}

impl<'a, K, V> CachedStorage<K, V>
where
    K: Serialize + for<'b> Deserialize<'b> + Hash + Eq + ToString + Send + Sync + Clone + Display,
    V: Serialize + for<'b> Deserialize<'b> + Send + Sync + Clone,
{
    pub(super) async fn new(storage: ShuttleStorage<K, V>) -> Result<CachedStorage<K, V>> {
        let storage_name = storage.get_name();

        debug!("Initializing {} storage...", storage_name);

        let mut hm = HashMap::new();

        debug!("Loading {} storage index...", storage_name);
        let keys = match storage.load_index().await {
            Ok(keys) => keys,
            Err(e) => {
                warn!("Can not load {} storage index: {}", storage_name, e);

                Vec::new()
            }
        };
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

    pub(super) async fn len(&self) -> usize {
        let storage_name = self.storage.get_name();

        debug!("Getting {} storage length...", storage_name);
        let hm_lock = self.state.read().await;

        hm_lock.len()
    }

    pub(super) async fn get(&self, key: &K) -> Option<V> {
        let storage_name = self.storage.get_name();

        debug!("Getting {} storage data for key {}...", storage_name, key);
        let hm_lock = self.state.read().await;

        match hm_lock.get(key) {
            Some(value) => {
                let value = (*value.lock().await).clone();

                debug!("{} storage data for key {} returned.", storage_name, key);

                Some(value)
            }
            None => None,
        }
    }

    pub(super) async fn values(&self) -> Vec<V> {
        let storage_name = self.storage.get_name();

        debug!("Getting all {} storage data...", storage_name);

        let read_lock = self.state.read().await;

        let mut ret = Vec::with_capacity(read_lock.len());

        for value in read_lock.values() {
            let value = (*value.lock().await).clone();

            ret.push(value);
        }

        debug!("All {} storage data cloned and returned.", storage_name);

        ret
    }

    pub(super) async fn contains_key(&self, key: &K) -> bool {
        let read_lock = self.state.read().await;

        read_lock.contains_key(key)
    }

    pub(super) async fn get_and_modify_or_insert<ModifyFunc, InsertFunc>(
        &self,
        key: &K,
        modify_func: ModifyFunc,
        insert_func: InsertFunc,
    ) -> Result<Option<V>>
    where
        ModifyFunc: FnOnce(&mut MutexGuard<V>),
        InsertFunc: FnOnce() -> Option<V>,
    {
        let storage_name = self.storage.get_name();

        debug!(
            "Modifying or inserting {} storage data for key {}...",
            storage_name, key
        );
        let mut write_lock = self.state.write().await;

        if let Some(value_mutex) = write_lock.get(key) {
            debug!("{} storage data for key {} exists", storage_name, key);

            let value_mutex_guard = &mut value_mutex.lock().await;

            modify_func(value_mutex_guard);

            debug!("{} storage data for key {} modified", storage_name, key);

            Ok(Some(
                self.storage
                    .save(key.clone(), (*value_mutex_guard).clone())
                    .await?,
            ))
        } else {
            debug!(
                "{} storage data for key {} does not exists",
                storage_name, key
            );

            if let Some(value) = insert_func() {
                write_lock.insert(key.clone(), Mutex::new(value.clone()));

                debug!("{} storage data for key {} inserted", storage_name, key);

                let value = self.storage.save(key.clone(), value).await?;

                drop(write_lock);

                self.update_index().await?;

                Ok(Some(value))
            } else {
                Ok(None)
            }
        }
    }

    pub(super) async fn set(&self, key: &K, value: V) -> Result<V> {
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
        let value = self.storage.save(key.clone(), value).await?;

        if let Some(_key) = key_added {
            self.update_index().await?;

            debug!(
                "{} storage data for key {} set (previously NOT existed)",
                storage_name, key
            );

            return Ok(value);
        }

        debug!(
            "{} storage data for key {} set (previously existed)",
            storage_name, key
        );

        Ok(value)
    }

    pub(super) async fn remove(&self, key: &K) -> Result<bool> {
        let storage_name = self.storage.get_name();

        debug!("Removing key {} from {} storage...", key, storage_name);

        let mut write_lock = self.state.write().await;
        let previous = write_lock.remove(key);

        drop(write_lock);

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

pub(super) struct ShuttleStorage<K, V>
where
    K: Serialize + for<'b> Deserialize<'b> + Hash + Eq + ToString + Send + Sync,
    V: Serialize + for<'b> Deserialize<'b> + Send + Sync,
{
    persist: Arc<PersistInstance>,
    name: String,
    _phantom_key: PhantomData<K>,
    _phantom_value: PhantomData<V>,
}

impl<'a, K, V> ShuttleStorage<K, V>
where
    K: Serialize + for<'b> Deserialize<'b> + Hash + Eq + ToString + Send + Sync,
    V: Serialize + for<'b> Deserialize<'b> + Send + Sync,
{
    pub fn new(name: &str, persist: Arc<PersistInstance>) -> ShuttleStorage<K, V> {
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

    async fn save(&self, key: K, value: V) -> Result<V> {
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

                    Ok(value)
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