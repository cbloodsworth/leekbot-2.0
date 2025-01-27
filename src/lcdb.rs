use anyhow::{Result, Context};
use rusqlite::{params, Connection};
use crate::models::*;

fn connect() -> Result<Connection> {
    Ok(Connection::open("leek.db")?)
}

pub fn initialize_db() -> Result<()> {
    // User table
    connect()?.execute(
        "CREATE TABLE IF NOT EXISTS users (
            username       TEXT        PRIMARY KEY
        )",
        []
    )?;

    // Submission table
    connect()?.execute(
        "CREATE TABLE IF NOT EXISTS completed_problems (
            id             INTEGER     PRIMARY KEY,

            problem_name   TEXT        NOT NULL,
            difficulty     TEXT        NOT NULL,
            timestamp      TIMESTAMP   NOT NULL,

            language       TEXT        NOT NULL,
            username       TEXT        NOT NULL    REFERENCES users(username)
        )",
        []
    )?;


    Ok(())
}

pub fn query_most_recent_problem(username: &str) -> Result<Submission> {
    let connection = connect()?;

    let mut stmt = connection.prepare(&format!(
            "SELECT problem_name, difficulty, language, timestamp
             FROM completed_problems
             WHERE username == {username}
             ORDER BY timestamp DESC")
    ).context(format!("Could not find recent problems for user: {username}"))?;

    // TODO: JESUS FIX
    let problem_iter = stmt.query_map([], |row| {
        Ok(Submission {
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
