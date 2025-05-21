use rand::seq::IndexedRandom;
use serenity::async_trait;
use serenity::model::channel::Message;
use serenity::model::gateway::Ready;
use serenity::prelude::*;

use chrono::{Timelike, Utc};

use std::time::Duration as StdDuration;
use tokio::time::{Duration, sleep};

use dotenv::dotenv;
use std::env;

use crate::lcapi;
use crate::lcdb;
use crate::models;

use rand::rng;
use rand::seq::SliceRandom;

use anyhow::{Context, Result, anyhow};

const MAX_CMD_LENGTH: usize = 12;

fn is_debug_mode() -> bool {
    getenv_call_token() == '!'
}

/// Get the announcements channel ID. May panic.
fn getenv_announcements_channel() -> u64 {
    env::var("ANNOUNCEMENTS_CHANNEL_ID")
        .expect(".env file does not contain 'ANNOUNCEMENTS_CHANNEL_ID.")
        .parse()
        .expect("'ANNOUNCEMENTS_CHANNEL_ID should be parseable into a u64.")
}

/// Get the call token from the environment (.env file)
///
/// # Panics
/// If $BOT_CALL_TOKEN is not defined, or is more than a single character, will panic.
fn getenv_call_token() -> char {
    let env_token = env::var("BOT_CALL_TOKEN")
        .unwrap_or_else(|_| {
            log::error!("$BOT_CALL_TOKEN not defined. \n Please define a single-character call-token (i.e., $ or !)");
            panic!()
        });

    let token = env_token.chars().next().expect("BOT_CALL_TOKEN is empty.");
    if env_token.len() > 1 {
        log::warn!(
            "$BOT_CALL_TOKEN not a single character. Truncating to {}",
            token
        );
    }

    token
}

pub async fn run_leekbot() -> Result<()> {
    // Load discord bot token
    dotenv().ok();
    let token = env::var("DISCORD_TOKEN")
        .context("Expected 'DISCORD_TOKEN=<token>' in .env in project root.")?;

    let intents = GatewayIntents::GUILD_MESSAGES
        | GatewayIntents::DIRECT_MESSAGES
        | GatewayIntents::MESSAGE_CONTENT;

    let mut client = Client::builder(&token, intents)
        .event_handler(LeekHandler)
        .await
        .context("Error creating client.")?;

    client.start().await?;

    Ok(())
}

pub struct Commands;
impl Commands {
    pub async fn run_command(ctx: &serenity::client::Context, msg: &Message) -> Result<String> {
        // Split the message's content (on whitespace) into:
        // - The command (first token)
        // - Its parameters (all tokens afterwards)
        let input = String::from(&msg.content[1..]); // skip the first letter for the command: it's '$'
        let split_tokens = input.split_whitespace().collect::<Vec<_>>();
        let (&[command], parameters) = split_tokens.split_at(1) else {
            return Err(anyhow!("easd"));
        };

        // Execute the command
        let result: String = match command {
            "audit" => {
                let username = parameters
                    .first()
                    .context("Expected username for audit, got none.")?
                    .to_string();

                lcapi::fetch_user(username).await.map(|user| {
                    let tracked = lcdb::is_tracked(&user).unwrap();
                    let output = format!(
                        "{}\nThis user is {}currently being tracked.",
                        user,
                        if tracked { "" } else { "not " }
                    );

                    output
                })?
            }
            "recent" => Self::get_recently_completed(parameters[0]).await?,
            "tracklist" => {
                let mut output = String::from("**Tracked users:**");
                let users = lcdb::query_tracked_users();
                match users {
                    Ok(users) => {
                        for user in users {
                            output += "\n\t";
                            output += &user.username;
                        }
                    }
                    Err(err) => {
                        output = format!("Error retrieving tracklist: {err}");
                    }
                }

                output
            }
            "track" => {
                let username = parameters
                    .first()
                    .context("Expected username for tracking, got none.")?
                    .to_string();

                let user = lcapi::fetch_user(username).await?;
                lcdb::track_user(&user)?;

                msg.react(
                    &ctx.http,
                    serenity::all::ReactionType::Unicode(String::from("✅")),
                )
                .await?;
                String::from("")
            }
            "untrack" => {
                let username = parameters
                    .first()
                    .context("Expected username for untracking, got none.")?
                    .to_string();

                let user = lcapi::fetch_user(username).await?;
                lcdb::untrack_user(&user)?;

                msg.react(
                    &ctx.http,
                    serenity::all::ReactionType::Unicode(String::from("✅")),
                )
                .await?;
                String::from("")
            }
            "help" => Self::get_help(),
            "clanker" => String::from("call me clanker one more mf time"),
            "insert" => {
                if !is_debug_mode() {
                    String::from("This command is only available in debug mode.")
                } else {
                    let (params, problem_name) = parameters.split_at_checked(2).context(
                        "Expected usage: `!insert <username> <success|failure> <problem_name>`",
                    )?;

                    let username = params
                        .first()
                        .context("Expected username for tracking, got none.")?
                        .to_string();

                    let user = lcapi::fetch_user(username).await?;

                    let success = parameters
                        .get(1)
                        .context("Expected problem result (success | failure), got none.")?
                        .eq(&"success");

                    let problem = problem_name.join(" ");

                    lcdb::insert_fake_submission(&user, &problem, success)?;

                    format!("Inserted fake submission: {problem}")
                }
            }
            _ => {
                if Commands::is_valid_cmd(command) {
                    log::info!("User submitted unknown command: {}", command);
                    return Err(anyhow!(
                        "No such command found: {}, see $help for commands.",
                        command
                    ));
                } else {
                    log::info!("User submitted invalid command: {}", command);
                    return Err(anyhow!("Invalid command syntax."));
                }
            }
        };

        Ok(result)
    }

