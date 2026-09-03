use std::{collections::HashMap, sync::Arc, time::Duration};

use log::{debug, info};
use serenity::model::id::ChannelId;

use crate::{
    bot::bot::Bot,
    database::config::get_all_config,
    util::{cache::CacheData, logging::log_result},
};

pub struct ConnectionChecker {
    bot: Arc<Bot>,
    api_key: String,
}

impl ConnectionChecker {
    pub fn new(bot: Arc<Bot>, api_key: String) -> Self {
        Self { bot, api_key }
    }

    async fn test_connection(&self, endpoint: &str) -> reqwest::Result<bool> {
        let request_url = format!("{}/clients", endpoint);

        info!("Testing connection for endpoint: {}", request_url);

        debug!("{}", self.api_key.clone());

        let timeout = Duration::from_secs(5);
        let client = reqwest::ClientBuilder::new().timeout(timeout).build()?;
        let response = client
            .get(request_url)
            .header("x-api-key", self.api_key.clone())
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
            let mut endpoints: Vec<String> = Vec::new();
            let mut channel_ids: Vec<ChannelId> = Vec::new();
            let mut guids: Vec<i64> = Vec::new();

            // Separated for lock
            let caches: HashMap<i64, CacheData>;
            {
                // Load configs from Cache
                let guard = self.bot.get_cache().lock().await;
                caches = guard.get_all_caches();
            }

            debug!("Cache Length: {}", caches.len());

            if caches.len() == 0 {
                let caches = get_all_config(self.bot.get_db()).await;

                log_result(&caches, "Fill Cache");

                if caches.is_ok() {
                    let caches = caches.unwrap();

                    for cache in caches.iter() {
                        let foundry_status_channel_id = cache.get_foundry_status_channel();
                        let endpoint = cache.get_endpoint();
                        let guid = cache.get_guid().get() as i64;

                        endpoints.push(endpoint.clone());
                        channel_ids.push(foundry_status_channel_id);
                        guids.push(guid);

                        self.bot.get_cache().lock().await.set_cache_data(guid, &CacheData::new(foundry_status_channel_id, endpoint));
                    }
                }
            } else {
                // Push all values to the vectors
                for (guid, cache) in caches {
                    endpoints.push(cache.get_endpoint());
                    channel_ids.push(*cache.get_foundry_status_channel());
                    guids.push(guid);
                }
            }

            // If successful loop through all endpoints
            for (i, endpoint) in endpoints.iter().enumerate() {
                let connected = self.test_connection(endpoint).await.unwrap_or(false);

                self.bot
                    .send_disconnect(
                        connected,
                        *channel_ids.get(i).unwrap(),
                        *guids.get(i).unwrap(),
                    )
                    .await;
            }

            // Wait a minute between polls
            tokio::time::sleep(Duration::from_secs(60)).await;
        }
    }
}
