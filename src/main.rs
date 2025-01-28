use log::info;
use serenity::async_trait;
use serenity::model::channel::Message;
use serenity::prelude::*;

use dotenv::dotenv;
use std::env;

use leekbot::*;

use anyhow::{Result, Context, anyhow};

const MAX_CMD_LENGTH: usize = 12;

pub struct Commands;
/// Async command implementations
impl Commands {
    pub async fn run_command(input: String) -> String {
        let input = &input[1..];
        let parsed = input.split_whitespace().collect::<Vec<_>>();
        let (command, parameters) = parsed.split_at(1);
        let command = command[0];  // turn a [&str] into a &str

        match command {
            "audit" => {
                Self::get_user_data(parameters[0]).await
            }
            "recent" => {
                Self::get_recently_completed(parameters[0]).await
            }
            "help" => {
                Ok(Self::get_help())
            }
            "clanker" => {
                Ok(String::from("call me clanker one more mf time"))
            }
            _ => {
                if Commands::is_valid_cmd(&command) {
                    info!("User submitted unknown command: {}", command);
                    Err(anyhow!("No such command found: {}, see $help for commands.", command))
                }
                else {
                    info!("User submitted invalid command: {}", command);
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

    async fn get_user_data(username: &str) -> Result<String> {
        lcapi::fetch_user(username).await.and_then(|u| Ok(format!("{:?}", u)))
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
`$audit <leetcode username>`:  Get stats on a leetcode user
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
            let response = Commands::run_command(content).await;
            if let Err(why) = channel.say(&ctx.http, response).await {
                let _ = channel.say(&ctx.http, "Oops, internal error.").await;
                log::error!("Error sending message: {why:?}");
            }
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    // Begin logger
    env_logger::init();

    // Load discord bot token
    dotenv().ok();
    let token = env::var("DISCORD_TOKEN")
        .expect("Expected 'DISCORD_TOKEN=<token>' in .env in project root.");

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