    async fn get_recently_completed(username: &str) -> Result<String> {
        Ok(format!(
            "{}",
            lcapi::fetch_recently_completed(username)
                .await?
                .first()
                .context(format!("No recently completed problems for {}", username))?
        ))
    }
}

/// Non-async helpers
impl Commands {
    /// Ensures that the string slice conforms to C-like identifier regex
    fn is_valid_cmd(s: &str) -> bool {
        s.len() <= MAX_CMD_LENGTH
            && regex::Regex::new(r"^[a-zA-Z_][a-zA-Z0-9_]*$")
                .unwrap()
                .is_match(s)
    }

    /// Gets a help string. Should be updated after a new command is added
    /// TODO: Generate automatically?
    pub fn get_help() -> String {
        let T = getenv_call_token();
        format!(
            r#"
**Command List:**
`{T}audit <leetcode username>`:  Get stats on a leetcode user.
`{T}recent <leetcode username>`:  Get the most recent submission from a leetcode user.
`{T}track <leetcode username>`:  Track a user. This will cause the bot to announce new submissions from this user.
`{T}untrack <leetcode username>`:  Untrack a user.
`{T}tracklist`:  List all tracked users.
`{T}help`:  Get information on supported commands
"#,
        )
    }
}

/// Checks recent Leetcode submissions for all tracked users,
///   compares with the ones represented in the recency cache,
///   and returns the ones that should be announced.
///
/// Intended to be run regularly.
/// Interfaces with all three modules: discord, leetcode API, database.
async fn check_recent_submissions() -> Result<Vec<models::Submission>> {
    let mut result = Vec::new();
    for user in lcdb::query_tracked_users()? {
        // Update database
        log::info!("[lcbot] Fetching recent problems for {}", user.username);
        let recent_submissions = lcapi::fetch_recently_submitted(&user.username).await?;
        for submission in recent_submissions {
            lcdb::insert_submission(&submission)?;
            lcdb::insert_problem(&submission.problem)?;
        }

        // Perform the query
        result.extend(lcdb::query_uncached_submissions(&user)?);
    }

    Ok(result)
}

async fn streak_handler(ctx: &serenity::client::Context, channel_id: u64) -> Result<()> {
    let channel = serenity::model::id::ChannelId::new(channel_id);
    for user in lcdb::query_tracked_users()? {
        let active = lcdb::is_active(&user)?;
        let streak = lcdb::query_streak(&user)?;
        if active {
            lcdb::streak_increment(&user)?;
            channel
                .say(
                    &ctx.http,
                    format!(
                        "{} is on a roll with a streak of {}!",
                        &user.username,
                        streak + 1
                    ),
                )
                .await?;
        } else if streak > 0 {
            lcdb::streak_break(&user)?;
            channel
                .say(&ctx.http, format!("{} lost their streak!", &user.username))
                .await?;
        }
    }

    Ok(())
}

async fn sleep_until_midnight_utc() {
    const TARGET_HOUR: u32 = 0; // 00:00 UTC (midnight)
    let now = Utc::now();

    let now_minutes = now.hour() * 60 + now.minute();
    let target_minutes = TARGET_HOUR * 60;

    // Calculate minutes to wait until the next midnight
    let mins_to_wait = (24 * 60 - now_minutes) + target_minutes;

    let sleep_duration = Duration::from_secs((mins_to_wait * 60) as u64);
    log::info!(
        "Next announcement in {} minutes.",
        sleep_duration.as_secs() / 60
    );

    sleep(sleep_duration).await;
}

