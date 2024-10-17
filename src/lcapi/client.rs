use reqwest::header::{self, HeaderMap, HeaderName, HeaderValue};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::error::Error;
use std::fs;

#[derive(Serialize)]
struct RequestBody {
    query: String,
    variables: Value,
}

#[derive(Deserialize)]
struct QueryResponse {
    data: Option<Value>,
}

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

fn read_query(path: &str) -> Result<String, Box<dyn Error>> {
    Ok(fs::read_to_string(path)?)
}

fn extract_u64(value: &Value, keys: &[&str]) -> Result<u64, Box<dyn Error>> {
    let mut current = value;
    for &key in keys {
        current = current
            .get(key)
            .ok_or_else(|| format!("Missing key: {}", key))?;
    }
    current
        .as_u64()
        .ok_or_else(|| "Expected a u64 value.".into())
}

pub async fn fetch_user(username: String) -> Result<UserData, Box<dyn Error>> {
    let query = read_query("src/lcapi/lcuser.graphql")?;
    let variables = serde_json::json!({ "username": username });
    let body = RequestBody { query, variables };
    let headers = HeaderMap::from_iter([
        (header::CONTENT_TYPE, HeaderValue::from_static("application/json")),
        (HeaderName::from_static("referer"), HeaderValue::from_str("https://leetcode.com")?)
    ]);

    let response = Client::new()
        .post("https://leetcode.com/graphql")
        .headers(headers)
        .json(&body)
        .send()
        .await?
        .json::<QueryResponse>()
        .await?;

    let data = response.data.ok_or("No data found in the response.")?;

    let solved_array = data.get("matchedUser")
        .and_then(|user| user.get("submitStats"))
        .and_then(|stats| stats.get("acSubmissionNum"))
        .and_then(|array| array.as_array())
        .ok_or("Malformed submission data; check JSON schema.")?;

    Ok(UserData {
        total_solved:  extract_u64(&solved_array[0], &["count"])?,
        easy_solved:   extract_u64(&solved_array[0], &["count"])?, 
        medium_solved: extract_u64(&solved_array[0], &["count"])?, 
        hard_solved:   extract_u64(&solved_array[0], &["count"])?, 
        ranking: extract_u64(&data, &["matchedUser", "profile", "ranking"])?
    })
}
