use log::debug;
use sqlx::{AssertSqlSafe, Pool, Sqlite, sqlite::SqliteQueryResult};

use crate::{database::config::Config, util::logging::log_result};

pub async fn set_config(
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

    let result = sqlx::query(r#"REPLACE INTO config (guid, dms, admins, foundry_status_channel) VALUES (?1, ?2, ?3, ?4)"#)
        .bind(config.get_guid().get() as i64)
        .bind(serde_json::to_value(dms).unwrap())
        .bind(serde_json::to_value(admins).unwrap())
        .bind(config.get_foundry_status_channel().get() as i64)
        .execute(db)
        .await;

    log_result(&result, "Set Foundry Status Channel");

    result
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
