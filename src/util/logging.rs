use log::{info, warn};
use serenity::{Error, all::Message};

pub fn log_message_result(result: Result<Message, Error>, message: &str) {
    match result {
        Ok(_) => info!("{}", message),
        Err(e) => warn!("{}: {}", message, e)
    }
}