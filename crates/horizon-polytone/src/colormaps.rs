use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde_json::Value as Json;

use crate::colors::parse_color;
use crate::expr::{Field, Host, Program, Root, Value};

pub const ALL_TINTS: i32 = -1;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Texture {
    width: u32,
    height: u32,
    pixels: Vec<u32>,
}

impl Texture {
    pub fn new(width: u32, height: u32, pixels: Vec<u32>) -> Option<Self> {
        let count = (width as usize).checked_mul(height as usize)?;
        (count != 0 && count == pixels.len()).then_some(Self {
            width,
            height,
            pixels,
        })
    }

    pub fn decode_png(bytes: &[u8]) -> Result<Self, String> {
        let mut decoder = png::Decoder::new(std::io::Cursor::new(bytes));
        decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
        let mut reader = decoder.read_info().map_err(|e| e.to_string())?;
        let size = reader.output_buffer_size().ok_or("colormap too large")?;
        let mut buffer = vec![0; size];
        let info = reader.next_frame(&mut buffer).map_err(|e| e.to_string())?;
        let channels = info.color_type.samples();
        let pixels = buffer[..info.buffer_size()]
            .chunks_exact(channels)
            .map(|p| {
                let (r, g, b) = if channels >= 3 {
                    (p[0], p[1], p[2])
                } else {
                    (p[0], p[0], p[0])
                };
                u32::from(r) << 16 | u32::from(g) << 8 | u32::from(b)
            })
            .collect();
        Self::new(info.width, info.height, pixels).ok_or_else(|| "empty colormap".to_owned())
    }

    pub fn width(&self) -> u32 {
        self.width
    }

    pub fn height(&self) -> u32 {
        self.height
    }

    pub fn at(&self, x: u32, y: u32) -> u32 {
        let x = x.min(self.width - 1) as usize;
        let y = y.min(self.height - 1) as usize;
        self.pixels[y * self.width as usize + x]
    }

    pub fn center(&self) -> u32 {
        self.at(self.width / 2, self.height / 2)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct SampleContext {
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub temperature: f64,
    pub downfall: f64,
}

struct Sampler<'a>(&'a SampleContext);

impl Sampler<'_> {
    fn read(&self, name: &str) -> Option<f64> {
        Some(match name {
            "x" => self.0.x,
            "y" => self.0.y,
            "z" => self.0.z,
            "temperature" => self.0.temperature,
            "downfall" => self.0.downfall,
            _ => return None,
        })
    }
}

impl Host for Sampler<'_> {
    fn root(&mut self, root: Root) -> Value {
        if root == Root::Object {
            Value::Object(0)
        } else {
            Value::Null
        }
    }

    fn method(&mut self, target: &Value, name: &str, args: &[Value]) -> Result<Value, String> {
        match (target, args.is_empty(), self.read(name)) {
            (Value::Object(0), true, Some(value)) => Ok(Value::Num(value)),
            _ => Err(format!("{name} is not available in a colormap")),
        }
    }

    fn function(&mut self, name: &str, _: &[Value]) -> Result<Value, String> {
        Err(format!("{name} is not available in a colormap"))
    }

    fn field(&mut self, root: Root, field: Field) -> Option<Value> {
        if root != Root::Object {
            return None;
        }
        let value = match field {
            Field::X => self.0.x,
            Field::Y => self.0.y,
            Field::Z => self.0.z,
            Field::Temperature => self.0.temperature,
            Field::Downfall => self.0.downfall,
            _ => return None,
        };
        Some(Value::Num(value))
    }
}

fn axis(json: Option<&Json>, default: &str) -> Result<Program, String> {
    let source = match json {
        None => default.to_owned(),
        Some(Json::String(s)) => match s.trim() {
            "temperature" => "TEMPERATURE".to_owned(),
            "downfall" => "DOWNFALL".to_owned(),
            "legacy_temperature" | "biome_id" => {
                return Err(format!("the {} sampler isn't read yet", s.trim()));
            }
            other => other.to_owned(),
        },
        Some(Json::Number(n)) => n.to_string(),
        Some(_) => return Err("an axis is an expression".to_owned()),
    };
    Program::compile(&source)
}

