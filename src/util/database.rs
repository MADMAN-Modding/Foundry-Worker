use serde_json::Value;
use serenity::all::{GuildId, UserId};
use sqlx::{Pool, Sqlite, sqlite::SqliteQueryResult};

use crate::{api::foundry_user::FoundryUser, database::config::Config, util::logging::log_result};

/// Serializes ids as the JSON array of integers stored in the config table
pub fn ids_to_json<I: Copy + Into<u64>>(ids: &[I]) -> Value {
    let ids: Vec<i64> = ids.iter().map(|id| (*id).into() as i64).collect();

    Value::from(ids)
}

/// Reads back the JSON array written by `ids_to_json`
pub fn ids_from_json<I: From<u64>>(column: &str, json: &str) -> Result<Vec<I>, sqlx::Error> {
    let ids: Vec<i64> = serde_json::from_str(json).map_err(|e| sqlx::Error::ColumnDecode {
        index: column.into(),
        source: Box::new(e),
    })?;

    Ok(ids.into_iter().map(|id| I::from(id as u64)).collect())
}

pub async fn insert_struct(
    db: &Pool<Sqlite>,
    config: Config,
) -> Result<SqliteQueryResult, sqlx::Error> {
    let result = sqlx::query(r#"REPLACE INTO config (guid, dms, admins, foundry_status_channel, world_id) VALUES (?1, ?2, ?3, ?4, ?5)"#)
        .bind(config.get_guid().get() as i64)
        .bind(ids_to_json(&config.get_dms()))
        .bind(ids_to_json(&config.get_admins()))
        .bind(config.get_foundry_status_channel().get() as i64)
        .bind(config.get_world_id())
        .execute(db)
        .await;

    log_result(&result, "Set Foundry Status Channel");

    result
}

pub async fn insert_foundry_user(
    db: &Pool<Sqlite>,
    user: FoundryUser,
) -> Result<SqliteQueryResult, sqlx::Error> {
    sqlx::query(
        "INSERT INTO foundry_users (discord_id, foundry_id, actor_id)
         VALUES (?, ?, ?)
         ON CONFLICT(discord_id) DO UPDATE SET
            foundry_id = excluded.foundry_id,
            actor_id = excluded.actor_id",
    )
    .bind(user.get_discord_id().get() as i64)
    .bind(user.get_foundry_id())
    .bind(user.get_actor_id())
    .execute(db)
    .await
}

// For /sheet later
pub async fn load_foundry_user(
    db: &Pool<Sqlite>,
    discord_id: UserId,
) -> Result<Option<FoundryUser>, sqlx::Error> {
    sqlx::query_as::<_, FoundryUser>(
        "SELECT discord_id, foundry_id, actor_id FROM foundry_users WHERE discord_id = ?",
    )
    .bind(discord_id.get() as i64)
    .fetch_optional(db)
    .await
}

pub async fn load(db: &Pool<Sqlite>) -> Result<Config, sqlx::Error> {
    let result = sqlx::query_as::<_, Config>(
        r#"
            SELECT * FROM config
            LIMIT 1;
        "#,
    )
    .fetch_one(db)
    .await;

    log_result(&result, "Load Config");

    result
}

/// Loads the stored config, or a default one for `guid` when nothing is stored yet
pub async fn load_config(db: &Pool<Sqlite>, guid: GuildId) -> Config {
    match load(db).await {
        Ok(config) => config,
        Err(_) => {
            let mut config = Config::default();
            config.set_guid(guid);
            config
        }
    }
}

pub async fn get_all_config(db: &Pool<Sqlite>) -> Result<Vec<Config>, sqlx::Error> {
    let result = sqlx::query_as::<_, Config>("SELECT * FROM config")
        .fetch_all(db)
        .await;

    log_result(&result, "Find any Config Data");

    result
}

#[cfg(test)]
mod tests {
    use serenity::all::ChannelId;

    use super::*;
    use crate::util::test_support::memory_db;

    fn config(guid: u64) -> Config {
        Config::new(
            GuildId::new(guid),
            vec![UserId::new(1), UserId::new(2)],
            vec![UserId::new(3)],
            ChannelId::new(4),
            "world".to_string(),
        )
    }

    #[test]
    fn ids_round_trip_through_json() {
        let ids = vec![UserId::new(1), UserId::new(u64::from(u32::MAX))];

        let json = ids_to_json(&ids).to_string();
        let back: Vec<UserId> = ids_from_json("dms", &json).unwrap();

        assert_eq!(back, ids);
    }

    #[test]
    fn empty_id_lists_serialize_as_an_empty_array() {
        assert_eq!(ids_to_json::<UserId>(&[]).to_string(), "[]");
    }

    #[test]
    fn ids_from_json_reports_the_failing_column() {
        let err = ids_from_json::<UserId>("admins", "{").unwrap_err();

        match err {
            sqlx::Error::ColumnDecode { index, .. } => assert_eq!(index, "admins"),
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn ids_from_json_works_for_other_id_types() {
        let channels: Vec<ChannelId> = ids_from_json("c", "[5,6]").unwrap();

        assert_eq!(channels, vec![ChannelId::new(5), ChannelId::new(6)]);
    }

    #[tokio::test]
    async fn load_fails_on_an_empty_table() {
        let db = memory_db().await;

        assert!(load(&db).await.is_err());
        assert!(get_all_config(&db).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn config_round_trips() {
        let db = memory_db().await;

        insert_struct(&db, config(10)).await.unwrap();

        assert_eq!(load(&db).await.unwrap(), config(10));
    }

    #[tokio::test]
    async fn insert_struct_replaces_the_row_for_the_same_guild() {
        let db = memory_db().await;
        let mut updated = config(10);
        updated.set_world_id("other".to_string());

        insert_struct(&db, config(10)).await.unwrap();
        insert_struct(&db, updated).await.unwrap();

        let all = get_all_config(&db).await.unwrap();

        assert_eq!(all.len(), 1);
        assert_eq!(all[0].get_world_id(), "other");
    }

    #[tokio::test]
    async fn get_all_config_returns_every_guild() {
        let db = memory_db().await;

        insert_struct(&db, config(10)).await.unwrap();
        insert_struct(&db, config(11)).await.unwrap();

        assert_eq!(get_all_config(&db).await.unwrap().len(), 2);
    }

    #[tokio::test]
    async fn load_config_prefers_the_stored_config() {
        let db = memory_db().await;

        insert_struct(&db, config(10)).await.unwrap();

        assert_eq!(load_config(&db, GuildId::new(99)).await, config(10));
    }

    #[tokio::test]
    async fn load_config_falls_back_to_a_default_for_the_guild() {
        let db = memory_db().await;

        let loaded = load_config(&db, GuildId::new(99)).await;

        assert_eq!(loaded.get_guid(), &GuildId::new(99));
        assert!(loaded.get_dms().is_empty());
        assert!(loaded.get_world_id().is_empty());
    }

    #[tokio::test]
    async fn foundry_user_round_trips() {
        let db = memory_db().await;
        let user = FoundryUser::new(UserId::new(5), "fid".to_string(), "aid".to_string());

        insert_foundry_user(&db, user).await.unwrap();

        let loaded = load_foundry_user(&db, UserId::new(5)).await.unwrap().unwrap();

        assert_eq!(loaded.get_discord_id(), &UserId::new(5));
        assert_eq!(loaded.get_foundry_id(), "fid");
        assert_eq!(loaded.get_actor_id(), "aid");
    }

    #[tokio::test]
    async fn unbound_users_load_as_none() {
        let db = memory_db().await;

        assert!(load_foundry_user(&db, UserId::new(5)).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn rebinding_a_user_updates_the_existing_row() {
        let db = memory_db().await;

        for actor in ["first", "second"] {
            let user = FoundryUser::new(UserId::new(5), "fid".to_string(), actor.to_string());
            insert_foundry_user(&db, user).await.unwrap();
        }

        let loaded = load_foundry_user(&db, UserId::new(5)).await.unwrap().unwrap();
        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM foundry_users")
            .fetch_one(&db)
            .await
            .unwrap();

        assert_eq!(loaded.get_actor_id(), "second");
        assert_eq!(count, 1);
    }

    #[tokio::test]
    async fn bindings_are_kept_per_discord_user() {
        let db = memory_db().await;

        for (id, actor) in [(1, "a"), (2, "b")] {
            let user = FoundryUser::new(UserId::new(id), "fid".to_string(), actor.to_string());
            insert_foundry_user(&db, user).await.unwrap();
        }

        let second = load_foundry_user(&db, UserId::new(2)).await.unwrap().unwrap();

        assert_eq!(second.get_actor_id(), "b");
    }
}
