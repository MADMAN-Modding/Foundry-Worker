use serenity::{
    all::{
        CommandInteraction, CommandOptionType, ComponentInteraction, ComponentInteractionDataKind,
        CreateActionRow, CreateCommand, CreateCommandOption, CreateInteractionResponse,
        CreateSelectMenu, CreateSelectMenuKind, CreateSelectMenuOption, Member, prelude::Context,
    },
    builder::CreateInteractionResponseMessage,
    model::{
        Permissions,
        id::{GuildId, UserId},
    },
};

use crate::{
    api::{foundry_user::FoundryUser, pull_character_data},
    bot::bot::Bot,
    database::config::Config,
    util::{
        database::{insert_foundry_user, load},
        logging::log_result,
        wrappers::{get_user_arg, quick_create_interaction_response},
    },
};

pub fn bind_foundry_user_command() -> CreateCommand {
    CreateCommand::new("bind_foundry_user")
        .description("Bind a Discord user to their Foundry player and character")
        .add_option(
            CreateCommandOption::new(CommandOptionType::User, "user", "Discord user to bind")
                .required(true),
        )
}

// Loads the guild config the same way the other commands do
pub(crate) async fn load_config(bot: &Bot, guid: GuildId) -> Config {
    let mut config = Config::default();

    let _ = load(bot.get_db())
        .await
        .map(|v| config = v)
        .map_err(|_| config.set_guid(guid));

    config
}

// Same rule as the other commands: Discord admin, bot admin, or DM
fn can_manage(config: &Config, member: &Option<&Member>) -> bool {
    let Some(member) = member else {
        return false;
    };

    let is_admin = member
        .permissions
        .map(|perms| perms.contains(Permissions::ADMINISTRATOR))
        .unwrap_or(false);

    is_admin || config.check_for_admin(member.user.id) || config.check_for_dm(member.user.id)
}

pub async fn bind_foundry_user(
    bot: &Bot,
    _ctx: &Context,
    command: &CommandInteraction,
) -> CreateInteractionResponse {
    let config = load_config(bot, command.guild_id.unwrap()).await;

    if !can_manage(&config, &command.member.as_deref()) {
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

    let target = get_user_arg(command);

    let res = pull_character_data::list_users(&client_id).await;
    log_result(&res, "Foundry user list");

    let Ok(players) = res else {
        return quick_create_interaction_response("Couldn't reach Foundry".to_string(), true);
    };

    // Players without an assigned character have no actor to bind to, so they're skipped.
    // The value carries both ids so the select handler needs no second API call.
    let options: Vec<CreateSelectMenuOption> = players
        .iter()
        .filter_map(|player| {
            let character = player.character.as_ref()?;

            Some(CreateSelectMenuOption::new(
                format!("{} ({})", player.name, character.name),
                format!("{}:{}", player.id, character.id),
            ))
        })
        .take(25)
        .collect();

    if options.is_empty() {
        return quick_create_interaction_response(
            "No Foundry users have a character assigned".to_string(),
            true,
        );
    }

    let menu = CreateSelectMenu::new(
        format!("bind_foundry:{}", target.id),
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

    let Some(discord_id) = component
        .data
        .custom_id
        .strip_prefix("bind_foundry:")
        .and_then(|id| id.parse::<u64>().ok())
    else {
        return quick_create_interaction_response("Invalid selection".to_string(), true);
    };

    let Some((foundry_id, actor_id)) = values.first().and_then(|value| value.split_once(':'))
    else {
        return quick_create_interaction_response("Invalid selection".to_string(), true);
    };

    let config = load_config(bot, component.guild_id.unwrap()).await;

    if !can_manage(&config, &component.member.as_ref()) {
        return quick_create_interaction_response("Only DMs can bind users".to_string(), true);
    }

    let user = FoundryUser::new(
        UserId::new(discord_id),
        foundry_id.to_string(),
        actor_id.to_string()
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
