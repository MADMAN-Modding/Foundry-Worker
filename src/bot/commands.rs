use serenity::{
    all::{
        CommandInteraction, CommandOptionType, CreateCommand, CreateCommandOption,
        CreateInteractionResponse, prelude::Context,
    },
    builder::{CreateEmbed, CreateInteractionResponseMessage, CreateMessage},
    model::{Color, Permissions, application::ResolvedValue, id::UserId},
};

use crate::{
    bot::bot::Bot,
    database::config::Config,
    util::{
        database::{load, set_config},
        logging::log_result,
        wrappers::{get_user_arg, quick_create_interaction_response},
    },
};

pub fn create_commands() -> Vec<CreateCommand> {
    vec![
        set_foundry_status_channel_command(),
        add_dm_command(),
        remove_dm_command(),
        add_admin_command(),
        remove_admin_command(),
    ]
}

fn set_foundry_status_channel_command() -> CreateCommand {
    CreateCommand::new("set_foundry_status_channel")
        .description("Channel for Foundry status data to be sent")
        .add_option(
            CreateCommandOption::new(
                CommandOptionType::Channel,
                "channel",
                "Channel to send Status Updates to",
            )
            .required(true),
        )
}

pub async fn set_foundry_status_channel(
    bot: &Bot,
    ctx: &Context,
    command: &CommandInteraction,
) -> CreateInteractionResponse {
    let guid = command.guild_id.unwrap();

    let channel_id = command
        .data
        .options()
        .iter()
        .find(|option| option.name == "channel")
        .and_then(|option| match option.value {
            ResolvedValue::Channel(id) => Some(id),
            _ => None,
        })
        .unwrap();

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

    let guid = command.guild_id.unwrap();

    let mut config = Config::default();

    let _ = load(bot.get_db())
        .await
        .map(|v| config = v)
        .map_err(|_| guid);

    config.set_foundry_status_channel(channel_id.id);

    set_config(bot.get_db(), config).await.unwrap();

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
    CreateCommand::new("add_dm")
        .description("Add a Dungeon Master to allow them to run DM-level Commands")
        .add_option(
            CreateCommandOption::new(CommandOptionType::User, "user", "User to become a DM")
                .required(true),
        )
}

pub async fn add_dm(
    bot: &Bot,
    _ctx: &Context,
    command: &CommandInteraction,
) -> CreateInteractionResponse {
    let user = get_user_arg(command);

    let guid = command.guild_id.unwrap();

    let mut config = Config::default();

    let _ = load(bot.get_db())
        .await
        .map(|v| config = v)
        .map_err(|_| config.set_guid(guid));

    if !config.check_for_admin(command.user.id) {
        return quick_create_interaction_response(
            "Invalid Permission Level you must be an Administrator".to_string(),
            true,
        );
    }

    if config.get_dms().contains(&user.id) {
        return quick_create_interaction_response("User is already a DM".to_string(), true);
    } else {
        let mut dms = config.get_dms();

        dms.push(user.id);

        config.set_dms(dms);

        let res = set_config(bot.get_db(), config).await;
        log_result(&res, "Set DMs");
    }

    quick_create_interaction_response(
        format!("Successfully made <@{}> a DM", user.id.to_string()),
        true,
    )
}

fn remove_dm_command() -> CreateCommand {
    CreateCommand::new("remove_dm")
        .description("Remove a user from the DM list")
        .add_option(
            CreateCommandOption::new(
                CommandOptionType::User,
                "user",
                "User to remove from DM list",
            )
            .required(true),
        )
}

pub async fn remove_dm(
    bot: &Bot,
    _ctx: &Context,
    command: &CommandInteraction,
) -> CreateInteractionResponse {
    let guid = command.guild_id.unwrap();

    let mut config = Config::default();

    let _ = load(bot.get_db())
        .await
        .map(|v| config = v)
        .map_err(|_| config.set_guid(guid));

    if !config.check_for_admin(command.user.id) {
        return quick_create_interaction_response(
            "Invalid Permission Level you must be an Administrator".to_string(),
            true,
        );
    }

    let target_user = get_user_arg(command);

    if !config.check_for_dm(target_user.id) {
        return quick_create_interaction_response(
            format!("<@{}> is not a DM", target_user.id.get()),
            true,
        );
    }

    let mut dms: Vec<UserId> = Vec::new();

    for user in config.get_dms() {
        if user != target_user.id {
            dms.push(user);
        }
    }

    config.set_dms(dms.clone());

    let res = set_config(bot.get_db(), config).await;
    log_result(&res, "Set DMs");

    quick_create_interaction_response(format!("Removed <@{}> as a DM", target_user.id), true)
}

fn add_admin_command() -> CreateCommand {
    CreateCommand::new("add_admin")
        .description("Add an Admin to allow them to run Admin-level Commands")
        .add_option(
            CreateCommandOption::new(CommandOptionType::User, "user", "User to become an Admin")
                .required(true),
        )
        .default_member_permissions(Permissions::ADMINISTRATOR)
}

pub async fn add_admin(
    bot: &Bot,
    _ctx: &Context,
    command: &CommandInteraction,
) -> CreateInteractionResponse {
    let user = get_user_arg(command);

    let guid = command.guild_id.unwrap();

    let mut config = Config::default();

    let _ = load(bot.get_db())
        .await
        .map(|v| config = v)
        .map_err(|_| config.set_guid(guid));

    if config.get_admins().contains(&user.id) {
        return quick_create_interaction_response("User is already an Admin".to_string(), true);
    } else {
        let mut admins = config.get_admins();

        admins.push(user.id);

        config.set_admins(admins);

        let res = set_config(bot.get_db(), config).await;
        log_result(&res, "Set Admins");
    }

    quick_create_interaction_response(
        format!("Successfully made <@{}> an Admin", user.id.to_string()),
        true,
    )
}

fn remove_admin_command() -> CreateCommand {
    CreateCommand::new("remove_admin")
        .description("Remove a user from the Admin list")
        .add_option(
            CreateCommandOption::new(
                CommandOptionType::User,
                "user",
                "User to remove from Admin list",
            )
            .required(true),
        )
        .default_member_permissions(Permissions::ADMINISTRATOR)
}

pub async fn remove_admin(
    bot: &Bot,
    _ctx: &Context,
    command: &CommandInteraction,
) -> CreateInteractionResponse {
    let guid = command.guild_id.unwrap();

    let mut config = Config::default();

    let _ = load(bot.get_db())
        .await
        .map(|v| config = v)
        .map_err(|_| config.set_guid(guid));

    let target_user = get_user_arg(command);

    if !config.check_for_admin(target_user.id) {
        return quick_create_interaction_response(
            format!("<@{}> is not an Admin", target_user.id.get()),
            true,
        );
    }

    let mut admins: Vec<UserId> = Vec::new();

    for user in config.get_admins() {
        if user != target_user.id {
            admins.push(user);
        }
    }

    config.set_admins(admins.clone());

    let res = set_config(bot.get_db(), config).await;
    log_result(&res, "Set Admins");

    quick_create_interaction_response(format!("Removed <@{}> as an Admin", target_user.id), true)
}
