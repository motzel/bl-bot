use chrono::Utc;
use std::sync::Arc;

use tokio_util::sync::CancellationToken;
use tracing::{info, warn};

use crate::persist::CommonData;
use crate::storage::bsmaps::BsMapsRepository;

pub struct CommanderOrdersCleanupWorker {
    maps_repository: Arc<BsMapsRepository>,
    retention: std::time::Duration,
    token: CancellationToken,
}

impl CommanderOrdersCleanupWorker {
    pub fn new(data: CommonData, token: CancellationToken) -> Self {
        Self {
            maps_repository: data.maps_repository,
            retention: std::time::Duration::from_secs(
                data.settings.commander_orders_retention * 24 * 60 * 60,
            ),
            token,
        }
    }

    pub async fn run(self) {
        let interval = std::time::Duration::from_secs(60 * 60);

        info!(
            "Run Commander's order cleanup task with retention of {} day(s)",
            self.retention.as_secs() / (60 * 60 * 24)
        );

        'outer: loop {
            let commander_orders = self
                .maps_repository
                .all_commander_orders()
                .await
                .unwrap_or_else(|_| vec![]);

            info!("Found {} commander's orders.", commander_orders.len());

            let to_delete = commander_orders
                .into_iter()
                .filter(|map| {
                    map.created_at.is_none()
                        || map.created_at.unwrap() + self.retention < Utc::now()
                })
                .collect::<Vec<_>>();

            info!("Deleting {} commander's orders.", to_delete.len());

            for map in to_delete.iter() {
                match self.maps_repository.remove(map.get_id()).await {
                    Ok(_) => info!(
                        "Deleted commander's order {} / {} / {}",
                        map.song_name, map.diff_name, map.diff_characteristic
                    ),
                    Err(e) => warn!(
                        "Failed to delete commander's order {} / {} / {}: {}",
                        map.song_name, map.diff_name, map.diff_characteristic, e
                    ),
                }
            }

            tokio::select! {
                _ = self.token.cancelled() => {
                    warn!("Commander's order cleanup task is shutting down...");
                    break 'outer;
                }
                _ = tokio::time::sleep(interval) => {}
            }
        }

        warn!("Commander's order task shut down.");
    }
}
