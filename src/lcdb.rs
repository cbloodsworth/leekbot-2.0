use anyhow::{Context, Result};
use rusqlite::{params, Connection};

use std::time::{SystemTime, UNIX_EPOCH};

use crate::models::{self, AnnouncementPreferences};

type DBResult<T> = Result<T, rusqlite::Error>;

fn connect() -> DBResult<Connection> {
    Connection::open("db/leek.db")
}

pub fn initialize_db() -> DBResult<()> {
    // User table
    log::info!("[initialize_db] creating Users table...");
    connect()?.execute(
        "CREATE TABLE IF NOT EXISTS Users (
            username       TEXT        PRIMARY KEY,

            easy_solved    INTEGER     NOT NULL,
            medium_solved  INTEGER     NOT NULL,
            hard_solved    INTEGER     NOT NULL,
            total_solved   INTEGER     NOT NULL,

            ranking        INTEGER     NOT NULL,
            streak         INTEGER     NOT NULL
        )",
        [],
    )?;

    // Submission table
    log::info!("[initialize_db] creating Submissions table...");
    connect()?.execute(
        "CREATE TABLE IF NOT EXISTS Submissions (
            problem_name   TEXT        NOT NULL    REFERENCES Problems(problem_name),

            username       TEXT        NOT NULL    REFERENCES Users(username),
            language       TEXT        NOT NULL,
            timestamp      TIMESTAMP   NOT NULL,
            accepted       BOOLEAN     NOT NULL,

            url TEXT        NOT NULL,

            UNIQUE(problem_name, username, timestamp)
        )",
        [],
    )?;

    // Problem table
    log::info!("[initialize_db] creating Problems table...");
    connect()?.execute(
        "CREATE TABLE IF NOT EXISTS Problems (
            problem_name   TEXT        PRIMARY KEY,
            problem_link   TEXT        NOT NULL,
            difficulty     TEXT        NOT NULL,

            UNIQUE(problem_name, problem_link, difficulty)
        )",
        [],
    )?;

    // Recent Submission Cache
    log::info!("[initialize_db] creating RecentCache table...");
    connect()?.execute(
        "CREATE TABLE IF NOT EXISTS RecentCache (
            problem_name   TEXT        NOT NULL    REFERENCES Problems(problem_name),
            username       TEXT        NOT NULL    REFERENCES Users(username),
            timestamp      TIMESTAMP   NOT NULL,
            accepted       BOOLEAN     NOT NULL,

            UNIQUE (problem_name, username, timestamp, accepted)
        )",
        [],
    )?;

    // UserPreferences
    log::info!("[initialize_db] creating UserPrefs table...");
    connect()?.execute(
        "CREATE TABLE IF NOT EXISTS UserPrefs (
            username          TEXT        NOT NULL    REFERENCES Users(username),

            tracked           BOOLEAN     NOT NULL,
            announce          BOOLEAN     NOT NULL,
            announce_fail     BOOLEAN     NOT NULL,
            announce_link     BOOLEAN     NOT NULL,

            UNIQUE (username)
        )",
        [],
    )?;

    Ok(())
}

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
        .map_or_else(swallow_constraint_violation, |_| Ok(true))
}

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
        .map_or_else(swallow_constraint_violation, |_| Ok(true))
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

/////*============== USER QUERIES ==============*/
impl<'a> TryFrom<&'a rusqlite::Row<'a>> for models::User {
    type Error = rusqlite::Error;

    fn try_from(row: &rusqlite::Row) -> Result<Self, rusqlite::Error> {
        Ok(Self {
            username: row.get("username")?,
            easy_solved: row.get("easy_solved")?,
            medium_solved: row.get("medium_solved")?,
            hard_solved: row.get("hard_solved")?,
            total_solved: row.get("total_solved")?,
            ranking: row.get("ranking")?,
            streak: row.get("streak")?,
        })
    }
}

impl<'a> TryFrom<&'a rusqlite::Row<'a>> for models::UserPreferences {
    type Error = rusqlite::Error;

    fn try_from(row: &rusqlite::Row) -> Result<Self, rusqlite::Error> {
        Ok(Self {
            tracked: row.get("tracked")?,
            announcement: row.get::<_, bool>("announce")?
                .then(|| AnnouncementPreferences {
                            announce_failures: row.get("announce_fail").unwrap_or(false),
                            has_submission_link: row.get("announce_link").unwrap_or(false),
                         }
                )
        })
    }
}

/// Returns the user with the username: `username`, if they exist.
pub fn query_user(username: &str) -> DBResult<Option<models::User>> {
    let connection = connect()?;

    connection
        .prepare("SELECT * FROM Users WHERE username = :username")?
        .query(rusqlite::named_params! { ":username": username })?
        .next()?
        .map(|x| x.try_into())
        .transpose()
}

