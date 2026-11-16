use std::path::Path;

use crate::attributes::{Argument, Attributes, Layer, Modifier};
use crate::files;
use crate::modifiers::BiomeModifier;
use crate::set::Modifiers;

const SKY: &str = "minecraft:visual/sky_color";
const FOG: &str = "minecraft:visual/fog_color";
const WATER_FOG: &str = "minecraft:visual/water_fog_color";

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct BiomeVisuals {
    pub sky: Option<u32>,
    pub fog: Option<u32>,
    pub water_fog: Option<u32>,
}

impl BiomeVisuals {
    pub fn is_empty(&self) -> bool {
        *self == Self::default()
    }

    fn apply(&mut self, attributes: &Attributes) {
        let pick = |id: &str| match attributes.get(id)? {
            Layer::Apply {
                modifier: Modifier::Override,
                argument,
                ..
            } => match argument {
                Argument::Color(argb) => Some(argb & 0xFF_FFFF),
                Argument::Number(n) if n.is_finite() => Some((*n as i64 as u32) & 0xFF_FFFF),
                _ => None,
            },
            _ => None,
        };
        for (slot, id) in [
            (&mut self.sky, SKY),
            (&mut self.fog, FOG),
            (&mut self.water_fog, WATER_FOG),
        ] {
            if let Some(color) = pick(id) {
                *slot = Some(color);
            }
        }
    }
}

#[derive(Debug, Default)]
pub struct BiomeAttributes {
    modifiers: Modifiers<BiomeModifier>,
}

impl BiomeAttributes {
    pub fn read(root: &Path) -> Self {
        let mut modifiers = Modifiers::default();
        for entry in files::scan(root, "biome_modifiers") {
            let Some(json) = &entry.json else { continue };
            match BiomeModifier::parse(json, entry.id.as_deref()) {
                Ok(Some(m)) => modifiers.push(m.targets.clone(), m.priority, m),
                Ok(None) => {}
                Err(e) => log::warn!("{}: {e}", entry.label),
            }
        }
        Self { modifiers }
    }

    pub fn is_empty(&self) -> bool {
        self.modifiers.is_empty()
    }

    pub fn visuals(&self, biome: &str) -> BiomeVisuals {
        let mut visuals = BiomeVisuals::default();
        if let Some(m) = self.modifiers.resolve(biome, &|_| false) {
            visuals.apply(&m.base);
            visuals.apply(&m.post_process);
        }
        visuals
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::files::testing::{put, scratch};

    #[test]
    fn a_biome_modifier_overrides_sky_fog_and_water_fog_of_its_target() {
        let root = scratch("biome-attrs");
        put(
            &root,
            "assets/foo/polytone/biome_modifiers/swamp.json",
            r##"{"targets": "minecraft:swamp", "attributes_modifiers": {
                "visual/sky_color": "#102030", "visual/fog_color": 255, "visual/water_fog_color": "#ff010203"}}"##,
        );
        let attrs = BiomeAttributes::read(&root);
        assert!(!attrs.is_empty());
        let swamp = attrs.visuals("minecraft:swamp");
        assert_eq!(swamp.sky, Some(0x102030));
        assert_eq!(swamp.fog, Some(0x0000FF));
        assert_eq!(swamp.water_fog, Some(0x010203));
        assert!(attrs.visuals("minecraft:plains").is_empty());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn post_process_wins_over_base_and_modifying_layers_are_left_alone() {
        let root = scratch("biome-attrs-post");
        put(
            &root,
            "assets/foo/polytone/biome_modifiers/plains.json",
            r##"{"targets": "minecraft:plains", "attributes_modifiers": {
                "base": {"visual/sky_color": "#111111", "visual/fog_color": {"argument": 0.5, "modifier": "multiply"}},
                "post_process": {"visual/sky_color": "#222222"}}}"##,
        );
        let attrs = BiomeAttributes::read(&root);
        assert!(attrs.visuals("foo:plains").is_empty());
        let plains = attrs.visuals("minecraft:plains");
        assert_eq!(plains.sky, Some(0x222222));
        assert_eq!(plains.fog, None);
        let _ = std::fs::remove_dir_all(&root);
    }
}
