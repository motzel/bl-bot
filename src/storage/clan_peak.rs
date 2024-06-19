use chrono::{DateTime, Utc};
use std::fmt::{Display, Formatter};
use std::sync::Arc;

use crate::beatleader::clan::{ClanId, ClanTag};
use crate::storage::persist::PersistInstance;
use crate::storage::{CachedStorage, Storage, StorageKey, StorageValue};
use serde::{Deserialize, Serialize};

use super::Result;

#[derive(Serialize, Default, Deserialize, Debug, Clone, Hash, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
#[serde(default)]
pub struct ClanPeak {
    pub clan_id: ClanId,
    pub clan_tag: ClanTag,
    pub peak: i32,
    pub peak_date: DateTime<Utc>,
}

impl ClanPeak {
    pub fn new(clan_id: ClanId, clan_tag: ClanTag, peak: i32, peak_date: DateTime<Utc>) -> Self {
        Self {
            clan_id,
            clan_tag,
            peak,
            peak_date,
        }
    }
}

impl Display for ClanPeak {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} peak: {} map(s)", self.clan_tag, self.peak)
    }
}

impl StorageKey for ClanId {}
impl StorageValue<ClanId> for ClanPeak {
    fn get_key(&self) -> ClanId {
        self.clan_id
    }
}

#[derive(Debug)]
pub(crate) struct ClanPeakRepository {
    storage: CachedStorage<ClanId, ClanPeak>,
}

impl<'a> ClanPeakRepository {
    pub(crate) async fn new(persist: Arc<PersistInstance>) -> Result<ClanPeakRepository> {
        Ok(Self {
            storage: CachedStorage::new(Storage::new("clan-peak", persist)).await?,
        })
    }

    pub(crate) async fn all(&self) -> Vec<ClanPeak> {
        self.storage.values().await
    }

    pub(crate) async fn len(&self) -> usize {
        self.storage.len().await
    }

    pub(crate) async fn get(&self, clan_id: &ClanId) -> Result<Option<ClanPeak>> {
        Ok(self.storage.get(clan_id).await)
    }

    pub(crate) async fn set(&self, clan_peak: ClanPeak) -> Result<ClanPeak> {
        let clan_peak = self.storage.set(&clan_peak.get_key(), clan_peak).await?;

        self.storage.update_index().await?;

        Ok(clan_peak)
    }
}
