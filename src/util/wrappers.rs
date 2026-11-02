use serenity::{
    builder::{CreateInteractionResponse, CreateInteractionResponseMessage},
    model::{
        application::{CommandData, CommandInteraction, ResolvedValue},
        channel::PartialChannel,
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

/// Finds the option called `name` and converts its resolved value with `extract`
pub fn get_arg<'a, T>(
    data: &'a CommandData,
    name: &str,
    extract: impl FnOnce(ResolvedValue<'a>) -> Option<T>,
) -> Option<T> {
    data.options()
        .into_iter()
        .find(|option| option.name == name)
        .and_then(|option| extract(option.value))
}

/// Defines a typed getter for a named slash command option
macro_rules! arg_getter {
    ($(#[$attr:meta])* $fn_name:ident, $name:literal, $variant:pat => $value:expr, $out:ty) => {
        $(#[$attr])*
        pub fn $fn_name(command: &CommandInteraction) -> Option<$out> {
            get_arg(&command.data, $name, |value| match value {
                $variant => Some($value),
                _ => None,
            })
        }
    };
}

arg_getter!(
    /// The `user` option
    get_user_arg, "user", ResolvedValue::User(user, _) => user, &User
);
arg_getter!(
    /// The `channel` option
    get_channel_arg, "channel", ResolvedValue::Channel(channel) => channel, &PartialChannel
);
arg_getter!(
    /// The `id` option
    get_id_arg, "id", ResolvedValue::String(id) => id, &str
);

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn data(options: serde_json::Value) -> CommandData {
        serde_json::from_value(json!({
            "id": "1",
            "name": "test",
            "type": 1,
            "resolved": {
                "users": {
                    "123": { "id": "123", "username": "bob", "discriminator": "0", "avatar": null }
                },
                "channels": {
                    "456": { "id": "456", "type": 0, "permissions": "0" }
                }
            },
            "options": options,
        }))
        .unwrap()
    }

    #[test]
    fn quick_response_is_an_ephemeral_message() {
        let response = quick_create_interaction_response("hi".to_string(), true);
        let value = serde_json::to_value(response).unwrap();

        assert_eq!(value["data"]["content"], "hi");
        assert_eq!(value["data"]["flags"], 64);
    }

    #[test]
    fn quick_response_can_be_public() {
        let response = quick_create_interaction_response("hi".to_string(), false);
        let value = serde_json::to_value(response).unwrap();

        assert_eq!(value["data"]["content"], "hi");
        assert_ne!(value["data"]["flags"], 64);
    }

    #[test]
    fn get_arg_finds_options_by_name() {
        let data = data(json!([
            { "name": "other", "type": 3, "value": "no" },
            { "name": "id", "type": 3, "value": "yes" },
        ]));

        let found = get_arg(&data, "id", |value| match value {
            ResolvedValue::String(text) => Some(text.to_string()),
            _ => None,
        });

        assert_eq!(found.as_deref(), Some("yes"));
    }

    #[test]
    fn get_arg_is_none_for_missing_options() {
        let data = data(json!([]));

        assert!(get_arg(&data, "id", |_| Some(())).is_none());
    }

    #[test]
    fn get_arg_is_none_when_extract_rejects_the_value() {
        let data = data(json!([{ "name": "id", "type": 3, "value": "yes" }]));

        let found = get_arg(&data, "id", |value| match value {
            ResolvedValue::Integer(number) => Some(number),
            _ => None,
        });

        assert!(found.is_none());
    }

    #[test]
    fn user_getter_resolves_the_mentioned_user() {
        let data = data(json!([{ "name": "user", "type": 6, "value": "123" }]));

        let user = get_arg(&data, "user", |value| match value {
            ResolvedValue::User(user, _) => Some(user.name.clone()),
            _ => None,
        });

        assert_eq!(user.as_deref(), Some("bob"));
    }
}
