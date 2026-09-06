CREATE TABLE IF NOT EXISTS config (
    guid INTEGER PRIMARY KEY DEFAULT 0,
    dms TEXT NOT NULL DEFAULT '[]',
    admins TEXT NOT NULL DEFAULT '[]',
    foundry_status_channel INTEGER DEFAULT 0
)