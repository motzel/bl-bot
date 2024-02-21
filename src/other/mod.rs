use log::{info, warn};
use peak_alloc::PeakAlloc;
use tokio_util::sync::CancellationToken;

#[global_allocator]
static PEAK_ALLOC: PeakAlloc = PeakAlloc;

pub struct RamReporter {
    token: CancellationToken,
}
impl RamReporter {
    pub fn new(token: CancellationToken) -> Self {
        Self { token }
    }

    pub async fn start(self) {
        let interval = std::time::Duration::from_secs(70);
        info!("Run RAM reporter task that reports every {:?}", interval);

        'outer: loop {
            info!("RAM usage: {} MB", PEAK_ALLOC.current_usage_as_mb());
            info!("Peak RAM usage: {} MB", PEAK_ALLOC.peak_usage_as_mb());

            tokio::select! {
                _ = self.token.cancelled() => {
                    warn!("RAM reporter task is shutting down...");
                    break 'outer;
                }
                _ = tokio::time::sleep(interval) => {}
            }
        }
    }
}
