use std::{sync::Arc, time::Duration};

use log::{info, warn};
use serde_json::Value;

use crate::{bot::bot::Bot, constants::{self, WORLD_ID}, util::logging::log_result};

pub struct ConnectionChecker {
    bot: Arc<Bot>,
}

impl ConnectionChecker {
    pub fn new(bot: Arc<Bot>) -> Self {
        Self { bot }
    }

    async fn test_connection(&self) -> reqwest::Result<bool> {
        let endpoint = constants::ENDPOINT.get().unwrap().clone();

        let request_url = format!("{}/clients", endpoint);

        info!("Testing connection for endpoint: {}", request_url);

        let timeout = Duration::from_secs(5);
        let client = reqwest::ClientBuilder::new().timeout(timeout).build()?;
        let response = client
            .get(request_url)
            .header("x-api-key", constants::API_KEY.get().unwrap())
            .send()
            .await;

        log_result(&response, "Format Connection Request");

        // Change to Response data type and throw error if it fails
        let response = response?;

        // Parse payload
        let payload: Value = response
            .json()
            .await
            .map_err(|_| {
                warn!("Invalid JSON in request response");
                return false;
            })
            .unwrap();

        let mut success: bool = false;
        
        for client in payload["clients"].as_array().unwrap().iter() {
            let world_id = client.get("worldId").unwrap().as_str().unwrap();

            if world_id == WORLD_ID.get().unwrap() {
                success = client["isOnline"].as_bool().unwrap();
            }
        }

        Ok(success)
    }

    pub fn start_thread(self) {
        tokio::task::spawn(async move { self.thread_runner().await });
    }

    /// Runs the loop for checking endpoints
    async fn thread_runner(&self) {
        loop {
            let connected = self.test_connection().await.unwrap_or(false);

            self.bot.send_disconnect(connected).await;

            // Wait a minute between polls
            tokio::time::sleep(Duration::from_secs(60)).await;
        }
    }
}
