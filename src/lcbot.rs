use serenity::async_trait;
use serenity::model::channel::Message;
use serenity::model::gateway::Ready;
use serenity::prelude::*;

use chrono::{Timelike, Utc};

use std::time::Duration as StdDuration;
use tokio::time::{Duration, sleep};

use dotenv::dotenv;

use crate::lcapi;
use crate::lcdb;
use crate::models;

mod commands;
use commands::Commands;

use anyhow::{Context, Result};

struct LeekHandler;
#[async_trait]
impl EventHandler for LeekHandler {
    async fn ready(&self, ctx: serenity::client::Context, _ready: Ready) {
        log::info!("Bot is connected and ready!");
        let channel_id = getenv_announcements_channel();

        // Display most recent commit on restart.
        if !commands::is_debug_mode() {
            let commit_msg = std::fs::read("commit.txt")
                .map(|bytes| String::from_utf8_lossy(&bytes).to_string())
                .unwrap_or(String::from("no commit message"));

            serenity::model::id::ChannelId::new(channel_id)
                .say(&ctx.http, format!("LeekBot 2.0 updated: `{}`", commit_msg.trim()))
                .await
                .map_or_else(|err| log::error!("Couldn't send welcome message: {err}"), |_|{});
        }

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
                            announce_submission(&submission, &recent_checker_ctx, channel_id).await;
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
        if content.starts_with(commands::getenv_call_token()) && content.len() > 1 {
            let response = match Commands::run_command(&ctx, &msg).await {
                Ok(message) => message,
                Err(err) => {
                    log::error!("{err}");
                    format!("Error: {err}")
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

/// Get the announcements channel ID. May panic.
fn getenv_announcements_channel() -> u64 {
    std::env::var("ANNOUNCEMENTS_CHANNEL_ID")
        .expect(".env file does not contain 'ANNOUNCEMENTS_CHANNEL_ID.")
        .parse()
        .expect("'ANNOUNCEMENTS_CHANNEL_ID should be parseable into a u64.")
}

pub async fn run_leekbot() -> Result<()> {
    // Load discord bot token
    dotenv().ok();
    let token = std::env::var("DISCORD_TOKEN")
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

/// Checks recent Leetcode submissions for all tracked users, compares with the ones represented in
/// the recency cache, and returns the ones that may be announced.
///
/// Intended to be run regularly.
async fn check_recent_submissions() -> Result<Vec<models::Submission>> {
    let mut result = Vec::new();

    let users = lcdb::query_tracked_users()?;

    update_db_from_leetcode(&users).await?;

    for user in users {
        match lcdb::query_uncached_submissions(&user) {
            Ok(subs) => result.extend(subs),
            Err(err) => log::error!("[check_recent_submissions] Error querying database for \
                                    uncached submissions for {}: {}", 
                                    user.username, err)
        }
    }

    Ok(result)
}

/// Reaches out to LeetCode and sees if any of our tracked users have any new submissions:
/// if they have any, updates the Submissions table of the database.
async fn update_db_from_leetcode(users: &[models::User]) -> Result<()> {
    for user in users {
        match lcapi::fetch_recently_submitted(&user.username).await {
            Ok(recent_subs) => {
                for submission in recent_subs {
                    if let Err(err) = lcdb::insert_problem(&submission.problem) {
                        log::warn!("[update_db_from_leetcode] Could not insert problem: {}: {err}", 
                                    submission.problem.title);
                    }

                    if let Err(err) = lcdb::insert_submission(&submission) {
                        log::warn!("[update_db_from_leetcode] Could not insert submission: \
                                    {submission}: {err}");
                    }
                }
            },
            Err(err) => {
                log::error!("[update_db_from_leetcode] Error updating submissions for {}: {}",
                             user.username, err);
            }
        }
    }

    Ok(())
}

/// Handles streaks by checking if tracked users have submitted a problem recently.
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

/// Sleeps until 00:00 UTC.
async fn sleep_until_midnight_utc() {
    const TARGET_HOUR: u32 = 0; // 00:00 UTC (midnight)
    let now = Utc::now();

    let now_minutes = now.hour() * 60 + now.minute();
    let target_minutes = TARGET_HOUR * 60;

    // Calculate minutes to wait until the next midnight
    let mins_to_wait = (24 * 60 - now_minutes) + target_minutes;

    let sleep_duration = Duration::from_secs((mins_to_wait * 60) as u64);
    log::info!(
        "Next streak announcement in {} minutes.",
        sleep_duration.as_secs() / 60
    );

    sleep(sleep_duration).await;
}

/// Potentially announces a submission and adds it to the RecentCache.
///
/// Note that some users may not want their submissions announced; we reflect that here.
async fn announce_submission(
    submission: &models::Submission,
    ctx: &serenity::client::Context,
    channel_id: u64) 
{
    log::trace!("[announce_submission] Updating RecentCache...");

    let username = &submission.username;
    let problem = &submission.problem.title;

    // Get the User object
    let Ok(Some(user)) = lcdb::query_user(&submission.username) else {
        log::error!("[announce_submission] Attempted to announce submission for {}, but
                     couldn't find the user in the database.", submission.username);
        return;
    };

    // Get the UserPreferences object for this user
    let Ok(Some(prefs)) = lcdb::query_user_preferences(&user) else {
        log::error!("[announce_submission] Attempted to gather preferences for {}, but
                     couldn't find them in the database.", user.username);
        return;
    };

    match lcdb::insert_cache_submission(submission) {
        Ok(true) => log::debug!("[announce_submission] Added {username}'s submission '{problem}' \
                                 to recent cache."),

        Ok(false) => log::error!("[announce_submission] Didn't add {username}'s submission \
                                  '{problem}' to recent cache: it was (unexpectedly) already \
                                  there."),

        Err(err) => log::error!("[announce_submission] Couldn't insert cache submission: {err}"),
    }

    if prefs.announcement.is_some() {

        let Some(msg) = submission_announcement(submission, prefs) else {
            log::info!("{username} has a new submission for {problem}, but they don't want to \
                        have it announced (likely due to failure).");
            return;
        };

        log::info!("Sending message for {username}'s new submission: {problem}");
        if let Err(err) = serenity::model::id::ChannelId::new(channel_id)
            .say(&ctx.http, msg)
            .await
        {
            log::error!("Error sending scheduled message: {}", err);
        }
    } else {
        log::info!("{user} submitted a new problem '{problem}', but prefers to move in silence.")
    }
}

/// Creates a submission announcement String from a Submission.
fn submission_announcement(
    submission: &models::Submission,
    prefs: models::UserPreferences
) -> Option<String>
{
    let has_link = prefs.announcement?.has_submission_link;
    let announce_failures = prefs.announcement?.announce_failures;

    if submission.accepted {
        let mut msg = format!(
            "✅ {} just completed [{}]({})!",
            submission.username, submission.problem.title, submission.problem.url);

        if has_link {
            msg += &format!("\n\t{}", submission.url);
        }

        Some(msg)
    } else {
        announce_failures.then(|| {
            format!(
                "❌ {} just submitted an attempt for [{}]({}), but {}\n\t{}",
                submission.username,
                submission.problem.title,
                submission.problem.url,
                generate_misattempt_msg(),
                submission.url
            )
        })
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

    let mut rng = rand::rng();
    use rand::seq::IndexedRandom;

    format!(
        "{} {}",
        first.choose(&mut rng).unwrap_or(&"they borked it."),
        second.choose(&mut rng).unwrap_or(&"")
    )
}