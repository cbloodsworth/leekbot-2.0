use anyhow::{Result, Context};
use rusqlite::{params, Connection};

use std::time::{SystemTime, UNIX_EPOCH};

use crate::models;

impl<'a> TryFrom<&'a rusqlite::Row<'a>> for models::Submission {
    type Error = rusqlite::Error;

    fn try_from(row: &rusqlite::Row) -> Result<Self, rusqlite::Error> {
        let problem = models::Problem {
            title:      row.get("problem_name")?,
            titleSlug:  row.get("problem_link")?,
            difficulty: row.get("difficulty")?,
        };

        Ok(Self {
            problem,

            username:   row.get("username")?,
            accepted:   row.get("accepted")?,
            language:   row.get("language")?,
            timestamp:  row.get("timestamp")?,
        })
    }
}


fn connect() -> Result<Connection> {
    Ok(Connection::open("leek.db")?)
}

pub fn initialize_db() -> Result<()> {
    // User table
    log::debug!("[initialize_db] creating Users table...");
    connect()?.execute(
        "CREATE TABLE IF NOT EXISTS Users (
            username       TEXT        PRIMARY KEY,
            tracked        BOOLEAN     NOT NULL,

            easy_solved    INTEGER     NOT NULL,
            medium_solved  INTEGER     NOT NULL,
            hard_solved    INTEGER     NOT NULL,
            total_solved   INTEGER     NOT NULL,

            ranking        INTEGER     NOT NULL
        )",
        []
    )?;

    // Submission table
    log::debug!("[initialize_db] creating Submissions table...");
    connect()?.execute(
        "CREATE TABLE IF NOT EXISTS Submissions (
            id             INTEGER     PRIMARY KEY,

            problem_name   TEXT        NOT NULL    REFERENCES Problems(problem_name),

            username       TEXT        NOT NULL    REFERENCES Users(username),
            language       TEXT        NOT NULL,
            timestamp      TIMESTAMP   NOT NULL,
            accepted       BOOLEAN     NOT NULL
        )",
        []
    )?;

    // Problem table
    log::debug!("[initialize_db] creating Problems table...");
    connect()?.execute(
        "CREATE TABLE IF NOT EXISTS Problems (
            problem_name   TEXT        PRIMARY KEY,

            problem_link   TEXT        NOT NULL,
            difficulty     TEXT        NOT NULL
        )",
        []
    )?;

    Ok(())
}


pub fn insert_user(user: &models::User) -> Result<()> {
    let connection = connect()?;

    log::info!("[insert_user] Inserting user {} into Users...", user.username);
    connection.prepare(
        "INSERT INTO Users (username, 
                            tracked, 
                            easy_solved, 
                            medium_solved, 
                            hard_solved, 
                            total_solved, 
                            ranking)
         VALUES (?, ?, ?, ?, ?, ?, ?)"
    )?.execute(params![user.username, 
                0,
                user.easy_solved,
                user.medium_solved,
                user.hard_solved,
                user.total_solved,
                user.ranking])?;

    Ok(())
}

pub fn insert_submission(submission: &models::Submission) -> Result<()> {
    todo!()
}

/// Tracks a user by updating the "tracked" field to true.
///   Inserts the user if it isn't in the database already.
pub fn track_user(user: &models::User) -> Result<()> {
    if !user_exists(user)? { insert_user(user)?; }

    let connection = connect()?;
    connection.prepare(
        "UPDATE Users
         SET tracked = 1
         WHERE username = ?
        "
    )?.execute(params![&user.username])?;

    Ok(())
}

/// Untracks a user by updating the "tracked" field to false.
///   Inserts the user if it isn't in the database already.
pub fn untrack_user(user: &models::User) -> Result<()> {
    if !user_exists(user)? { insert_user(user)?; }

    let connection = connect()?;
    connection.prepare(
        "UPDATE Users
         SET tracked = 0
         WHERE username = ?
        "
    )?.execute(params![&user.username])?;

    Ok(())
}

/// [internal] Checks if the user is in the database.
fn user_exists(user: &models::User) -> Result<bool> {
    let connection = connect()?;
    let exists = connection.prepare(
            "SELECT *
             FROM Users
             WHERE username = ?"
    )?.exists(params![&user.username])?;

    Ok(exists)
}

pub fn is_tracked(user: &models::User) -> Result<bool> {
    let connection = connect()?;
    let is_tracked = connection.prepare(&format!(
            "SELECT *
             FROM Users
             WHERE username = ? and tracked = 1"
    ))?.exists(params![&user.username])?;

    Ok(is_tracked)
}

/// Gathers all tracked users.
pub fn query_tracked_users() -> Result<Vec<models::User>> {
    todo!()
}

/// Gathers all recent submissions for a user.
pub fn query_all_recent_submissions(user: &models::User) -> Result<Vec<models::Submission>> {
    let connection = connect()?;
    let username = &user.username;

    // Get the current timestamp, approximately
    let current_timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .context("Time went backwards????")?
        .as_millis() as usize;

    // Parameters for our query. We're mainly trying to only grab submissions
    // that have been posted in the last `models::RECENT_THRESHOLD` milliseconds.
    let query_params = rusqlite::named_params! {
            ":username": username, 
            ":current_timestamp": current_timestamp, 
            ":recent_threshold": models::RECENT_THRESHOLD
    };

    // Preparation for the query.
    let mut stmt = connection.prepare(
            "SELECT s.username, s.timestamp, s.accepted,
                    p.problem_name, p.problem_link, p.difficulty
             FROM Submissions s
             JOIN Problems p ON s.problem_name = p.problem_name
             WHERE s.username = :username
               and :current_timestamp - s.timestamp < :recent_threshold
             ORDER BY s.timestamp DESC"
    )?;

    // Query!
    // PERFORMANCE CONSIDERATION: 
    //   We eagerly evaluate the iterator into a vector, here. Though it doesn't really make sense
    //   for us to really need _all_ 
    let submissions = stmt
        .query_map(query_params, |row| models::Submission::try_from(row))
        .context(format!("Could not find recent submissions for user: {username}"))?
        .collect::<Result<Vec<models::Submission>, _>>()?;

    Ok(submissions)
}
