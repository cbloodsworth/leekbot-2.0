use rusqlite::{params, Connection, Result};

fn initialize_db() -> Result<()> {
    let connection = Connection::open("leek.db")?;

    connection.execute(
        "CREATE TABLE IF NOT EXISTS users (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            username TEXT NOT NULL
        )",
        []
    )?;

    Ok(())
}
