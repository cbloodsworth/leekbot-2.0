use anyhow::{Context, Result};

use std::time::{SystemTime, UNIX_EPOCH};

use crate::{lcdb::{connect, DBResult}, models};

/////*============== RECENT CACHE QUERIES ==============*/
/// Queries the database for submissions that haven't already been announced to the server.
pub fn query_uncached_submissions(user: &models::User) -> DBResult<Vec<models::Submission>> {
    let connection = connect()?;
    log::trace!("[query_uncached_submissions] Querying {} for uncached submissions...",
                 user.username);

    // Get the current timestamp, approximately
    let current_timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("Time went backwards????")
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
        "SELECT s.*,
                    p.problem_name, p.problem_link, p.difficulty
             FROM Submissions s
             JOIN Problems p ON s.problem_name = p.problem_name
             WHERE s.username = :username
               and :current_timestamp - s.timestamp < :recent_threshold
               and NOT EXISTS (
                 SELECT 1 
                 FROM RecentCache r 
                 WHERE r.timestamp = s.timestamp
                   and r.username = s.username
               )
             ORDER BY s.timestamp DESC",
    )?;

    let submissions = stmt
        .query_map(query_params, |row| {
            models::Submission::try_from(row)
                .inspect(|sub| 
                    log::trace!("[query_uncached_submissions] Found uncached submission: {sub}"))
                .inspect_err(|err| 
                    log::error!("[query_uncached_submissions] Could not convert row into \
                                 submission: {err}"))
            }
        )?
        .collect::<DBResult<Vec<models::Submission>>>()?;

    Ok(submissions)
}

/// Adds the (problem, user) entry into the recent cache if it doesn't exist.
/// Returns `true` if it was newly added, false otherwise.
pub fn insert_cache_submission(submission: &models::Submission) -> DBResult<bool> {
    log::trace!("[insert_cache_submission] Inserting submission into the cache.");
    let connection = connect()?;

    let query_params = rusqlite::named_params! {
            ":username": &submission.username,
            ":problem_name": &submission.problem.title,
            ":timestamp": &submission.timestamp,
            ":accepted": &submission.accepted,
    };

    // Preparation for the query.
    connection
        .prepare(
            "INSERT INTO RecentCache (username, problem_name, timestamp, accepted)
             VALUES (:username, :problem_name, :timestamp, :accepted)",
        )?
        .execute(query_params)
        .map_or_else(crate::lcdb::swallow_constraint_violation, |_| Ok(true))
}

/// Cleans the cache and returns the removed submissions.
pub fn clean_cache() -> Result<()> {
    log::trace!("[clean_cache] Clearing the cache.");
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
    connection
        .prepare(
            "DELETE FROM RecentCache
             WHERE :current_timestamp - timestamp > :recent_threshold",
        )?
        .execute(query_params)?;

    Ok(())
}