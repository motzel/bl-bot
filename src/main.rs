use env_logger::Env;
use log::info;
use crate::beatleader::{Client, Result};

mod beatleader;

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::Builder::from_env(Env::default().default_filter_or("info")).init();

    info!("Starting up...");

    let player_id = "76561198035381239".to_string();
    let client = Client::default();

    let player = client.player().get_by_id(player_id).await?;

    println!("Player: {:#?}", player);

    info!("Shutting down...");

    Ok(())
}
