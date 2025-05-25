use anyhow::{Context, Result, anyhow};
use reqwest::Client;
use reqwest::header::{self, HeaderMap, HeaderName, HeaderValue};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fs;

use crate::models::*;

#[derive(Serialize)]
struct RequestBody {
    query: String,
    variables: Value,
}

#[derive(Deserialize)]
struct QueryResponse {
    data: Option<Value>,
}

/// Reaches out to LeetCode to see if `username` has any problems that have been
/// submitted in the last few days.
pub async fn fetch_recently_submitted(username: &str) -> Result<Vec<Submission>> {
    log::trace!("[fetch_recently_submitted] Fetching recently submitted for '{username}'");
    let response = query_user(username)
        .await
        .inspect_err(|err| 
            log::error!("[fetch_recently_submitted] Couldn't query user '{username}': {err}"))?;

    let data = response.data.context("No data found in the response.")?;

    let raw_submissions = data
        .get("recentSubmissionList")
        .context(err_cant_get("recentSubmissionList", username))?;

    match raw_submissions {
        Value::Array(submissions) => Ok(submissions
            .iter()
            .filter_map(|val| {
                let problem = Problem {
                    title: val.get("title")?.as_str()?.to_string(),
                    url: val.get("titleSlug")?.as_str()?.to_string(),
                    difficulty: String::from("NULL"),
                };
                let sub = Submission {
                    username: username.to_string(),
                    language: val.get("lang")?.as_str()?.to_string(),
                    timestamp: val.get("timestamp")?.as_str()?.parse::<usize>().ok()? * 1000,
                    accepted: val.get("statusDisplay")?.as_str()? == "Accepted",
                    url: {
                        format!(
                            "https://leetcode.com/problems/{}/submissions/{}/",
                            problem.url.clone(),
                            val.get("id")?.as_str()?
                        )
                    },

                    problem,
                };

                Some(sub)
            })
            .collect()),
        Value::Null => Err(anyhow!(
            "Could not find recent submissions for user {username}."
        )),
        _ => Err(anyhow!(
            "Recent submissions list for {username} was not an array."
        )),
    }
}

pub async fn fetch_recently_completed(username: &str) -> Result<Vec<Submission>> {
    let submitted = fetch_recently_submitted(username).await?;

    // Only grab the ones that were accepted
    Ok(submitted.into_iter().filter(|sub| sub.accepted).collect())
}

// i wont lie this is a hot mess
pub async fn fetch_user(username: &str) -> Result<User> {
    let response = query_user(&username).await?;
    let data = response.data.context("No data found in the response.")?;

    // Retrieve user, or raise error if it doesn't exist
    let user = data
        .get("matchedUser")
        .and_then(|user| {
            if *user == Value::Null {
                None
            } else {
                Some(user)
            }
        })
        .with_context(|| format!("Leetcode user {} does not exist.", username))?;

    // Get the number of solved problems array
    let num_solved_array = user
        .get("submitStats")
        .context(err_cant_get("submission statistics", &username))?
        .get("acSubmissionNum")
        .context("Couldn't retrieve submission statistics.")?
        .as_array()
        .context("Malformed submission data; check JSON schema.")?;

    let ranking = user
        .get("profile")
        .context(err_cant_get("profile", &username))?
        .get("ranking")
        .context(err_cant_get("ranking", &username))?
        .as_u64()
        .context("Could not convert ranking to u64.")?;

    Ok(User {
        username: username.to_owned(),
        ranking,
        total_solved: extract_u64_from_json(&num_solved_array[0], "count")
            .context("Couldn't get total_solved")?,
        easy_solved: extract_u64_from_json(&num_solved_array[1], "count")
            .context("Couldn't get easy_solved")?,
        medium_solved: extract_u64_from_json(&num_solved_array[2], "count")
            .context("Couldn't get medium_solved")?,
        hard_solved: extract_u64_from_json(&num_solved_array[3], "count")
            .context("Couldn't get hard_solved")?,
        streak: 0,
    })
}

/// Runs a GraphQL query on the leetcode servers for `username`.
async fn query_user(username: &str) -> Result<QueryResponse> {
    let query = read_query_from_file("queries/lcuser.graphql")?;
    let variables = serde_json::json!({ "username": username });
    let body = RequestBody { query, variables };
    let headers = HeaderMap::from_iter([
        (
            header::CONTENT_TYPE,
            HeaderValue::from_static("application/json"),
        ),
        (
            HeaderName::from_static("referer"),
            HeaderValue::from_str("https://leetcode.com")?,
        ),
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

fn read_query_from_file(path: &str) -> Result<String> {
    fs::read_to_string(path).with_context(|| format!("No such file or directory: {}", path))
}

/// Returns an error message for when a JSON attribute can't be obtained.
fn err_cant_get(attribute: &str, username: &str) -> String {
    format!("Couldn't get {} for {}", attribute, username)
}

fn extract_u64_from_json(value: &Value, key: &str) -> Result<u64> {
    value
        .get(key)
        .with_context(|| format!("Missing key: {}", key))?
        .as_u64()
        .context("Could not convert json integer into u64")
}