fn flag(json: &Json, key: &str, default: bool) -> bool {
    json.get(key).and_then(Json::as_bool).unwrap_or(default)
}

fn pick(unit: f64, size: u32, rounds: bool) -> u32 {
    let scaled = (1.0 - unit) * f64::from(size - 1);
    let index = if rounds {
        scaled.round()
    } else {
        scaled.floor()
    };
    (index as u32).min(size - 1)
}

fn unit(value: f64) -> f64 {
    if value.is_nan() {
        0.0
    } else {
        value.clamp(0.0, 1.0)
    }
}

#[derive(Debug)]
pub struct Colormap {
    x: Program,
    y: Program,
    triangular: bool,
    rounds: bool,
    blend: bool,
    default_color: Option<u32>,
    textures: BTreeMap<i32, Texture>,
    biome_only: bool,
}

impl Colormap {
    pub fn new(json: Option<&Json>, textures: BTreeMap<i32, Texture>) -> Result<Self, String> {
        if textures.is_empty() {
            return Err("no texture".to_owned());
        }
        let (x, y, triangular, rounds, blend, default_color) = match json {
            None => (
                axis(None, "TEMPERATURE")?,
                axis(None, "DOWNFALL")?,
                true,
                false,
                true,
                None,
            ),
            Some(json) => {
                if !json.is_object() {
                    return Err("a colormap is an object".to_owned());
                }
                if json.get("color_modifier").is_some_and(|v| !v.is_null()) {
                    return Err("color_modifier isn't read yet".to_owned());
                }
                (
                    axis(json.get("x_axis"), "TEMPERATURE")?,
                    axis(json.get("y_axis"), "DOWNFALL")?,
                    flag(json, "triangular", false),
                    flag(json, "rounds", true),
                    flag(json, "biome_blend", true),
                    json.get("default_color")
                        .and_then(parse_color)
                        .map(|c| c & 0xFF_FFFF),
                )
            }
        };
        let only_biome = |root: Root, name: &str| {
            root == Root::Object && matches!(name, "temperature" | "downfall")
        };
        let biome_only = x.reads_only(&only_biome) && y.reads_only(&only_biome);
        Ok(Self {
            x,
            y,
            triangular,
            rounds,
            blend,
            default_color,
            textures,
            biome_only,
        })
    }

    pub fn texture(&self, tint: i32) -> Option<&Texture> {
        self.textures
            .get(&tint)
            .or_else(|| self.textures.get(&ALL_TINTS))
    }

    pub fn tints(&self) -> impl Iterator<Item = i32> + '_ {
        self.textures.keys().copied()
    }

    pub fn blends(&self) -> bool {
        self.blend
    }

    pub fn depends_only_on_biome(&self) -> bool {
        self.biome_only
    }

    pub fn default_color(&self) -> Option<u32> {
        self.default_color
            .or_else(|| self.textures.values().next().map(Texture::center))
    }

    pub fn sample(&self, tint: i32, at: &SampleContext) -> Option<u32> {
        let texture = self.texture(tint)?;
        let mut host = Sampler(at);
        let u = unit(self.x.eval(&mut host).ok()?.num());
        let mut v = unit(self.y.eval(&mut host).ok()?.num());
        if self.triangular {
            v *= u;
        }
        Some(texture.at(
            pick(u, texture.width, self.rounds),
            pick(v, texture.height, self.rounds),
        ))
    }

    pub fn sample_biome(&self, tint: i32, temperature: f64, downfall: f64) -> Option<u32> {
        if !self.biome_only {
            return None;
        }
        self.sample(
            tint,
            &SampleContext {
                temperature,
                downfall,
                ..SampleContext::default()
            },
        )
    }
}

#[derive(Debug, Default)]
pub struct Colormaps {
    maps: BTreeMap<String, Colormap>,
}

#[derive(Default)]
struct Files {
    json: Option<PathBuf>,
    textures: BTreeMap<i32, PathBuf>,
}

