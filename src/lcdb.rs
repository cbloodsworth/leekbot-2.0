use anyhow::Result;
use rusqlite::Connection;

use std::time::{SystemTime, UNIX_EPOCH};

use crate::models;

mod submissions;
mod users;
mod problems;
mod recentcache;
mod leekcoins;

mod schema;

pub use users::*;
pub use submissions::*;
pub use problems::*;
pub use recentcache::*;
pub use leekcoins::*;

type DBResult<T> = Result<T, rusqlite::Error>;

fn connect() -> DBResult<Connection> {
    Connection::open("db/leek.db")
}

pub fn initialize_db() -> DBResult<()> {
    use schema::*;

    // User table
    log::info!("[initialize_db] creating Users table...");
    connect()?.execute(USER_SCHEMA, [])?;

    // Submission table
    log::info!("[initialize_db] creating Submissions table...");
    connect()?.execute(SUBMISSIONS_SCHEMA, [])?;

    // Problem table
    log::info!("[initialize_db] creating Problems table...");
    connect()?.execute(PROBLEMS_SCHEMA, [])?;

    // Recent Submission Cache
    log::info!("[initialize_db] creating RecentCache table...");
    connect()?.execute(RECENT_CACHE_SCHEMA, [])?;

    // UserPreferences
    log::info!("[initialize_db] creating UserPrefs table...");
    connect()?.execute(USER_PREFS_SCHEMA, [])?;

    // UserCoins
    log::info!("[initialize_db] creating UserCoins table...");
    connect()?.execute(LEEK_COINS_SCHEMA, [])?;

    Ok(())
}



/////*============== INTERNAL API ==============*/
fn swallow_constraint_violation(err: rusqlite::Error) -> DBResult<bool> {
    match err.sqlite_error_code() {
        Some(rusqlite::ErrorCode::ConstraintViolation) => { Ok(false) },
        _ => Err(err)
    }
}

pub fn insert_fake_submission(
    user: &models::User,
    problem_name: String,
    accepted: bool,
) -> DBResult<()> {
    let problem = models::Problem {
        title: problem_name,
        url: String::from("no_url"),
        difficulty: String::from("no difficulty"),
    };

    insert_problem(&problem)?;

    let submission = models::Submission {
        username: user.username.to_owned(),
        problem,
        language: String::from("no_language"),
        timestamp: {
            // Get the current timestamp, approximately
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("Time went backwards????")
                .as_millis() as usize
        },
        accepted,
        url: String::from("no_url"),
    };

    insert_submission(&submission)?;

    Ok(())
}
