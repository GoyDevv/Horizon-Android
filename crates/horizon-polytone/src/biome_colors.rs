use std::path::Path;

use crate::colormaps::Colormaps;
use crate::colors::{BiomeColor, PackColors};
use crate::fluid_modifiers::{FluidModifier, water_tag};
use crate::tint::Tint;

const BUILTIN: [&str; 3] = ["grass_color", "foliage_color", "water_color"];
const TINT: i32 = 0;

#[derive(Debug)]
enum Choice {
    Flat(u32),
    Map(String),
}

#[derive(Debug, Default)]
pub struct BiomeOverrides {
    grass: Option<Choice>,
    foliage: Option<Choice>,
    dry_foliage: Option<Choice>,
    water: Option<Choice>,
    fluid_water: Option<Tint>,
    maps: Colormaps,
}

impl BiomeOverrides {
    pub fn read(root: &Path) -> Self {
        let colors = PackColors::read(root);
        let fluids = FluidModifier::read(root);
        if colors.is_empty() && fluids.is_empty() {
            return Self::default();
        }
        let maps = Colormaps::read(root);
        let mut overrides = Self::from_parts(&colors, maps);
        overrides.fluid_water = fluids
            .into_resolved("minecraft:water", &water_tag)
            .and_then(|m| m.tint)
            .filter(|tint| {
                let biome_only = tint.depends_only_on_biome(&overrides.maps);
                if !biome_only {
                    log::warn!("fluid_modifiers: the tint of minecraft:water reads the position, which isn't applied yet");
                }
                biome_only
            });
        overrides
    }