/// Gathers all tracked users.
pub fn query_tracked_users() -> DBResult<Vec<models::User>> {
    log::trace!("[query_tracked_users)] Querying all tracked users.");
    let connection = connect()?;

    // Preparation for the query.
    let mut stmt = connection.prepare(
        "SELECT u.username, u.easy_solved, u.medium_solved, u.hard_solved,
                u.total_solved, u.ranking, u.streak
         FROM Users u
         JOIN UserPrefs p ON u.username = p.username
         WHERE p.tracked = 1",
    )?;

    // Query!
    let submissions = stmt
        .query_map([], |row| models::User::try_from(row))?
        .collect::<Result<Vec<models::User>, _>>()?;

    Ok(submissions)
}

pub fn insert_user(user: &models::User, prefs: &models::UserPreferences) -> DBResult<()> {
    let connection = connect()?;

    log::trace!(
        "[insert_user] Inserting user {} into Users...",
        user.username
    );

    let query_params = rusqlite::named_params! {
            ":username":      user.username,
            ":easy_solved":   user.easy_solved,
            ":medium_solved": user.medium_solved,
            ":hard_solved":   user.hard_solved,
            ":total_solved":  user.total_solved,
            ":ranking":       user.ranking,
            ":streak":        user.streak,
    };

    connection.prepare(
        "INSERT INTO Users ( username,  easy_solved,  medium_solved,  hard_solved,
                             total_solved,  ranking,  streak)
         VALUES            (:username, :easy_solved, :medium_solved, :hard_solved,
                            :total_solved, :ranking, :streak)"
    )?.execute(query_params)?;

    log::info!("User {} has been added to the database.", user.username);

    insert_user_preferences(user, prefs)?;
    log::info!("User preferences for {} have been initialized.", user.username);

    Ok(())
}

/// Tracks a user by updating the "tracked" field in UserPrefs to true.
///   Inserts the user if it isn't in the database already.
pub fn track_user(user: &models::User) -> DBResult<()> {
    let username = &user.username;
    log::trace!("[track_user] Tracking user {}...", username);

    if !user_exists(user)? {
        log::trace!("[track_user] User '{}' does not already exist, adding to database.", username);
        insert_user(user, &models::DEFAULT_USER_PREFERENCES)?;
    }

    if query_user_preferences(user)?.is_none() {
        insert_user_preferences(user, &models::DEFAULT_USER_PREFERENCES)?;
    }

    let connection = connect()?;
    connection
        .prepare("UPDATE UserPrefs SET tracked = 1 WHERE username = :username")?
        .execute(rusqlite::named_params! { ":username": username, })
        .inspect_err(|err| log::error!("[track_user] Error tracking user '{username}': {err}"))?;

    Ok(())
}

/// Untracks a user by updating the "tracked" field to false.
///   Inserts the user if it isn't in the database already.
pub fn untrack_user(user: &models::User) -> DBResult<()> {
    if !user_exists(user)? {
        insert_user(user, &models::DEFAULT_USER_PREFERENCES)?;
    }

    let connection = connect()?;
    connection
        .prepare("UPDATE UserPrefs SET tracked = 0 WHERE username = :username")?
        .execute(rusqlite::named_params! { ":username": user.username, })?;

    Ok(())
}

/// Return whether a user is being tracked.
pub fn is_tracked(user: &models::User) -> DBResult<bool> {
    let connection = connect()?;
    connection
        .prepare("SELECT * FROM UserPrefs WHERE username = :username AND tracked = 1")?
        .exists(rusqlite::named_params! { ":username": user.username, })
}

/// Retrieves a user's preferences from the database.
///
/// Returns None if no such `user` is in the database.
pub fn query_user_preferences(user: &models::User) -> DBResult<Option<models::UserPreferences>> {
    let connection = connect()?;
    connection
        .prepare("SELECT * FROM UserPrefs WHERE username = :username")?
        .query(rusqlite::named_params! { ":username": user.username })?
        .next()?
        .map(|row| row.try_into())
        .transpose()
}

