use serenity::async_trait;
use serenity::model::channel::Message;
use serenity::prelude::*;

use dotenv::dotenv;
use std::env;

mod lcapi;
use lcapi::client::*;

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

async fn bot_stuff_temp() {
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

#[tokio::main]
async fn main() {
    let user = "cbloodsworth";
    match fetch_user(user.to_string()).await {
        Ok(user_data) => {
            println!("{}", user_data);
            Ok(())
        }
        Err(e) => {
            eprintln!("Error fetching user data: {}", e);
            Err(e)
        }
    }.unwrap();
}
