use serenity::{all::{Context, CreateMessage, EventHandler, Message}, async_trait};

pub struct Bot {
    pub database: sqlx::SqlitePool,
}

#[async_trait]
impl EventHandler for Bot {
    async fn message(&self, context: Context, msg: Message) {
        if msg.author.bot {return}

        let builder = CreateMessage::new().content("content");

        msg.channel_id.send_message(&context.http, builder).await.unwrap();
    }
}