    fn from_parts(colors: &PackColors, maps: Colormaps) -> Self {
        let choose = |function: &str| match colors.biome_color(function)? {
            BiomeColor::Flat(color) => Some(Choice::Flat(color)),
            BiomeColor::Colormap(id) => {
                let name = id.strip_prefix("minecraft:").unwrap_or(&id);
                if BUILTIN.contains(&name) {
                    return None;
                }
                match maps.get(&id) {
                    Some(map) if map.depends_only_on_biome() => Some(Choice::Map(id)),
                    Some(_) => {
                        log::warn!(
                            "biome_colors.{function}: {id} reads the position, which isn't applied yet"
                        );
                        None
                    }
                    None => {
                        log::warn!("biome_colors.{function}: no colormap {id}");
                        None
                    }
                }
            }
        };
        Self {
            grass: choose("grass"),
            foliage: choose("foliage"),
            dry_foliage: choose("dry_foliage"),
            water: choose("water"),
            fluid_water: None,
            maps,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.grass.is_none()
            && self.foliage.is_none()
            && self.dry_foliage.is_none()
            && self.water.is_none()
            && self.fluid_water.is_none()
    }

    fn resolve(&self, choice: &Option<Choice>, temperature: f64, downfall: f64) -> Option<u32> {
        match choice.as_ref()? {
            Choice::Flat(color) => Some(*color),
            Choice::Map(id) => self
                .maps
                .get(id)?
                .sample_biome(TINT, temperature, downfall)
                .map(|c| c & 0xFF_FFFF),
        }
    }

    pub fn grass(&self, temperature: f64, downfall: f64) -> Option<u32> {
        self.resolve(&self.grass, temperature, downfall)
    }

    pub fn foliage(&self, temperature: f64, downfall: f64) -> Option<u32> {
        self.resolve(&self.foliage, temperature, downfall)
    }

    pub fn dry_foliage(&self, temperature: f64, downfall: f64) -> Option<u32> {
        self.resolve(&self.dry_foliage, temperature, downfall)
    }

    pub fn water(&self, temperature: f64, downfall: f64) -> Option<u32> {
        if let Some(tint) = &self.fluid_water {
            if let Some(color) = tint.biome_color(TINT, &self.maps, temperature, downfall) {
                return Some(color);
            }
        }
        self.resolve(&self.water, temperature, downfall)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn scratch(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("polytone-biomes-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("assets/foo/polytone/colormaps")).unwrap();
        dir
    }

    fn png(width: u32, height: u32, rgb: &[u8]) -> Vec<u8> {
        let mut out = Vec::new();
        let mut encoder = png::Encoder::new(&mut out, width, height);
        encoder.set_color(png::ColorType::Rgb);
        encoder.set_depth(png::BitDepth::Eight);
        encoder
            .write_header()
            .unwrap()
            .write_image_data(rgb)
            .unwrap();
        out
    }

    fn write(root: &Path, relative: &str, bytes: impl AsRef<[u8]>) {
        std::fs::write(root.join(relative), bytes).unwrap();
    }

    fn two_by_two() -> Vec<u8> {
        png(
            2,
            2,
            &[
                0x10, 0, 0, 0x20, 0, 0, //
                0, 0x30, 0, 0, 0x40, 0,
            ],
        )
    }

    #[test]
    fn a_fluid_modifier_tints_water_and_beats_the_biome_colours_file() {
        let root = scratch("fluid");
        write(
            &root,
            "assets/foo/polytone/colors.json",
            r#"{"biome_colors": {"water": 16711680}}"#,
        );
        std::fs::create_dir_all(root.join("assets/foo/polytone/fluid_modifiers")).unwrap();
        write(
            &root,
            "assets/foo/polytone/fluid_modifiers/water.json",
            r##"{"targets": "minecraft:water", "colormap": "#0000FF"}"##,
        );
        let overrides = BiomeOverrides::read(&root);
        assert!(!overrides.is_empty());
        assert_eq!(overrides.water(0.5, 0.5), Some(0x0000FF));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_fluid_colormap_sampled_by_biome_reads_temperature_and_downfall() {
        let root = scratch("fluid-map");
        std::fs::create_dir_all(root.join("assets/foo/polytone/fluid_modifiers")).unwrap();
        write(
            &root,
            "assets/foo/polytone/fluid_modifiers/water.json",
            r#"{"targets": "minecraft:water", "colormap": {"default_color": 1, "x_axis": "TEMPERATURE"}}"#,
        );
        write(
            &root,
            "assets/foo/polytone/fluid_modifiers/water.png",
            two_by_two(),
        );
        let overrides = BiomeOverrides::read(&root);
        assert!(overrides.water(0.0, 0.0).is_some());
        assert_ne!(overrides.water(0.0, 0.0), overrides.water(1.0, 0.0));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_fluid_tint_that_reads_the_position_is_left_alone() {
        let root = scratch("fluid-pos");
        std::fs::create_dir_all(root.join("assets/foo/polytone/fluid_modifiers")).unwrap();
        write(
            &root,
            "assets/foo/polytone/fluid_modifiers/water.json",
            r#"{"targets": "minecraft:water", "colormap": {"default_color": 1, "x_axis": "POS_X / 256", "y_axis": 0}}"#,
        );
        write(
            &root,
            "assets/foo/polytone/fluid_modifiers/water.png",
            two_by_two(),
        );
        let overrides = BiomeOverrides::read(&root);
        assert_eq!(overrides.water(0.5, 0.5), None);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_folder_without_colours_overrides_nothing() {
        let root = scratch("none");
        let overrides = BiomeOverrides::read(&root);
        assert!(overrides.is_empty());
        assert_eq!(overrides.grass(0.5, 0.5), None);
        assert_eq!(overrides.water(0.5, 0.5), None);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn flat_colours_replace_the_function_for_every_biome() {
        let root = scratch("flat");
        write(
            &root,
            "assets/foo/polytone/colors.json",
            r##"{"biome_colors": {"grass": "#112233", "water": 4521796}}"##,
        );
        let overrides = BiomeOverrides::read(&root);
        assert!(!overrides.is_empty());
        assert_eq!(overrides.grass(0.0, 0.0), Some(0x112233));
        assert_eq!(overrides.grass(1.0, 1.0), Some(0x112233));
        assert_eq!(overrides.water(0.3, 0.3), Some(4521796));
        assert_eq!(overrides.foliage(0.3, 0.3), None);
        assert_eq!(overrides.dry_foliage(0.3, 0.3), None);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_colormap_is_sampled_by_the_biomes_temperature_and_downfall() {
        let root = scratch("map");
        write(
            &root,
            "assets/foo/polytone/colormaps/lush.png",
            two_by_two(),
        );
        write(
            &root,
            "assets/foo/polytone/colormaps/lush.json",
            r#"{"rounds": false}"#,
        );
        write(
            &root,
            "assets/foo/polytone/colors.json",
            r#"{"biome_colors": {"foliage": "foo:lush"}}"#,
        );
        let overrides = BiomeOverrides::read(&root);
        assert_eq!(overrides.foliage(1.0, 1.0), Some(0x100000));
        assert_eq!(overrides.foliage(0.0, 1.0), Some(0x200000));
        assert_eq!(overrides.foliage(1.0, 0.0), Some(0x003000));
        assert_eq!(overrides.foliage(0.0, 0.0), Some(0x004000));
        assert_eq!(overrides.grass(1.0, 1.0), None);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn vanillas_own_functions_and_unusable_colormaps_override_nothing() {
        let root = scratch("skip");
        write(&root, "assets/foo/polytone/colormaps/pos.png", two_by_two());
        write(
            &root,
            "assets/foo/polytone/colormaps/pos.json",
            r#"{"x_axis": "POS_X / 16"}"#,
        );
        write(
            &root,
            "assets/foo/polytone/colors.json",
            r#"{"biome_colors": {"grass": "grass_color", "foliage": "foo:pos", "water": "foo:missing"}}"#,
        );
        let overrides = BiomeOverrides::read(&root);
        assert!(overrides.is_empty());
        assert_eq!(overrides.grass(0.5, 0.5), None);
        assert_eq!(overrides.foliage(0.5, 0.5), None);
        assert_eq!(overrides.water(0.5, 0.5), None);
        let _ = std::fs::remove_dir_all(&root);
    }
}
