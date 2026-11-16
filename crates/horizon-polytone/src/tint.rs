use std::collections::BTreeMap;

use serde_json::Value as Json;

use crate::colormaps::{Colormap, Colormaps, SampleContext, Texture};
use crate::colors::parse_color;

const COLORMAP_KEYS: [&str; 7] = [
    "default_color",
    "x_axis",
    "y_axis",
    "triangular",
    "rounds",
    "biome_blend",
    "color_modifier",
];

#[derive(Debug)]
pub enum ColorSource {
    Flat(u32),
    Linked(String),
    Inline(Colormap),
}

impl ColorSource {
    fn leaf(json: &Json) -> Result<Self, String> {
        match json {
            Json::Number(_) => parse_color(json)
                .map(Self::Flat)
                .ok_or_else(|| format!("{json} is no colour")),
            Json::String(s) => {
                let s = s.trim();
                if s.is_empty() {
                    return Err("an empty colormap id".to_owned());
                }
                if s.starts_with('#') || s.starts_with("0x") || s.starts_with("0X") {
                    return parse_color(json)
                        .map(Self::Flat)
                        .ok_or_else(|| format!("{s:?} is no colour"));
                }
                Ok(Self::Linked(s.to_owned()))
            }
            _ => Err("a colormap is an id, a colour or an object".to_owned()),
        }
    }

    fn inline(json: &Json, textures: &BTreeMap<i32, Texture>) -> Result<Self, String> {
        if textures.is_empty() {
            let only_default = json
                .as_object()
                .is_some_and(|o| o.keys().all(|k| k == "default_color"));
            return match json.get("default_color").and_then(parse_color) {
                Some(color) if only_default => Ok(Self::Flat(color)),
                _ => Err("an inline colormap needs its texture".to_owned()),
            };
        }
        Colormap::new(Some(json), textures.clone()).map(Self::Inline)
    }

    pub fn depends_only_on_biome(&self, maps: &Colormaps) -> bool {
        match self {
            Self::Flat(_) => true,
            Self::Linked(id) => maps.get(id).is_some_and(Colormap::depends_only_on_biome),
            Self::Inline(map) => map.depends_only_on_biome(),
        }
    }

    pub fn biome_color(
        &self,
        index: i32,
        maps: &Colormaps,
        temperature: f64,
        downfall: f64,
    ) -> Option<u32> {
        let color = match self {
            Self::Flat(color) => *color,
            Self::Linked(id) => maps.get(id)?.sample_biome(index, temperature, downfall)?,
            Self::Inline(map) => map.sample_biome(index, temperature, downfall)?,
        };
        Some(color & 0xFF_FFFF)
    }

    pub fn color(&self, index: i32, maps: &Colormaps, at: &SampleContext) -> Option<u32> {
        let color = match self {
            Self::Flat(color) => *color,
            Self::Linked(id) => maps.get(id)?.sample(index, at)?,
            Self::Inline(map) => map.sample(index, at)?,
        };
        Some(color & 0xFF_FFFF)
    }
}

#[derive(Debug)]
pub enum Tint {
    All(ColorSource),
    Indexed(BTreeMap<i32, ColorSource>),
}

impl Tint {
    pub fn parse(json: &Json, textures: &BTreeMap<i32, Texture>) -> Result<Self, String> {
        match json {
            Json::Object(o) if o.keys().any(|k| COLORMAP_KEYS.contains(&k.as_str())) => {
                ColorSource::inline(json, textures).map(Self::All)
            }
            Json::Object(o) => {
                if o.is_empty() {
                    return Err("an empty colormap".to_owned());
                }
                let mut sources = BTreeMap::new();
                for (key, value) in o {
                    let index: i32 = key.parse().map_err(|_| {
                        format!("{key:?} is neither a colormap setting nor a tint index")
                    })?;
                    sources.insert(index, ColorSource::leaf(value)?);
                }
                Ok(Self::Indexed(sources))
            }
            other => ColorSource::leaf(other).map(Self::All),
        }
    }

    pub fn source(&self, index: i32) -> Option<&ColorSource> {
        match self {
            Self::All(source) => Some(source),
            Self::Indexed(sources) => sources.get(&index),
        }
    }

    pub fn depends_only_on_biome(&self, maps: &Colormaps) -> bool {
        match self {
            Self::All(source) => source.depends_only_on_biome(maps),
            Self::Indexed(sources) => sources.values().all(|s| s.depends_only_on_biome(maps)),
        }
    }

    pub fn biome_color(
        &self,
        index: i32,
        maps: &Colormaps,
        temperature: f64,
        downfall: f64,
    ) -> Option<u32> {
        self.source(index)?
            .biome_color(index, maps, temperature, downfall)
    }

