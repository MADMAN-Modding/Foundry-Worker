use std::{sync::Arc, time::Duration};

use log::info;

use crate::{
    api::pull_character_data::resolve_client_id,
    bot::bot::Bot,
    constants,
    util::{database::load, logging::log_result},
};

pub struct ConnectionChecker {
    bot: Arc<Bot>,
}

impl ConnectionChecker {
    pub fn new(bot: Arc<Bot>) -> Self {
        Self { bot }
    }

    /// Whether the world has an online client connected to the relay
    async fn test_connection(&self, world_id: String) -> reqwest::Result<bool> {
        info!(
            "Testing connection for endpoint: {}/clients",
            constants::URL.get().unwrap()
        );

        let response = resolve_client_id(&world_id).await;

        log_result(&response, "Format Connection Request");

        response.map(|client| client.is_some())
    }

    pub fn start_thread(self) {
        tokio::task::spawn(async move { self.thread_runner().await });
    }

    /// Runs the loop for checking endpoints
    async fn thread_runner(&self) {
        loop {
            if let Ok(config) = load(self.bot.get_db()).await {
                let connected = self
                    .test_connection(config.get_world_id())
                    .await
                    .unwrap_or(false);

                self.bot.send_disconnect(connected).await;
            }

            // Wait a minute between polls
            tokio::time::sleep(Duration::from_secs(60)).await;
        }
    }
}
