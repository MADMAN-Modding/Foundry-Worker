use std::{sync::Arc, time::Duration};

use crate::bot::bot::Bot;

pub struct ConnectionChecker {
    bot: Arc<Bot>,
    api_key: String
}

impl ConnectionChecker {
    pub fn new(bot: Arc<Bot>, api_key: String) -> Self {
        Self { bot, api_key }
    }

    async fn test_connection(&self) -> reqwest::Result<bool> {
        let request_url = "http://localhost:3010/clients";
        let timeout = Duration::from_secs(5);
        let client = reqwest::ClientBuilder::new().timeout(timeout).build()?;
        let response = client.get(request_url).header("x-api-key", self.api_key.clone()).send().await?;

        let success = response.status().is_success();

        Ok(success)
    }

    pub fn start_thread(self) {
        tokio::task::spawn(async move { self.thread_runner().await });
    }

    async fn thread_runner(&self) {
        loop {
            let connected = self.test_connection().await.unwrap_or(false);

            self.bot.send_disconnect(connected).await;

            tokio::time::sleep(Duration::from_secs(5)).await;
        }
    }
}