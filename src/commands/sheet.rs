use serde_json::Value;
use serenity::{
    all::{
        ButtonStyle, CommandInteraction, ComponentInteraction, CreateActionRow, CreateButton,
        CreateCommand, CreateInteractionResponse, prelude::Context,
    },
    builder::{CreateEmbed, CreateEmbedFooter, CreateInteractionResponseMessage},
    model::{
        Color,
        id::{GuildId, UserId},
    },
};

use crate::{
    api::pull_character_data,
    bot::bot::Bot,
    commands::character::load_config,
    util::{
        database::load_foundry_user,
        logging::log_result,
        wrappers::quick_create_interaction_response,
    },
};

const PAGES: [&str; 6] = ["Overview", "Abilities", "Skills", "Features", "Spells", "Inventory"];

const ABILITIES: [(&str, &str); 6] = [
    ("str", "Strength"),
    ("dex", "Dexterity"),
    ("con", "Constitution"),
    ("int", "Intelligence"),
    ("wis", "Wisdom"),
    ("cha", "Charisma"),
];

const SKILLS: [(&str, &str); 18] = [
    ("acr", "Acrobatics"),
    ("ani", "Animal Handling"),
    ("arc", "Arcana"),
    ("ath", "Athletics"),
    ("dec", "Deception"),
    ("his", "History"),
    ("ins", "Insight"),
    ("itm", "Intimidation"),
    ("inv", "Investigation"),
    ("med", "Medicine"),
    ("nat", "Nature"),
    ("prc", "Perception"),
    ("prf", "Performance"),
    ("per", "Persuasion"),
    ("rel", "Religion"),
    ("slt", "Sleight of Hand"),
    ("ste", "Stealth"),
    ("sur", "Survival"),
];

pub fn sheet_command() -> CreateCommand {
    CreateCommand::new("sheet").description("Show your character sheet")
}

pub async fn sheet(
    bot: &Bot,
    _ctx: &Context,
    command: &CommandInteraction,
) -> CreateInteractionResponse {
    match build_page(bot, command.guild_id.unwrap(), command.user.id, 0).await {
        Ok((embed, buttons)) => CreateInteractionResponse::Message(
            CreateInteractionResponseMessage::new()
                .embed(embed)
                .components(vec![buttons])
                .ephemeral(true),
        ),
        Err(text) => quick_create_interaction_response(text, true),
    }
}

// custom_id format: sheet:<page>
pub async fn sheet_page(
    bot: &Bot,
    _ctx: &Context,
    component: &ComponentInteraction,
) -> CreateInteractionResponse {
    let page = component
        .data
        .custom_id
        .strip_prefix("sheet:")
        .and_then(|page| page.parse::<usize>().ok())
        .unwrap_or(0)
        .min(PAGES.len() - 1);

    match build_page(bot, component.guild_id.unwrap(), component.user.id, page).await {
        // UpdateMessage swaps the page in place
        Ok((embed, buttons)) => CreateInteractionResponse::UpdateMessage(
            CreateInteractionResponseMessage::new()
                .embed(embed)
                .components(vec![buttons]),
        ),
        Err(text) => quick_create_interaction_response(text, true),
    }
}

async fn build_page(
    bot: &Bot,
    guid: GuildId,
    user: UserId,
    page: usize,
) -> Result<(CreateEmbed, CreateActionRow), String> {
    let binding = load_foundry_user(bot.get_db(), user).await;
    log_result(&binding.as_ref().map(|_| ()), "Load Foundry user binding");

    let Ok(Some(binding)) = binding else {
        return Err("No character is linked to you, ask a DM to bind you".to_string());
    };

    let config = load_config(bot, guid).await;

    let client_id = match pull_character_data::resolve_client_id(&config.get_world_id()).await {
        Ok(Some(id)) => id,
        Ok(None) => return Err("The Foundry world isn't connected to the relay".to_string()),
        Err(_) => return Err("Couldn't reach Foundry".to_string()),
    };

    let res =
        pull_character_data::get_actor(&client_id, &format!("Actor.{}", binding.get_actor_id())).await;
    log_result(&res, "Foundry actor fetch");

    let Ok(actor) = res else {
        return Err("Couldn't load your character".to_string());
    };

    let embed = match page {
        0 => overview_embed(&actor),
        1 => abilities_embed(&actor),
        2 => skills_embed(&actor),
        3 => features_embed(&actor),
        4 => spells_embed(&actor),
        _ => inventory_embed(&actor),
    }
    .footer(CreateEmbedFooter::new(format!(
        "{} - {}/{}",
        PAGES[page],
        page + 1,
        PAGES.len()
    )));

    Ok((embed, nav_buttons(page)))
}

