use std::sync::Arc;

use serenity::{
    all::{ChannelId, Context, CreateEmbed, CreateMessage, EventHandler, Http, Message},
    async_trait,
    futures::lock::Mutex,
};

#[derive(Clone)]
pub struct Bot {
    database: sqlx::SqlitePool,
    api_key: String,
    http: Arc<Http>,
    previous_con_state: Arc<Mutex<bool>>,
}

impl Bot {
    pub fn new(database: sqlx::SqlitePool, api_key: String, http: Arc<Http>) -> Bot {
        Self {
            database,
            api_key,
            http,
            previous_con_state: Arc::from(Mutex::from(false)),
        }
    }

    pub async fn send_disconnect(&self, new_con: bool) {
        let channel_id = ChannelId::new(1529932950458859551);

        let mut previous_con_state = self.previous_con_state.lock().await;

        if new_con && !*previous_con_state {
            let embed = CreateEmbed::new().title("FoundryVTT has Reconnected");
            let builder = CreateMessage::new().embed(embed).tts(true);

            let _ = channel_id.send_message(&self.http, builder).await;
        } else if !new_con && *previous_con_state {
            let embed = CreateEmbed::new().title("FoundryVTT has Disconnected");
            let builder = CreateMessage::new().embed(embed).tts(true);

            let _ = channel_id.send_message(&self.http, builder).await;
        }

        *previous_con_state = new_con;
    }
}

#[async_trait]
impl EventHandler for Bot {
    async fn message(&self, context: Context, msg: Message) {
        if msg.author.bot {
            return;
        }

        // let builder = CreateMessage::new().content("content");

        // msg.channel_id.send_message(&context.http, builder).await.unwrap();
    }
}
