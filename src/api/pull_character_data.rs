use std::{sync::LazyLock, time::Duration};

use reqwest::Client;
use serde::{Deserialize, de::DeserializeOwned};
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

/// The `{ "data": ... }` envelope most relay endpoints wrap their payload in
#[derive(Deserialize)]
struct Data<T> {
    data: T,
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

#[derive(Debug, Deserialize)]
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

/// GET `path` on the relay and decode the JSON body as `T`
async fn get_json<T: DeserializeOwned>(
    path: &str,
    query: &[(&str, &str)],
) -> Result<T, reqwest::Error> {
    HTTP.get(format!("{}{}", base_url(), path))
        .header("x-api-key", api_key())
        .query(query)
        .send()
        .await?
        .error_for_status()?
        .json::<T>()
        .await
}

/// GET `path` and unwrap the `data` envelope
async fn get_data<T: DeserializeOwned>(
    path: &str,
    query: &[(&str, &str)],
) -> Result<T, reqwest::Error> {
    get_json::<Data<T>>(path, query).await.map(|res| res.data)
}

fn find_online_client(clients: Vec<ClientEntry>, world_id: &str) -> Option<String> {
    clients
        .into_iter()
        .find(|client| client.world_id == world_id && client.is_online)
        .map(|client| client.client_id)
}

pub async fn resolve_client_id(world_id: &str) -> Result<Option<String>, reqwest::Error> {
    let res = get_json::<ClientsResponse>("/clients", &[]).await?;

    Ok(find_online_client(res.clients, world_id))
}

/// List the users
/// Note: this returns every user's full actor, so the payload is large. Only call it when binding.
pub async fn list_users(client_id: &str) -> Result<Vec<FoundryPlayer>, reqwest::Error> {
    get_data("/users", &[("clientId", client_id)]).await
}

pub async fn get_actor(client_id: &str, uuid: &str) -> Result<Value, reqwest::Error> {
    get_data("/get", &[("clientId", client_id), ("uuid", uuid)]).await
}

fn roll_body(formula: &str, actor_uuid: &str) -> Value {
    json!({
        "formula": formula,
        "speaker": actor_uuid,
        "createChatMessage": true,
    })
}

pub async fn roll(
    client_id: &str,
    formula: &str,
    actor_uuid: &str,
) -> Result<Value, reqwest::Error> {
    HTTP.post(format!("{}/roll", base_url()))
        .header("x-api-key", api_key())
        .query(&[("clientId", client_id)])
        .json(&roll_body(formula, actor_uuid))
        .send()
        .await?
        .error_for_status()?
        .json::<Value>()
        .await
}

#[cfg(test)]
mod tests {
    use std::{
        io::{BufRead, BufReader, Read, Write},
        net::TcpListener,
        sync::{Mutex, OnceLock},
        thread,
    };

    use super::*;

    const KEY: &str = "secret-key";

    // Every request the mock relay has seen: (method, path with query, lowercase headers, body)
    type Seen = (String, String, String, String);

    static REQUESTS: Mutex<Vec<Seen>> = Mutex::new(Vec::new());
    static SERVER: OnceLock<String> = OnceLock::new();

    fn respond(method: &str, target: &str) -> (u16, String) {
        let path = target.split('?').next().unwrap();

        match (method, path) {
            ("GET", "/clients") => (
                200,
                json!({ "clients": [
                    { "clientId": "offline", "worldId": "w1", "isOnline": false },
                    { "clientId": "c1", "worldId": "w1", "isOnline": true },
                    { "clientId": "c2", "worldId": "w2" },
                ]})
                .to_string(),
            ),
            ("GET", "/users") => (
                200,
                json!({ "data": [
                    { "id": "u1", "name": "Alice", "character": { "_id": "a1", "name": "Ayla" } },
                    { "id": "u2", "name": "Bob", "character": null },
                ]})
                .to_string(),
            ),
            ("GET", "/get") => (
                200,
                json!({ "data": { "name": "Ayla", "items": [] } }).to_string(),
            ),
            ("POST", "/roll") => (200, json!({ "total": 14 }).to_string()),
            ("GET", "/garbage") => (200, "not json".to_string()),
            _ => (500, "{}".to_string()),
        }
    }

    fn handle(mut stream: std::net::TcpStream) {
        let mut reader = BufReader::new(stream.try_clone().unwrap());
        let mut request_line = String::new();
        reader.read_line(&mut request_line).unwrap();

        let mut headers = String::new();
        let mut length = 0;

        loop {
            let mut line = String::new();
            reader.read_line(&mut line).unwrap();

            if line.trim().is_empty() {
                break;
            }

            if let Some(value) = line.to_lowercase().strip_prefix("content-length:") {
                length = value.trim().parse().unwrap();
            }

            headers.push_str(&line.to_lowercase());
        }

        let mut body = vec![0; length];
        reader.read_exact(&mut body).unwrap();

        let mut parts = request_line.split_whitespace();
        let method = parts.next().unwrap().to_string();
        let target = parts.next().unwrap().to_string();

        let authorized = headers.contains(&format!("x-api-key: {}", KEY));
        let (status, payload) = if authorized {
            respond(&method, &target)
        } else {
            (401, "{}".to_string())
        };

        REQUESTS.lock().unwrap().push((
            method,
            target,
            headers,
            String::from_utf8(body).unwrap(),
        ));

        let response = format!(
            "HTTP/1.1 {status} X\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{payload}",
            payload.len()
        );
        stream.write_all(response.as_bytes()).unwrap();
    }