fn submission_announcement(submission: &models::Submission) -> String {
    if submission.accepted {
        format!(
            "✅ {} just completed [{}]({})!\n\t{}",
            submission.username, submission.problem.title, submission.problem.url, submission.url
        )
    } else {
        format!(
            "❌ {} just submitted an attempt for [{}]({}), but {}\n\t{}",
            submission.username,
            submission.problem.title,
            submission.problem.url,
            generate_misattempt_msg(),
            submission.url
        )
    }
}

fn generate_misattempt_msg() -> String {
    // In the format of "They tried, but {first} {second}"
    let first = [
        "they missed the mark.",
        "they flubbed it.",
        "didn't quite make it.",
        "no cigar.",
        "they need to try again.",
        "didn't get it.",
        "didn't succeed.",
        "they clank'd it up.",
        "they missed a few cases.",
        "they got caught by the edge cases.",
        "they might have missed a case.",
    ];

    let second = [
        "Try again!",
        "Keep trying!",
        "Do you think they'll make it?",
        "I wonder if they'll give up...",
        "Oops...",
        "Ouch.",
        "Maybe stick to writing React components?",
        "Scratch might be more your speed...",
        "Are they cooked?",
        "And you say I'm the clanker...",
        "A horrible performance, really.",
        "Wow.",
    ];

    let mut rng = rng();
    format!(
        "{} {}",
        first.choose(&mut rng).unwrap_or(&"they borked it."),
        second.choose(&mut rng).unwrap_or(&"")
    )
}

struct LeekHandler;
#[async_trait]
impl EventHandler for LeekHandler {
    async fn ready(&self, ctx: serenity::client::Context, _ready: Ready) {
        log::info!("Bot is connected and ready!");
        let channel_id = getenv_announcements_channel();

        let daily_checker_ctx = ctx.clone();
        tokio::spawn(async move {
            loop {
                sleep_until_midnight_utc().await;
                if let Err(err) = streak_handler(&daily_checker_ctx, channel_id).await {
                    log::error!("Error sending scheduled message: {}", err);
                }
                if let Err(err) = lcdb::clean_cache() {
                    log::error!("Error clearing recent cache: {}", err);
                }
            }
        });

        let recent_checker_ctx = ctx.clone();
        tokio::spawn(async move {
            const RECENT_TIME_INTERVAL_SECS: u64 = 30; // 30 second cooldown between checks
            let mut interval =
                tokio::time::interval(StdDuration::from_secs(RECENT_TIME_INTERVAL_SECS));
            loop {
                interval.tick().await;

                match check_recent_submissions().await {
                    Ok(new_submissions) => {
                        for submission in new_submissions {
                            if let Err(err) = serenity::model::id::ChannelId::new(channel_id)
                                .say(
                                    &recent_checker_ctx.http,
                                    submission_announcement(&submission),
                                )
                                .await
                            {
                                log::error!("Error sending scheduled message: {}", err);
                            }
                        }
                    }
                    Err(err) => {
                        log::error!("Error checking recent submissions: {}", err);
                    }
                }
            }
        });
    }
    async fn message(&self, ctx: serenity::client::Context, msg: Message) {
        let channel = msg.channel_id;
        let content = msg.content.clone();

        // Clanker detection!
        if content.to_lowercase().contains("clanker") {
            log::error!("Clanker");
            let _ = msg
                .react(
                    &ctx.http,
                    serenity::all::ReactionType::Unicode(String::from("😡")),
                )
                .await;
        }

        // Commands
        if content.starts_with(getenv_call_token()) && content.len() > 1 {
            let response = match Commands::run_command(&ctx, &msg).await {
                Ok(message) => message,
                Err(err) => {
                    format!("Error: {}", err)
                }
            };

            // Discord doesn't like sending empty messages.
            // If everything is ok and the bot doesn't have anything to say, return early.
            if response.is_empty() {
                return;
            }

            // Attempt to send response.
            // If something goes wrong, we want to let the user know, if possible,
            //   so we try to send another "Oops, internal error" before exiting.
            // If *that* message can't be sent, it can't be helped...
            //   but it will be logged on our end anyways.
            if let Err(why) = channel.say(&ctx.http, response).await {
                let _ = channel.say(&ctx.http, "Oops, internal error.").await;
                log::error!("Error sending message: {why:?}");
            }
        }
    }
}
