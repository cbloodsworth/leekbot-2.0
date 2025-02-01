use serde::{Serialize, Deserialize};
use serde::de::{self, Deserializer};

use std::time::Duration;

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
             \tRanking: {}",
            self.easy_solved, self.medium_solved, self.hard_solved, self.total_solved, self.ranking
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
}

#[derive(Debug)]
#[allow(non_snake_case)] // for serialization
pub struct Problem {
    pub title: String,

    pub titleSlug: String,
    pub difficulty: String,
}

fn string_to_usize<'de, D>(deserializer: D) -> Result<usize, D::Error>
where D: Deserializer<'de>
{
    let s = String::deserialize(deserializer)?;
    s.parse::<usize>().map_err(de::Error::custom)
}

impl std::fmt::Display for Submission {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "**Submission**: {}\n\
            https://leetcode.com/problems/{}/\n\
            \tAccepted?: *{}*\n\
            \tTimestamp: {} \n\
            \tLanguage: `{}`",
            self.problem.title, self.problem.titleSlug, self.accepted, self.timestamp, self.language
        )
    }
}