/// Updates a user's preferences into the database.
pub fn update_user_preferences(
    user: &models::User,
    prefs: &models::UserPreferences
) -> DBResult<()>
{
    let connection = connect()?;
    let query_params = rusqlite::named_params! {
            ":username":      user.username,
            ":tracked":       prefs.tracked,
            ":announce":      prefs.announcement.is_some(),
            ":announce_fail": prefs.announcement.as_ref().is_some_and(|a| a.announce_failures),
            ":announce_link": prefs.announcement.as_ref().is_some_and(|a| a.has_submission_link)
    };

    connection
        .prepare(
            "UPDATE UserPrefs SET
                tracked = :tracked,
                announce = :announce,
                announce_fail = :announce_fail,
                announce_link = :announce_link
             WHERE username = :username"
        )?
        .execute(query_params)
        .inspect_err(|err| log::error!("[update_user_preferences] Could not update user \
                                        preferences: {err}"))?;

    Ok(())
}

/// Inserts user's preferences into the database, doing nothing if they're already there.
pub fn insert_user_preferences(
    user: &models::User,
    prefs: &models::UserPreferences
) -> DBResult<bool>
{
    let connection = connect()?;
    let query_params = rusqlite::named_params! {
            ":username":      user.username,
            ":tracked":       prefs.tracked,
            ":announce":      prefs.announcement.is_some(),
            ":announce_fail": prefs.announcement.as_ref().is_some_and(|a| a.announce_failures),
            ":announce_link": prefs.announcement.as_ref().is_some_and(|a| a.has_submission_link)
    };

    connection
        .prepare(
            "INSERT INTO UserPrefs (username,   tracked,  announce,  announce_fail,  announce_link)
             VALUES                (:username, :tracked, :announce, :announce_fail, :announce_link)"
        )?
        .execute(query_params)
        .map_or_else(swallow_constraint_violation, |_| Ok(true))
}


/// Return whether a user has completed a problem in the last day.
pub fn is_active(user: &models::User) -> DBResult<bool> {
    let connection = connect()?;

    // Get the current timestamp, approximately
    let current_timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("Time went backwards????")
        .as_millis() as usize;

    const DAY_IN_MILLIS: u64 = 86_400_000;
    let query_params = rusqlite::named_params! {
            ":username":      user.username,
            ":current_timestamp": current_timestamp,
            ":DAY_IN_MILLIS": DAY_IN_MILLIS,
    };

    let is_tracked = connection
        .prepare(
            "SELECT 1
             FROM Users u
             JOIN UserPrefs   p ON p.username = u.username
             JOIN Submissions s ON s.username = u.username
             WHERE u.username = :username
               and p.tracked = 1
               and s.accepted = 1
               and :current_timestamp - s.timestamp < :DAY_IN_MILLIS",
        )?
        .exists(query_params)
        .inspect_err(|err| log::error!("[is_active] Could not check if user was active: {err}"))?;

    Ok(is_tracked)
}

pub fn streak_increment(user: &models::User) -> DBResult<()> {
    let connection = connect()?;
    connection
        .prepare(
            "UPDATE Users 
                         SET streak = streak + 1 
                         WHERE username = ?",
        )?
        .execute(params![&user.username])?;

    Ok(())
}

pub fn query_streak(user: &models::User) -> DBResult<u64> {
    log::trace!("[query_streak] Querying streak for {}...", user.username);
    let connection = connect()?;
    let mut stmt = connection.prepare("SELECT streak FROM Users WHERE username = ?")?;
    stmt.query_row(params![&user.username], |row| row.get("streak"))
}

// Breaks the user's streak.
pub fn streak_break(user: &models::User) -> DBResult<()> {
    let connection = connect()?;
    connection
        .prepare("UPDATE Users SET streak = 0 WHERE username = ?")?
        .execute(params![&user.username])?;

    Ok(())
}

/////*============== PROBLEM QUERIES ==============*/
/// Inserts the problem into Problems, or does nothing if it already is there.
/// Returns `true` if it was newly added, false otherwise.
pub fn insert_problem(problem: &models::Problem) -> DBResult<bool> {
    let connection = connect()?;

    log::trace!(
        "[insert_problem] Inserting problem {} into Problems...",
        problem.title
    );

    let query_params = rusqlite::named_params! {
            ":problem_name": problem.title,
            ":problem_link": format!("https://leetcode.com/problems/{}", problem.url),
            ":difficulty":   problem.difficulty
    };

    connection
        .prepare(
            "INSERT INTO Problems ( problem_name,  problem_link,  difficulty)
         VALUES                         (:problem_name, :problem_link, :difficulty)",
        )?
        .execute(query_params)
        .map_or_else(swallow_constraint_violation, |_| Ok(true))
}

/////*============== INTERNAL API ==============*/
/// [internal] Checks if the user is in the database.
fn user_exists(user: &models::User) -> DBResult<bool> {
    let connection = connect()?;
    connection
        .prepare("SELECT * FROM Users WHERE username = :username")?
        .exists(rusqlite::named_params!{ ":username": user.username })
}

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
