use std::fmt;

use serenity::{
    all::{ChannelId, GuildId, Permissions, UserId},
};
use sqlx::{FromRow, Row, sqlite::SqliteRow};

use crate::{getset, util::database::ids_from_json};

getset! {
/// Config Structure to Represent Row in config table
///
/// |   guid   | dms             | admins      | foundry_status_channel          | World Id                |
/// |----------|-----------------|-------------|---------------------------------|-------------------------|
/// | Guild ID | Dungeon Masters | Admin Users | Channel to Send Foundry Updates | ID of the Foundry World |
#[derive(Debug, Default, PartialEq)]
pub struct Config {
    guid: GuildId,
    dms: Vec<UserId> => clone,
    admins: Vec<UserId> => clone,
    foundry_status_channel: ChannelId => copy,
    world_id: String => clone
}
}

impl Config {
    /// Checks for user to be a DM
    pub fn check_for_dm(&self, id: UserId) -> bool {
        self.dms.contains(&id)
    }

    /// Checks for user to be an admin
    pub fn check_for_admin(&self, id: UserId) -> bool {
        self.admins.contains(&id)
    }

    /// Discord administrators and configured admins
    pub fn has_admin_access(&self, id: UserId, permissions: Option<Permissions>) -> bool {
        permissions.is_some_and(|perms| perms.contains(Permissions::ADMINISTRATOR))
            || self.check_for_admin(id)
    }

    /// Anyone with admin access, plus configured DMs
    pub fn has_dm_access(&self, id: UserId, permissions: Option<Permissions>) -> bool {
        self.has_admin_access(id, permissions) || self.check_for_dm(id)
    }
}

fn format_ids(ids: &[UserId]) -> String {
    let lines: String = ids.iter().map(|id| format!("{},\n", id.get())).collect();

    format!("[\n{}]", lines)
}

impl fmt::Display for Config {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Config Outline\nGUID: {}\nFoundry Status Channel: {}\nDMs: {}\nAdmins: {}",
            self.guid,
            self.foundry_status_channel,
            format_ids(&self.dms),
            format_ids(&self.admins)
        )
    }
}

impl<'r> FromRow<'r, SqliteRow> for Config {
    fn from_row(row: &'r SqliteRow) -> Result<Self, sqlx::Error> {
        let guid: i64 = row.try_get("guid")?;
        let dms_json: String = row.try_get("dms")?;
        let admins_json: String = row.try_get("admins")?;
        let foundry_status_channel: i64 = row.try_get("foundry_status_channel")?;
        let world_id: String = row.try_get("world_id")?;

        Ok(Config {
            guid: GuildId::new(guid as u64),
            dms: ids_from_json("dms", &dms_json)?,
            admins: ids_from_json("admins", &admins_json)?,
            foundry_status_channel: ChannelId::new(foundry_status_channel as u64),
            world_id,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn user(id: u64) -> UserId {
        UserId::new(id)
    }

    fn config() -> Config {
        Config::new(
            GuildId::new(1),
            vec![user(10), user(11)],
            vec![user(20)],
            ChannelId::new(5),
            "world".to_string(),
        )
    }

    #[test]
    fn default_is_empty() {
        let config = Config::default();

        assert_eq!(config.get_guid(), &GuildId::default());
        assert!(config.get_dms().is_empty());
        assert!(config.get_admins().is_empty());
        assert_eq!(config.get_foundry_status_channel(), ChannelId::default());
        assert!(config.get_world_id().is_empty());
    }

    #[test]
    fn dm_and_admin_checks_look_at_their_own_list() {
        let config = config();

        assert!(config.check_for_dm(user(10)));
        assert!(!config.check_for_dm(user(20)));
        assert!(config.check_for_admin(user(20)));
        assert!(!config.check_for_admin(user(10)));
        assert!(!config.check_for_dm(user(99)));
    }

    #[test]
    fn admin_access_accepts_config_admins_and_discord_administrators() {
        let config = config();

        assert!(config.has_admin_access(user(20), None));
        assert!(config.has_admin_access(user(99), Some(Permissions::ADMINISTRATOR)));
        assert!(!config.has_admin_access(user(10), Some(Permissions::SEND_MESSAGES)));
        assert!(!config.has_admin_access(user(99), None));
    }

    #[test]
    fn dm_access_adds_configured_dms() {
        let config = config();

        assert!(config.has_dm_access(user(10), None));
        assert!(config.has_dm_access(user(20), None));
        assert!(config.has_dm_access(user(99), Some(Permissions::ADMINISTRATOR)));
        assert!(!config.has_dm_access(user(99), Some(Permissions::SEND_MESSAGES)));
    }

    #[test]
    fn display_lists_ids() {
        let text = config().to_string();

        assert!(text.starts_with("Config Outline\nGUID: "));
        assert!(text.contains("DMs: [\n10,\n11,\n]"));
        assert!(text.contains("Admins: [\n20,\n]"));
    }

    #[test]
    fn display_of_empty_lists() {
        let text = Config::default().to_string();

        assert!(text.contains("DMs: [\n]"));
        assert!(text.contains("Admins: [\n]"));
    }

    #[tokio::test]
    async fn from_row_decodes_a_stored_config() {
        let db = crate::util::test_support::memory_db().await;

        sqlx::query(
            "INSERT INTO config (guid, dms, admins, foundry_status_channel, world_id)
             VALUES (7, '[1,2]', '[3]', 9, 'w')",
        )
        .execute(&db)
        .await
        .unwrap();

        let loaded: Config = sqlx::query_as("SELECT * FROM config")
            .fetch_one(&db)
            .await
            .unwrap();

        assert_eq!(
            loaded,
            Config::new(
                GuildId::new(7),
                vec![user(1), user(2)],
                vec![user(3)],
                ChannelId::new(9),
                "w".to_string()
            )
        );
    }

    #[tokio::test]
    async fn from_row_rejects_malformed_json() {
        let db = crate::util::test_support::memory_db().await;

        sqlx::query("INSERT INTO config (guid, dms, admins) VALUES (7, 'not json', '[]')")
            .execute(&db)
            .await
            .unwrap();

        let loaded = sqlx::query_as::<_, Config>("SELECT * FROM config")
            .fetch_one(&db)
            .await;

        assert!(matches!(loaded, Err(sqlx::Error::ColumnDecode { .. })));
    }
}