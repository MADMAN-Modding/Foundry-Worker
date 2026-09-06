use std::fmt;

use serenity::{all::{ChannelId, GuildId, UserId}};
use sqlx::{
    FromRow, Row, sqlite::SqliteRow,
};

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

    pub fn default() -> Self {
        Self { guid: GuildId::default(), dms: Vec::new(), admins: Vec::new(), foundry_status_channel: ChannelId::default() }
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

    pub fn get_dms(&self) -> Vec<UserId> {
        self.dms.clone()
    }

    pub fn get_admins(&self) -> Vec<UserId> {
        self.admins.clone()
    }

    pub fn set_foundry_status_channel(&mut self, foundry_status_channel: ChannelId) {
        self.foundry_status_channel = foundry_status_channel;
    }

    pub fn set_dms(&mut self, dms: Vec<UserId>) {
        self.dms = dms
    }

    pub fn set_admins(&mut self, admins: Vec<UserId>) {
        self.admins = admins;
    }

    pub fn set_guid(&mut self, guid: GuildId) {
        self.guid = guid;
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