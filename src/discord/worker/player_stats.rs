use std::sync::Arc;

use crate::discord::bot::beatleader::player::Player;
use tokio_util::sync::CancellationToken;

use crate::discord::BotData;
use crate::storage::player::PlayerRepository;
use crate::storage::player_scores::PlayerScoresRepository;
use crate::storage::StorageError;

pub struct BlPlayersStatsWorker {
    players_repository: Arc<PlayerRepository>,
    player_scores_repository: Arc<PlayerScoresRepository>,
    token: CancellationToken,
}

impl BlPlayersStatsWorker {
    pub fn new(data: BotData, token: CancellationToken) -> Self {
        Self {
            players_repository: data.players_repository,
            player_scores_repository: data.player_scores_repository,
            token,
        }
    }

    pub async fn run(&self) -> Result<Vec<Player>, StorageError> {
        self.players_repository
            .update_all_players_stats(
                &self.player_scores_repository,
                false,
                Some(self.token.clone()),
            )
            .await
    }
}
