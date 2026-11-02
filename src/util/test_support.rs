use std::sync::Arc;

use serde_json::{Value, json};
use serenity::{
    builder::CreateInteractionResponse, http::Http, model::application::CommandInteraction,
};
use sqlx::{Pool, Sqlite, sqlite::SqlitePoolOptions};

use crate::bot::bot::Bot;

pub const GUILD: u64 = 100;

/// A fresh in-memory database with every migration applied.
/// A single connection keeps the in-memory database alive and shared.
pub async fn memory_db() -> Pool<Sqlite> {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("in-memory database");

    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("migrations");

    pool
}

/// A bot backed by an in-memory database. Its HTTP client never talks to Discord in these tests.
pub async fn test_bot() -> Bot {
    Bot::new(memory_db().await, Arc::new(Http::new("token")))
}

fn user_json(id: u64) -> Value {
    json!({ "id": id.to_string(), "username": format!("user{id}"), "discriminator": "0", "avatar": null })
}

/// A guild slash command invoked by `caller` with the given Discord permission bits.
/// User options are resolved automatically, so `options` can reference user ids directly.
pub fn command(name: &str, options: Value, caller: u64, permissions: u64) -> CommandInteraction {
    let users: serde_json::Map<String, Value> = options
        .as_array()
        .into_iter()
        .flatten()
        .filter(|option| option["type"] == 6)
        .filter_map(|option| option["value"].as_str())
        .map(|id| (id.to_string(), user_json(id.parse().unwrap())))
        .collect();

    serde_json::from_value(json!({
        "id": "1",
        "application_id": "2",
        "data": {
            "id": "3",
            "name": name,
            "type": 1,
            "resolved": { "users": users },
            "options": options,
        },
        "guild_id": GUILD.to_string(),
        "channel_id": "4",
        "member": {
            "user": user_json(caller),
            "roles": [],
            "deaf": false,
            "mute": false,
            "flags": 0,
            "permissions": permissions.to_string(),
        },
        "user": user_json(caller),
        "token": "token",
        "version": 1,
        "locale": "en-US",
        "entitlements": [],
        "attachment_size_limit": 0,
    }))
    .expect("command interaction")
}

pub fn user_option(id: u64) -> Value {
    json!([{ "name": "user", "type": 6, "value": id.to_string() }])
}

/// The visible text of a message response
pub fn content(response: CreateInteractionResponse) -> String {
    serde_json::to_value(response).unwrap()["data"]["content"]
        .as_str()
        .unwrap_or_default()
        .to_string()
}
