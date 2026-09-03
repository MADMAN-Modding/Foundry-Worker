use std::{collections::HashMap, sync::Arc};

use log::debug;
use serenity::{
    all::{
        ChannelId, Command, Context, CreateEmbed, CreateMessage, EventHandler, Http, Interaction,
        Ready,
    }, async_trait,
};
use sqlx::{Pool, Sqlite};
use tokio::sync::Mutex;

use crate::{
    bot::commands::{create_commands, set_foundry_status_channel}, util::{cache::Cache, logging::log_result},
};

#[derive(Clone)]
pub struct Bot {
    database: sqlx::SqlitePool,
    api_key: String,
    http: Arc<Http>,
    previous_con_state: Arc<Mutex<HashMap<i64, bool>>>,
    cache: Arc<Mutex<Cache>>
}

impl Bot {
    pub fn new(database: sqlx::SqlitePool, api_key: String, http: Arc<Http>, cache: Arc<Mutex<Cache>>) -> Bot {
        Self {
            database,
            api_key,
            http,
            previous_con_state: Arc::from(Mutex::from(HashMap::new())),
            cache
        }
    }

    pub async fn send_disconnect(&self, new_con: bool, channel_id: ChannelId, guid: i64) {
        let mut previous_con_state = self.previous_con_state.lock().await;

        let prev_con_state = *previous_con_state.get(&guid).unwrap_or(&false);

        debug!("Conn for {} is {}", guid, new_con);

        if new_con && !prev_con_state {
            let embed = CreateEmbed::new().title("FoundryVTT has Reconnected");
            let builder = CreateMessage::new().embed(embed).tts(true);

            debug!("Stat ChannelId: {}", channel_id.get());

            let res = channel_id.send_message(&self.http, builder).await;

            log_result(&res, "Sent Reconnect Update");
        } else if !new_con && prev_con_state {
            let embed = CreateEmbed::new().title("FoundryVTT has Disconnected");
            let builder = CreateMessage::new().embed(embed).tts(true);

            let res = channel_id.send_message(&self.http, builder).await;

            log_result(&res, "Sent Reconnect Update");
        }

        (*previous_con_state).insert(guid, new_con);
    }

    pub fn get_db(&self) -> &Pool<Sqlite> {
        &self.database
    }

    pub fn get_cache(&self) -> &Arc<Mutex<Cache>> {
        &self.cache
    }

    pub fn get_api_key(&self) -> String {
        self.api_key.clone()
    }
}

#[async_trait]
impl EventHandler for Bot {
    async fn interaction_create(&self, context: Context, interaction: Interaction) {
        if let Some(command) = interaction.command() {
            match command.data.name.as_str() {
                "set_foundry_status_channel" => {
                    command
                        .create_response(
                            &context.http,
                            set_foundry_status_channel(self, &context, &command).await,
                        )
                        .await
                        .ok();
                }
                _ => {}
            }
        }
    }

    /// Runs when the bot is connected to Discord
    async fn ready(&self, ctx: Context, ready: Ready) {
        println!("{} is connected!", ready.user.name);

        for command in create_commands() {
            Command::create_global_command(&ctx.http, command)
                .await
                .unwrap();
        }
    }
}
