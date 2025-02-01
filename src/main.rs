
use leekbot::lcapi;
use leekbot::lcdb;
use leekbot::lcbot;

use anyhow::{Result, Context};

/// Checks recent Leetcode submissions for all tracked users and sends
///   any new submissions to Discord.
/// 
/// Intended to be run regularly.
fn check_recent_submissions() -> Result<()> {
    let users = lcdb::query_tracked_users()?;
    todo!()
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Begin logger
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
