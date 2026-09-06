use std::sync::Arc;

use log::debug;
use serenity::{
    all::{
        Command, Context, CreateEmbed, CreateMessage, EventHandler, Http, Interaction,
        Ready,
    }, async_trait,
};
use sqlx::{Pool, Sqlite};
use tokio::sync::Mutex;

use crate::{
    bot::commands::{create_commands, set_foundry_status_channel}, database::config::load, util::logging::log_result,
};

#[derive(Clone)]
pub struct Bot {
    database: sqlx::SqlitePool,
    http: Arc<Http>,
    previous_con_state: Arc<Mutex<bool>>
}

impl Bot {
    pub fn new(database: sqlx::SqlitePool, http: Arc<Http>) -> Bot {
        Self {
            database,
            http,
            previous_con_state: Arc::from(Mutex::from(false))
        }
    }

    pub async fn send_disconnect(&self, new_con: bool) {
        let mut previous_con_state = self.previous_con_state.lock().await;

        let channel_id = load(&self.database).await.map_err(|_| return).unwrap().get_foundry_status_channel();

        if new_con && !*previous_con_state {
            let embed = CreateEmbed::new().title("FoundryVTT has Reconnected");
            let builder = CreateMessage::new().embed(embed).tts(true);

            debug!("Stat ChannelId: {}", channel_id.get());

            let res = channel_id.send_message(&self.http, builder).await;

            log_result(&res, "Sent Reconnect Update");
        } else if !new_con && *previous_con_state {
            let embed = CreateEmbed::new().title("FoundryVTT has Disconnected");
            let builder = CreateMessage::new().embed(embed).tts(true);

            let res = channel_id.send_message(&self.http, builder).await;

            log_result(&res, "Sent Reconnect Update");
        }

        *previous_con_state = new_con;
    }

    pub fn get_db(&self) -> &Pool<Sqlite> {
        &self.database
    }

    pub async fn get_prev_con_state(&self) -> bool {
        *self.previous_con_state.lock().await
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
