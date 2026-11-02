use serenity::all::UserId;
use sqlx::{FromRow, Row, sqlite::SqliteRow};

use crate::getset;

getset! {
#[derive(Debug, PartialEq)]
pub struct FoundryUser {
    discord_id: UserId,
    foundry_id: String,
    actor_id: String => clone
}
}

impl<'r> FromRow<'r, SqliteRow> for FoundryUser {
    fn from_row(row: &'r SqliteRow) -> Result<Self, sqlx::Error> {
        let discord_id: i64 = row.try_get("discord_id")?;
        let foundry_id: String = row.try_get("foundry_id")?;
        let actor_id: String = row.try_get("actor_id")?;

        Ok(FoundryUser::new(
            UserId::new(discord_id as u64),
            foundry_id,
            actor_id,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn user() -> FoundryUser {
        FoundryUser::new(UserId::new(1), "foundry".to_string(), "actor".to_string())
    }

    #[test]
    fn accessors_return_what_was_stored() {
        let user = user();

        assert_eq!(user.get_discord_id(), &UserId::new(1));
        assert_eq!(user.get_foundry_id(), "foundry");
        assert_eq!(user.get_actor_id(), "actor");
    }

    #[test]
    fn setters_replace_values() {
        let mut user = user();

        user.set_actor_id("other".to_string());

        assert_eq!(user.get_actor_id(), "other");
    }
}
