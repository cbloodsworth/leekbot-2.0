use serenity::async_trait;
use serenity::model::channel::Message;
use serenity::prelude::*;

use dotenv::dotenv;
use std::env;

mod lcapi;
use lcapi::client::fetch_user;

pub struct Commands;
impl Commands {
    pub async fn get_user_data(user: &str) -> String {
        match fetch_user(user.to_string()).await {
            Ok(user) => format!("{}", user),
            Err(why) => { 
                log::error!("Fetching user data: {}", why);
                format!("ERROR: {}", why)
            }
        }
    }

    pub fn get_help() -> String {
        String::from(
r#"
**Command List:**
`$audit <leetcode username>`:  Get stats on a leetcode user
`$help`:  Get information on supported commands
"#)
    }

    pub async fn run_command(input: String) -> String {
        let input = &input[1..];
        let parsed = input.split_whitespace().collect::<Vec<_>>();
        let command = parsed[0];
        let (_, parameters) = parsed.split_at(1);

        match command {
            "audit" => Self::get_user_data(parameters[0]).await,
            "help" => Self::get_help(),
            "clanker" => String::from("call me clanker one more mf time"),
            _ => format!("Unknown command: {}", command)
        }
    }
}

struct LeekHandler;
#[async_trait]
impl EventHandler for LeekHandler {
    async fn message(&self, ctx: Context, msg: Message) {
        let channel = msg.channel_id;

        // Clanker detection!
        if msg.content.to_lowercase().find("clanker").is_some() {
            log::error!("Clanker");
            let _ = msg.react(&ctx.http, 
                serenity::all::ReactionType::Unicode(String::from("😡"))).await;
        }

        // Commands
        if msg.content.starts_with("$") {
            let response = Commands::run_command(msg.content).await;
            if let Err(why) = channel.say(&ctx.http, response).await {
                let _ = channel.say(&ctx.http, "Oops, internal error.").await;
                log::error!("Error sending message: {why:?}");
            }
        }
    }
}

#[tokio::main]
async fn main() {
    // Begin logger
    env_logger::init();

    // Load discord bot token
    dotenv().ok();
    let token = env::var("DISCORD_TOKEN")
        .expect("Expected 'DISCORD_TOKEN=<token>' in .env in project root.");

    let intents = GatewayIntents::GUILD_MESSAGES
        | GatewayIntents::DIRECT_MESSAGES
        | GatewayIntents::MESSAGE_CONTENT;

    let mut client =
        Client::builder(&token, intents).event_handler(LeekHandler).await
            .expect("Error creating client.");

    if let Err(why) = client.start().await {
        log::error!("Client error: {why:?}");
    }
}
