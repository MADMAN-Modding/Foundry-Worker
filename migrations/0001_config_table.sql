CREATE TABLE IF NOT EXISTS config (
    guid INTEGER PRIMARY KEY DEFAULT 0,
    dms INTEGER [] DEFAULT [],
    admins INTEGER [] DEFAULT [],
    foundry_status_channel INTEGER DEFAULT 0,
    api_Key VARCHAR DEFAULT ''
)