pub(crate) fn split_tint(stem: &str) -> (&str, i32) {
    if let Some((name, tail)) = stem.rsplit_once('_') {
        if !name.is_empty() {
            if let Ok(tint) = tail.parse::<i32>() {
                return (name, tint);
            }
        }
    }
    (stem, ALL_TINTS)
}

fn collect(base: &Path, namespace: &str, found: &mut BTreeMap<String, Files>) {
    let mut stack = vec![base.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir).into_iter().flatten().flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
                continue;
            };
            let Ok(relative) = path.strip_prefix(base) else {
                continue;
            };
            let folder = relative
                .parent()
                .map(|p| p.to_string_lossy().replace('\\', "/"))
                .filter(|p| !p.is_empty())
                .map(|p| format!("{p}/"))
                .unwrap_or_default();
            match path.extension().and_then(|e| e.to_str()) {
                Some("png") => {
                    let (name, tint) = split_tint(stem);
                    found
                        .entry(format!("{namespace}:{folder}{name}"))
                        .or_default()
                        .textures
                        .insert(tint, path);
                }
                Some("json") => {
                    let id = format!("{namespace}:{folder}{stem}");
                    found.entry(id).or_default().json = Some(path);
                }
                _ => {}
            }
        }
    }
}

fn normalize(id: &str) -> String {
    let id = id.trim();
    if id.contains(':') {
        id.to_owned()
    } else {
        format!("minecraft:{id}")
    }
}

impl Colormaps {
    pub fn read(root: &Path) -> Self {
        let mut namespaces: Vec<_> = std::fs::read_dir(root.join("assets"))
            .into_iter()
            .flatten()
            .flatten()
            .filter(|e| e.path().is_dir())
            .collect();
        namespaces.sort_by_key(|e| e.file_name());
        let mut found = BTreeMap::new();
        for namespace in namespaces {
            let name = namespace.file_name().to_string_lossy().into_owned();
            collect(
                &namespace.path().join("polytone/colormaps"),
                &name,
                &mut found,
            );
        }
        let mut maps = BTreeMap::new();
        for (id, files) in found {
            if files.textures.is_empty() {
                continue;
            }
            match Self::build(&files) {
                Ok(map) => {
                    maps.insert(id, map);
                }
                Err(e) => log::warn!("colormap {id}: {e}"),
            }
        }
        Self { maps }
    }

    fn build(files: &Files) -> Result<Colormap, String> {
        let json = match &files.json {
            None => None,
            Some(path) => {
                let text = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
                Some(serde_json::from_str::<Json>(&text).map_err(|e| e.to_string())?)
            }
        };
        let mut textures = BTreeMap::new();
        for (tint, path) in &files.textures {
            let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
            let texture =
                Texture::decode_png(&bytes).map_err(|e| format!("{}: {e}", path.display()))?;
            textures.insert(*tint, texture);
        }
        Colormap::new(json.as_ref(), textures)
    }

    pub fn get(&self, id: &str) -> Option<&Colormap> {
        self.maps.get(&normalize(id))
    }

    pub fn ids(&self) -> impl Iterator<Item = &str> {
        self.maps.keys().map(String::as_str)
    }

    pub fn len(&self) -> usize {
        self.maps.len()
    }

