#![allow(dead_code)]
#![allow(clippy::blocks_in_conditions)]

use lazy_static::lazy_static;
use tokio_util::sync::CancellationToken;
use tokio_util::task::TaskTracker;
use tracing::info;

use other::ram_reporter::RamReporter;

use crate::beatleader::Client;
use crate::config::Settings;
use crate::discord::DiscordClient;
use crate::other::commander_orders::CommanderOrdersCleanupWorker;
use crate::webserver::WebServer;

mod beatleader;
mod config;
mod discord;
mod embed;
mod log;
mod other;
mod persist;
mod storage;
mod webserver;

lazy_static! {
    static ref BL_CLIENT: Client = Client::default();
}

pub(crate) type Error = Box<dyn std::error::Error + Send + Sync>;

#[tokio::main]
async fn main() -> Result<(), Error> {
    let settings = Settings::new().unwrap();

    let _tracing_guard = log::init(settings.tracing.clone());

    info!("Starting up...");

    let common_data = persist::init(settings).await;

    let tracker = TaskTracker::new();
    let token = CancellationToken::new();

    let ram_reporter = RamReporter::new(token.clone());
    let webserver = WebServer::new(common_data.clone(), tracker.clone(), token.clone());
    let discord = DiscordClient::new(common_data.clone(), tracker.clone(), token.clone()).await;
    let commander_orders = CommanderOrdersCleanupWorker::new(common_data.clone(), token.clone());

    tracker.spawn(discord.start());
    tracker.spawn(ram_reporter.start());
    tracker.spawn(webserver.start());
    tracker.spawn(commander_orders.run());

    tracker.close();

    tracker.wait().await;

    info!("Bye!");

    Ok(())
}
