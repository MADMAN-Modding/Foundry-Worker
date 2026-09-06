use std::{env, sync::Arc};

use foundry_worker::{bot::{bot::Bot, connection_check::ConnectionChecker}, constants};
use serenity::{
    Client, all::{GatewayIntents, Http}
};
use simple_logger::SimpleLogger;

#[tokio::main]
async fn main() {
    SimpleLogger::new()
        .with_level(log::LevelFilter::Warn)
        .with_module_level("foundry_worker", log::LevelFilter::Debug).init().unwrap();

    constants::inti();

    let bot_token = constants::BOT_TOKEN.get().unwrap().clone();

    unsafe {
        env::set_var("DATABASE_URL", "sqlite://database.sqlite");
    }

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

    let http = Arc::new(Http::new(&bot_token));

    // Create the bot instance with the database connection.
    let bot = Bot::new(database, http);

    let connection_check = ConnectionChecker::new(Arc::from(bot.clone()));
    connection_check.start_thread();

    // Create a new client with the bot token and intents, and set the event handler to the bot.
    let intents = GatewayIntents::GUILD_MESSAGES
        | GatewayIntents::DIRECT_MESSAGES
        | GatewayIntents::MESSAGE_CONTENT;
    let mut client = Client::builder(&bot_token, intents)
        .event_handler(bot)
        .await
        .expect("Err creating client");
    if let Err(error) = client.start().await {
        eprintln!("Client error: {error:?}");
    }


}
