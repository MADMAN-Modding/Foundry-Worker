use std::{env, sync::Arc};

use dotenv::dotenv;
use foundry_worker::bot::{bot::Bot, connection_check::ConnectionChecker};
use serenity::{
    Client, all::{GatewayIntents, Http}
};

#[tokio::main]
async fn main() {
    // Load environment variables from the .env file
    dotenv().ok();

    unsafe {
        env::set_var("DATABASE_URL", "sqlite://database.sqlite");
    }

    // Configure the client with your Discord bot token in the environment.
    let token = env::var("DISCORD_TOKEN").expect("Expected DISCORD_TOKEN to be set in .env");
    let api_key = env::var("API_KEY").expect("Expected API_KEY to be set in .env");

    // Initiate a connection to the database file, creating the file if required.
    let database = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(5)
        .connect_with(
            sqlx::sqlite::SqliteConnectOptions::new()
                .filename("database.sqlite")
                .create_if_missing(true),
        )
        .await
        .expect("Couldn't connect to database");

    // Run the database migrations to ensure the schema is up to date.
    sqlx::migrate!("./migrations")
        .run(&database)
        .await
        .expect("Couldn't run database migrations");

    let http = Arc::new(Http::new(&token));

    // Create the bot instance with the database connection.
    let bot = Bot::new(database, api_key.clone(), http);

    let connection_check = ConnectionChecker::new(Arc::from(bot.clone()), api_key);
    connection_check.start_thread();

    // Create a new client with the bot token and intents, and set the event handler to the bot.
    let intents = GatewayIntents::GUILD_MESSAGES
        | GatewayIntents::DIRECT_MESSAGES
        | GatewayIntents::MESSAGE_CONTENT;
    let mut client = Client::builder(&token, intents)
        .event_handler(bot)
        .await
        .expect("Err creating client");
    if let Err(error) = client.start().await {
        eprintln!("Client error: {error:?}");
    }


}
