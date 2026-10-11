use serenity::all::UserId;
use sqlx::{FromRow, Row, sqlite::SqliteRow};

use crate::getset;

getset! {
pub struct FoundryUser {
    discord_id: UserId,
    foundry_id: String,
    actor_id: String => clone
}
}

impl FoundryUser {
    pub fn new(
        discord_id: UserId,
        foundry_id: String,
        actor_id: String
    ) -> Self {
        Self {
            discord_id,
            foundry_id,
            actor_id
        }
    }
}

impl<'r> FromRow<'r, SqliteRow> for FoundryUser {
    fn from_row(row: &'r SqliteRow) -> Result<Self, sqlx::Error> {
        let discord_id: i64 = row.try_get("discord_id")?;
        let foundry_id: String = row.try_get("foundry_id")?;
        let actor_id: String = row.try_get("actor_id")?;

        Ok(FoundryUser { discord_id: UserId::new(discord_id as u64), foundry_id, actor_id })
    }
}

