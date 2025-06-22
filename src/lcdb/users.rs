use anyhow::Result;
use rusqlite::params;

use std::time::{SystemTime, UNIX_EPOCH};

use crate::{lcdb::{DBResult, connect}, models::{self, AnnouncementPreferences}};

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
        .map_or_else(crate::lcdb::swallow_constraint_violation, |_| Ok(true))
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

/// [internal] Checks if the user is in the database.
fn user_exists(user: &models::User) -> DBResult<bool> {
    let connection = connect()?;
    connection
        .prepare("SELECT * FROM Users WHERE username = :username")?
        .exists(rusqlite::named_params!{ ":username": user.username })
}
