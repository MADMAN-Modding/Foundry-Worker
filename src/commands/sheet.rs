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
    util::{
        database::{load_config, load_foundry_user},
        logging::log_result,
        wrappers::quick_create_interaction_response,
    },
};

pub const SHEET_PREFIX: &str = "sheet:";

type PageBuilder = fn(&Value) -> CreateEmbed;

// Each page's name paired with the embed that renders it
const PAGES: [(&str, PageBuilder); 6] = [
    ("Overview", overview_embed),
    ("Abilities", abilities_embed),
    ("Skills", skills_embed),
    ("Features", features_embed),
    ("Spells", spells_embed),
    ("Inventory", inventory_embed),
];

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
    let page = parse_page(&component.data.custom_id);

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

/// The page a `sheet:<page>` custom id points at, clamped to the pages that exist
fn parse_page(custom_id: &str) -> usize {
    custom_id
        .strip_prefix(SHEET_PREFIX)
        .and_then(|page| page.parse::<usize>().ok())
        .unwrap_or(0)
        .min(PAGES.len() - 1)
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

    let config = load_config(bot.get_db(), guid).await;

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

    let (name, build) = PAGES[page];
    let embed = build(&actor).footer(CreateEmbedFooter::new(format!(
        "{} - {}/{}",
        name,
        page + 1,
        PAGES.len()
    )));

    Ok((embed, nav_buttons(page)))
}

fn nav_buttons(page: usize) -> CreateActionRow {
    let last = PAGES.len() - 1;

    CreateActionRow::Buttons(vec![
        CreateButton::new(format!("{}{}", SHEET_PREFIX, page.saturating_sub(1)))
            .label("◀")
            .style(ButtonStyle::Secondary)
            .disabled(page == 0),
        CreateButton::new(format!("{}label", SHEET_PREFIX))
            .label(PAGES[page].0)
            .style(ButtonStyle::Primary)
            .disabled(true),
        CreateButton::new(format!("{}{}", SHEET_PREFIX, (page + 1).min(last)))
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

fn actor_name(actor: &Value) -> &str {
    actor["name"].as_str().unwrap_or("Unknown")
}

/// An embed titled "<actor name> - <page>" in the sheet's color
fn page_embed(actor: &Value, page: &str) -> CreateEmbed {
    CreateEmbed::new()
        .title(format!("{} - {}", actor_name(actor), page))
        .color(Color::BLITZ_BLUE)
}

fn item_names(items: &[&Value]) -> Vec<String> {
    items
        .iter()
        .map(|item| item["name"].as_str().unwrap_or("?").to_string())
        .collect()
}

/// Adds one field per group, skipping groups with no lines.
/// `lines_for` produces the lines for a group key and `separator` joins them.
fn grouped_fields<K, H: Into<String>>(
    mut embed: CreateEmbed,
    groups: impl IntoIterator<Item = (K, H)>,
    separator: &str,
    lines_for: impl Fn(K) -> Vec<String>,
) -> CreateEmbed {
    for (key, heading) in groups {
        let lines = lines_for(key);

        if !lines.is_empty() {
            embed = embed.field(heading, clip(lines.join(separator)), false);
        }
    }

    embed
}

// 0 none, 0.5 half, 1 proficient, 2 expertise
fn proficiency_marker(value: f64) -> &'static str {
    if value >= 2.0 {
        "◉"
    } else if value >= 1.0 {
        "●"
    } else if value > 0.0 {
        "◐"
    } else {
        "○"
    }
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
        .title(actor_name(actor))
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

    let mut embed = page_embed(actor, "Abilities");

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

            let marker = proficiency_marker(value);

            format!("{} `{:+}` {} ({})", marker, bonus, name, ability.to_uppercase())
        })
        .collect();

    page_embed(actor, "Skills").description(lines.join("\n"))
}

fn features_embed(actor: &Value) -> CreateEmbed {
    let feats = items(actor, "feat");

    grouped_fields(
        page_embed(actor, "Features"),
        [("class", "Class Features"), ("race", "Species Traits"), ("feat", "Feats")],
        "\n",
        |group| {
            let matching: Vec<&Value> = feats
                .iter()
                .copied()
                .filter(|item| item.pointer("/system/type/value").and_then(Value::as_str) == Some(group))
                .collect();

            item_names(&matching).iter().map(|name| format!("• {}", name)).collect()
        },
    )
}

