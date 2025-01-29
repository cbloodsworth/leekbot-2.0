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
            username       TEXT        PRIMARY KEY
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

pub fn is_tracked(user: &models::User) -> Result<bool> {
    let connection = connect()?;
    let username = &user.username;

    // MIght not work
    let exists = connection.prepare(&format!(
            "SELECT *
             FROM Users
             WHERE username == {username} && tracked"
    )).is_ok();

    Ok(exists)
}

pub fn query_most_recent_problem(username: &str) -> Result<models::Submission> {
    let connection = connect()?;

    let mut stmt = connection.prepare(&format!(
            "SELECT problem_name, difficulty, language, timestamp
             FROM CompletedProblems
             WHERE username == {username}
             ORDER BY timestamp DESC")
    ).context(format!("Could not find recent problems for user: {username}"))?;

    // TODO: JESUS FIX
    let problem_iter = stmt.query_map([], |row| {
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
