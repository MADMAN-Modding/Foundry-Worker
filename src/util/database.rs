use log::debug;
use serenity::all::UserId;
use sqlx::{AssertSqlSafe, Pool, Row, Sqlite, sqlite::SqliteQueryResult};

use crate::{api::foundry_user::FoundryUser, database::config::Config, util::logging::log_result};

pub async fn insert_struct(
    db: &Pool<Sqlite>,
    config: Config,
) -> Result<SqliteQueryResult, sqlx::Error> {
    let mut dms: Vec<i64> = Vec::new();

    for dm in config.get_dms() {
        dms.push(dm.get() as i64);
    }

    let mut admins: Vec<i64> = Vec::new();

    for admin in config.get_admins() { 
        admins.push(admin.get() as i64);
    }

    let result = sqlx::query(r#"REPLACE INTO config (guid, dms, admins, foundry_status_channel, world_id) VALUES (?1, ?2, ?3, ?4, ?5)"#)
        .bind(config.get_guid().get() as i64)
        .bind(serde_json::to_value(dms).unwrap())
        .bind(serde_json::to_value(admins).unwrap())
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
    let row = sqlx::query(
        "SELECT foundry_id, actor_id FROM foundry_users WHERE discord_id = ?",
    )
    .bind(discord_id.get() as i64)
    .fetch_optional(db)
    .await?;

    Ok(row.map(|row| {
        FoundryUser::new(
            discord_id,
            row.get("foundry_id"),
            row.get("actor_id")
        )
    }))
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

pub async fn get_all_config(db: &Pool<Sqlite>) -> Result<Vec<Config>, sqlx::Error> {
    let sql = format!(r#"SELECT ALL * FROM config"#);

    debug!("{}", sql);

    let result = sqlx::query_as::<_, Config>(AssertSqlSafe(sql))
        .fetch_all(db)
        .await;

    log_result(&result, "Find any Config Data");

    result
}
