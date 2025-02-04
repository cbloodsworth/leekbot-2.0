use leekbot::lcdb;
use leekbot::lcbot;

use anyhow::Context;
use dotenv::dotenv;


#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Begin logger
    dotenv().ok();
    env_logger::Builder::from_env("LOG_LEVEL").init();

    // Initialize database
    lcdb::initialize_db()
        .context("Error initializing database.")
        .unwrap();

    // Run the discord bot
    lcbot::run_leekbot()
        .await
        .context("Error initializing discord bot.")
        .unwrap();

    Ok(())
}
