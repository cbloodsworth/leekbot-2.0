use serde::{Serialize, Deserialize};
use serde::de::{self, Deserializer};


#[derive(Debug, Clone)]
pub struct User {
    pub username: String,

    pub total_solved: u64,
    pub easy_solved: u64,
    pub medium_solved: u64,
    pub hard_solved: u64,
    pub ranking: u64,
}

impl std::fmt::Display for User {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "User Stats:\n\
             Total Solved: {}\n\
             Easy Solved: {}\n\
             Medium Solved: {}\n\
             Hard Solved: {}\n\
             Ranking: {}",
            self.total_solved, self.easy_solved, self.medium_solved, self.hard_solved, self.ranking
        )
    }
}

#[derive(Serialize, Deserialize)]
#[allow(non_snake_case)]
pub struct Submission {
    pub statusDisplay: String,
    pub lang: String,
    #[serde(deserialize_with = "string_to_usize")]
    pub timestamp: usize,
    pub title: String,
    pub titleSlug: String,
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
            \tStatus: *{}*\n\
            \tTimestamp: {} \n\
            \tLanguage: `{}`",
            self.title, self.titleSlug, self.statusDisplay, self.timestamp, self.lang
        )
    }
}
