use std::path::Path;

use serde_json::Value as Json;

use crate::colormaps::Colormap;
use crate::files::{self, Entry};
use crate::modifiers::common;
use crate::set::Modifiers;
use crate::target::Targets;
use crate::tint::{ColorSource, Tint};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BlockRenderType {
    Solid,
    Cutout,
    Translucent,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BlockOffset {
    None,
    Xz,
    Xyz,
}

#[derive(Debug)]
pub struct BlockModifier {
    pub targets: Targets,
    pub priority: i32,
    pub tint: Option<Tint>,
    pub render_type: Option<BlockRenderType>,
    pub offset: Option<BlockOffset>,
    pub map_color: Option<String>,
    pub can_occlude: Option<bool>,
    pub spawn_particles_on_break: Option<bool>,
    pub tinted_breaking_particles: Option<bool>,
    pub unread: Vec<String>,
}

const UNREAD: [&str; 2] = ["sound_type", "block_set_type"];

fn flag(json: &Json, key: &str) -> Result<Option<bool>, String> {
    match json.get(key) {
        None => Ok(None),
        Some(Json::Bool(b)) => Ok(Some(*b)),
        Some(_) => Err(format!("{key} is true or false")),
    }
}

impl BlockModifier {
    fn bare(targets: Targets) -> Self {
        Self {
            targets,
            priority: 0,
            tint: None,
            render_type: None,
            offset: None,
            map_color: None,
            can_occlude: None,
            spawn_particles_on_break: None,
            tinted_breaking_particles: None,
            unread: Vec::new(),
        }
    }

    pub(crate) fn parse(entry: &Entry) -> Result<Option<Self>, String> {
        let textures = files::textures(&entry.dir, &entry.stem)?;
        let Some(json) = &entry.json else {
            if textures.is_empty() {
                return Ok(None);
            }
            let mut modifier = Self::bare(Targets::parse(None, entry.id.as_deref())?);
            let map = Colormap::new(None, textures)?;
            modifier.tint = Some(Tint::All(ColorSource::Inline(map)));
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
        modifier.render_type = match json.get("render_type") {
            None => None,
            Some(Json::String(s)) => Some(match s.as_str() {
                "solid" => BlockRenderType::Solid,
                "cutout" => BlockRenderType::Cutout,
                "translucent" => BlockRenderType::Translucent,
                other => {
                    return Err(format!(
                        "render_type {other:?} is not solid, cutout or translucent"
                    ));
                }
            }),
            Some(_) => return Err("render_type is a string".to_owned()),
        };
        match json.get("offset_type") {
            None => {}
            Some(Json::String(s)) => {
                modifier.offset = Some(match s.as_str() {
                    "none" => BlockOffset::None,
                    "xz" => BlockOffset::Xz,
                    "xyz" => BlockOffset::Xyz,
                    other => return Err(format!("offset_type {other:?} is not none, xz or xyz")),
                });
            }
            Some(Json::Object(_)) => modifier.unread.push("offset_type".to_owned()),
            Some(_) => return Err("offset_type is a string or an object".to_owned()),
        }
        modifier.map_color = match json.get("map_color") {
            None => None,
            Some(Json::String(s)) => Some(s.trim_start_matches("map.").to_owned()),
            Some(_) => return Err("map_color is a string".to_owned()),
        };
        modifier.can_occlude = flag(json, "can_occlude")?;
        modifier.spawn_particles_on_break = flag(json, "spawn_particles_on_break")?;
        modifier.tinted_breaking_particles = flag(json, "tinted_breaking_particles")?;
        for key in UNREAD {
            if json.get(key).is_some() {
                modifier.unread.push(key.to_owned());
            }
        }
        Ok(Some(modifier))
    }

    pub fn read(root: &Path) -> Modifiers<Self> {
        let mut set = Modifiers::default();
        for entry in files::scan(root, "block_modifiers") {
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

    const DIR: &str = "assets/foo/polytone/block_modifiers";

    fn no_tags(_: &str) -> bool {
        false
    }

    #[test]
    fn a_png_alone_tints_the_block_it_is_named_after() {
        let root = scratch("block-png");
        put(&root, &format!("{DIR}/ash.png"), pixel(9, 8, 7));
        let set = BlockModifier::read(&root);
        let modifier = set.resolve("foo:ash", &no_tags).unwrap();
        let tint = modifier.tint.as_ref().unwrap();
        assert_eq!(
            tint.biome_color(0, &Colormaps::default(), 0.3, 0.8),
            Some(0x090807)
        );
        assert!(set.resolve("foo:other", &no_tags).is_none());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_json_gives_a_block_its_tint_render_type_offset_and_flags() {
        let root = scratch("block-json");
        put(
            &root,
            &format!("{DIR}/fancy.json"),
            r##"{
                "targets": ["foo:a", "#minecraft:leaves"],
                "priority": 4,
                "colormap": {"0": "#112233", "1": 255},
                "render_type": "translucent",
                "offset_type": "xz",
                "map_color": "map.plant",
                "can_occlude": false,
                "spawn_particles_on_break": true,
                "tinted_breaking_particles": false,
                "sound_type": "sculk",
                "block_set_type": "minecraft:cherry"
            }"##,
        );
        let set = BlockModifier::read(&root);
        assert_eq!(set.len(), 1);
        let m = set
            .resolve("minecraft:oak_leaves", &|t| t == "minecraft:leaves")
            .unwrap();
        assert!(set.resolve("foo:a", &no_tags).is_some());
        assert!(set.resolve("minecraft:oak_leaves", &no_tags).is_none());
        assert_eq!(m.priority, 4);
        assert_eq!(m.render_type, Some(BlockRenderType::Translucent));
        assert_eq!(m.offset, Some(BlockOffset::Xz));
        assert_eq!(m.map_color.as_deref(), Some("plant"));
        assert_eq!(m.can_occlude, Some(false));
        assert_eq!(m.spawn_particles_on_break, Some(true));
        assert_eq!(m.tinted_breaking_particles, Some(false));
        assert_eq!(m.unread, ["sound_type", "block_set_type"]);
        let maps = Colormaps::default();
        let tint = m.tint.as_ref().unwrap();
        assert_eq!(tint.biome_color(0, &maps, 0.0, 0.0), Some(0x112233));
        assert_eq!(tint.biome_color(1, &maps, 0.0, 0.0), Some(255));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_file_without_targets_aims_at_the_block_its_name_gives() {
        let root = scratch("block-implicit");
        put(
            &root,
            &format!("{DIR}/dirt.json"),
            r#"{"render_type": "cutout"}"#,
        );
        put(
            &root,
            &format!("{DIR}/sub/moss.json"),
            r#"{"offset_type": "xyz"}"#,
        );
        let set = BlockModifier::read(&root);
        assert_eq!(
            set.resolve("foo:dirt", &no_tags).unwrap().render_type,
            Some(BlockRenderType::Cutout)
        );
        assert_eq!(
            set.resolve("foo:sub/moss", &no_tags).unwrap().offset,
            Some(BlockOffset::Xyz)
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn an_inline_colormap_reads_the_texture_beside_the_json() {
        let root = scratch("block-inline");
        put(
            &root,
            &format!("{DIR}/leafy.json"),
            r#"{"colormap": {"default_color": 1, "x_axis": "TEMPERATURE"}}"#,
        );
        put(&root, &format!("{DIR}/leafy.png"), pixel(0, 200, 0));
        let set = BlockModifier::read(&root);
        let tint = set
            .resolve("foo:leafy", &no_tags)
            .unwrap()
            .tint
            .as_ref()
            .unwrap();
        assert_eq!(
            tint.biome_color(0, &Colormaps::default(), 0.5, 0.5),
            Some(0x00C800)
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn what_cannot_be_read_refuses_the_file_and_leaves_the_others() {
        let root = scratch("block-bad");
        put(
            &root,
            &format!("{DIR}/a.json"),
            r#"{"render_type": "shiny"}"#,
        );
        put(
            &root,
            &format!("{DIR}/b.json"),
            r#"{"offset_type": "diagonal"}"#,
        );
        put(&root, &format!("{DIR}/c.json"), r#"{"can_occlude": "yes"}"#);
        put(
            &root,
            &format!("{DIR}/d.json"),
            r#"{"colormap": {"x_axis": "TEMPERATURE"}}"#,
        );
        put(
            &root,
            &format!("{DIR}/e.json"),
            r#"{"targets": "(", "render_type": "solid"}"#,
        );
        put(
            &root,
            &format!("{DIR}/ok.json"),
            r#"{"render_type": "solid"}"#,
        );
        put(
            &root,
            &format!("{DIR}/off.json"),
            r#"{"polytone_ignore": true, "render_type": "solid"}"#,
        );
        let set = BlockModifier::read(&root);
        assert_eq!(set.len(), 1);
        assert!(set.resolve("foo:ok", &no_tags).is_some());
        assert!(set.resolve("foo:off", &no_tags).is_none());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_custom_offset_object_is_reported_not_guessed() {
        let root = scratch("block-offset");
        put(
            &root,
            &format!("{DIR}/odd.json"),
            r#"{"offset_type": {"max_y": 0.2}}"#,
        );
        let set = BlockModifier::read(&root);
        let m = set.resolve("foo:odd", &no_tags).unwrap();
        assert_eq!(m.offset, None);
        assert_eq!(m.unread, ["offset_type"]);
        let _ = std::fs::remove_dir_all(&root);
    }
}
