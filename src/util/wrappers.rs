use serenity::{
    builder::{CreateInteractionResponse, CreateInteractionResponseMessage},
    model::{
        application::{CommandInteraction, ResolvedValue},
        user::User,
    },
};

/// Quickly build a CreateInteractionResponse of type Message
pub fn quick_create_interaction_response(
    content: String,
    ephemeral: bool,
) -> CreateInteractionResponse {
    CreateInteractionResponse::Message(
        CreateInteractionResponseMessage::new()
            .content(content)
            .ephemeral(ephemeral),
    )
}

pub fn get_user_arg(command: &CommandInteraction) -> &User {
    command
        .data
        .options()
        .iter()
        .find(|option| option.name == "user")
        .and_then(|option| match option.value {
            ResolvedValue::User(user, _) => Some(user),
            _ => None,
        })
        .unwrap()
}
