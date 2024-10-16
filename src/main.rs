use serenity::async_trait;
use serenity::model::channel::Message;
use serenity::prelude::*;

use dotenv::dotenv;
use std::env;

struct Handler;
#[async_trait]
impl EventHandler for Handler {
    async fn message(&self, ctx: Context, msg: Message) {
        if msg.content == "!ping" {
            if let Err(why) = msg.channel_id.say(&ctx.http, "Pong!").await {
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
        Client::builder(&token, intents).event_handler(Handler).await
            .expect("Error creating client.");

    if let Err(why) = client.start().await {
        println!("Client error: {why:?}");
    }
}
