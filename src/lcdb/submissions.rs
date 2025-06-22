use anyhow::Result;
use std::time::{SystemTime, UNIX_EPOCH};
use crate::{lcdb::{DBResult, connect}, models};

/////*============== SUBMISSION QUERIES ==============*/
impl<'a> TryFrom<&'a rusqlite::Row<'a>> for models::Submission {
    type Error = rusqlite::Error;

    fn try_from(row: &rusqlite::Row) -> Result<Self, rusqlite::Error> {
        let problem = models::Problem {
            title: row.get("problem_name")?,
            url: row.get("problem_link")?,
            difficulty: row.get("difficulty")?,
        };

        Ok(Self {
            problem,

            username: row.get("username")?,
            accepted: row.get("accepted")?,
            language: row.get("language")?,
            timestamp: row.get("timestamp")?,

            url: row.get("url")?,
        })
    }
}

/// Gathers all recent submissions for a user.
pub fn query_submissions_recent_all(user: &models::User) -> DBResult<Vec<models::Submission>> {
    let connection = connect()?;
    let username = &user.username;

    // Get the current timestamp, approximately
    let current_timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("Time went backwards...?")
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
           AND :current_timestamp - s.timestamp < :recent_threshold
         ORDER BY s.timestamp DESC",
    )?;

    // Query!
    // PERFORMANCE CONSIDERATION:
    //   We eagerly evaluate the iterator into a vector, here. Though it doesn't really make sense
    //   for us to really need _all_
    let submissions = stmt
        .query_map(query_params, |row| models::Submission::try_from(row))?
        .collect::<DBResult<Vec<models::Submission>>>()?;

    Ok(submissions)
}

/// Inserts a Submission into the database.
/// Returns `true` if it was newly added, false otherwise.
pub fn insert_submission(submission: &models::Submission) -> DBResult<bool> {
    let connection = connect()?;

    log::trace!("[insert_submission] Inserting submission for {} into Submissions...",
        submission.problem.title);

    let query_params = rusqlite::named_params! {
            ":problem_name":   submission.problem.title,
            ":username":       submission.username,
            ":language":       submission.language,
            ":timestamp":      submission.timestamp,
            ":accepted":       submission.accepted,
            ":url":            submission.url,
    };

    connection
        .prepare(
            "INSERT INTO Submissions 
                ( problem_name,  username,  language,  timestamp,  accepted,  url)
            VALUES 
                (:problem_name, :username, :language, :timestamp, :accepted, :url)"
        )?
        .execute(query_params)
        .map_or_else(crate::lcdb::swallow_constraint_violation, |_| Ok(true))
}