fn nav_buttons(page: usize) -> CreateActionRow {
    let last = PAGES.len() - 1;

    CreateActionRow::Buttons(vec![
        CreateButton::new(format!("sheet:{}", page.saturating_sub(1)))
            .label("◀")
            .style(ButtonStyle::Secondary)
            .disabled(page == 0),
        CreateButton::new("sheet:label")
            .label(PAGES[page])
            .style(ButtonStyle::Primary)
            .disabled(true),
        CreateButton::new(format!("sheet:{}", (page + 1).min(last)))
            .label("▶")
            .style(ButtonStyle::Secondary)
            .disabled(page == last),
    ])
}

// Helpers over the raw actor data

fn int_at(actor: &Value, path: &str) -> Option<i64> {
    actor.pointer(path).and_then(Value::as_i64)
}

fn items<'a>(actor: &'a Value, kind: &str) -> Vec<&'a Value> {
    actor
        .pointer("/items")
        .and_then(Value::as_array)
        .map(|items| items.iter().filter(|item| item["type"] == kind).collect())
        .unwrap_or_default()
}

fn item_name(actor: &Value, id_path: &str) -> String {
    let id = actor.pointer(id_path).and_then(Value::as_str).unwrap_or("");

    actor
        .pointer("/items")
        .and_then(Value::as_array)
        .and_then(|items| items.iter().find(|item| item["_id"] == id))
        .and_then(|item| item["name"].as_str())
        .unwrap_or("-")
        .to_string()
}

fn ability_score(actor: &Value, key: &str) -> i64 {
    int_at(actor, &format!("/system/abilities/{}/value", key)).unwrap_or(10)
}

fn modifier(score: i64) -> i64 {
    (score - 10).div_euclid(2)
}

fn class_levels(class: &Value) -> i64 {
    class
        .pointer("/system/levels")
        .and_then(Value::as_i64)
        .unwrap_or(0)
}

fn total_level(actor: &Value) -> i64 {
    items(actor, "class").iter().map(|class| class_levels(class)).sum()
}

fn proficiency(level: i64) -> i64 {
    2 + (level.max(1) - 1) / 4
}

// Foundry doesn't send max HP for this data, so it's rebuilt from each class's hit die advancement
fn max_hp(actor: &Value) -> i64 {
    let con = modifier(ability_score(actor, "con"));
    let mut total = 0;

    for class in items(actor, "class") {
        let levels = class_levels(class);

        let faces = class
            .pointer("/system/hd/denomination")
            .and_then(Value::as_str)
            .and_then(|die| die.trim_start_matches('d').parse::<i64>().ok())
            .unwrap_or(0);

        let advancement = class.pointer("/system/advancement").and_then(Value::as_object);

        for level in 1..=levels {
            let value = advancement
                .and_then(|advancement| advancement.values().find(|a| a["type"] == "HitPoints"))
                .and_then(|a| a.pointer(&format!("/value/{}", level)));

            total += match value {
                Some(Value::String(rule)) if rule == "max" => faces,
                Some(Value::String(rule)) if rule == "avg" => faces / 2 + 1,
                Some(Value::Number(number)) => number.as_i64().unwrap_or(0),
                _ => 0,
            };
        }

        total += con * levels;
    }

    total
}

fn hp_bar(current: i64, max: i64) -> String {
    let filled = if max > 0 {
        ((current.clamp(0, max) * 10) / max) as usize
    } else {
        0
    };

    format!("{}{}", "▰".repeat(filled), "▱".repeat(10 - filled))
}

// Embed field values cap at 1024 characters
fn clip(text: String) -> String {
    if text.chars().count() <= 1000 {
        return text;
    }

    let mut clipped: String = text.chars().take(997).collect();
    clipped.push_str("...");
    clipped
}

fn overview_embed(actor: &Value) -> CreateEmbed {
    let level = total_level(actor);
    let hp = int_at(actor, "/system/attributes/hp/value").unwrap_or(0);
    let temp = int_at(actor, "/system/attributes/hp/temp").unwrap_or(0);
    let hp_max = max_hp(actor);
    let dex = modifier(ability_score(actor, "dex"));

    // Armor isn't calculated, so this is the unarmored value unless the sheet uses a flat AC
    let ac = if actor.pointer("/system/attributes/ac/calc").and_then(Value::as_str) == Some("flat") {
        int_at(actor, "/system/attributes/ac/flat").unwrap_or(10)
    } else {
        10 + dex
    };

    let classes = items(actor, "class")
        .iter()
        .map(|class| format!("{} {}", class["name"].as_str().unwrap_or("?"), class_levels(class)))
        .collect::<Vec<_>>()
        .join(" / ");

    let hp_text = if temp > 0 {
        format!("{} {}/{} (+{} temp)", hp_bar(hp, hp_max), hp, hp_max, temp)
    } else {
        format!("{} {}/{}", hp_bar(hp, hp_max), hp, hp_max)
    };

    CreateEmbed::new()
        .title(actor["name"].as_str().unwrap_or("Unknown"))
        .description(format!(
            "Level {} {}\n{} - {}",
            level,
            classes,
            item_name(actor, "/system/details/race"),
            item_name(actor, "/system/details/background"),
        ))
        .color(Color::BLITZ_BLUE)
        .field("HP", hp_text, false)
        .field("AC", ac.to_string(), true)
        .field("Initiative", format!("{:+}", dex), true)
        .field("Proficiency", format!("{:+}", proficiency(level)), true)
}