    /// Starts (once) a mock relay on its own thread and points the globals at it
    async fn relay() {
        let address = SERVER.get_or_init(|| {
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            let address = format!("http://{}", listener.local_addr().unwrap());

            thread::spawn(move || {
                for stream in listener.incoming() {
                    thread::spawn(move || handle(stream.unwrap()));
                }
            });

            address
        });

        URL.get_or_init(|| async { address.clone() }).await;
        API_KEY.get_or_init(|| async { KEY.to_string() }).await;
    }

    fn seen(method: &str, target: &str) -> Option<Seen> {
        REQUESTS
            .lock()
            .unwrap()
            .iter()
            .find(|(m, t, _, _)| m == method && t == target)
            .cloned()
    }

    fn clients(value: Value) -> Vec<ClientEntry> {
        serde_json::from_value::<ClientsResponse>(json!({ "clients": value }))
            .unwrap()
            .clients
    }

    #[test]
    fn online_client_matches_world_and_status() {
        let list = clients(json!([
            { "clientId": "a", "worldId": "w1", "isOnline": false },
            { "clientId": "b", "worldId": "w1", "isOnline": true },
            { "clientId": "c", "worldId": "w2", "isOnline": true },
        ]));

        assert_eq!(find_online_client(list, "w1").as_deref(), Some("b"));
    }

    #[test]
    fn offline_or_unknown_worlds_resolve_to_none() {
        let list = || {
            clients(json!([
                { "clientId": "a", "worldId": "w1", "isOnline": false },
                { "clientId": "b", "worldId": "w2" },
            ]))
        };

        assert!(find_online_client(list(), "w1").is_none());
        assert!(find_online_client(list(), "w2").is_none());
        assert!(find_online_client(list(), "nope").is_none());
        assert!(find_online_client(Vec::new(), "w1").is_none());
    }

    #[test]
    fn clients_response_defaults_to_no_clients() {
        let response: ClientsResponse = serde_json::from_value(json!({})).unwrap();

        assert!(response.clients.is_empty());
    }

    #[test]
    fn players_deserialize_with_and_without_characters() {
        let players: Vec<FoundryPlayer> = serde_json::from_value(json!([
            { "id": "1", "name": "A", "character": { "_id": "c", "name": "Char" } },
            { "id": "2", "name": "B" },
            { "id": "3", "name": "C", "character": null },
        ]))
        .unwrap();

        assert_eq!(players[0].character.as_ref().unwrap().id, "c");
        assert!(players[1].character.is_none());
        assert!(players[2].character.is_none());
    }

    #[test]
    fn data_envelope_unwraps_any_payload() {
        let numbers: Data<Vec<u8>> = serde_json::from_value(json!({ "data": [1, 2] })).unwrap();
        let text: Data<String> = serde_json::from_value(json!({ "data": "x" })).unwrap();

        assert_eq!(numbers.data, vec![1, 2]);
        assert_eq!(text.data, "x");
    }

    #[test]
    fn roll_body_has_the_relay_shape() {
        assert_eq!(
            roll_body("1d20+5", "Actor.abc"),
            json!({
                "formula": "1d20+5",
                "speaker": "Actor.abc",
                "createChatMessage": true,
            })
        );
    }

    #[tokio::test]
    async fn resolve_client_id_finds_the_online_client() {
        relay().await;

        assert_eq!(resolve_client_id("w1").await.unwrap().as_deref(), Some("c1"));
        assert_eq!(resolve_client_id("w2").await.unwrap(), None);
        assert_eq!(resolve_client_id("missing").await.unwrap(), None);
    }

    #[tokio::test]
    async fn requests_carry_the_api_key() {
        relay().await;

        resolve_client_id("w1").await.unwrap();

        let (_, _, headers, _) = seen("GET", "/clients").unwrap();

        assert!(headers.contains("x-api-key: secret-key"));
    }

    #[tokio::test]
    async fn list_users_unwraps_the_data_envelope() {
        relay().await;

        let players = list_users("client-1").await.unwrap();

        assert_eq!(players.len(), 2);
        assert_eq!(players[0].name, "Alice");
        assert!(seen("GET", "/users?clientId=client-1").is_some());
    }

    #[tokio::test]
    async fn get_actor_sends_both_query_params() {
        relay().await;

        let actor = get_actor("client-2", "Actor.xyz").await.unwrap();

        assert_eq!(actor["name"], "Ayla");
        assert!(seen("GET", "/get?clientId=client-2&uuid=Actor.xyz").is_some());
    }

    #[tokio::test]
    async fn roll_posts_the_formula_as_json() {
        relay().await;

        let result = roll("client-3", "2d6", "Actor.roller").await.unwrap();

        assert_eq!(result["total"], 14);

        let (_, _, _, body) = seen("POST", "/roll?clientId=client-3").unwrap();

        assert_eq!(
            serde_json::from_str::<Value>(&body).unwrap(),
            roll_body("2d6", "Actor.roller")
        );
    }

    #[tokio::test]
    async fn http_errors_surface_as_errors() {
        relay().await;

        let res = get_json::<Value>("/unknown", &[]).await;

        assert!(res.unwrap_err().is_status());
    }

    #[tokio::test]
    async fn invalid_json_is_a_decode_error() {
        relay().await;

        let res = get_json::<Value>("/garbage", &[]).await;

        assert!(res.unwrap_err().is_decode());
    }
}