use std::path::Path;

use regex::Regex;
use serde_json::Value as Json;

use crate::colormaps::Colormap;
use crate::files::{self, Entry};
use crate::modifiers::common;
use crate::set::Modifiers;
use crate::target::Targets;
use crate::tint::{ColorSource, Tint};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Rarity {
    Common,
    Uncommon,
    Rare,
    Epic,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ArmPose {
    Item,
    Block,
    BowAndArrow,
    CrossbowHold,
    Spyglass,
    TootHorn,
    Spear,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Tooltip {
    pub text: Option<String>,
    pub component: Option<Json>,
    pub color: Option<Json>,
    pub bold: Option<bool>,
    pub italic: Option<bool>,
    pub position: Option<i32>,
}

#[derive(Debug)]
pub struct ItemModifier {
    pub targets: Targets,
    pub priority: i32,
    pub tint: Option<Tint>,
    pub bar: Option<Tint>,
    pub rarity: Option<Rarity>,
    pub tooltips: Vec<Tooltip>,
    pub removed_tooltips: Vec<Regex>,
    pub arm_pose: Option<ArmPose>,
    pub bob_as_entity: Option<bool>,
    pub spread_as_entity: Option<bool>,
    pub armor_texture: Option<String>,
    pub scope_overlay: Option<String>,
    pub unread: Vec<String>,
}

const UNREAD: [&str; 3] = ["custom_models", "worn_model", "armor_tint"];

fn flag(json: &Json, key: &str) -> Result<Option<bool>, String> {
    match json.get(key) {
        None => Ok(None),
        Some(Json::Bool(b)) => Ok(Some(*b)),
        Some(_) => Err(format!("{key} is true or false")),
    }
}

fn text(json: &Json, key: &str) -> Result<Option<String>, String> {
    match json.get(key) {
        None => Ok(None),
        Some(Json::String(s)) => Ok(Some(s.clone())),
        Some(_) => Err(format!("{key} is a string")),
    }
}

fn tooltip(json: &Json) -> Result<Tooltip, String> {
    if !json.is_object() {
        return Err("a tooltip is an object".to_owned());
    }
    let tip = Tooltip {
        text: text(json, "text")?,
        component: json.get("component").cloned(),
        color: json.get("color").cloned(),
        bold: flag(json, "bold")?,
        italic: flag(json, "italic")?,
        position: match json.get("position") {
            None => None,
            Some(v) => Some(
                v.as_i64()
                    .and_then(|p| i32::try_from(p).ok())
                    .ok_or("position is a whole number")?,
            ),
        },
    };
    if tip.text.is_none() && tip.component.is_none() {
        return Err("a tooltip needs text or a component".to_owned());
    }
    Ok(tip)
}

impl ItemModifier {
    fn bare(targets: Targets) -> Self {
        Self {
            targets,
            priority: 0,
            tint: None,
            bar: None,
            rarity: None,
            tooltips: Vec::new(),
            removed_tooltips: Vec::new(),
            arm_pose: None,
            bob_as_entity: None,
            spread_as_entity: None,
            armor_texture: None,
            scope_overlay: None,
            unread: Vec::new(),
        }
    }

    pub(crate) fn parse(entry: &Entry) -> Result<Option<Self>, String> {
        let textures = files::textures(&entry.dir, &entry.stem)?;
        let bar_textures = files::textures(&entry.dir, &format!("{}_bar", entry.stem))?;
        let Some(json) = &entry.json else {
            if textures.is_empty() && bar_textures.is_empty() {
                return Ok(None);
            }
            let mut modifier = Self::bare(Targets::parse(None, entry.id.as_deref())?);
            if !textures.is_empty() {
                modifier.tint = Some(Tint::All(ColorSource::Inline(Colormap::new(
                    None, textures,
                )?)));
            }
            if !bar_textures.is_empty() {
                modifier.unread.push("bar_color".to_owned());
            }
            return Ok(Some(modifier));
        };
        let Some((targets, priority)) = common(json, entry.id.as_deref())? else {
            return Ok(None);
        };
        let mut modifier = Self::bare(targets);
        modifier.priority = priority;
        modifier.tint = json
            .get("colormap")
            .map(|v| Tint::parse(v, &textures))
            .transpose()?;
        let bar_source = if bar_textures.is_empty() {
            &textures
        } else {
            &bar_textures
        };
        modifier.bar = json
            .get("bar_color")
            .map(|v| Tint::parse(v, bar_source))
            .transpose()?;
        modifier.rarity = match text(json, "rarity")?.as_deref() {
            None => None,
            Some("common") => Some(Rarity::Common),
            Some("uncommon") => Some(Rarity::Uncommon),
            Some("rare") => Some(Rarity::Rare),
            Some("epic") => Some(Rarity::Epic),
            Some(other) => {
                return Err(format!(
                    "rarity {other:?} is not common, uncommon, rare or epic"
                ));
            }
        };
        if let Some(list) = json.get("tooltips") {
            let list = list.as_array().ok_or("tooltips is a list")?;
            modifier.tooltips = list.iter().map(tooltip).collect::<Result<_, _>>()?;
        }
        if let Some(list) = json.get("removed_tooltips") {
            let list = list.as_array().ok_or("removed_tooltips is a list")?;
            for pattern in list {
                let pattern = pattern.as_str().ok_or("removed_tooltips holds patterns")?;
                modifier
                    .removed_tooltips
                    .push(Regex::new(pattern).map_err(|e| format!("{pattern:?}: {e}"))?);
            }
        }
        modifier.arm_pose = match text(json, "arm_pose")?.as_deref() {
            None => None,
            Some("item") => Some(ArmPose::Item),
            Some("block") => Some(ArmPose::Block),
            Some("bow_and_arrow") => Some(ArmPose::BowAndArrow),
            Some("crossbow_hold") => Some(ArmPose::CrossbowHold),
            Some("spyglass") => Some(ArmPose::Spyglass),
            Some("toot_horn") => Some(ArmPose::TootHorn),
            Some("spear") => Some(ArmPose::Spear),
            Some(other) => return Err(format!("arm_pose {other:?} is unknown")),
        };
        modifier.bob_as_entity = flag(json, "bob_as_entity")?;
        modifier.spread_as_entity = flag(json, "spread_as_entity")?;
        modifier.armor_texture = text(json, "armor_texture")?;
        modifier.scope_overlay = text(json, "scope_overlay")?;
        for key in UNREAD {
            if json.get(key).is_some() {
                modifier.unread.push(key.to_owned());
            }
        }
        Ok(Some(modifier))
    }

    pub fn read(root: &Path) -> Modifiers<Self> {
        let mut set = Modifiers::default();
        for entry in files::scan(root, "item_modifiers") {
            match Self::parse(&entry) {
                Ok(Some(modifier)) => {
                    if !modifier.unread.is_empty() {
                        log::warn!(
                            "{}: not read yet: {}",
                            entry.label,
                            modifier.unread.join(", ")
                        );
                    }
                    let (targets, priority) = (modifier.targets.clone(), modifier.priority);
                    set.push(targets, priority, modifier);
                }
                Ok(None) => {}
                Err(e) => log::warn!("{}: {e}", entry.label),
            }
        }
        set
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::colormaps::Colormaps;
    use crate::files::testing::*;
    use serde_json::json;

    const DIR: &str = "assets/foo/polytone/item_modifiers";

    fn nothing(_: &str) -> bool {
        false
    }

    #[test]
    fn a_png_alone_is_the_tint_and_a_bar_png_is_reported_unread() {
        let root = scratch("item-png");
        put(&root, &format!("{DIR}/quiver.png"), pixel(1, 2, 3));
        put(&root, &format!("{DIR}/quiver_bar.png"), pixel(4, 5, 6));
        put(&root, &format!("{DIR}/sack_bar.png"), pixel(4, 5, 6));
        let set = ItemModifier::read(&root);
        let quiver = set.resolve("foo:quiver", &nothing).unwrap();
        let tint = quiver.tint.as_ref().unwrap();
        assert_eq!(
            tint.biome_color(0, &Colormaps::default(), 0.5, 0.5),
            Some(0x010203)
        );
        assert_eq!(quiver.unread, ["bar_color"]);
        let sack = set.resolve("foo:sack", &nothing).unwrap();
        assert!(sack.tint.is_none());
        assert_eq!(sack.unread, ["bar_color"]);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_json_reads_its_colours_rarity_pose_flags_and_ids() {
        let root = scratch("item-json");
        put(
            &root,
            &format!("{DIR}/spear.json"),
            r##"{
                "targets": "#foo:spears",
                "priority": 3,
                "colormap": {"0": "#FF0000", "1": "#00FF00"},
                "bar_color": "#0000FF",
                "rarity": "epic",
                "arm_pose": "spear",
                "bob_as_entity": true,
                "spread_as_entity": false,
                "armor_texture": "foo:textures/worn",
                "scope_overlay": "foo:textures/scope",
                "worn_model": {"model": "x"},
                "custom_models": [],
                "armor_tint": 1
            }"##,
        );
        let set = ItemModifier::read(&root);
        let m = set
            .resolve("foo:long_spear", &|t| t == "foo:spears")
            .unwrap();
        let maps = Colormaps::default();
        assert_eq!(m.priority, 3);
        assert_eq!(
            m.tint.as_ref().unwrap().biome_color(1, &maps, 0.0, 0.0),
            Some(0x00FF00)
        );
        assert_eq!(
            m.bar.as_ref().unwrap().biome_color(0, &maps, 0.0, 0.0),
            Some(0x0000FF)
        );
        assert_eq!(m.rarity, Some(Rarity::Epic));
        assert_eq!(m.arm_pose, Some(ArmPose::Spear));
        assert_eq!(m.bob_as_entity, Some(true));
        assert_eq!(m.spread_as_entity, Some(false));
        assert_eq!(m.armor_texture.as_deref(), Some("foo:textures/worn"));
        assert_eq!(m.scope_overlay.as_deref(), Some("foo:textures/scope"));
        assert_eq!(m.unread, ["custom_models", "worn_model", "armor_tint"]);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn tooltips_are_added_by_text_or_component_and_removed_by_pattern() {
        let root = scratch("item-tooltips");
        put(
            &root,
            &format!("{DIR}/bow.json"),
            r##"{
                "tooltips": [
                    {"text": "Sharp", "color": "gold", "bold": true, "italic": false, "position": 1},
                    {"component": {"translate": "x.y"}}
                ],
                "removed_tooltips": ["^item\\.modifiers.*", "enchantment\\..*"]
            }"##,
        );
        let set = ItemModifier::read(&root);
        let m = set.resolve("foo:bow", &nothing).unwrap();
        assert_eq!(
            m.tooltips[0],
            Tooltip {
                text: Some("Sharp".into()),
                component: None,
                color: Some(json!("gold")),
                bold: Some(true),
                italic: Some(false),
                position: Some(1),
            }
        );
        assert_eq!(m.tooltips[1].component, Some(json!({"translate": "x.y"})));
        assert_eq!(m.tooltips[1].position, None);
        assert!(m.removed_tooltips[0].is_match("item.modifiers.mainhand"));
        assert!(m.removed_tooltips[1].is_match("enchantment.minecraft.sharpness"));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_bar_colormap_reads_the_bar_png_and_falls_back_to_the_plain_one() {
        let root = scratch("item-bar");
        put(
            &root,
            &format!("{DIR}/a.json"),
            r#"{"bar_color": {"default_color": 1, "x_axis": "TEMPERATURE"}}"#,
        );
        put(&root, &format!("{DIR}/a.png"), pixel(10, 0, 0));
        put(&root, &format!("{DIR}/a_bar.png"), pixel(0, 20, 0));
        put(
            &root,
            &format!("{DIR}/b.json"),
            r#"{"bar_color": {"default_color": 1, "x_axis": "TEMPERATURE"}}"#,
        );
        put(&root, &format!("{DIR}/b.png"), pixel(0, 0, 30));
        let set = ItemModifier::read(&root);
        let maps = Colormaps::default();
        let bar = |id: &str| {
            set.resolve(id, &nothing)
                .unwrap()
                .bar
                .as_ref()
                .unwrap()
                .biome_color(0, &maps, 0.5, 0.5)
        };
        assert_eq!(bar("foo:a"), Some(0x001400));
        assert_eq!(bar("foo:b"), Some(0x00001E));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn invalid_fields_refuse_only_their_own_file() {
        let root = scratch("item-bad");
        for (name, body) in [
            ("rarity", r#"{"rarity": "mythic"}"#),
            ("pose", r#"{"arm_pose": "wave"}"#),
            ("bob", r#"{"bob_as_entity": "yes"}"#),
            ("tip", r#"{"tooltips": [{"bold": true}]}"#),
            ("tips", r#"{"tooltips": {"text": "x"}}"#),
            ("regex", r#"{"removed_tooltips": ["("]}"#),
            ("removed", r#"{"removed_tooltips": [1]}"#),
            (
                "position",
                r#"{"tooltips": [{"text": "x", "position": 1.5}]}"#,
            ),
            ("armor", r#"{"armor_texture": 4}"#),
            ("colormap", r#"{"colormap": {}}"#),
            ("fine", r#"{"rarity": "rare"}"#),
            ("off", r#"{"polytone_ignore": true, "rarity": "rare"}"#),
        ] {
            put(&root, &format!("{DIR}/{name}.json"), body);
        }
        let set = ItemModifier::read(&root);
        assert_eq!(set.len(), 1);
        assert_eq!(
            set.resolve("foo:fine", &nothing).unwrap().rarity,
            Some(Rarity::Rare)
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_higher_priority_file_wins_for_the_same_item() {
        let root = scratch("item-priority");
        put(
            &root,
            &format!("{DIR}/low.json"),
            r#"{"targets": "foo:gem", "rarity": "common"}"#,
        );
        put(
            &root,
            &format!("{DIR}/high.json"),
            r#"{"targets": "foo:gem", "priority": 9, "rarity": "epic"}"#,
        );
        let set = ItemModifier::read(&root);
        assert_eq!(
            set.resolve("foo:gem", &nothing).unwrap().rarity,
            Some(Rarity::Epic)
        );
        let _ = std::fs::remove_dir_all(&root);
    }
}
