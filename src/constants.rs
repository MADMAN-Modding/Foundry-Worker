use std::env;

use tokio::sync::OnceCell;

pub const BOT_TOKEN: tokio::sync::OnceCell<std::string::String> = OnceCell::<String>::const_new();
pub const API_KEY: tokio::sync::OnceCell<std::string::String> = OnceCell::<String>::const_new();
pub const ENDPOINT: tokio::sync::OnceCell<std::string::String> = OnceCell::<String>::const_new();

pub fn inti() {
    // Load environment variables from the .env file
    dotenv::dotenv().ok();

    // Configure the client with your Discord bot token in the environment.
    let token = env::var("DISCORD_TOKEN").expect("Expected DISCORD_TOKEN to be set in .env");
    let api_key = env::var("API_KEY").expect("Expected API_KEY to be set in .env");
    let endpoint = env::var("ENDPOINT").expect("Expected ENDPOINT to be set in .env");

    BOT_TOKEN.set(token).unwrap();
    API_KEY.set(api_key).unwrap();
    ENDPOINT.set(endpoint).unwrap();
}
