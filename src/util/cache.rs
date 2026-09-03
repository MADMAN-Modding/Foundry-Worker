use std::collections::HashMap;

use log::info;
use serenity::model::id::ChannelId;

use crate::util::logging::log_result;

#[derive(Clone)]
pub struct Cache {
    cache_data: HashMap<i64, CacheData>,
}

impl Cache {
    pub fn new() -> Self {
        Self { cache_data: HashMap::new() }
    }

    pub fn set_cache_data(&mut self, guid: i64, cache_data: &CacheData) {
        let _ = &self.cache_data.insert(guid, *cache_data);

        info!("Set Cache Data for: {guid}");
    }

    pub fn get_cache_data(&self, guid: i64) -> Option<&CacheData> {
        let data = self.cache_data.get(&guid);

        data
    }

    pub fn get_all_caches(&self) -> HashMap<i64, CacheData> {
        self.cache_data.clone()
    }
}

#[derive(Clone, Copy)]
pub struct CacheData {
    foundry_status_channel_id: ChannelId,
    endpoint: [u8; 100]
}

impl CacheData {
    pub fn default() -> Self {
        Self { foundry_status_channel_id: ChannelId::default(), endpoint: [0u8; 100] }
    }

    pub fn new(foundry_status_channel_id: ChannelId, endpoint: String) -> Self {
        let bytes = Self::string_to_bytes(endpoint);

        Self { foundry_status_channel_id, endpoint: bytes }
    }

    pub fn set_foundry_status_channel_id(mut self, foundry_status_channel_id: ChannelId) {
        self.foundry_status_channel_id = foundry_status_channel_id;
    }

    pub fn get_foundry_status_channel(&self) -> &ChannelId {
        &self.foundry_status_channel_id
    }

    pub fn get_endpoint(&self) -> String {
        let res = String::from_utf8(self.endpoint.to_vec());

        log_result(&res, "Convert Endpoint");

        match res {
            Ok(v) => v,
            Err(_) => String::new()
        }
    }

    /// Converts the first 100 characters to an array
    fn string_to_bytes(endpoint: String) -> [u8; 100] {
        let mut array: [u8; 100] = [0; 100];

        let bytes = endpoint.as_bytes();

        for (i, byte) in bytes.iter().enumerate() {
            if i < 100 {
                array[i] = *byte;
            } else {
                break;
            }
        }

        array
    }
}