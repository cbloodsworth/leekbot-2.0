use serenity::async_trait;
use serenity::model::channel::Message;
use serenity::prelude::*;

use dotenv::dotenv;
use std::env;

use leekbot::lcapi;
use leekbot::lcdb;

use anyhow::{Result, Context, anyhow};

const MAX_CMD_LENGTH: usize = 12;

pub struct Commands;
/// Async command implementations
impl Commands {
    pub async fn run_command(ctx: &serenity::client::Context, msg: &Message) -> String {
        let input = &String::from(&msg.content)[1..];
        let parsed = input.split_whitespace().collect::<Vec<_>>();
        let (command, parameters) = parsed.split_at(1);
        let command = command[0];  // turn a [&str] into a &str

        match command {
            "audit" => {
                lcapi::fetch_user(String::from(parameters[0]))
                    .await
                    .map(|user| {
                        let tracked = lcdb::is_tracked(&user).unwrap();
                        let output = format!("{}\nThis user is {}currently being tracked.", user, if tracked {""} else {"not "});

                        output
                    })
            }
            "recent" => {
                Self::get_recently_completed(parameters[0]).await
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

                Ok(output)
            }
            "track" => {
                let result = lcapi::fetch_user(String::from(parameters[0]))
                    .await
                    .and_then(|user| {
                        lcdb::track_user(&user)?;
                        Ok(user)
                });

                match result {
                    Ok(_) => {
                        let _ = msg.react(&ctx.http, serenity::all::ReactionType::Unicode(String::from("✅"))).await;
                        Ok(String::from(""))
                    }
                    Err(err) => {
                        Err(anyhow!("Could not track user {}: {}", parameters[0], err))
                    }
                }
            }
            "untrack" => {
                lcapi::fetch_user(String::from(parameters[0]))
                    .await
                    .and_then(|user| {
                        lcdb::untrack_user(&user)?;
                        Ok(user)
                    })
                    .map(|user| format!("Successfully untracked {}.", user.username))
            }
            "help" => {
                Ok(Self::get_help())
            }
            "clanker" => {
                Ok(String::from("call me clanker one more mf time"))
            }
            _ => {
                if Commands::is_valid_cmd(&command) {
                    log::info!("User submitted unknown command: {}", command);
                    Err(anyhow!("No such command found: {}, see $help for commands.", command))
                }
                else {
                    log::info!("User submitted invalid command: {}", command);
                    Err(anyhow!("Invalid command syntax."))
                }
            }
        }.unwrap_or_else(|err| format!("ERROR: {}", err))
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
`$tracklist`:  Untrack a user.
`$help`:  Get information on supported commands
"#)
    }
}


struct LeekHandler;
#[async_trait]
impl EventHandler for LeekHandler {
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
            let response = Commands::run_command(&ctx, &msg).await;
            if response.is_empty() { return; }

            if let Err(why) = channel.say(&ctx.http, response).await {
                let _ = channel.say(&ctx.http, "Oops, internal error.").await;
                log::error!("Error sending message: {why:?}");
            }
        }
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

#[tokio::main]
async fn main() -> Result<()> {
    // Load discord bot token
    dotenv().ok();
    let token = env::var("DISCORD_TOKEN")
        .expect("Expected 'DISCORD_TOKEN=<token>' in .env in project root.");

    // Begin logger
    env_logger::Builder::from_env("LOG_LEVEL").init();

    // Initialize database
    lcdb::initialize_db()?;

    let intents = GatewayIntents::GUILD_MESSAGES
        | GatewayIntents::DIRECT_MESSAGES
        | GatewayIntents::MESSAGE_CONTENT;

    let mut client =
        Client::builder(&token, intents).event_handler(LeekHandler).await
            .expect("Error creating client.");

    if let Err(why) = client.start().await {
        log::error!("Client error: {why:?}");
    }

    Ok(())
}
