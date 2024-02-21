#![allow(dead_code)]
#![allow(clippy::blocks_in_conditions)]

use lazy_static::lazy_static;
use log::{info, warn};

use crate::beatleader::Client;
use crate::other::RamReporter;
use tokio::signal;
use tokio_util::sync::CancellationToken;
use tokio_util::task::TaskTracker;

mod beatleader;
mod config;
mod discord;
mod embed;
mod file_storage;
mod other;
mod storage;

lazy_static! {
    static ref BL_CLIENT: Client = Client::default();
}
pub(crate) type Error = Box<dyn std::error::Error + Send + Sync>;

#[tokio::main]
async fn main() -> Result<(), Error> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("bl_bot=info"))
        .target(env_logger::Target::Stdout)
        .init();

    info!("Starting up...");

    let common_data = config::init().await;

    let tracker = TaskTracker::new();
    let token = CancellationToken::new();

    let ram_reporter = RamReporter::new(token.clone());

    let discord_framework =
        discord::init(common_data.clone(), tracker.clone(), token.clone()).await?;

    #[cfg(windows)]
    let discord_framework_clone_win = discord_framework.clone();

    #[cfg(unix)]
    let discord_framework_clone_unix = discord_framework.clone();

    #[cfg(windows)]
    tracker.spawn(async move {
        let _ = signal::ctrl_c().await;
        warn!("CTRL+C pressed, shutting down...");
        token.cancel();

        warn!("Discord client is shutting down...");
        discord_framework_clone_win
            .shard_manager()
            .lock()
            .await
            .shutdown_all()
            .await;
        warn!("Discord client shut down.");
    });

    #[cfg(unix)]
    tracker.spawn(async move {
        let mut sigint = signal::unix::signal(signal::unix::SignalKind::interrupt()).unwrap();
        let mut sighup = signal::unix::signal(signal::unix::SignalKind::hangup()).unwrap();
        let mut sigterm = signal::unix::signal(signal::unix::SignalKind::terminate()).unwrap();

        tokio::select! {
            _ = sigint.recv() => {
                warn!("SIGINT received, shutting down...");
                token.cancel();
                warn!("Discord client is shutting down...");
                discord_framework_clone_unix
                    .shard_manager()
                    .lock()
                    .await
                    .shutdown_all()
                    .await;
                warn!("Discord client shut down.");
            }
            _ = sighup.recv() => {
                warn!("SIGHUP received, shutting down...");
                token.cancel();
                warn!("Discord client is shutting down...");
                discord_framework_clone_unix
                    .shard_manager()
                    .lock()
                    .await
                    .shutdown_all()
                    .await;
                warn!("Discord client shut down.");
            }
            _ = sigterm.recv() => {
                warn!("SIGTERM received, shutting down...");
                token.cancel();
                warn!("Discord client is shutting down...");
                discord_framework_clone_unix
                    .shard_manager()
                    .lock()
                    .await
                    .shutdown_all()
                    .await;
                warn!("Discord client shut down.");
            }
        }
    });

    tracker.spawn(discord_framework.start());
    tracker.spawn(ram_reporter.start());

    tracker.close();

    tracker.wait().await;

    info!("Bye!");

    Ok(())
}
