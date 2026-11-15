use std::path::Path;

use crate::colormaps::Colormaps;
use crate::colors::{BiomeColor, PackColors};

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
    maps: Colormaps,
}

impl BiomeOverrides {
    pub fn read(root: &Path) -> Self {
        let colors = PackColors::read(root);
        if colors.is_empty() {
            return Self::default();
        }
        Self::from_parts(&colors, Colormaps::read(root))
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
            maps,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.grass.is_none()
            && self.foliage.is_none()
            && self.dry_foliage.is_none()
            && self.water.is_none()
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