    pub fn color(&self, index: i32, maps: &Colormaps, at: &SampleContext) -> Option<u32> {
        self.source(index)?.color(index, maps, at)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::colormaps::ALL_TINTS;
    use serde_json::json;

    fn gradient() -> BTreeMap<i32, Texture> {
        let pixels = (0..256u32)
            .flat_map(|y| (0..256u32).map(move |x| x << 16 | y << 8))
            .collect();
        BTreeMap::from([(ALL_TINTS, Texture::new(256, 256, pixels).unwrap())])
    }

    fn none() -> BTreeMap<i32, Texture> {
        BTreeMap::new()
    }

    fn maps() -> Colormaps {
        Colormaps::default()
    }

    #[test]
    fn a_number_or_hex_string_is_one_flat_colour_for_every_tint_index() {
        for json in [json!(77777), json!("#012CD1"), json!("0x012CD1")] {
            let tint = Tint::parse(&json, &none()).unwrap();
            let expected = if json.is_number() { 77777 } else { 0x012CD1 };
            for index in [-1, 0, 1, 9] {
                assert_eq!(tint.biome_color(index, &maps(), 0.5, 0.5), Some(expected));
            }
            assert!(tint.depends_only_on_biome(&maps()));
        }
    }

    #[test]
    fn flat_colours_lose_their_alpha_byte() {
        let tint = Tint::parse(&json!("#80FF8040"), &none()).unwrap();
        assert_eq!(tint.biome_color(0, &maps(), 0.0, 0.0), Some(0xFF8040));
    }

    #[test]
    fn another_string_links_a_colormap_by_id() {
        let tint = Tint::parse(&json!("foo:lush"), &none()).unwrap();
        assert!(matches!(
            tint.source(0),
            Some(ColorSource::Linked(id)) if id == "foo:lush"
        ));
        assert_eq!(tint.biome_color(0, &maps(), 0.5, 0.5), None);
        assert!(!tint.depends_only_on_biome(&maps()));
    }

    #[test]
    fn a_compound_colormap_is_keyed_by_tint_index() {
        let tint = Tint::parse(&json!({"0": 255, "1": "#00FF00", "3": "foo:x"}), &none()).unwrap();
        assert_eq!(tint.biome_color(0, &maps(), 0.0, 0.0), Some(255));
        assert_eq!(tint.biome_color(1, &maps(), 0.0, 0.0), Some(0x00FF00));
        assert!(matches!(tint.source(3), Some(ColorSource::Linked(_))));
        assert!(tint.source(2).is_none());
        assert_eq!(tint.biome_color(2, &maps(), 0.0, 0.0), None);
    }

    #[test]
    fn an_inline_colormap_with_a_texture_samples_it() {
        let tint = Tint::parse(&json!({"x_axis": "TEMPERATURE"}), &gradient()).unwrap();
        assert!(tint.depends_only_on_biome(&maps()));
        let c = tint.biome_color(0, &maps(), 1.0, 1.0).unwrap();
        assert_eq!((c >> 16 & 255, c >> 8 & 255), (0, 0));
        let c = tint.biome_color(0, &maps(), 0.0, 0.0).unwrap();
        assert_eq!((c >> 16 & 255, c >> 8 & 255), (255, 255));
    }

    #[test]
    fn an_inline_colormap_reading_the_position_is_not_biome_only() {
        let tint = Tint::parse(&json!({"x_axis": "POS_X / 256"}), &gradient()).unwrap();
        assert!(!tint.depends_only_on_biome(&maps()));
        assert_eq!(tint.biome_color(0, &maps(), 0.5, 0.5), None);
        let at = SampleContext {
            x: 128.0,
            ..SampleContext::default()
        };
        assert!(tint.color(0, &maps(), &at).is_some());
    }

    #[test]
    fn an_inline_colormap_with_only_a_default_colour_is_flat() {
        let tint = Tint::parse(&json!({"default_color": "#336699"}), &none()).unwrap();
        assert_eq!(tint.biome_color(0, &maps(), 0.9, 0.1), Some(0x336699));
    }

    #[test]
    fn an_inline_colormap_without_a_texture_is_refused() {
        assert!(Tint::parse(&json!({"x_axis": "TEMPERATURE"}), &none()).is_err());
        assert!(Tint::parse(&json!({"default_color": 1, "y_axis": "DOWNFALL"}), &none()).is_err());
    }

    #[test]
    fn unreadable_colormaps_are_refused() {
        for json in [
            json!({}),
            json!({"x": 1}),
            json!({"0": [1]}),
            json!(true),
            json!(null),
            json!([1]),
            json!(""),
            json!(1.5),
            json!({"x_axis": "temperature", "color_modifier": {"red": "1"}}),
        ] {
            let textures = if json.get("x_axis").is_some() {
                gradient()
            } else {
                none()
            };
            assert!(Tint::parse(&json, &textures).is_err(), "{json}");
        }
    }
}
