use serenity::{
    all::{
        CommandInteraction, CommandOptionType, CreateCommand, CreateCommandOption,
        CreateInteractionResponse, prelude::Context,
    },
    builder::{CreateEmbed, CreateInteractionResponseMessage, CreateMessage},
    model::{Color, Permissions, id::UserId},
};

use crate::{
    bot::bot::Bot,
    commands::{character::bind_foundry_user_command, sheet::sheet_command},
    database::config::Config,
    util::{
        collections::{add_unique, remove_item},
        database::{insert_struct, load_config},
        logging::log_result,
        wrappers::{
            get_channel_arg, get_id_arg, get_user_arg, quick_create_interaction_response,
        },
    },
};

const NOT_ADMIN: &str = "Invalid Permission Level you must be an Administrator";

pub fn create_commands() -> Vec<CreateCommand> {
    vec![
        set_foundry_status_channel_command(),
        add_dm_command(),
        remove_dm_command(),
        add_admin_command(),
        remove_admin_command(),
        set_world_id_command(),
        bind_foundry_user_command(),
        sheet_command(),
    ]
}

/// Builds a command that takes a single required option of the given type
fn single_option_command(
    name: &str,
    description: &str,
    kind: CommandOptionType,
    option: &str,
    option_description: &str,
) -> CreateCommand {
    CreateCommand::new(name).description(description).add_option(
        CreateCommandOption::new(kind, option, option_description).required(true),
    )
}

/// The invoking member's id and Discord permissions, if the command came from a guild
fn caller(command: &CommandInteraction) -> Option<(UserId, Option<Permissions>)> {
    command
        .member
        .as_ref()
        .map(|member| (member.user.id, member.permissions))
}

/// Ok when the caller is a Discord or bot admin, otherwise the message to show them
fn admin_check(
    config: &Config,
    caller: Option<(UserId, Option<Permissions>)>,
) -> Result<(), &'static str> {
    let (id, permissions) = caller.ok_or("Internal Server Error")?;

    if config.has_admin_access(id, permissions) {
        Ok(())
    } else {
        Err(NOT_ADMIN)
    }
}

fn missing_option(name: &str) -> CreateInteractionResponse {
    quick_create_interaction_response(format!("Missing the {} option", name), true)
}

fn set_foundry_status_channel_command() -> CreateCommand {
    single_option_command(
        "set_foundry_status_channel",
        "Channel for Foundry status data to be sent",
        CommandOptionType::Channel,
        "channel",
        "Channel to send Status Updates to",
    )
}

pub async fn set_foundry_status_channel(
    bot: &Bot,
    ctx: &Context,
    command: &CommandInteraction,
) -> CreateInteractionResponse {
    let guid = command.guild_id.unwrap();

    let Some(channel_id) = get_channel_arg(command) else {
        return missing_option("channel");
    };

    // Fetch the channel and make sure it's a guild channel
    let channel = channel_id.id.to_channel(&ctx.http).await.unwrap();
    let Some(guild_channel) = channel.guild() else {
        return CreateInteractionResponse::Message(
            CreateInteractionResponseMessage::new()
                .content("That's not a valid text channel.")
                .ephemeral(true),
        );
    };

    // Fetch guild + bot member to compute effective permissions
    let bot_id = ctx.cache.current_user().id;
    let guild = guid.to_partial_guild(&ctx.http).await.unwrap();
    let bot_member = guild.member(&ctx.http, bot_id).await.unwrap();
    let permissions = guild.user_permissions_in(&guild_channel, &bot_member);

    if !permissions.send_messages() {
        return CreateInteractionResponse::Message(
            CreateInteractionResponseMessage::new().content(format!(
                "I don't have permission to send messages in <#{}>. Please update my permissions and try again.",
                channel_id.id
            )).ephemeral(true),
        );
    }

    let mut config = load_config(bot.get_db(), guid).await;

    config.set_foundry_status_channel(channel_id.id);

    insert_struct(bot.get_db(), config).await.unwrap();

    let embed = CreateEmbed::new()
        .title("This is now the FoundryVTT Status Channel")
        .color(Color::BLITZ_BLUE);
    let builder = CreateMessage::new().embed(embed).tts(true);

    let res = channel_id.id.send_message(&ctx.http, builder).await;

    log_result(
        &res,
        &format!("Foundry VTT Status Channel set to: {}", channel_id.id),
    );

    CreateInteractionResponse::Message(
        CreateInteractionResponseMessage::new()
            .content(format!("Channel Updated to <#{}>", channel_id.id))
            .ephemeral(true),
    )
}

