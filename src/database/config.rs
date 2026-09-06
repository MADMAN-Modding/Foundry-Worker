use log::debug;
use serenity::all::{ChannelId, GuildId, UserId};
use sqlx::{
    AssertSqlSafe, FromRow, Pool, Row, Sqlite,
    sqlite::{SqliteQueryResult, SqliteRow},
};

use crate::util::logging::log_result;

/// Config Structure to Represent Row in config table
///
/// |   guid   | dms             | admins      | foundry_status_channel          |
/// |----------|-----------------|-------------|---------------------------------|
/// | Guild ID | Dungeon Masters | Admin Users | Channel to Send Foundry Updates |
pub struct Config {
    guid: GuildId,
    dms: Vec<UserId>,
    admins: Vec<UserId>,
    foundry_status_channel: ChannelId
}

impl Config {
    /// Creates new Config
    pub fn new(
        guid: GuildId,
        dms: Vec<UserId>,
        admins: Vec<UserId>,
        foundry_status_channel: ChannelId
    ) -> Self {
        Self {
            guid,
            dms,
            admins,
            foundry_status_channel
        }
    }

    /// Checks for user to be a DM
    pub fn check_for_dm(&self, id: UserId) -> bool {
        self.dms.contains(&id)
    }

    /// Checks for user to be an admin
    pub fn check_for_admin(&self, id: UserId) -> bool {
        self.admins.contains(&id)
    }

    /// Gest the channel id for the foundry status channel
    pub fn get_foundry_status_channel(&self) -> ChannelId {
        self.foundry_status_channel
    }

    pub fn get_guid(&self) -> GuildId {
        self.guid
    }
}

impl<'r> FromRow<'r, SqliteRow> for Config {
    fn from_row(row: &'r SqliteRow) -> Result<Self, sqlx::Error> {
        let guid: i64 = row.try_get("guid")?;
        let dms_json: String = row.try_get("dms")?;
        let admins_json: String = row.try_get("admins")?;
        let foundry_status_channel: i64 = row.try_get("foundry_status_channel")?;

        let dms: Vec<i64> =
            serde_json::from_str(&dms_json).map_err(|e| sqlx::Error::ColumnDecode {
                index: "dms".into(),
                source: Box::new(e),
            })?;
        let admins: Vec<i64> =
            serde_json::from_str(&admins_json).map_err(|e| sqlx::Error::ColumnDecode {
                index: "admins".into(),
                source: Box::new(e),
            })?;

        Ok(Config {
            guid: GuildId::new(guid as u64),
            dms: dms.into_iter().map(|id| UserId::new(id as u64)).collect(),
            admins: admins
                .into_iter()
                .map(|id| UserId::new(id as u64))
                .collect(),
            foundry_status_channel: ChannelId::new(foundry_status_channel as u64),
        })
    }
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

pub async fn set_value<V>(
    db: &Pool<Sqlite>,
    guid: GuildId,
    value: V,
    column: &str,
) -> Result<SqliteQueryResult, sqlx::Error>
where
    V: for<'a> sqlx::Encode<'a, Sqlite> + sqlx::Type<Sqlite> + Send,
{
    let sql = format!(r#"REPLACE INTO config (guid, {column}) VALUES (?1, ?2)"#);

    debug!("{}", sql);

    let result = sqlx::query(AssertSqlSafe(sql))
        .bind(guid.get() as i64)
        .bind(value)
        .execute(db)
        .await;

    log_result(&result, "Set Foundry Status Channel");

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
