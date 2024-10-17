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
            Err(e) => e.to_string()
        }
    }

    pub async fn run_command(input: String) -> String {
        let input = &input[1..];
        let parsed = input.split_whitespace().collect::<Vec<_>>();
        let command = parsed[0];
        let (_, parameters) = parsed.split_at(1);

        match command {
            "audit" => Self::get_user_data(parameters[0]).await,
            _ => format!("Unknown command: {}", command)
        }
    }
}

struct LeekHandler;
#[async_trait]
impl EventHandler for LeekHandler {
    async fn message(&self, ctx: Context, msg: Message) {
        if msg.content.starts_with("$") {
            let response = Commands::run_command(msg.content).await;
            if let Err(why) = msg.channel_id.say(&ctx.http, response).await {
                println!("Error sending message: {why:?}");
            }
        }
    }
}

#[tokio::main]
async fn main() {
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
        println!("Client error: {why:?}");
    }
}