    pub fn is_empty(&self) -> bool {
        self.maps.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn gradient() -> Texture {
        let pixels = (0..256u32)
            .flat_map(|y| (0..256u32).map(move |x| x << 16 | y << 8))
            .collect();
        Texture::new(256, 256, pixels).unwrap()
    }

    fn only(texture: Texture) -> BTreeMap<i32, Texture> {
        BTreeMap::from([(ALL_TINTS, texture)])
    }

    fn xy(color: u32) -> (u32, u32) {
        (color >> 16 & 0xFF, color >> 8 & 0xFF)
    }

    fn biome(map: &Colormap, temperature: f64, downfall: f64) -> (u32, u32) {
        xy(map.sample_biome(ALL_TINTS, temperature, downfall).unwrap())
    }

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("polytone-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn png_bytes(width: u32, height: u32, color: png::ColorType, data: &[u8]) -> Vec<u8> {
        let mut out = Vec::new();
        let mut encoder = png::Encoder::new(&mut out, width, height);
        encoder.set_color(color);
        encoder.set_depth(png::BitDepth::Eight);
        encoder
            .write_header()
            .unwrap()
            .write_image_data(data)
            .unwrap();
        out
    }

    #[test]
    fn a_texture_needs_exactly_its_pixels() {
        assert!(Texture::new(2, 2, vec![0; 3]).is_none());
        assert!(Texture::new(0, 2, vec![]).is_none());
        assert!(Texture::new(2, 2, vec![0; 4]).is_some());
    }

    #[test]
    fn png_files_decode_to_rgb_whatever_their_colour_type() {
        let rgb = png_bytes(2, 1, png::ColorType::Rgb, &[1, 2, 3, 250, 251, 252]);
        let texture = Texture::decode_png(&rgb).unwrap();
        assert_eq!((texture.width(), texture.height()), (2, 1));
        assert_eq!(texture.at(0, 0), 0x010203);
        assert_eq!(texture.at(1, 0), 0xFAFBFC);

        let rgba = png_bytes(1, 1, png::ColorType::Rgba, &[9, 8, 7, 0]);
        assert_eq!(Texture::decode_png(&rgba).unwrap().at(0, 0), 0x090807);

        let grey = png_bytes(1, 1, png::ColorType::Grayscale, &[0x40]);
        assert_eq!(Texture::decode_png(&grey).unwrap().at(0, 0), 0x404040);

        assert!(Texture::decode_png(b"not a png").is_err());
    }

    #[test]
    fn a_texture_alone_samples_like_vanillas_grass() {
        let map = Colormap::new(None, only(gradient())).unwrap();
        for t in 0..=20 {
            for d in 0..=20 {
                let (temperature, downfall) = (f64::from(t) / 20.0, f64::from(d) / 20.0);
                let humidity = downfall * temperature;
                let x = ((1.0 - temperature) * 255.0) as u32;
                let y = ((1.0 - humidity) * 255.0) as u32;
                assert_eq!(
                    biome(&map, temperature, downfall),
                    (x, y),
                    "t={temperature} d={downfall}"
                );
            }
        }
    }

    #[test]
    fn the_corners_of_the_triangle() {
        let map = Colormap::new(None, only(gradient())).unwrap();
        assert_eq!(biome(&map, 1.0, 1.0), (0, 0));
        assert_eq!(biome(&map, 1.0, 0.0), (0, 255));
        assert_eq!(biome(&map, 0.0, 1.0), (255, 255));
        assert_eq!(biome(&map, 0.0, 0.0), (255, 255));
    }

    #[test]
    fn a_json_colormap_rounds_to_the_nearest_pixel_unless_told_not_to() {
        let rounded = Colormap::new(Some(&json!({})), only(gradient())).unwrap();
        assert_eq!(biome(&rounded, 0.5, 1.0), (128, 0));
        let floored = Colormap::new(Some(&json!({"rounds": false})), only(gradient())).unwrap();
        assert_eq!(biome(&floored, 0.5, 1.0), (127, 0));
    }

    #[test]
    fn triangular_samples_downfall_scaled_by_temperature() {
        let flat = Colormap::new(Some(&json!({})), only(gradient())).unwrap();
        let triangular =
            Colormap::new(Some(&json!({"triangular": true})), only(gradient())).unwrap();
        assert_eq!(biome(&flat, 0.5, 1.0), (128, 0));
        assert_eq!(biome(&triangular, 0.5, 1.0), (128, 128));
    }

    #[test]
    fn samplers_outside_zero_to_one_are_clamped() {
        let map = Colormap::new(Some(&json!({})), only(gradient())).unwrap();
        assert_eq!(biome(&map, 2.0, -3.0), (0, 255));
        assert_eq!(biome(&map, -1.0, 9.0), (255, 0));
        assert_eq!(biome(&map, f64::NAN, f64::NAN), (255, 255));
    }

    #[test]
    fn axes_are_expressions_over_the_position() {
        let map = Colormap::new(
            Some(&json!({"x_axis": "POS_X / 256", "y_axis": 0})),
            only(gradient()),
        )
        .unwrap();
        assert!(!map.depends_only_on_biome());
        assert!(map.sample_biome(ALL_TINTS, 0.5, 0.5).is_none());
        let at = SampleContext {
            x: 64.0,
            ..SampleContext::default()
        };
        assert_eq!(xy(map.sample(ALL_TINTS, &at).unwrap()), (191, 255));
        let scripted = Colormap::new(
            Some(&json!({"x_axis": "o.getX() / 256", "y_axis": "o.getZ() / 256"})),
            only(gradient()),
        )
        .unwrap();
        let at = SampleContext {
            x: 0.0,
            z: 256.0,
            ..SampleContext::default()
        };
        assert_eq!(xy(scripted.sample(ALL_TINTS, &at).unwrap()), (255, 0));
    }

    #[test]
    fn the_named_samplers_are_the_biome_variables() {
        let named = Colormap::new(
            Some(&json!({"x_axis": "temperature", "y_axis": "downfall"})),
            only(gradient()),
        )
        .unwrap();
        let bare = Colormap::new(Some(&json!({})), only(gradient())).unwrap();
        assert!(named.depends_only_on_biome());
        assert!(bare.depends_only_on_biome());
        assert_eq!(biome(&named, 0.3, 0.9), biome(&bare, 0.3, 0.9));
        let swapped = Colormap::new(
            Some(&json!({"x_axis": "DOWNFALL", "y_axis": "TEMPERATURE"})),
            only(gradient()),
        )
        .unwrap();
        assert_eq!(biome(&swapped, 0.0, 1.0), (0, 255));
    }

    #[test]
    fn tints_fall_back_to_the_all_tints_texture() {
        let tinted = |value: u32| Texture::new(1, 1, vec![value]).unwrap();
        let both = Colormap::new(
            Some(&json!({})),
            BTreeMap::from([(0, tinted(0xAA0000)), (1, tinted(0x00BB00))]),
        )
        .unwrap();
        assert_eq!(both.sample_biome(0, 0.5, 0.5), Some(0xAA0000));
        assert_eq!(both.sample_biome(1, 0.5, 0.5), Some(0x00BB00));
        assert_eq!(both.sample_biome(2, 0.5, 0.5), None);
        let shared = Colormap::new(
            Some(&json!({})),
            BTreeMap::from([(ALL_TINTS, tinted(0x123456)), (1, tinted(0x00BB00))]),
        )
        .unwrap();
        assert_eq!(shared.sample_biome(1, 0.5, 0.5), Some(0x00BB00));
        assert_eq!(shared.sample_biome(7, 0.5, 0.5), Some(0x123456));
        assert_eq!(shared.tints().collect::<Vec<_>>(), vec![-1, 1]);
    }

    #[test]
    fn the_default_colour_is_set_or_the_texture_centre() {
        let set =
            Colormap::new(Some(&json!({"default_color": "#ff00ff"})), only(gradient())).unwrap();
        assert_eq!(set.default_color(), Some(0xFF00FF));
        let centre = Colormap::new(Some(&json!({})), only(gradient())).unwrap();
        assert_eq!(xy(centre.default_color().unwrap()), (128, 128));
    }

    #[test]
    fn biome_blend_is_on_unless_switched_off() {
        let on = Colormap::new(Some(&json!({})), only(gradient())).unwrap();
        let off = Colormap::new(Some(&json!({"biome_blend": false})), only(gradient())).unwrap();
        assert!(on.blends());
        assert!(!off.blends());
    }

    #[test]
    fn what_cannot_be_read_faithfully_is_refused() {
        let refused = |json: Json| Colormap::new(Some(&json), only(gradient())).is_err();
        assert!(refused(json!({"color_modifier": {"red": "RED * 0.5"}})));
        assert!(refused(json!({"x_axis": "biome_id"})));
        assert!(refused(json!({"y_axis": "legacy_temperature"})));
        assert!(refused(json!({"x_axis": "POS_X +"})));
        assert!(refused(json!({"x_axis": "BIOME"})));
        assert!(refused(json!({"x_axis": ["a"]})));
        assert!(refused(json!([1])));
        assert!(Colormap::new(None, BTreeMap::new()).is_err());
    }

    #[test]
    fn tints_are_read_from_the_end_of_a_file_name() {
        assert_eq!(split_tint("lush_0"), ("lush", 0));
        assert_eq!(split_tint("lush_12"), ("lush", 12));
        assert_eq!(split_tint("lush_-1"), ("lush", -1));
        assert_eq!(split_tint("lush"), ("lush", ALL_TINTS));
        assert_eq!(split_tint("grass_color"), ("grass_color", ALL_TINTS));
        assert_eq!(split_tint("_3"), ("_3", ALL_TINTS));
    }

    #[test]
    fn a_pack_folder_gives_each_colormap_its_id_textures_and_settings() {
        let root = scratch("read");
        let base = root.join("assets/foo/polytone/colormaps");
        std::fs::create_dir_all(base.join("nested")).unwrap();
        let pixel = |r: u8, g: u8, b: u8| png_bytes(1, 1, png::ColorType::Rgb, &[r, g, b]);
        std::fs::write(base.join("lush_0.png"), pixel(1, 0, 0)).unwrap();
        std::fs::write(base.join("lush_1.png"), pixel(0, 2, 0)).unwrap();
        std::fs::write(base.join("lush.json"), r#"{"biome_blend": false}"#).unwrap();
        std::fs::write(base.join("plain.png"), pixel(0, 0, 3)).unwrap();
        std::fs::write(base.join("nested/deep.png"), pixel(4, 4, 4)).unwrap();
        std::fs::write(base.join("orphan.json"), "{}").unwrap();
        std::fs::write(base.join("bad.png"), b"nope").unwrap();
        std::fs::write(base.join("refused.png"), pixel(5, 5, 5)).unwrap();
        std::fs::write(
            base.join("refused.json"),
            r#"{"color_modifier": {"red": "1"}}"#,
        )
        .unwrap();

        let maps = Colormaps::read(&root);
        let ids: Vec<_> = maps.ids().collect();
        assert_eq!(ids, ["foo:lush", "foo:nested/deep", "foo:plain"]);
        let lush = maps.get("foo:lush").unwrap();
        assert!(!lush.blends());
        assert_eq!(lush.sample_biome(0, 0.5, 0.5), Some(0x010000));
        assert_eq!(lush.sample_biome(1, 0.5, 0.5), Some(0x000200));
        assert_eq!(
            maps.get("foo:plain").unwrap().sample_biome(9, 0.2, 0.9),
            Some(3)
        );
        assert_eq!(
            maps.get("foo:nested/deep").unwrap().default_color(),
            Some(0x040404)
        );
        assert!(maps.get("lush").is_none());
        assert!(maps.get("foo:orphan").is_none());
        assert!(maps.get("foo:refused").is_none());
        assert_eq!(maps.len(), 3);
        assert!(!maps.is_empty());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn an_id_without_a_namespace_is_minecrafts() {
        let root = scratch("namespace");
        let base = root.join("assets/minecraft/polytone/colormaps");
        std::fs::create_dir_all(&base).unwrap();
        std::fs::write(
            base.join("sky.png"),
            png_bytes(1, 1, png::ColorType::Rgb, &[7, 7, 7]),
        )
        .unwrap();
        let maps = Colormaps::read(&root);
        assert!(maps.get("sky").is_some());
        assert!(maps.get("minecraft:sky").is_some());
        assert!(maps.get("other:sky").is_none());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_folder_without_packs_reads_nothing() {
        let root = scratch("empty");
        assert!(Colormaps::read(&root).is_empty());
        assert!(Colormaps::read(&root.join("missing")).is_empty());
        let _ = std::fs::remove_dir_all(&root);
    }
}
