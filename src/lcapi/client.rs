use reqwest::header::{self, HeaderMap, HeaderName, HeaderValue};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use anyhow::{Error, Context, Result};
use std::collections::HashSet;
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

/// Returns an error message for when a JSON attribute can't be obtained.
fn err_cant_get(attribute: &str, username: &str) -> String {
    format!("Couldn't get {} for {}", attribute, username)
}

fn read_query(path: &str) -> Result<String> {
    Ok(fs::read_to_string(path)
        .with_context(|| format!("No such file or directory: {}", path))?)
}

fn extract_u64_from_json(value: &Value, key: &str) -> Result<u64> {
    value.get(key)
        .with_context(|| format!("Missing key: {}", key))?
        .as_u64().context("Could not convert json integer into u64")
}

/// Runs a GraphQL query on the leetcode servers for `username`.
async fn query_user(username: &str) -> Result<QueryResponse> {
    let query = read_query("src/lcapi/lcuser.graphql")?;
    let variables = serde_json::json!({ "username": username });
    let body = RequestBody { query, variables };
    let headers = HeaderMap::from_iter([
        (header::CONTENT_TYPE, HeaderValue::from_static("application/json")),
        (HeaderName::from_static("referer"), HeaderValue::from_str("https://leetcode.com")?)
    ]);

    Ok(Client::new()
            .post("https://leetcode.com/graphql")
            .headers(headers)
            .json(&body)
            .send()
            .await?
            .json::<QueryResponse>()
            .await?)
}

#[derive(Serialize, Deserialize)]
pub struct Submission {
    statusDisplay: String,
    lang: String,
    timestamp: String,
    title: String,
    titleSlug: String,
}

impl std::fmt::Display for Submission {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "**Submission**: {}\n\
            \tStatus: *{}*\n\
            \tLanguage: `{}`",
            self.title, self.statusDisplay, self.lang
        )
    }
}

pub async fn fetch_recently_submitted(username: &str) -> Result<Vec<Submission>> {
    let response = query_user(username).await?;
    let data = response
        .data
        .context("No data found in the response.")?;

    let raw_submissions = data
        .get("recentSubmissionList")
        .context(err_cant_get("recentSubmissionList", username))?
        .as_array()
        .context("Couldn't deserialize recentSubmissionList into an array.")?;

    raw_submissions
        .into_iter()
        .map(|val| serde_json::from_value::<Submission>(val.clone()).context("Couldn't deserialize values into Submissions."))
        .collect()

}

pub async fn fetch_recently_completed(username: &str) -> Result<Vec<Submission>> {
    let submitted = fetch_recently_submitted(username).await?;
    
    // Only grab the ones that were accepted
    Ok(submitted
        .into_iter()
        .filter(|sub| sub.statusDisplay == String::from("Accepted"))
        .collect())
}

pub async fn fetch_user(username: &str) -> Result<UserData> {
    let response = query_user(username).await?;
    let data = response.data.context("No data found in the response.")?;

    // Retrieve user, or raise error if it doesn't exist
    let user = data.get("matchedUser").and_then(|user| {
        if *user == Value::Null { None } 
        else { Some(user) }
    }).with_context(|| format!("Could not find leetcode user: {}", username))?;

    // Get the number of solved problems array
    let num_solved_array = user
        .get("submitStats").context(err_cant_get("submission statistics", username))?
        .get("acSubmissionNum").context("Couldn't retrieve submission statistics.")?
        .as_array().context("Malformed submission data; check JSON schema.")?;

    let ranking = user
        .get("profile").context(err_cant_get("profile", username))?
        .get("ranking").context(err_cant_get("ranking", username))?
        .as_u64().context("Could not convert ranking to u64.")?;

    Ok(UserData {
        ranking,
        total_solved:  extract_u64_from_json(&num_solved_array[0], "count").context("Couldn't get total_solved")?,
        easy_solved:   extract_u64_from_json(&num_solved_array[1], "count").context("Couldn't get easy_solved")?, 
        medium_solved: extract_u64_from_json(&num_solved_array[2], "count").context("Couldn't get medium_solved")?, 
        hard_solved:   extract_u64_from_json(&num_solved_array[3], "count").context("Couldn't get hard_solved")?, 
    })
}
