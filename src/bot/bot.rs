use std::sync::Arc;

use serenity::{
    all::{Command, Context, CreateEmbed, CreateMessage, EventHandler, Http, Interaction, Ready},
    async_trait,
    model::{Color, id::ChannelId},
};
use sqlx::{Pool, Sqlite};
use tokio::sync::Mutex;

use crate::{
    bot::commands::{
        add_admin, add_dm, create_commands, remove_admin, remove_dm, set_foundry_status_channel,
        set_world_id,
    }, commands::{
        character::{bind_foundry_select, bind_foundry_user}, sheet::{sheet, sheet_page},
    }, util::{database::load, logging::log_result, wrappers::quick_create_interaction_response},
};

#[derive(Clone)]
pub struct Bot {
    database: sqlx::SqlitePool,
    http: Arc<Http>,
    previous_con_state: Arc<Mutex<bool>>,
}

impl Bot {
    pub fn new(database: sqlx::SqlitePool, http: Arc<Http>) -> Bot {
        Self {
            database,
            http,
            previous_con_state: Arc::from(Mutex::from(false)),
        }
    }

    pub async fn send_disconnect(&self, new_con: bool) {
        let mut previous_con_state = self.previous_con_state.lock().await;

        let res_channel_id = load(&self.database).await.map_err(|_| return);

        let channel_id: ChannelId;

        if res_channel_id.is_ok() {
            channel_id = res_channel_id.unwrap().get_foundry_status_channel();
        } else {
            return;
        }

        if new_con && !*previous_con_state {
            let embed = CreateEmbed::new()
                .title("FoundryVTT has Reconnected")
                .color(Color::DARK_GREEN);
            let builder = CreateMessage::new().embed(embed).tts(true);

            let res = channel_id.send_message(&self.http, builder).await;

            log_result(&res, "Sent Reconnect Update");
        } else if !new_con && *previous_con_state {
            let embed = CreateEmbed::new()
                .title("FoundryVTT has Disconnected")
                .color(Color::RED);
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
    async fn interaction_create(&self, ctx: Context, interaction: Interaction) {
        if let Some(component) = interaction.as_message_component() {
            if component.data.custom_id.starts_with("bind_foundry:") {
                component
                    .create_response(&ctx.http, bind_foundry_select(self, &ctx, component).await)
                    .await
                    .ok();
            }
            if component.data.custom_id.starts_with("sheet:") {
                component
                    .create_response(&ctx.http, sheet_page(self, &ctx, component).await)
                    .await
                    .ok();
            }
        }

        if let Some(command) = interaction.command() {
            match command.data.name.as_str() {
                "set_foundry_status_channel" => command.create_response(
                    &ctx.http,
                    set_foundry_status_channel(self, &ctx, &command).await,
                ),
                "add_dm" => command.create_response(&ctx.http, add_dm(self, &ctx, &command).await),
                "remove_dm" => {
                    command.create_response(&ctx.http, remove_dm(self, &ctx, &command).await)
                }
                "add_admin" => {
                    command.create_response(&ctx.http, add_admin(self, &ctx, &command).await)
                }
                "remove_admin" => {
                    command.create_response(&ctx.http, remove_admin(self, &ctx, &command).await)
                }
                "set_world_id" => {
                    command.create_response(&ctx.http, set_world_id(self, &ctx, &command).await)
                }
                "bind_foundry_user" => command
                    .create_response(&ctx.http, bind_foundry_user(self, &ctx, &command).await),
                "sheet" => command.create_response(&ctx.http, sheet(self, &ctx, &command).await),
                _ => command.create_response(
                    &ctx.http,
                    quick_create_interaction_response("Invalid Command".to_string(), true),
                ),
            }
            .await
            .ok();
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
