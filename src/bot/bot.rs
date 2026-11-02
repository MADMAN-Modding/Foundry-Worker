use std::sync::Arc;

use serenity::{
    all::{Command, Context, CreateEmbed, CreateMessage, EventHandler, Http, Interaction, Ready},
    async_trait,
    model::Color,
};
use sqlx::{Pool, Sqlite};
use tokio::sync::Mutex;

use crate::{
    bot::commands::{
        add_admin, add_dm, create_commands, remove_admin, remove_dm, set_foundry_status_channel,
        set_world_id,
    },
    commands::{
        character::{BIND_PREFIX, bind_foundry_select, bind_foundry_user},
        sheet::{SHEET_PREFIX, sheet, sheet_page},
    },
    util::{database::load, logging::log_result, wrappers::quick_create_interaction_response},
};

/// What a change in the Foundry connection state should announce
#[derive(Debug, PartialEq)]
pub enum ConnectionChange {
    Reconnected,
    Disconnected,
}

impl ConnectionChange {
    fn title(&self) -> &'static str {
        match self {
            Self::Reconnected => "FoundryVTT has Reconnected",
            Self::Disconnected => "FoundryVTT has Disconnected",
        }
    }

    fn color(&self) -> Color {
        match self {
            Self::Reconnected => Color::DARK_GREEN,
            Self::Disconnected => Color::RED,
        }
    }
}

/// The announcement owed when the connection goes from `previous` to `current`, if any
pub fn connection_change(previous: bool, current: bool) -> Option<ConnectionChange> {
    match (previous, current) {
        (false, true) => Some(ConnectionChange::Reconnected),
        (true, false) => Some(ConnectionChange::Disconnected),
        _ => None,
    }
}

/// Answers every slash command by name with its handler, falling back to "Invalid Command"
macro_rules! route_commands {
    ($bot:expr, $ctx:expr, $command:expr, { $($name:literal => $handler:ident),+ $(,)? }) => {
        match $command.data.name.as_str() {
            $($name => $command.create_response(
                &$ctx.http,
                $handler($bot, &$ctx, &$command).await,
            ),)+
            _ => $command.create_response(
                &$ctx.http,
                quick_create_interaction_response("Invalid Command".to_string(), true),
            ),
        }
        .await
        .ok()
    };
}

/// Answers message components whose custom id starts with a known prefix
macro_rules! route_components {
    ($bot:expr, $ctx:expr, $component:expr, { $($prefix:expr => $handler:ident),+ $(,)? }) => {
        $(
            if $component.data.custom_id.starts_with($prefix) {
                $component
                    .create_response(&$ctx.http, $handler($bot, &$ctx, $component).await)
                    .await
                    .ok();
            }
        )+
    };
}

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

        let Ok(config) = load(&self.database).await else {
            return;
        };

        if let Some(change) = connection_change(*previous_con_state, new_con) {
            let embed = CreateEmbed::new().title(change.title()).color(change.color());
            let builder = CreateMessage::new().embed(embed).tts(true);

            let res = config
                .get_foundry_status_channel()
                .send_message(&self.http, builder)
                .await;

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
            route_components!(self, ctx, component, {
                BIND_PREFIX => bind_foundry_select,
                SHEET_PREFIX => sheet_page,
            });
        }

        if let Some(command) = interaction.command() {
            route_commands!(self, ctx, command, {
                "set_foundry_status_channel" => set_foundry_status_channel,
                "add_dm" => add_dm,
                "remove_dm" => remove_dm,
                "add_admin" => add_admin,
                "remove_admin" => remove_admin,
                "set_world_id" => set_world_id,
                "bind_foundry_user" => bind_foundry_user,
                "sheet" => sheet,
            });
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::util::test_support::memory_db;

    #[test]
    fn connection_change_only_fires_on_transitions() {
        assert_eq!(connection_change(false, true), Some(ConnectionChange::Reconnected));
        assert_eq!(connection_change(true, false), Some(ConnectionChange::Disconnected));
        assert_eq!(connection_change(true, true), None);
        assert_eq!(connection_change(false, false), None);
    }

    #[test]
    fn changes_have_distinct_titles_and_colors() {
        let up = ConnectionChange::Reconnected;
        let down = ConnectionChange::Disconnected;

        assert_eq!(up.title(), "FoundryVTT has Reconnected");
        assert_eq!(down.title(), "FoundryVTT has Disconnected");
        assert_eq!(up.color(), Color::DARK_GREEN);
        assert_eq!(down.color(), Color::RED);
    }

    #[tokio::test]
    async fn connection_state_starts_disconnected_and_tracks_updates() {
        let bot = Bot::new(memory_db().await, Arc::new(Http::new("token")));

        assert!(!bot.get_prev_con_state().await);

        // With no config stored there's nowhere to announce, so the state is left alone
        bot.send_disconnect(true).await;
        assert!(!bot.get_prev_con_state().await);
    }

    #[tokio::test]
    async fn clones_share_the_database_and_connection_state() {
        let bot = Bot::new(memory_db().await, Arc::new(Http::new("token")));
        let clone = bot.clone();

        *bot.previous_con_state.lock().await = true;

        assert!(clone.get_prev_con_state().await);
    }
}
