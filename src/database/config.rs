use std::fmt;

use serenity::{all::{ChannelId, GuildId, UserId}};
use sqlx::{
    FromRow, Row, sqlite::SqliteRow,
};

use crate::getset;

getset! {
/// Config Structure to Represent Row in config table
///
/// |   guid   | dms             | admins      | foundry_status_channel          | World Id                |
/// |----------|-----------------|-------------|---------------------------------|-------------------------|
/// | Guild ID | Dungeon Masters | Admin Users | Channel to Send Foundry Updates | ID of the Foundry World |
pub struct Config {
    guid: GuildId,
    dms: Vec<UserId> => clone,
    admins: Vec<UserId> => clone,
    foundry_status_channel: ChannelId => copy,
    world_id: String => clone
}
}

impl Config {
    /// Creates new Config
    pub fn new(
        guid: GuildId,
        dms: Vec<UserId>,
        admins: Vec<UserId>,
        foundry_status_channel: ChannelId,
        world_id: String,
    ) -> Self {
        Self {
            guid,
            dms,
            admins,
            foundry_status_channel,
            world_id
        }
    }

    pub fn default() -> Self {
        Self { guid: GuildId::default(), dms: Vec::new(), admins: Vec::new(), foundry_status_channel: ChannelId::default(), world_id: String::new() }
    }

    /// Checks for user to be a DM
    pub fn check_for_dm(&self, id: UserId) -> bool {
        self.dms.contains(&id)
    }

    /// Checks for user to be an admin
    pub fn check_for_admin(&self, id: UserId) -> bool {
        self.admins.contains(&id)
    }
}

impl fmt::Display for Config {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut dms = String::from("[\n");
        let mut admins = String::from("[\n");

        for dm in self.dms.clone() {
            dms += &format!("{},\n", &dm.get().to_string());
        }
        dms += "]";

        for admin in self.admins.clone() {
            admins += &format!("{},\n", &admin.get().to_string());
        }
        admins += "]";

        write!(f, "Config Outline\nGUID: {}\nFoundry Status Channel: {}\nDMs: {}\nAdmins: {}", self.guid, self.foundry_status_channel, dms, admins)
    }
}

impl<'r> FromRow<'r, SqliteRow> for Config {
    fn from_row(row: &'r SqliteRow) -> Result<Self, sqlx::Error> {
        let guid: i64 = row.try_get("guid")?;
        let dms_json: String = row.try_get("dms")?;
        let admins_json: String = row.try_get("admins")?;
        let foundry_status_channel: i64 = row.try_get("foundry_status_channel")?;
        let world_id: String = row.try_get("world_id")?;

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
            world_id
        })
    }
}