fn add_dm_command() -> CreateCommand {
    single_option_command(
        "add_dm",
        "Add a Dungeon Master to allow them to run DM-level Commands",
        CommandOptionType::User,
        "user",
        "User to become a DM",
    )
}

fn remove_dm_command() -> CreateCommand {
    single_option_command(
        "remove_dm",
        "Remove a user from the DM list",
        CommandOptionType::User,
        "user",
        "User to remove from DM list",
    )
}

fn add_admin_command() -> CreateCommand {
    single_option_command(
        "add_admin",
        "Add an Admin to allow them to run Admin-level Commands",
        CommandOptionType::User,
        "user",
        "User to become an Admin",
    )
    .default_member_permissions(Permissions::ADMINISTRATOR)
}

fn remove_admin_command() -> CreateCommand {
    single_option_command(
        "remove_admin",
        "Remove a user from the Admin list",
        CommandOptionType::User,
        "user",
        "User to remove from Admin list",
    )
    .default_member_permissions(Permissions::ADMINISTRATOR)
}

/// Generates the add and remove handlers for a list of users stored in the config.
/// `guarded` makes both handlers require an admin; the admin list itself relies on
/// the command's default member permissions instead.
macro_rules! role_handlers {
    (
        $add:ident, $remove:ident, $get:ident, $set:ident,
        noun: $noun:literal, log: $log:literal, guarded: $guarded:literal
    ) => {
        pub async fn $add(
            bot: &Bot,
            _ctx: &Context,
            command: &CommandInteraction,
        ) -> CreateInteractionResponse {
            $crate::paste::paste! { [<$add _inner>](bot, command).await }
        }

        pub async fn $remove(
            bot: &Bot,
            _ctx: &Context,
            command: &CommandInteraction,
        ) -> CreateInteractionResponse {
            $crate::paste::paste! { [<$remove _inner>](bot, command).await }
        }

        $crate::paste::paste! {
        // The handlers above only forward here so tests can run without a Discord context
        async fn [<$add _inner>](
            bot: &Bot,
            command: &CommandInteraction,
        ) -> CreateInteractionResponse {
            let Some(user) = get_user_arg(command) else {
                return missing_option("user");
            };

            let mut config = load_config(bot.get_db(), command.guild_id.unwrap()).await;

            if $guarded {
                if let Err(message) = admin_check(&config, caller(command)) {
                    return quick_create_interaction_response(message.to_string(), true);
                }
            }

            let mut users = config.$get();

            if !add_unique(&mut users, user.id) {
                return quick_create_interaction_response(
                    format!("User is already {}", $noun),
                    true,
                );
            }

            config.$set(users);

            let res = insert_struct(bot.get_db(), config).await;
            log_result(&res, $log);

            quick_create_interaction_response(
                format!("Successfully made <@{}> {}", user.id, $noun),
                true,
            )
        }

        async fn [<$remove _inner>](
            bot: &Bot,
            command: &CommandInteraction,
        ) -> CreateInteractionResponse {
            let mut config = load_config(bot.get_db(), command.guild_id.unwrap()).await;

            if $guarded {
                if let Err(message) = admin_check(&config, caller(command)) {
                    return quick_create_interaction_response(message.to_string(), true);
                }
            }

            let Some(target) = get_user_arg(command) else {
                return missing_option("user");
            };

            let mut users = config.$get();

            if !remove_item(&mut users, &target.id) {
                return quick_create_interaction_response(
                    format!("<@{}> is not {}", target.id.get(), $noun),
                    true,
                );
            }

            config.$set(users);

            let res = insert_struct(bot.get_db(), config).await;
            log_result(&res, $log);

            quick_create_interaction_response(
                format!("Removed <@{}> as {}", target.id, $noun),
                true,
            )
        }
        }
    };
}

