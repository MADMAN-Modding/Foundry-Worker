use serenity::{
    all::{
        CommandInteraction, CommandOptionType, CreateCommand, CreateCommandOption,
        CreateInteractionResponse, prelude::Context,
    }, builder::{CreateEmbed, CreateInteractionResponseMessage, CreateMessage}, model::{Color, application::ResolvedValue},
};

use crate::{bot::bot::Bot, database::config::set_value, util::logging::log_result};

pub fn create_commands() -> Vec<CreateCommand> {
    vec![set_foundry_status_channel_command()]
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
            CreateInteractionResponseMessage::new().content("That's not a valid text channel."),
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
            )),
        );
    }

    set_value(
        bot.get_db(),
        guid,
        channel_id.id.get() as i64,
        "foundry_status_channel",
    )
    .await
    .unwrap();

    let embed = CreateEmbed::new().title("This is now the FoundryVTT Status Channel").color(Color::BLITZ_BLUE);
    let builder = CreateMessage::new().embed(embed).tts(true);

    let res = channel_id.id.send_message(&ctx.http, builder).await;

    log_result(
        &res,
        &format!("Foundry VTT Status Channel set to: {}", channel_id.id),
    );

    CreateInteractionResponse::Message(
        CreateInteractionResponseMessage::new()
            .content(format!("Channel Updated to <#{}>", channel_id.id)),
    )
}
