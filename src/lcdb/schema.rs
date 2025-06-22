pub const USER_SCHEMA: &str = 
    "CREATE TABLE IF NOT EXISTS Users (
        username       TEXT        PRIMARY KEY,

        easy_solved    INTEGER     NOT NULL,
        medium_solved  INTEGER     NOT NULL,
        hard_solved    INTEGER     NOT NULL,
        total_solved   INTEGER     NOT NULL,

        ranking        INTEGER     NOT NULL,
        streak         INTEGER     NOT NULL
    )";

pub const SUBMISSIONS_SCHEMA: &str =
    "CREATE TABLE IF NOT EXISTS Submissions (
        problem_name   TEXT        NOT NULL    REFERENCES Problems(problem_name),

        username       TEXT        NOT NULL    REFERENCES Users(username),
        language       TEXT        NOT NULL,
        timestamp      TIMESTAMP   NOT NULL,
        accepted       BOOLEAN     NOT NULL,

        url TEXT        NOT NULL,

        UNIQUE(problem_name, username, timestamp)
    )";

pub const PROBLEMS_SCHEMA: &str =
    "CREATE TABLE IF NOT EXISTS Problems (
        problem_name   TEXT        PRIMARY KEY,
        problem_link   TEXT        NOT NULL,
        difficulty     TEXT        NOT NULL,

        UNIQUE(problem_name, problem_link, difficulty)
    )";

pub const RECENT_CACHE_SCHEMA: &str = 
    "CREATE TABLE IF NOT EXISTS RecentCache (
        problem_name   TEXT        NOT NULL    REFERENCES Problems(problem_name),
        username       TEXT        NOT NULL    REFERENCES Users(username),
        timestamp      TIMESTAMP   NOT NULL,
        accepted       BOOLEAN     NOT NULL,

        UNIQUE (problem_name, username, timestamp, accepted)
    )";

pub const USER_PREFS_SCHEMA: &str = 
    "CREATE TABLE IF NOT EXISTS UserPrefs (
        username          TEXT        NOT NULL    REFERENCES Users(username),

        tracked           BOOLEAN     NOT NULL,
        announce          BOOLEAN     NOT NULL,
        announce_fail     BOOLEAN     NOT NULL,
        announce_link     BOOLEAN     NOT NULL,

        UNIQUE (username)
    )";

pub const USER_COINS_SCHEMA: &str =
    "CREATE TABLE IF NOT EXISTS UserCoins (
        username       TEXT        NOT NULL    REFERENCES Users(username),
        coins          INTEGER     DEFAULT 0
    )";