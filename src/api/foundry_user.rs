use serenity::all::UserId;

use crate::getters;

getters! {
pub struct FoundryUser {
    discord_id: UserId,
    foundry_id: String,
    actor_id: String => clone,
    client_id: String => clone
}
}

impl FoundryUser {
    pub fn new(
        discord_id: UserId,
        foundry_id: String,
        actor_id: String,
        client_id: String,
    ) -> Self {
        Self {
            discord_id,
            foundry_id,
            actor_id,
            client_id,
        }
    }
}
