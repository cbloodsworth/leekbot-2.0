use serde::{Serialize, Deserialize};

#[derive(Debug, Clone)]
pub struct User {
    pub id: i32,
    pub username: String,
    pub data: UserData,
}

#[derive(Debug, Clone)]
pub struct UserData {
    pub total_solved: u64,
    pub easy_solved: u64,
    pub medium_solved: u64,
    pub hard_solved: u64,
    pub ranking: u64,
}

impl std::fmt::Display for UserData {
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
    pub timestamp: String,
    pub title: String,
    pub titleSlug: String,
}

impl std::fmt::Display for Submission {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "**Submission**: {}\n\
            https://leetcode.com/problems/{}/\n\
            \tStatus: *{}*\n\
            \tLanguage: `{}`",
            self.title, self.titleSlug, self.statusDisplay, self.lang
        )
    }
}
