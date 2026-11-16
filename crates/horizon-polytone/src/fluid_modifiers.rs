use std::path::Path;

use crate::colormaps::Colormap;
use crate::files::{self, Entry};
use crate::modifiers::common;
use crate::set::Modifiers;
use crate::target::Targets;
use crate::tint::{ColorSource, Tint};

#[derive(Debug)]
pub struct FluidModifier {
    pub targets: Targets,
    pub priority: i32,
    pub tint: Option<Tint>,
}

impl FluidModifier {
    pub(crate) fn parse(entry: &Entry) -> Result<Option<Self>, String> {
        let textures = files::textures(&entry.dir, &entry.stem)?;
        let Some(json) = &entry.json else {
            if textures.is_empty() {
                return Ok(None);
            }
            return Ok(Some(Self {
                targets: Targets::parse(None, entry.id.as_deref())?,
                priority: 0,
                tint: Some(Tint::All(ColorSource::Inline(Colormap::new(
                    None, textures,
                )?))),
            }));
        };
        let Some((targets, priority)) = common(json, entry.id.as_deref())? else {
            return Ok(None);
        };
        let tint = json
            .get("colormap")
            .map(|v| Tint::parse(v, &textures))
            .transpose()?;
        Ok(Some(Self {
            targets,
            priority,
            tint,
        }))
    }

    pub fn read(root: &Path) -> Modifiers<Self> {
        let mut set = Modifiers::default();
        for entry in files::scan(root, "fluid_modifiers") {
            match Self::parse(&entry) {
                Ok(Some(modifier)) => {
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

pub fn water_tag(tag: &str) -> bool {
    tag == "minecraft:water"
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::colormaps::Colormaps;
    use crate::files::testing::*;

    const DIR: &str = "assets/foo/polytone/fluid_modifiers";

    #[test]
    fn a_png_alone_tints_the_fluid_it_is_named_after() {
        let root = scratch("fluid-png");
        put(&root, &format!("{DIR}/honey.png"), pixel(250, 180, 10));
        let set = FluidModifier::read(&root);
        let tint = set
            .resolve("foo:honey", &|_| false)
            .unwrap()
            .tint
            .as_ref()
            .unwrap();
        assert_eq!(
            tint.biome_color(0, &Colormaps::default(), 0.1, 0.9),
            Some(0xFAB40A)
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_json_colormap_and_targets_tint_the_listed_fluids_and_fog_fields_are_left_alone() {
        let root = scratch("fluid-json");
        put(
            &root,
            &format!("{DIR}/both.json"),
            r##"{"targets": ["minecraft:water", "minecraft:flowing_water"],
                 "priority": 2, "colormap": "#3050A0", "fog_radius": 0.5, "fog_colormap": "#000000"}"##,
        );
        let set = FluidModifier::read(&root);
        for id in ["minecraft:water", "minecraft:flowing_water"] {
            let m = set.resolve(id, &|_| false).unwrap();
            assert_eq!(m.priority, 2);
            let tint = m.tint.as_ref().unwrap();
            assert_eq!(
                tint.biome_color(0, &Colormaps::default(), 0.0, 0.0),
                Some(0x3050A0)
            );
        }
        assert!(set.resolve("minecraft:lava", &|_| false).is_none());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_fluid_file_that_only_sets_fog_has_no_tint() {
        let root = scratch("fluid-fog");
        put(&root, &format!("{DIR}/lava.json"), r#"{"fog_radius": 2}"#);
        let set = FluidModifier::read(&root);
        assert!(set.resolve("foo:lava", &|_| false).unwrap().tint.is_none());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn broken_and_switched_off_files_are_skipped() {
        let root = scratch("fluid-bad");
        put(&root, &format!("{DIR}/a.json"), r#"{"colormap": {}}"#);
        put(
            &root,
            &format!("{DIR}/b.json"),
            r#"{"polytone_ignore": true, "colormap": 1}"#,
        );
        assert!(FluidModifier::read(&root).is_empty());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn only_the_water_tag_is_known() {
        assert!(water_tag("minecraft:water"));
        assert!(!water_tag("minecraft:lava"));
    }
}
