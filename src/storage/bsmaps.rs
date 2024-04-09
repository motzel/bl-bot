use std::fmt::Display;
use std::sync::Arc;

use poise::serenity_prelude::UserId;
use serde::{Deserialize, Serialize};

use crate::beatleader::player::{Leaderboard, LeaderboardId};
use crate::storage::persist::PersistInstance;
use crate::storage::{CachedStorage, Storage, StorageValue};

use super::Result;

pub(crate) type BsMapId = String;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) enum BsMapType {
    CommanderOrder,
    MapListSkip,
    Personal,
    PersonalBan,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct BsMap {
    map_id: BsMapId,
    created_by: UserId,
    leaderboard_id: LeaderboardId,
    user_id: Option<UserId>,
    song_name: String,
    level_author_name: String,
    hash: String,
    diff_characteristic: String,
    diff_name: String,
    map_type: BsMapType,
}

impl BsMap {
    pub(crate) fn new(
        added_by: UserId,
        leaderboard: Leaderboard,
        map_type: BsMapType,
        user_id: Option<UserId>,
    ) -> Self {
        Self {
            map_id: Self::generate_map_id(),
            created_by: added_by,
            leaderboard_id: leaderboard.id,
            user_id,
            song_name: leaderboard.song.name,
            level_author_name: leaderboard.song.author,
            hash: leaderboard.song.hash,
            diff_characteristic: leaderboard.difficulty.mode_name,
            diff_name: leaderboard.difficulty.difficulty_name,
            map_type,
        }
    }

    pub fn get_id(&self) -> &BsMapId {
        &self.map_id
    }

    pub fn get_leaderboard_id(&self) -> &LeaderboardId {
        &self.leaderboard_id
    }

    pub fn get_user_id(&self) -> Option<&UserId> {
        self.user_id.as_ref()
    }

    fn generate_map_id() -> BsMapId {
        uuid::Uuid::new_v4()
            .hyphenated()
            .encode_lower(&mut uuid::Uuid::encode_buffer())
            .to_owned()
    }
}

impl Display for BsMap {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "[{} / {}](<https://www.beatleader.xyz/leaderboard/clanranking/{}/1>)",
            &self.song_name, &self.diff_name, &self.leaderboard_id,
        )
    }
}

impl StorageValue<BsMapId> for BsMap {
    fn get_key(&self) -> BsMapId {
        self.get_id().clone()
    }
}

#[derive(Debug)]
pub(crate) struct BsMapsRepository {
    storage: CachedStorage<BsMapId, BsMap>,
}

impl<'a> BsMapsRepository {
    pub(crate) async fn new(persist: Arc<PersistInstance>) -> Result<Self> {
        Ok(Self {
            storage: CachedStorage::new(Storage::new("maps", persist)).await?,
        })
    }

    pub(crate) async fn all(&self) -> Vec<BsMap> {
        self.storage.values().await
    }

    pub(crate) async fn commander_orders(&self) -> Result<Vec<BsMap>> {
        Ok(self.by_map_type(&BsMapType::CommanderOrder).await)
    }

    pub(crate) async fn get_commander_order(
        &self,
        leaderboard_id: &LeaderboardId,
    ) -> Result<Option<BsMap>> {
        Ok(self
            .by_leaderboard(leaderboard_id)
            .await?
            .into_iter()
            .filter(|map| Self::filter_map_type(map, &BsMapType::CommanderOrder))
            .collect::<Vec<_>>()
            .first()
            .cloned())
    }

    pub(crate) async fn map_list_bans(&self) -> Result<Vec<BsMap>> {
        Ok(self.by_map_type(&BsMapType::MapListSkip).await)
    }

    pub(crate) async fn get_map_list_ban(
        &self,
        leaderboard_id: &LeaderboardId,
    ) -> Result<Option<BsMap>> {
        Ok(self
            .by_leaderboard(leaderboard_id)
            .await?
            .into_iter()
            .filter(|map| Self::filter_map_type(map, &BsMapType::MapListSkip))
            .collect::<Vec<_>>()
            .first()
            .cloned())
    }

    pub(crate) async fn by_leaderboard(
        &self,
        leaderboard_id: &LeaderboardId,
    ) -> Result<Vec<BsMap>> {
        Ok(self
            .storage
            .filtered(|map| Self::filter_leaderboard(map, leaderboard_id))
            .await)
    }

    pub(crate) async fn by_map_type(&self, map_type: &BsMapType) -> Vec<BsMap> {
        self.storage
            .filtered(|map| Self::filter_map_type(map, map_type))
            .await
    }

    pub(crate) async fn len(&self) -> usize {
        self.storage.len().await
    }

    pub(crate) async fn get(&self, map_id: &BsMapId) -> Option<BsMap> {
        self.storage.get(map_id).await
    }

    pub(crate) async fn save(&self, map: BsMap) -> Result<BsMap> {
        let result = self.storage.set(&map.get_id().clone(), map).await?;

        self.storage.update_index().await?;

        Ok(result)
    }

    pub(crate) async fn remove(&self, key: &BsMapId) -> Result<bool> {
        self.storage.remove(key).await
    }

    pub(crate) async fn restore(&self, values: Vec<BsMap>) -> Result<()> {
        self.storage.restore(values).await
    }

    fn filter_leaderboard(map: &BsMap, leaderboard_id: &LeaderboardId) -> bool {
        &map.leaderboard_id == leaderboard_id
    }

    fn filter_user(map: &BsMap, user_id: Option<UserId>) -> bool {
        (map.user_id.is_none() && user_id.is_none())
            || (map.user_id.is_some() && map.user_id == user_id)
    }
    fn filter_map_type(map: &BsMap, map_type: &BsMapType) -> bool {
        &map.map_type == map_type
    }
}
