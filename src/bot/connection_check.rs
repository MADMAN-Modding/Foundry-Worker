use std::{sync::Arc, time::Duration};

use log::{debug, info};

use crate::{
    bot::bot::Bot, constants, util::logging::log_result,
};

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

        let response = response?;

        let success = response.status().is_success();

        debug!("{}", response.status());

        Ok(success)
    }

    pub fn start_thread(self) {
        tokio::task::spawn(async move { self.thread_runner().await });
    }

    /// Runs the loop for checking endpoints
    async fn thread_runner(&self) {
        loop {
                let connected = self.test_connection().await.unwrap_or(false);

                self.bot
                    .send_disconnect(
                        connected
                    )
                    .await;
            

            // Wait a minute between polls
            tokio::time::sleep(Duration::from_secs(60)).await;
        }
    }
}