role_handlers!(
    add_dm, remove_dm, get_dms, set_dms,
    noun: "a DM", log: "Set DMs", guarded: true
);
role_handlers!(
    add_admin, remove_admin, get_admins, set_admins,
    noun: "an Admin", log: "Set Admins", guarded: false
);

fn set_world_id_command() -> CreateCommand {
    single_option_command(
        "set_world_id",
        "Set the id of the World for API calls",
        CommandOptionType::String,
        "id",
        "WorldId to use in the API calls",
    )
}

pub async fn set_world_id(
    bot: &Bot,
    _ctx: &Context,
    command: &CommandInteraction,
) -> CreateInteractionResponse {
    set_world_id_inner(bot, command).await
}

async fn set_world_id_inner(bot: &Bot, command: &CommandInteraction) -> CreateInteractionResponse {
    let Some(world_id) = get_id_arg(command) else {
        return missing_option("id");
    };

    let mut config = load_config(bot.get_db(), command.guild_id.unwrap()).await;

    if let Err(message) = admin_check(&config, caller(command)) {
        return quick_create_interaction_response(message.to_string(), true);
    }

    config.set_world_id(world_id.to_string());

    let res = insert_struct(bot.get_db(), config).await;

    log_result(&res, "Set World Id");

    quick_create_interaction_response(format!("Successfully set World Id to: {}", world_id), true)
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use serenity::model::id::GuildId;

    use super::*;
    use crate::util::{
        database::load,
        test_support::{GUILD, command, content, test_bot, user_option},
    };

    const ADMIN: u64 = Permissions::ADMINISTRATOR.bits();
    const NONE: u64 = 0;

    fn names() -> Vec<String> {
        create_commands()
            .into_iter()
            .map(|command| serde_json::to_value(command).unwrap()["name"].as_str().unwrap().to_string())
            .collect()
    }

    #[test]
    fn every_command_is_registered_once() {
        let mut names = names();
        names.sort();

        assert_eq!(
            names,
            [
                "add_admin",
                "add_dm",
                "bind_foundry_user",
                "remove_admin",
                "remove_dm",
                "set_foundry_status_channel",
                "set_world_id",
                "sheet",
            ]
        );
    }

    #[test]
    fn single_option_commands_have_one_required_option() {
        let command = single_option_command(
            "demo",
            "A demo",
            CommandOptionType::String,
            "thing",
            "The thing",
        );
        let value = serde_json::to_value(command).unwrap();

        assert_eq!(value["name"], "demo");
        assert_eq!(value["description"], "A demo");
        assert_eq!(value["options"].as_array().unwrap().len(), 1);
        assert_eq!(value["options"][0]["name"], "thing");
        assert_eq!(value["options"][0]["required"], true);
    }

    #[test]
    fn option_types_match_what_the_handlers_read() {
        let find = |name: &str| {
            create_commands()
                .into_iter()
                .map(|command| serde_json::to_value(command).unwrap())
                .find(|command| command["name"] == name)
                .unwrap()
        };

        assert_eq!(find("set_foundry_status_channel")["options"][0]["name"], "channel");
        assert_eq!(find("add_dm")["options"][0]["name"], "user");
        assert_eq!(find("remove_dm")["options"][0]["name"], "user");
        assert_eq!(find("add_admin")["options"][0]["name"], "user");
        assert_eq!(find("remove_admin")["options"][0]["name"], "user");
        assert_eq!(find("set_world_id")["options"][0]["name"], "id");
    }

    #[test]
    fn only_the_admin_list_commands_are_locked_by_default() {
        let find = |name: &str| {
            create_commands()
                .into_iter()
                .map(|command| serde_json::to_value(command).unwrap())
                .find(|command| command["name"] == name)
                .unwrap()
        };
        let locked = |name: &str| !find(name)["default_member_permissions"].is_null();

        assert!(locked("add_admin"));
        assert!(locked("remove_admin"));
        assert!(!locked("add_dm"));
        assert!(!locked("set_world_id"));
    }

    #[test]
    fn admin_check_requires_a_member() {
        assert_eq!(admin_check(&Config::default(), None), Err("Internal Server Error"));
    }

    #[test]
    fn admin_check_accepts_discord_administrators_and_bot_admins() {
        let mut config = Config::default();
        config.set_admins(vec![UserId::new(5)]);

        assert_eq!(admin_check(&config, Some((UserId::new(1), Some(Permissions::ADMINISTRATOR)))), Ok(()));
        assert_eq!(admin_check(&config, Some((UserId::new(5), None))), Ok(()));
    }

    #[test]
    fn admin_check_rejects_everyone_else() {
        let mut config = Config::default();
        config.set_dms(vec![UserId::new(7)]);

        assert_eq!(admin_check(&config, Some((UserId::new(7), None))), Err(NOT_ADMIN));
        assert_eq!(
            admin_check(&config, Some((UserId::new(1), Some(Permissions::SEND_MESSAGES)))),
            Err(NOT_ADMIN)
        );
    }

    #[test]
    fn caller_reads_the_member() {
        let interaction = command("add_dm", user_option(50), 9, ADMIN);

        assert_eq!(caller(&interaction), Some((UserId::new(9), Some(Permissions::ADMINISTRATOR))));
    }

    #[tokio::test]
    async fn admins_can_add_a_dm() {
        let bot = test_bot().await;
        let interaction = command("add_dm", user_option(50), 1, ADMIN);

        let text = content(add_dm_inner(&bot, &interaction).await);
        let config = load(bot.get_db()).await.unwrap();

        assert_eq!(text, "Successfully made <@50> a DM");
        assert_eq!(config.get_dms(), vec![UserId::new(50)]);
        assert_eq!(config.get_guid(), &GuildId::new(GUILD));
    }

    #[tokio::test]
    async fn adding_a_dm_twice_is_rejected() {
        let bot = test_bot().await;
        let interaction = command("add_dm", user_option(50), 1, ADMIN);

        add_dm_inner(&bot, &interaction).await;
        let text = content(add_dm_inner(&bot, &interaction).await);

        assert_eq!(text, "User is already a DM");
        assert_eq!(load(bot.get_db()).await.unwrap().get_dms().len(), 1);
    }

    #[tokio::test]
    async fn non_admins_cannot_add_a_dm() {
        let bot = test_bot().await;
        let interaction = command("add_dm", user_option(50), 2, NONE);

        let text = content(add_dm_inner(&bot, &interaction).await);

        assert_eq!(text, NOT_ADMIN);
        assert!(load(bot.get_db()).await.is_err());
    }

    #[tokio::test]
    async fn configured_admins_can_manage_dms() {
        let bot = test_bot().await;

        add_admin_inner(&bot, &command("add_admin", user_option(2), 1, ADMIN)).await;
        let text = content(add_dm_inner(&bot, &command("add_dm", user_option(50), 2, NONE)).await);

        assert_eq!(text, "Successfully made <@50> a DM");
    }

    #[tokio::test]
    async fn dms_can_be_removed() {
        let bot = test_bot().await;
        let add = command("add_dm", user_option(50), 1, ADMIN);
        let remove = command("remove_dm", user_option(50), 1, ADMIN);

        add_dm_inner(&bot, &add).await;
        let text = content(remove_dm_inner(&bot, &remove).await);

        assert_eq!(text, "Removed <@50> as a DM");
        assert!(load(bot.get_db()).await.unwrap().get_dms().is_empty());
    }

    #[tokio::test]
    async fn removing_someone_who_is_not_a_dm_is_reported() {
        let bot = test_bot().await;
        let remove = command("remove_dm", user_option(50), 1, ADMIN);

        assert_eq!(content(remove_dm_inner(&bot, &remove).await), "<@50> is not a DM");
    }

    #[tokio::test]
    async fn only_admins_can_remove_dms() {
        let bot = test_bot().await;

        add_dm_inner(&bot, &command("add_dm", user_option(50), 1, ADMIN)).await;
        let text = content(remove_dm_inner(&bot, &command("remove_dm", user_option(50), 2, NONE)).await);

        assert_eq!(text, NOT_ADMIN);
        assert_eq!(load(bot.get_db()).await.unwrap().get_dms().len(), 1);
    }

    #[tokio::test]
    async fn dm_removal_keeps_the_other_dms() {
        let bot = test_bot().await;

        for id in [50, 51, 52] {
            add_dm_inner(&bot, &command("add_dm", user_option(id), 1, ADMIN)).await;
        }
        remove_dm_inner(&bot, &command("remove_dm", user_option(51), 1, ADMIN)).await;

        let dms = load(bot.get_db()).await.unwrap().get_dms();

        assert_eq!(dms, vec![UserId::new(50), UserId::new(52)]);
    }

    #[tokio::test]
    async fn admins_can_be_added_and_removed() {
        let bot = test_bot().await;
        let add = command("add_admin", user_option(60), 1, ADMIN);
        let remove = command("remove_admin", user_option(60), 1, ADMIN);

        assert_eq!(content(add_admin_inner(&bot, &add).await), "Successfully made <@60> an Admin");
        assert_eq!(content(add_admin_inner(&bot, &add).await), "User is already an Admin");
        assert_eq!(load(bot.get_db()).await.unwrap().get_admins(), vec![UserId::new(60)]);

        assert_eq!(content(remove_admin_inner(&bot, &remove).await), "Removed <@60> as an Admin");
        assert_eq!(content(remove_admin_inner(&bot, &remove).await), "<@60> is not an Admin");
        assert!(load(bot.get_db()).await.unwrap().get_admins().is_empty());
    }

    #[tokio::test]
    async fn dm_and_admin_lists_are_independent() {
        let bot = test_bot().await;

        add_dm_inner(&bot, &command("add_dm", user_option(50), 1, ADMIN)).await;
        add_admin_inner(&bot, &command("add_admin", user_option(60), 1, ADMIN)).await;

        let config = load(bot.get_db()).await.unwrap();

        assert_eq!(config.get_dms(), vec![UserId::new(50)]);
        assert_eq!(config.get_admins(), vec![UserId::new(60)]);
    }

    #[tokio::test]
    async fn missing_user_option_is_reported_instead_of_panicking() {
        let bot = test_bot().await;

        let add = content(add_dm_inner(&bot, &command("add_dm", json!([]), 1, ADMIN)).await);
        let remove = content(remove_admin_inner(&bot, &command("remove_admin", json!([]), 1, ADMIN)).await);

        assert_eq!(add, "Missing the user option");
        assert_eq!(remove, "Missing the user option");
    }

    #[tokio::test]
    async fn world_id_is_saved_for_admins() {
        let bot = test_bot().await;
        let options = json!([{ "name": "id", "type": 3, "value": "world-1" }]);

        let text = content(set_world_id_inner(&bot, &command("set_world_id", options, 1, ADMIN)).await);

        assert_eq!(text, "Successfully set World Id to: world-1");
        assert_eq!(load(bot.get_db()).await.unwrap().get_world_id(), "world-1");
    }

    #[tokio::test]
    async fn world_id_keeps_the_rest_of_the_config() {
        let bot = test_bot().await;
        let options = json!([{ "name": "id", "type": 3, "value": "world-1" }]);

        add_dm_inner(&bot, &command("add_dm", user_option(50), 1, ADMIN)).await;
        set_world_id_inner(&bot, &command("set_world_id", options, 1, ADMIN)).await;

        let config = load(bot.get_db()).await.unwrap();

        assert_eq!(config.get_dms(), vec![UserId::new(50)]);
        assert_eq!(config.get_world_id(), "world-1");
    }

    #[tokio::test]
    async fn world_id_is_rejected_for_non_admins() {
        let bot = test_bot().await;
        let options = json!([{ "name": "id", "type": 3, "value": "world-1" }]);

        let text = content(set_world_id_inner(&bot, &command("set_world_id", options, 2, NONE)).await);

        assert_eq!(text, NOT_ADMIN);
        assert!(load(bot.get_db()).await.is_err());
    }

    #[tokio::test]
    async fn world_id_requires_the_id_option() {
        let bot = test_bot().await;

        let text = content(set_world_id_inner(&bot, &command("set_world_id", json!([]), 1, ADMIN)).await);

        assert_eq!(text, "Missing the id option");
    }
}
