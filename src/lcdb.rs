use anyhow::{Result, Context};
use rusqlite::{params, Connection};

use crate::models;

fn connect() -> Result<Connection> {
    Ok(Connection::open("leek.db")?)
}

pub fn initialize_db() -> Result<()> {
    // User table
    connect()?.execute(
        "CREATE TABLE IF NOT EXISTS Users (
            username       TEXT        PRIMARY KEY,
            tracked        BOOLEAN     NOT NULL
        )",
        []
    )?;

    // Submission table
    connect()?.execute(
        "CREATE TABLE IF NOT EXISTS CompletedProblems (
            id             INTEGER     PRIMARY KEY,

            problem_name   TEXT        NOT NULL,
            difficulty     TEXT        NOT NULL,
            timestamp      TIMESTAMP   NOT NULL,

            language       TEXT        NOT NULL,
            username       TEXT        NOT NULL    REFERENCES Users(username)
        )",
        []
    )?;


    Ok(())
}

pub fn query_tracked_users() -> Result<Vec<models::User>> {
    todo!()
}

pub fn insert_user(user: &models::User) -> Result<()> {
    let connection = connect()?;

    connection.prepare(
        "INSERT INTO Users (username, tracked)
         VALUES (?, ?)"
    )?.execute([&user.username, "0"])?;

    Ok(())
}

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

pub fn query_most_recent_problem(username: &str) -> Result<models::Submission> {
    let connection = connect()?;
    let mut stmt = connection.prepare(&format!(
            "SELECT problem_name, difficulty, language, timestamp
             FROM CompletedProblems
             WHERE username = ?
             ORDER BY timestamp DESC")
    ).context(format!("Could not find recent problems for user: {username}"))?;

    // TODO: JESUS FIX
    let problem_iter = stmt.query_map([username], |row| {
        Ok(models::Submission {
            titleSlug:  row.get(0)?,
            lang:       row.get(2)?,
            timestamp:  row.get(3)?,

            title:         String::from(""),
            statusDisplay: String::from(""),
        })
    })?;

    for problem in problem_iter {
        println!("{}", problem?);
    }

    todo!()
}
