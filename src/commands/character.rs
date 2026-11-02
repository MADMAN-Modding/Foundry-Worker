use serenity::{
    all::{
        CommandInteraction, CommandOptionType, ComponentInteraction, ComponentInteractionDataKind,
        CreateActionRow, CreateCommand, CreateCommandOption, CreateInteractionResponse,
        CreateSelectMenu, CreateSelectMenuKind, CreateSelectMenuOption, Member, prelude::Context,
    },
    builder::CreateInteractionResponseMessage,
    model::id::UserId,
};

use crate::{
    api::{
        foundry_user::FoundryUser,
        pull_character_data::{self, FoundryPlayer},
    },
    bot::bot::Bot,
    database::config::Config,
    util::{
        database::{insert_foundry_user, load_config},
        logging::log_result,
        wrappers::{get_user_arg, quick_create_interaction_response},
    },
};

pub const BIND_PREFIX: &str = "bind_foundry:";
const MAX_SELECT_OPTIONS: usize = 25;

pub fn bind_foundry_user_command() -> CreateCommand {
    CreateCommand::new("bind_foundry_user")
        .description("Bind a Discord user to their Foundry player and character")
        .add_option(
            CreateCommandOption::new(CommandOptionType::User, "user", "Discord user to bind")
                .required(true),
        )
}

/// Same rule as the other commands: Discord admin, bot admin, or DM
fn can_bind(config: &Config, member: Option<&Member>) -> bool {
    member.is_some_and(|member| config.has_dm_access(member.user.id, member.permissions))
}

/// The Discord user id encoded in a `bind_foundry:<id>` custom id
fn parse_bind_target(custom_id: &str) -> Option<u64> {
    custom_id
        .strip_prefix(BIND_PREFIX)
        .and_then(|id| id.parse::<u64>().ok())
}

/// Splits a select value of the form `<foundry user id>:<actor id>`
fn parse_select_value(value: &str) -> Option<(&str, &str)> {
    value.split_once(':')
}

// Players without an assigned character have no actor to bind to, so they're skipped.
// The value carries both ids so the select handler needs no second API call.
fn bind_options(players: &[FoundryPlayer]) -> Vec<CreateSelectMenuOption> {
    players
        .iter()
        .filter_map(|player| {
            let character = player.character.as_ref()?;

            Some(CreateSelectMenuOption::new(
                format!("{} ({})", player.name, character.name),
                format!("{}:{}", player.id, character.id),
            ))
        })
        .take(MAX_SELECT_OPTIONS)
        .collect()
}

pub async fn bind_foundry_user(
    bot: &Bot,
    _ctx: &Context,
    command: &CommandInteraction,
) -> CreateInteractionResponse {
    bind_foundry_user_inner(bot, command).await
}

async fn bind_foundry_user_inner(
    bot: &Bot,
    command: &CommandInteraction,
) -> CreateInteractionResponse {
    let config = load_config(bot.get_db(), command.guild_id.unwrap()).await;

    if !can_bind(&config, command.member.as_deref()) {
        return quick_create_interaction_response("Only DMs can bind users".to_string(), true);
    }

    let world_id = config.get_world_id();

    if world_id.is_empty() {
        return quick_create_interaction_response(
            "No World Id set, run /set_world_id first".to_string(),
            true,
        );
    }

    let res = pull_character_data::resolve_client_id(&world_id).await;
    log_result(&res, "Foundry client lookup");

    let client_id = match res {
        Ok(Some(id)) => id,
        Ok(None) => {
            return quick_create_interaction_response(
                "That world isn't connected to the relay".to_string(),
                true,
            );
        }
        Err(_) => {
            return quick_create_interaction_response("Couldn't reach Foundry".to_string(), true);
        }
    };

    let Some(target) = get_user_arg(command) else {
        return quick_create_interaction_response("Missing the user option".to_string(), true);
    };

    let res = pull_character_data::list_users(&client_id).await;
    log_result(&res, "Foundry user list");

    let Ok(players) = res else {
        return quick_create_interaction_response("Couldn't reach Foundry".to_string(), true);
    };

    let options = bind_options(&players);

    if options.is_empty() {
        return quick_create_interaction_response(
            "No Foundry users have a character assigned".to_string(),
            true,
        );
    }

    let menu = CreateSelectMenu::new(
        format!("{}{}", BIND_PREFIX, target.id),
        CreateSelectMenuKind::String { options },
    )
    .placeholder("Pick the Foundry player");

    CreateInteractionResponse::Message(
        CreateInteractionResponseMessage::new()
            .content(format!("Which Foundry player is <@{}>?", target.id))
            .components(vec![CreateActionRow::SelectMenu(menu)])
            .ephemeral(true),
    )
}

