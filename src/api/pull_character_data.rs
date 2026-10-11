use std::{sync::LazyLock, time::Duration};

use reqwest::Client;
use serde::Deserialize;
use serde_json::{Value, json};

use crate::constants::{API_KEY, URL};

// One shared client so connections get reused between commands
static HTTP: LazyLock<Client> = LazyLock::new(|| {
    Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .unwrap()
});

#[derive(Debug, Deserialize)]
pub struct ActorSummary {
    pub uuid: String,
    pub name: String,
}

#[derive(Deserialize)]
struct GetResponse {
    data: Value,
}

#[derive(Debug, Deserialize)]
pub struct PlayerCharacter {
    #[serde(rename = "_id")]
    pub id: String,
    pub name: String,
}

#[derive(Debug, Deserialize)]
pub struct FoundryPlayer {
    pub id: String,
    pub name: String,
    pub character: Option<PlayerCharacter>,
}

#[derive(Deserialize)]
struct UsersResponse {
    data: Vec<FoundryPlayer>,
}

#[derive(Deserialize)]
struct ClientEntry {
    #[serde(rename = "clientId")]
    client_id: String,
    #[serde(rename = "worldId")]
    world_id: String,
    #[serde(default, rename = "isOnline")]
    is_online: bool,
}

#[derive(Deserialize)]
struct ClientsResponse {
    #[serde(default)]
    clients: Vec<ClientEntry>,
}

// e.g. http://relay:3010 when the bot and relay share a compose network
fn base_url() -> String {
    URL.get().unwrap().to_string()
}

fn api_key() -> String {
    API_KEY.get().unwrap().to_string()
}

pub async fn resolve_client_id(world_id: &str) -> Result<Option<String>, reqwest::Error> {
    let res = HTTP
        .get(format!("{}/clients", base_url()))
        .header("x-api-key", api_key())
        .send()
        .await?
        .error_for_status()?
        .json::<ClientsResponse>()
        .await?;

    Ok(res
        .clients
        .into_iter()
        .find(|client| client.world_id == world_id && client.is_online)
        .map(|client| client.client_id))
}

/// List the users
/// Note: this returns every user's full actor, so the payload is large. Only call it when binding.
pub async fn list_users(client_id: &str) -> Result<Vec<FoundryPlayer>, reqwest::Error> {
    let res = HTTP
        .get(format!("{}/users", base_url()))
        .header("x-api-key", api_key())
        .query(&[("clientId", client_id)])
        .send()
        .await?
        .error_for_status()?
        .json::<UsersResponse>()
        .await?;

    Ok(res.data)
}

pub async fn get_actor(client_id: &str, uuid: &str) -> Result<Value, reqwest::Error> {
    let res = HTTP
        .get(format!("{}/get", base_url()))
        .header("x-api-key", api_key())
        .query(&[("clientId", client_id), ("uuid", uuid)])
        .send()
        .await?
        .error_for_status()?
        .json::<GetResponse>()
        .await?;

    Ok(res.data)
}

pub async fn roll(
    client_id: &str,
    formula: &str,
    actor_uuid: &str,
) -> Result<Value, reqwest::Error> {
    HTTP.post(format!("{}/roll", base_url()))
        .header("x-api-key", api_key())
        .query(&[("clientId", client_id)])
        .json(&json!({
            "formula": formula,
            "speaker": actor_uuid,
            "createChatMessage": true,
        }))
        .send()
        .await?
        .error_for_status()?
        .json::<Value>()
        .await
}