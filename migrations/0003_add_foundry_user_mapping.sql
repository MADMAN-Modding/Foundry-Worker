CREATE TABLE IF NOT EXISTS foundry_users (
    discord_id INTEGER PRIMARY KEY,
    foundry_id TEXT NOT NULL,
    actor_id TEXT NOT NULL
);