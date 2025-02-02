use anyhow::{Result, Context};
use reqwest::header::Keys;
use rusqlite::{params, Connection};

use std::time::{SystemTime, UNIX_EPOCH};

use crate::models;

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

            ranking        INTEGER     NOT NULL,
            streak         INTEGER     NOT NULL
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
            difficulty     TEXT        NOT NULL,

            UNIQUE(problem_name, problem_link, difficulty)
        )",
        []
    )?;

    // Recent Submission Cache
    log::debug!("[initialize_db] creating RecentCache table...");
    connect()?.execute(
        "CREATE TABLE IF NOT EXISTS RecentCache (
            problem_name   TEXT        NOT NULL    REFERENCES Problems(problem_name),
            username       TEXT        NOT NULL    REFERENCES Users(username),
            timestamp      TIMESTAMP   NOT NULL,

            UNIQUE (problem_name, username)
        )",
        []
    )?;

    Ok(())
}



/////*============== SUBMISSION QUERIES ==============*/
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

/// Gathers all recent submissions for a user.
pub fn query_submissions_recent_all(user: &models::User) -> Result<Vec<models::Submission>> {
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

pub fn insert_submission(submission: &models::Submission) -> Result<()> {
    let connection = connect()?;

    log::info!("[insert_submission] Inserting submission into Submissions...");

    let query_params = rusqlite::named_params! {
            ":problem_name": submission.problem.title, 
            ":username":     submission.username,
            ":language":     submission.language, 
            ":timestamp":    submission.timestamp, 
            ":accepted":     submission.accepted, 
    };

    connection.prepare(
        "INSERT INTO Submissions ( problem_name,  username,  language,  timestamp,  accepted)
         VALUES                  (:problem_name, :username, :language, :timestamp, :accepted)"
    )?.execute(query_params)?;

    Ok(())
}



/////*============== RECENT CACHE QUERIES ==============*/

/// Queries the database for (accepted) submissions that haven't already been announced to the server.
pub fn query_uncached_submissions(user: &models::User) -> Result<Vec<models::Submission>> {
    let connection = connect()?;

    // Get the current timestamp, approximately
    let current_timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .context("Time went backwards????")?
        .as_millis() as usize;

    // Parameters for our query. We're mainly trying to only grab submissions
    // that have been posted in the last `models::RECENT_THRESHOLD` seconds.
    let query_params = rusqlite::named_params! {
            ":username": &user.username, 
            ":current_timestamp": current_timestamp, 
            ":recent_threshold": models::RECENT_THRESHOLD
    };

    // Preparation for the query.
    let mut stmt = connection.prepare(
            "SELECT s.username, s.timestamp, s.accepted, s.language,
                    p.problem_name, p.problem_link, p.difficulty
             FROM Submissions s
             JOIN Problems p ON s.problem_name = p.problem_name
             WHERE s.username = :username
               and :current_timestamp - s.timestamp < :recent_threshold
               and s.accepted = 1
               and NOT EXISTS (
                 SELECT 1 
                 FROM RecentCache r 
                 WHERE r.problem_name = s.problem_name
                   and r.username = s.username
               )
             ORDER BY s.timestamp DESC"
    )?;

    let submissions = stmt
        .query_map(query_params, |row| models::Submission::try_from(row))
        .context(format!("No recent submissions...?"))?  // This might not be the right error msg
        .collect::<Result<Vec<models::Submission>, _>>()?;

    // Keep our new cache updated
    log::info!("[query_submissions_new] Updating submissions cache...");
    for submission in submissions.iter() {
        insert_cache_submission(submission)?;
    }

    Ok(submissions)
}

/// Adds the (problem, user) entry into the recent cache if it doesn't exist.
pub fn insert_cache_submission(submission: &models::Submission) -> Result<()> {
    let connection = connect()?;

    let query_params = rusqlite::named_params! {
            ":username": &submission.username, 
            ":problem_name": &submission.problem.title,
            ":timestamp": &submission.timestamp,
    };

    // Preparation for the query.
    log::debug!("[insert_cache_submission] caching submission...");
    connection.prepare(
            "INSERT OR IGNORE INTO RecentCache (username, problem_name, timestamp)
             VALUES (:username, :problem_name, :timestamp)"
    )?.execute(query_params)?;

    Ok(())
}

/// Cleans the cache and returns the removed submissions.
pub fn clean_cache() -> Result<()> {
    let connection = connect()?;

    // Get the current timestamp, approximately
    let current_timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .context("Time went backwards????")?
        .as_millis() as usize;

    // Parameters for our query. We're mainly trying to only grab submissions
    // that have been posted in the last `models::RECENT_THRESHOLD` milliseconds.
    let query_params = rusqlite::named_params! {
            ":current_timestamp": current_timestamp, 
            ":recent_threshold": models::RECENT_THRESHOLD
    };

    // Preparation for the query.
    connection.prepare(
            "DELETE FROM RecentCache
             WHERE :current_timestamp - timestamp > :recent_threshold"
    )?.execute(query_params)?;

    Ok(())
}



/////*============== USER QUERIES ==============*/
impl<'a> TryFrom<&'a rusqlite::Row<'a>> for models::User {
    type Error = rusqlite::Error;

    fn try_from(row: &rusqlite::Row) -> Result<Self, rusqlite::Error> {
        Ok(Self {
            username:      row.get("username")?,
            easy_solved:   row.get("easy_solved")?,
            medium_solved: row.get("medium_solved")?,
            hard_solved:   row.get("hard_solved")?,
            total_solved:  row.get("total_solved")?,
            ranking:       row.get("ranking")?,
            streak:        row.get("streak")?,
        })
    }
}

