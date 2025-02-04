use std::time::Duration;
use chrono::DateTime;

// Threshold for a problem to be considered 'recent' is 8 hours, or 28800 seconds
pub const RECENT_THRESHOLD: usize = Duration::new(28800, 0).as_millis() as usize;

#[derive(Debug, Clone)]
pub struct User {
    pub username: String,

    pub easy_solved: u64,
    pub medium_solved: u64,
    pub hard_solved: u64,
    pub total_solved: u64,

    pub ranking: u64,

    pub streak: u64,
}

impl std::fmt::Display for User {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "**User Stats:**\n\
             \tEasy Solved: {}\n\
             \tMedium Solved: {}\n\
             \tHard Solved: {}\n\
             \tTotal Solved: {}\n\
             \tRanking: {}\n\
             \tStreak: {}",
            self.easy_solved, self.medium_solved, self.hard_solved, self.total_solved, self.ranking, self.streak
        )
    }
}

#[derive(Debug)]
pub struct Submission {
    pub problem: Problem,

    pub username: String,
    pub language: String,
    pub timestamp: usize,
    pub accepted: bool,

    pub url: String,
}

#[derive(Debug)]
pub struct Problem {
    pub title: String,
    pub url: String,
    pub difficulty: String,
}

impl std::fmt::Display for Submission {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "**Submission**: {}\n\
            https://leetcode.com/problems/{}\n\
            \tAccepted?: *{}*\n\
            \tURL:       {} \n\
            \tTimestamp: {} \n\
            \tLanguage: `{}`",
            self.problem.title, self.problem.url, 
            self.url,
            self.accepted, 
            DateTime::from_timestamp(self.timestamp as i64, 0).unwrap_or_default(),
            self.language
        )
    }
}
