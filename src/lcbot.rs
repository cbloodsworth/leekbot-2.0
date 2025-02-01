use serenity::async_trait;
use serenity::model::channel::Message;
use serenity::model::gateway::Ready;
use serenity::prelude::*;

use chrono::{Utc, Timelike};

use tokio::time::{sleep, Duration, Instant};

use dotenv::dotenv;
use std::env;

use crate::lcapi;
use crate::lcdb;


use anyhow::{Result, Context, anyhow};

const MAX_CMD_LENGTH: usize = 12;
const ANNOUNCEMENTS_CHANNEL_ID: u64 = 1335276868215115906;

pub async fn run_leekbot() -> Result<()> {
    // Load discord bot token
    dotenv().ok();
    let token = env::var("DISCORD_TOKEN")
        .context("Expected 'DISCORD_TOKEN=<token>' in .env in project root.")?;

    let intents = GatewayIntents::GUILD_MESSAGES
        | GatewayIntents::DIRECT_MESSAGES
        | GatewayIntents::MESSAGE_CONTENT;

    let mut client =
        Client::builder(&token, intents).event_handler(LeekHandler).await
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
        let input = String::from(&msg.content[1..]);  // skip the first letter for the command: it's '$'
        let split_tokens = input.split_whitespace().collect::<Vec<_>>();
        let (&[command], parameters) = split_tokens.split_at(1) else { return Err(anyhow!("easd"))};

        // Execute the command
        let result: String = match command {
            "audit" => {
                let username = parameters
                    .get(0).context("Expected username for audit, got none.")?
                    .to_string();

                lcapi::fetch_user(username)
                    .await
                    .map(|user| {
                        let tracked = lcdb::is_tracked(&user).unwrap();
                        let output = format!("{}\nThis user is {}currently being tracked.", user, if tracked {""} else {"not "});

                        output
                    })?
            }
            "recent" => {
                Self::get_recently_completed(parameters[0]).await?
            }
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
                    .get(0).context("Expected username for tracking, got none.")?
                    .to_string();

                let user = lcapi::fetch_user(username).await?;
                lcdb::track_user(&user)?;

                msg.react(&ctx.http, serenity::all::ReactionType::Unicode(String::from("✅"))).await?;
                String::from("")
            }
            "untrack" => {
                let username = parameters
                    .get(0).context("Expected username for untracking, got none.")?
                    .to_string();

                let user = lcapi::fetch_user(username).await?;
                lcdb::untrack_user(&user)?;

                msg.react(&ctx.http, serenity::all::ReactionType::Unicode(String::from("✅"))).await?;
                String::from("")
            }
            "help" => {
                Self::get_help()
            }
            "clanker" => {
                String::from("call me clanker one more mf time")
            }
            _ => {
                if Commands::is_valid_cmd(&command) {
                    log::info!("User submitted unknown command: {}", command);
                    return Err(anyhow!("No such command found: {}, see $help for commands.", command));
                }
                else {
                    log::info!("User submitted invalid command: {}", command);
                    return Err(anyhow!("Invalid command syntax."))
                }
            }
        };

        Ok(result)
    }

    async fn get_recently_completed(username: &str) -> Result<String> {
        Ok(format!("{}", lcapi::fetch_recently_completed(username)
            .await?
            .get(0)
            .context(format!("No recently completed problems for {}", username))?))
    }
}

/// Non-async helpers
impl Commands {
    /// Ensures that the string slice conforms to C-like identifier regex
    fn is_valid_cmd(s: &str) -> bool {
        s.len() <= MAX_CMD_LENGTH &&
        regex::Regex::new(r"^[a-zA-Z_][a-zA-Z0-9_]*$").unwrap().is_match(s)
    }

    /// Gets a help string. Should be updated after a new command is added
    /// TODO: Generate automatically?
    pub fn get_help() -> String {
        String::from(
r#"
**Command List:**
`$audit <leetcode username>`:  Get stats on a leetcode user.
`$recent <leetcode username>`:  Get the most recent submission from a leetcode user.
`$track <leetcode username>`:  Track a user. This will cause the bot to announce new submissions from this user.
`$untrack <leetcode username>`:  Untrack a user.
`$tracklist`:  List all tracked users.
`$help`:  Get information on supported commands
"#)
    }
}

/// Checks recent Leetcode submissions for all tracked users and sends
///   any new submissions to Discord.
/// 
/// Intended to be run regularly.
fn check_recent_submissions() -> Result<()> {
    let users = lcdb::query_tracked_users()?;
    todo!()
}

async fn sleep_until_midnight_utc() {
    const TARGET_HOUR: u32 = 0; // 00:00 UTC (midnight)
    let now = Utc::now();

    let now_minutes = now.hour() * 60 + now.minute();
    let target_minutes = TARGET_HOUR * 60;

    // Calculate minutes to wait until the next midnight
    let mins_to_wait = (24 * 60 - now_minutes) + target_minutes;

    let sleep_duration = Duration::from_secs((mins_to_wait * 60) as u64);
    log::info!("Next announcement in {} minutes.", sleep_duration.as_secs() / 60);

    sleep(sleep_duration).await;
}


struct LeekHandler;
#[async_trait]
impl EventHandler for LeekHandler {
    async fn ready(&self, ctx: serenity::client::Context, _ready: Ready) {
        log::info!("Bot is connected and ready!");

        tokio::spawn(async move {
            let channel_id = ANNOUNCEMENTS_CHANNEL_ID;
            loop {
                sleep_until_midnight_utc().await;
                if let Err(err) = serenity::model::id::ChannelId::new(channel_id)
                    .say(&ctx.http, "This runs at 7:00PM every day!")
                    .await
                {
                    log::error!("Error sending scheduled message: {:?}", err);
                }
            }
        });
    }
    async fn message(&self, ctx: serenity::client::Context, msg: Message) {
        let channel = msg.channel_id;
        let content = msg.content.clone();

        // Clanker detection!
        if content.to_lowercase().find("clanker").is_some() {
            log::error!("Clanker");
            let _ = msg.react(&ctx.http, 
                serenity::all::ReactionType::Unicode(String::from("😡"))).await;
        }

        // Commands
        if content.starts_with("$") && content.len() > 1 {
            let response = match Commands::run_command(&ctx, &msg).await {
                Ok(message) => { message }
                Err(err) => { format!("Error: {}", err) }
            };

            // Discord doesn't like sending empty messages.
            // If everything is ok and the bot doesn't have anything to say, return early.
            if response.is_empty() { return; }

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