// custom_id format: bind_foundry:<discord user id>, select value format: <foundry user id>:<actor id>
pub async fn bind_foundry_select(
    bot: &Bot,
    _ctx: &Context,
    component: &ComponentInteraction,
) -> CreateInteractionResponse {
    let ComponentInteractionDataKind::StringSelect { values } = &component.data.kind else {
        return quick_create_interaction_response("Invalid selection".to_string(), true);
    };

    let Some(discord_id) = parse_bind_target(&component.data.custom_id) else {
        return quick_create_interaction_response("Invalid selection".to_string(), true);
    };

    let Some((foundry_id, actor_id)) = values.first().and_then(|value| parse_select_value(value))
    else {
        return quick_create_interaction_response("Invalid selection".to_string(), true);
    };

    let config = load_config(bot.get_db(), component.guild_id.unwrap()).await;

    if !can_bind(&config, component.member.as_ref()) {
        return quick_create_interaction_response("Only DMs can bind users".to_string(), true);
    }

    let user = FoundryUser::new(
        UserId::new(discord_id),
        foundry_id.to_string(),
        actor_id.to_string(),
    );

    let res = insert_foundry_user(bot.get_db(), user).await;
    log_result(&res, "Set Foundry user binding");

    if res.is_err() {
        return quick_create_interaction_response("Couldn't save the binding".to_string(), true);
    }

    // UpdateMessage swaps the menu for the confirmation in place
    CreateInteractionResponse::UpdateMessage(
        CreateInteractionResponseMessage::new()
            .content(format!("Bound <@{}> to that Foundry player", discord_id))
            .components(vec![]),
    )
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use serenity::model::{Permissions, id::GuildId};

    use super::*;
    use crate::util::{
        database::insert_struct,
        test_support::{GUILD, command, content, test_bot, user_option},
    };

    fn players(value: serde_json::Value) -> Vec<FoundryPlayer> {
        serde_json::from_value(value).unwrap()
    }

    #[test]
    fn parse_bind_target_reads_the_user_id() {
        assert_eq!(parse_bind_target("bind_foundry:12345"), Some(12345));
    }

    #[test]
    fn parse_bind_target_rejects_bad_ids() {
        assert_eq!(parse_bind_target("bind_foundry:abc"), None);
        assert_eq!(parse_bind_target("bind_foundry:"), None);
        assert_eq!(parse_bind_target("bind_foundry:-1"), None);
        assert_eq!(parse_bind_target("sheet:1"), None);
        assert_eq!(parse_bind_target("12345"), None);
    }

    #[test]
    fn parse_select_value_splits_on_the_first_colon() {
        assert_eq!(parse_select_value("user:actor"), Some(("user", "actor")));
        assert_eq!(parse_select_value("a:b:c"), Some(("a", "b:c")));
        assert_eq!(parse_select_value(":x"), Some(("", "x")));
        assert_eq!(parse_select_value("nocolon"), None);
    }

    #[test]
    fn bind_options_skip_players_without_a_character() {
        let players = players(json!([
            { "id": "u1", "name": "Alice", "character": { "_id": "a1", "name": "Ayla" } },
            { "id": "u2", "name": "Bob", "character": null },
            { "id": "u3", "name": "Gamemaster" },
        ]));

        let options = serde_json::to_value(bind_options(&players)).unwrap();

        assert_eq!(options.as_array().unwrap().len(), 1);
        assert_eq!(options[0]["label"], "Alice (Ayla)");
        assert_eq!(options[0]["value"], "u1:a1");
    }

    #[test]
    fn bind_options_are_capped_at_the_discord_limit() {
        let many: Vec<_> = (0..40)
            .map(|i| {
                json!({
                    "id": format!("u{i}"),
                    "name": format!("P{i}"),
                    "character": { "_id": format!("a{i}"), "name": format!("C{i}") },
                })
            })
            .collect();

        let options = bind_options(&players(json!(many)));

        assert_eq!(options.len(), MAX_SELECT_OPTIONS);
    }

    #[test]
    fn bind_options_for_nobody_is_empty() {
        assert!(bind_options(&[]).is_empty());
    }

    #[test]
    fn bound_values_round_trip_through_the_parsers() {
        let players = players(json!([
            { "id": "user", "name": "A", "character": { "_id": "actor", "name": "B" } },
        ]));
        let options = serde_json::to_value(bind_options(&players)).unwrap();
        let value = options[0]["value"].as_str().unwrap();

        assert_eq!(parse_select_value(value), Some(("user", "actor")));
    }

    #[test]
    fn command_requires_a_user_option() {
        let command = serde_json::to_value(bind_foundry_user_command()).unwrap();

        assert_eq!(command["name"], "bind_foundry_user");
        assert_eq!(command["options"][0]["name"], "user");
        assert_eq!(command["options"][0]["required"], true);
    }

    #[tokio::test]
    async fn binding_is_denied_for_regular_members() {
        let bot = test_bot().await;
        let interaction = command("bind_foundry_user", user_option(50), 2, 0);

        let text = content(bind_foundry_user_inner(&bot, &interaction).await);

        assert_eq!(text, "Only DMs can bind users");
    }

    #[tokio::test]
    async fn binding_needs_a_world_id_first() {
        let bot = test_bot().await;
        let interaction = command(
            "bind_foundry_user",
            user_option(50),
            1,
            Permissions::ADMINISTRATOR.bits(),
        );

        let text = content(bind_foundry_user_inner(&bot, &interaction).await);

        assert_eq!(text, "No World Id set, run /set_world_id first");
    }

    #[tokio::test]
    async fn dms_pass_the_permission_gate() {
        let bot = test_bot().await;
        let mut config = Config::default();
        config.set_guid(GuildId::new(GUILD));
        config.set_dms(vec![UserId::new(3)]);
        insert_struct(bot.get_db(), config).await.unwrap();

        let interaction = command("bind_foundry_user", user_option(50), 3, 0);
        let text = content(bind_foundry_user_inner(&bot, &interaction).await);

        // Past the gate, the next check is the missing world id
        assert_eq!(text, "No World Id set, run /set_world_id first");
    }
}