fn spells_embed(actor: &Value) -> CreateEmbed {
    let spells = items(actor, "spell");

    let levels = (0..=9).map(|level| {
        let heading = if level == 0 { "Cantrips".to_string() } else { format!("Level {}", level) };

        (level, heading)
    });

    let embed = grouped_fields(page_embed(actor, "Spells"), levels, ", ", |level| {
        let matching: Vec<&Value> = spells
            .iter()
            .copied()
            .filter(|spell| int_at(spell, "/system/level").unwrap_or(0) == level)
            .filter(|spell| spell["name"].as_str().is_some())
            .collect();

        item_names(&matching)
    });

    if spells.is_empty() {
        embed.description("No spells")
    } else {
        embed
    }
}

fn inventory_embed(actor: &Value) -> CreateEmbed {
    let coin = |key: &str| int_at(actor, &format!("/system/currency/{}", key)).unwrap_or(0);

    let embed = page_embed(actor, "Inventory").description(format!(
        "{} pp • {} gp • {} ep • {} sp • {} cp",
        coin("pp"),
        coin("gp"),
        coin("ep"),
        coin("sp"),
        coin("cp"),
    ));

    grouped_fields(
        embed,
        [
            ("weapon", "Weapons"),
            ("equipment", "Equipment"),
            ("consumable", "Consumables"),
            ("tool", "Tools"),
            ("container", "Containers"),
            ("loot", "Loot"),
        ],
        "\n",
        |kind| {
            items(actor, kind)
                .iter()
                .map(|item| {
                    let quantity = int_at(item, "/system/quantity").unwrap_or(1);
                    format!("• {} x{}", item["name"].as_str().unwrap_or("?"), quantity)
                })
                .collect()
        },
    )
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn actor() -> Value {
        json!({
            "name": "Ayla",
            "system": {
                "abilities": {
                    "str": { "value": 8, "proficient": 0 },
                    "dex": { "value": 16, "proficient": 1 },
                    "con": { "value": 14, "proficient": 1 },
                    "int": { "value": 12 },
                    "wis": { "value": 10 },
                    "cha": { "value": 9 },
                },
                "attributes": { "hp": { "value": 20, "temp": 5 }, "ac": { "calc": "default" } },
                "details": { "race": "race1", "background": "bg1" },
                "skills": {
                    "acr": { "ability": "dex", "value": 1 },
                    "ste": { "ability": "dex", "value": 2 },
                    "arc": { "ability": "int", "value": 0.5 },
                },
                "currency": { "gp": 15, "sp": 3 },
            },
            "items": [
                {
                    "_id": "c1", "type": "class", "name": "Fighter",
                    "system": {
                        "levels": 3,
                        "hd": { "denomination": "d10" },
                        "advancement": { "a": { "type": "HitPoints", "value": { "1": "max", "2": "avg", "3": 7 } } },
                    },
                },
                {
                    "_id": "c2", "type": "class", "name": "Wizard",
                    "system": {
                        "levels": 2,
                        "hd": { "denomination": "d6" },
                        "advancement": { "a": { "type": "HitPoints", "value": { "1": "max", "2": "avg" } } },
                    },
                },
                { "_id": "race1", "type": "race", "name": "Elf" },
                { "_id": "bg1", "type": "background", "name": "Sage" },
                { "type": "feat", "name": "Second Wind", "system": { "type": { "value": "class" } } },
                { "type": "feat", "name": "Darkvision", "system": { "type": { "value": "race" } } },
                { "type": "feat", "name": "Alert", "system": { "type": { "value": "feat" } } },
                { "type": "spell", "name": "Fire Bolt", "system": { "level": 0 } },
                { "type": "spell", "name": "Shield", "system": { "level": 1 } },
                { "type": "spell", "name": "Magic Missile", "system": { "level": 1 } },
                { "type": "spell", "system": { "level": 1 } },
                { "type": "weapon", "name": "Longsword", "system": { "quantity": 1 } },
                { "type": "consumable", "name": "Potion", "system": { "quantity": 3 } },
                { "type": "loot", "name": "Gem" },
            ],
        })
    }

    fn embed(builder: PageBuilder) -> Value {
        serde_json::to_value(builder(&actor())).unwrap()
    }

    fn field<'a>(embed: &'a Value, name: &str) -> Option<&'a str> {
        embed["fields"]
            .as_array()
            .into_iter()
            .flatten()
            .find(|field| field["name"] == name)
            .and_then(|field| field["value"].as_str())
    }

    #[test]
    fn modifier_rounds_down_around_ten() {
        let cases = [(1, -5), (7, -2), (8, -1), (9, -1), (10, 0), (11, 0), (12, 1), (20, 5)];

        for (score, expected) in cases {
            assert_eq!(modifier(score), expected, "score {score}");
        }
    }

    #[test]
    fn proficiency_scales_with_level() {
        let cases = [(-3, 2), (0, 2), (1, 2), (4, 2), (5, 3), (8, 3), (9, 4), (13, 5), (17, 6), (20, 6)];

        for (level, expected) in cases {
            assert_eq!(proficiency(level), expected, "level {level}");
        }
    }

    #[test]
    fn hp_bar_fills_proportionally() {
        assert_eq!(hp_bar(5, 10), "▰▰▰▰▰▱▱▱▱▱");
        assert_eq!(hp_bar(10, 10), "▰▰▰▰▰▰▰▰▰▰");
        assert_eq!(hp_bar(1, 3), "▰▰▰▱▱▱▱▱▱▱");
    }

    #[test]
    fn hp_bar_clamps_and_handles_zero_max() {
        assert_eq!(hp_bar(0, 10), "▱▱▱▱▱▱▱▱▱▱");
        assert_eq!(hp_bar(-4, 10), "▱▱▱▱▱▱▱▱▱▱");
        assert_eq!(hp_bar(99, 10), "▰▰▰▰▰▰▰▰▰▰");
        assert_eq!(hp_bar(5, 0), "▱▱▱▱▱▱▱▱▱▱");
    }

    #[test]
    fn clip_leaves_short_text_alone() {
        assert_eq!(clip("short".to_string()), "short");
        assert_eq!(clip("a".repeat(1000)), "a".repeat(1000));
    }

    #[test]
    fn clip_truncates_long_text_with_an_ellipsis() {
        let clipped = clip("b".repeat(1001));

        assert_eq!(clipped.chars().count(), 1000);
        assert!(clipped.ends_with("bb..."));
    }

    #[test]
    fn clip_counts_characters_not_bytes() {
        let text = "é".repeat(1000);

        assert_eq!(clip(text.clone()), text);
    }

    #[test]
    fn proficiency_markers_cover_every_tier() {
        assert_eq!(proficiency_marker(0.0), "○");
        assert_eq!(proficiency_marker(0.5), "◐");
        assert_eq!(proficiency_marker(1.0), "●");
        assert_eq!(proficiency_marker(2.0), "◉");
    }

    #[test]
    fn parse_page_reads_the_page_number() {
        assert_eq!(parse_page("sheet:0"), 0);
        assert_eq!(parse_page("sheet:3"), 3);
    }

    #[test]
    fn parse_page_clamps_and_defaults() {
        assert_eq!(parse_page("sheet:99"), PAGES.len() - 1);
        assert_eq!(parse_page("sheet:label"), 0);
        assert_eq!(parse_page("sheet:-1"), 0);
        assert_eq!(parse_page("other:2"), 0);
    }

    #[test]
    fn items_filters_by_type() {
        let actor = actor();

        assert_eq!(items(&actor, "class").len(), 2);
        assert_eq!(items(&actor, "spell").len(), 4);
        assert!(items(&actor, "vehicle").is_empty());
        assert!(items(&json!({}), "class").is_empty());
    }

    #[test]
    fn item_name_resolves_ids_and_falls_back() {
        let actor = actor();

        assert_eq!(item_name(&actor, "/system/details/race"), "Elf");
        assert_eq!(item_name(&actor, "/system/details/background"), "Sage");
        assert_eq!(item_name(&actor, "/system/details/missing"), "-");
        assert_eq!(item_name(&json!({}), "/system/details/race"), "-");
    }

    #[test]
    fn ability_scores_default_to_ten() {
        let actor = actor();

        assert_eq!(ability_score(&actor, "dex"), 16);
        assert_eq!(ability_score(&actor, "nope"), 10);
        assert_eq!(ability_score(&json!({}), "str"), 10);
    }

    #[test]
    fn levels_add_up_across_classes() {
        let actor = actor();

        assert_eq!(total_level(&actor), 5);
        assert_eq!(total_level(&json!({})), 0);
        assert_eq!(class_levels(&json!({})), 0);
    }

    #[test]
    fn max_hp_combines_hit_dice_advancement_and_constitution() {
        // Fighter: 10 + (10 / 2 + 1) + 7 = 23, Wizard: 6 + (6 / 2 + 1) = 10, plus +2 CON for 5 levels
        assert_eq!(max_hp(&actor()), 23 + 10 + 2 * 5);
    }

    #[test]
    fn max_hp_ignores_missing_advancement_values() {
        let actor = json!({
            "items": [{ "type": "class", "system": { "levels": 2, "hd": { "denomination": "d8" } } }],
            "system": { "abilities": { "con": { "value": 12 } } },
        });

        assert_eq!(max_hp(&actor), 2);
        assert_eq!(max_hp(&json!({})), 0);
    }

    #[test]
    fn overview_summarizes_the_character() {
        let embed = embed(overview_embed);

        assert_eq!(embed["title"], "Ayla");
        assert_eq!(embed["description"], "Level 5 Fighter 3 / Wizard 2\nElf - Sage");
        assert_eq!(field(&embed, "HP"), Some("▰▰▰▰▱▱▱▱▱▱ 20/43 (+5 temp)"));
        assert_eq!(field(&embed, "AC"), Some("13"));
        assert_eq!(field(&embed, "Initiative"), Some("+3"));
        assert_eq!(field(&embed, "Proficiency"), Some("+3"));
    }

    #[test]
    fn overview_omits_temp_hp_when_there_is_none() {
        let mut actor = actor();
        actor["system"]["attributes"]["hp"]["temp"] = json!(0);

        let embed = serde_json::to_value(overview_embed(&actor)).unwrap();

        assert_eq!(field(&embed, "HP"), Some("▰▰▰▰▱▱▱▱▱▱ 20/43"));
    }

    #[test]
    fn overview_uses_flat_armor_class_when_configured() {
        let mut actor = actor();
        actor["system"]["attributes"]["ac"] = json!({ "calc": "flat", "flat": 18 });

        let embed = serde_json::to_value(overview_embed(&actor)).unwrap();

        assert_eq!(field(&embed, "AC"), Some("18"));
    }

    #[test]
    fn overview_survives_an_empty_actor() {
        let embed = serde_json::to_value(overview_embed(&json!({}))).unwrap();

        assert_eq!(embed["title"], "Unknown");
        assert_eq!(field(&embed, "AC"), Some("10"));
    }

    #[test]
    fn abilities_show_scores_modifiers_and_saves() {
        let embed = embed(abilities_embed);

        assert_eq!(embed["title"], "Ayla - Abilities");
        assert_eq!(embed["fields"].as_array().unwrap().len(), ABILITIES.len());
        assert_eq!(field(&embed, "Strength"), Some("8 (-1)\nSave -1"));
        assert_eq!(field(&embed, "Dexterity"), Some("16 (+3)\nSave +6 ●"));
        assert_eq!(field(&embed, "Constitution"), Some("14 (+2)\nSave +5 ●"));
        assert_eq!(field(&embed, "Charisma"), Some("9 (-1)\nSave -1"));
    }

    #[test]
    fn skills_list_every_skill_with_bonuses() {
        let embed = embed(skills_embed);
        let description = embed["description"].as_str().unwrap();

        assert_eq!(description.lines().count(), SKILLS.len());
        assert!(description.contains("● `+6` Acrobatics (DEX)"));
        assert!(description.contains("◉ `+9` Stealth (DEX)"));
        assert!(description.contains("◐ `+2` Arcana (INT)"));
        assert!(description.contains("○ `+1` History (INT)"));
    }

    #[test]
    fn features_are_grouped_by_origin() {
        let embed = embed(features_embed);

        assert_eq!(field(&embed, "Class Features"), Some("• Second Wind"));
        assert_eq!(field(&embed, "Species Traits"), Some("• Darkvision"));
        assert_eq!(field(&embed, "Feats"), Some("• Alert"));
    }

    #[test]
    fn features_skip_empty_groups() {
        let embed = serde_json::to_value(features_embed(&json!({ "items": [] }))).unwrap();

        assert!(embed.get("fields").is_none_or(|fields| fields.as_array().unwrap().is_empty()));
    }

    #[test]
    fn spells_are_grouped_by_level() {
        let embed = embed(spells_embed);

        assert_eq!(field(&embed, "Cantrips"), Some("Fire Bolt"));
        assert_eq!(field(&embed, "Level 1"), Some("Shield, Magic Missile"));
        assert_eq!(field(&embed, "Level 2"), None);
        assert!(embed.get("description").is_none_or(Value::is_null));
    }

    #[test]
    fn spells_without_any_say_so() {
        let embed = serde_json::to_value(spells_embed(&json!({ "name": "Ayla" }))).unwrap();

        assert_eq!(embed["description"], "No spells");
    }

    #[test]
    fn inventory_lists_coins_and_item_groups() {
        let embed = embed(inventory_embed);

        assert_eq!(embed["description"], "0 pp • 15 gp • 0 ep • 3 sp • 0 cp");
        assert_eq!(field(&embed, "Weapons"), Some("• Longsword x1"));
        assert_eq!(field(&embed, "Consumables"), Some("• Potion x3"));
        assert_eq!(field(&embed, "Loot"), Some("• Gem x1"));
        assert_eq!(field(&embed, "Tools"), None);
    }

    #[test]
    fn long_item_lists_are_clipped() {
        let many: Vec<Value> = (0..200)
            .map(|i| json!({ "type": "loot", "name": format!("Trinket number {i}") }))
            .collect();

        let embed = serde_json::to_value(inventory_embed(&json!({ "items": many }))).unwrap();
        let loot = field(&embed, "Loot").unwrap();

        assert_eq!(loot.chars().count(), 1000);
        assert!(loot.ends_with("..."));
    }

    #[test]
    fn grouped_fields_supports_any_key_type() {
        let embed = grouped_fields(
            CreateEmbed::new(),
            [(1, "Odd".to_string()), (2, "Even".to_string()), (3, "None".to_string())],
            "|",
            |n| match n {
                1 => vec!["a".to_string(), "b".to_string()],
                2 => vec!["c".to_string()],
                _ => Vec::new(),
            },
        );
        let embed = serde_json::to_value(embed).unwrap();

        assert_eq!(field(&embed, "Odd"), Some("a|b"));
        assert_eq!(field(&embed, "Even"), Some("c"));
        assert_eq!(field(&embed, "None"), None);
    }

    #[test]
    fn every_page_builds_an_embed_for_an_empty_actor() {
        for (name, build) in PAGES {
            let embed = serde_json::to_value(build(&json!({}))).unwrap();

            assert!(embed["title"].is_string(), "{name} has no title");
        }
    }

    fn buttons(page: usize) -> Vec<Value> {
        let row = serde_json::to_value(nav_buttons(page)).unwrap();

        row["components"].as_array().unwrap().clone()
    }

    #[test]
    fn first_page_disables_back() {
        let buttons = buttons(0);

        assert_eq!(buttons[0]["disabled"], true);
        assert_eq!(buttons[0]["custom_id"], "sheet:0");
        assert_eq!(buttons[1]["label"], "Overview");
        assert_eq!(buttons[2]["disabled"], false);
        assert_eq!(buttons[2]["custom_id"], "sheet:1");
    }

    #[test]
    fn middle_pages_link_both_ways() {
        let buttons = buttons(2);

        assert_eq!(buttons[0]["custom_id"], "sheet:1");
        assert_eq!(buttons[0]["disabled"], false);
        assert_eq!(buttons[2]["custom_id"], "sheet:3");
        assert_eq!(buttons[2]["disabled"], false);
    }

    #[test]
    fn last_page_disables_forward() {
        let last = PAGES.len() - 1;
        let buttons = buttons(last);

        assert_eq!(buttons[2]["disabled"], true);
        assert_eq!(buttons[2]["custom_id"], format!("sheet:{last}"));
        assert_eq!(buttons[1]["label"], "Inventory");
    }

    #[test]
    fn nav_ids_always_parse_back_to_a_page() {
        for page in 0..PAGES.len() {
            for button in buttons(page) {
                let id = button["custom_id"].as_str().unwrap();

                assert!(id.starts_with(SHEET_PREFIX));
                assert!(parse_page(id) < PAGES.len());
            }
        }
    }

    #[test]
    fn sheet_command_is_registered() {
        let command = serde_json::to_value(sheet_command()).unwrap();

        assert_eq!(command["name"], "sheet");
    }

    #[tokio::test]
    async fn sheet_requires_a_binding() {
        let bot = crate::util::test_support::test_bot().await;

        let res = build_page(&bot, GuildId::new(1), UserId::new(2), 0).await;

        assert_eq!(
            res.err().as_deref(),
            Some("No character is linked to you, ask a DM to bind you")
        );
    }
}