fn abilities_embed(actor: &Value) -> CreateEmbed {
    let prof = proficiency(total_level(actor));

    let mut embed = CreateEmbed::new()
        .title(format!("{} - Abilities", actor["name"].as_str().unwrap_or("Unknown")))
        .color(Color::BLITZ_BLUE);

    for (key, name) in ABILITIES {
        let score = ability_score(actor, key);
        let modifier = modifier(score);

        let proficient = int_at(actor, &format!("/system/abilities/{}/proficient", key)).unwrap_or(0) == 1;
        let save = modifier + if proficient { prof } else { 0 };

        embed = embed.field(
            name,
            format!(
                "{} ({:+})\nSave {:+}{}",
                score,
                modifier,
                save,
                if proficient { " ●" } else { "" }
            ),
            true,
        );
    }

    embed
}

fn skills_embed(actor: &Value) -> CreateEmbed {
    let prof = proficiency(total_level(actor));

    let lines: Vec<String> = SKILLS
        .iter()
        .map(|(key, name)| {
            let ability = actor
                .pointer(&format!("/system/skills/{}/ability", key))
                .and_then(Value::as_str)
                .unwrap_or("int");

            // 0 none, 0.5 half, 1 proficient, 2 expertise
            let value = actor
                .pointer(&format!("/system/skills/{}/value", key))
                .and_then(Value::as_f64)
                .unwrap_or(0.0);

            let bonus = modifier(ability_score(actor, ability)) + (prof as f64 * value).floor() as i64;

            let marker = if value >= 2.0 {
                "◉"
            } else if value >= 1.0 {
                "●"
            } else if value > 0.0 {
                "◐"
            } else {
                "○"
            };

            format!("{} `{:+}` {} ({})", marker, bonus, name, ability.to_uppercase())
        })
        .collect();

    CreateEmbed::new()
        .title(format!("{} - Skills", actor["name"].as_str().unwrap_or("Unknown")))
        .description(lines.join("\n"))
        .color(Color::BLITZ_BLUE)
}

fn features_embed(actor: &Value) -> CreateEmbed {
    let mut embed = CreateEmbed::new()
        .title(format!("{} - Features", actor["name"].as_str().unwrap_or("Unknown")))
        .color(Color::BLITZ_BLUE);

    for (group, heading) in [("class", "Class Features"), ("race", "Species Traits"), ("feat", "Feats")] {
        let names: Vec<String> = items(actor, "feat")
            .iter()
            .filter(|item| item.pointer("/system/type/value").and_then(Value::as_str) == Some(group))
            .map(|item| format!("• {}", item["name"].as_str().unwrap_or("?")))
            .collect();

        if !names.is_empty() {
            embed = embed.field(heading, clip(names.join("\n")), false);
        }
    }

    embed
}

fn spells_embed(actor: &Value) -> CreateEmbed {
    let mut embed = CreateEmbed::new()
        .title(format!("{} - Spells", actor["name"].as_str().unwrap_or("Unknown")))
        .color(Color::BLITZ_BLUE);

    let spells = items(actor, "spell");

    for level in 0..=9 {
        let names: Vec<&str> = spells
            .iter()
            .filter(|spell| int_at(spell, "/system/level").unwrap_or(0) == level)
            .filter_map(|spell| spell["name"].as_str())
            .collect();

        if !names.is_empty() {
            let heading = if level == 0 { "Cantrips".to_string() } else { format!("Level {}", level) };
            embed = embed.field(heading, clip(names.join(", ")), false);
        }
    }

    if spells.is_empty() {
        embed = embed.description("No spells");
    }

    embed
}

fn inventory_embed(actor: &Value) -> CreateEmbed {
    let coin = |key: &str| int_at(actor, &format!("/system/currency/{}", key)).unwrap_or(0);

    let mut embed = CreateEmbed::new()
        .title(format!("{} - Inventory", actor["name"].as_str().unwrap_or("Unknown")))
        .description(format!(
            "{} pp • {} gp • {} ep • {} sp • {} cp",
            coin("pp"),
            coin("gp"),
            coin("ep"),
            coin("sp"),
            coin("cp"),
        ))
        .color(Color::BLITZ_BLUE);

    for (kind, heading) in [
        ("weapon", "Weapons"),
        ("equipment", "Equipment"),
        ("consumable", "Consumables"),
        ("tool", "Tools"),
        ("container", "Containers"),
        ("loot", "Loot"),
    ] {
        let lines: Vec<String> = items(actor, kind)
            .iter()
            .map(|item| {
                let quantity = int_at(item, "/system/quantity").unwrap_or(1);
                format!("• {} x{}", item["name"].as_str().unwrap_or("?"), quantity)
            })
            .collect();

        if !lines.is_empty() {
            embed = embed.field(heading, clip(lines.join("\n")), false);
        }
    }

    embed
}