/// Gathers all tracked users.
pub fn query_tracked_users() -> Result<Vec<models::User>> {
    let connection = connect()?;

    // Preparation for the query.
    let mut stmt = connection.prepare(
            "SELECT username, easy_solved, medium_solved, hard_solved, total_solved, ranking, streak
             FROM Users
             WHERE tracked = 1"
    )?;

    // Query!
    log::info!("[query_tracked_users)] Querying all tracked users.");
    let submissions = stmt
        .query_map([], |row| models::User::try_from(row))
        .context(format!("Could not find any users in the database."))?
        .collect::<Result<Vec<models::User>, _>>()?;

    Ok(submissions)
}

pub fn insert_user(user: &models::User) -> Result<()> {
    let connection = connect()?;

    log::info!("[insert_user] Inserting user {} into Users...", user.username);

    let query_params = rusqlite::named_params! {
            ":username":      user.username,
            ":tracked":       0,
            ":easy_solved":   user.easy_solved,
            ":medium_solved": user.medium_solved,
            ":hard_solved":   user.hard_solved,
            ":total_solved":  user.total_solved,
            ":ranking":       user.ranking, 
            ":streak":        user.streak, 
    };

    connection.prepare(
        "INSERT INTO Users ( username,  tracked,  easy_solved,  medium_solved,  hard_solved,  total_solved,  ranking,  streak)
         VALUES            (:username, :tracked, :easy_solved, :medium_solved, :hard_solved, :total_solved, :ranking, :streak)"
    )?.execute(query_params)?;

    Ok(())
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

/// Return whether a user is being tracked.
pub fn is_tracked(user: &models::User) -> Result<bool> {
    let connection = connect()?;
    let is_tracked = connection.prepare(&format!(
            "SELECT *
             FROM Users
             WHERE username = ? and tracked = 1"
    ))?.exists(params![&user.username])?;

    Ok(is_tracked)
}

/// Return whether a user has completed a problem in the last day.
pub fn is_active(user: &models::User) -> Result<bool> {
    let connection = connect()?;

    // Get the current timestamp, approximately
    let current_timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .context("Time went backwards????")?
        .as_millis() as usize;

    const DAY_IN_MILLIS: u64 = 86_400_000;
    let query_params = rusqlite::named_params! {
            ":username":      user.username,
            ":current_timestamp": current_timestamp,
            ":DAY_IN_MILLIS": DAY_IN_MILLIS,
    };

    let is_tracked = connection.prepare(&format!(
            "SELECT 1
             FROM Users u
             JOIN Submissions s ON s.username = u.username
             WHERE u.username = :username 
               and u.tracked = 1
               and :current_timestamp - s.timestamp < :DAY_IN_MILLIS"
    ))?.exists(query_params)?;

    Ok(is_tracked)
}

pub fn streak_increment(user: &models::User) -> Result<()> {
    let connection = connect()?;
    connection.prepare( "UPDATE Users 
                         SET streak = streak + 1 
                         WHERE username = ?")?
        .execute(params![&user.username])?;

    Ok(())
}

pub fn query_streak(user: &models::User) -> Result<u64> {
    let connection = connect()?;
    let mut stmt = connection.prepare( "SELECT streak FROM Users WHERE username = ?")?;
    Ok(stmt.query_row(params![&user.username], |row| row.get("streak"))?)
}

// Breaks the user's streak.
pub fn streak_break(user: &models::User) -> Result<()> {
    let connection = connect()?;
    connection.prepare( "UPDATE Users 
                         SET streak = 0
                         WHERE username = ?")?
        .execute(params![&user.username])?;

    Ok(())
}

/////*============== PROBLEM QUERIES ==============*/
pub fn insert_problem(problem: &models::Problem) -> Result<()> {
    let connection = connect()?;

    log::info!("[insert_problem] Inserting problem {} into Problems...", problem.title);

    let query_params = rusqlite::named_params! {
            ":problem_name": problem.title,
            ":problem_link": format!("https://leetcode.com/problems/{}", problem.titleSlug),
            ":difficulty":   problem.difficulty
    };

    connection.prepare(
        "INSERT OR IGNORE INTO Problems ( problem_name,  problem_link,  difficulty)
         VALUES                         (:problem_name, :problem_link, :difficulty)"
    )?.execute(query_params)?;

    Ok(())
}



/////*============== INTERNAL API ==============*/
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
