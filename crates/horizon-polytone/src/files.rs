use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use serde_json::Value as Json;

use crate::colormaps::{ALL_TINTS, Texture, split_tint};
use crate::target::{implicit_id, parse_id};

pub(crate) struct Entry {
    pub label: String,
    pub id: Option<String>,
    pub json: Option<Json>,
    pub dir: PathBuf,
    pub stem: String,
}

fn sorted(dir: &Path) -> Vec<PathBuf> {
    let mut paths: Vec<_> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .collect();
    paths.sort();
    paths
}

fn base_name(stem: &str) -> &str {
    stem.strip_suffix("_bar")
        .unwrap_or_else(|| split_tint(stem).0)
}

fn walk(base: &Path, namespace: &str, kind: &str, root: &Path, out: &mut Vec<Entry>) {
    let mut stack = vec![base.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let paths = sorted(&dir);
        let mut with_json = BTreeSet::new();
        let mut png_only = BTreeSet::new();
        for path in &paths {
            if path.is_dir() {
                stack.push(path.clone());
                continue;
            }
            let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
                continue;
            };
            match path.extension().and_then(|e| e.to_str()) {
                Some("json") => {
                    with_json.insert(stem.to_owned());
                }
                Some("png") => {
                    png_only.insert(base_name(stem).to_owned());
                }
                _ => {}
            }
        }
        for stem in &with_json {
            let path = dir.join(format!("{stem}.json"));
            let label = path
                .strip_prefix(root)
                .unwrap_or(&path)
                .display()
                .to_string();
            let json = match std::fs::read_to_string(&path)
                .map_err(|e| e.to_string())
                .and_then(|t| serde_json::from_str::<Json>(&t).map_err(|e| e.to_string()))
            {
                Ok(json) => json,
                Err(e) => {
                    log::warn!("{label}: {e}");
                    continue;
                }
            };
            out.push(Entry {
                id: implicit_id(&path.to_string_lossy(), kind),
                label,
                json: Some(json),
                dir: dir.clone(),
                stem: stem.clone(),
            });
        }
        for stem in png_only.difference(&with_json) {
            let relative = dir.strip_prefix(base).unwrap_or(Path::new(""));
            let folder = relative.to_string_lossy().replace('\\', "/");
            let folder = if folder.is_empty() {
                folder
            } else {
                format!("{folder}/")
            };
            out.push(Entry {
                label: format!("{kind}/{folder}{stem}.png"),
                id: parse_id(&format!("{namespace}:{folder}{stem}")),
                json: None,
                dir: dir.clone(),
                stem: stem.clone(),
            });
        }
    }
}

pub(crate) fn scan(root: &Path, kind: &str) -> Vec<Entry> {
    let mut namespaces = sorted(&root.join("assets"));
    namespaces.retain(|p| p.is_dir());
    let mut out = Vec::new();
    for namespace in namespaces {
        let name = namespace
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        walk(
            &namespace.join("polytone").join(kind),
            &name,
            kind,
            root,
            &mut out,
        );
    }
    out
}

pub(crate) fn textures(dir: &Path, stem: &str) -> Result<BTreeMap<i32, Texture>, String> {
    let mut found = BTreeMap::new();
    for path in sorted(dir) {
        if path.extension().and_then(|e| e.to_str()) != Some("png") {
            continue;
        }
        let Some(file) = path.file_stem().and_then(|s| s.to_str()) else {
            continue;
        };
        let tint = if file == stem {
            ALL_TINTS
        } else {
            match split_tint(file) {
                (name, tint) if name == stem && tint != ALL_TINTS => tint,
                _ => continue,
            }
        };
        let bytes = std::fs::read(&path).map_err(|e| e.to_string())?;
        let texture = Texture::decode_png(&bytes).map_err(|e| format!("{file}.png: {e}"))?;
        found.insert(tint, texture);
    }
    Ok(found)
}

#[cfg(test)]
pub(crate) mod testing {
    use std::path::{Path, PathBuf};

    pub fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("polytone-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    pub fn png(width: u32, height: u32, rgb: &[u8]) -> Vec<u8> {
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

    pub fn pixel(r: u8, g: u8, b: u8) -> Vec<u8> {
        png(1, 1, &[r, g, b])
    }

    pub fn put(root: &Path, relative: &str, bytes: impl AsRef<[u8]>) {
        let path = root.join(relative);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, bytes).unwrap();
    }
}

#[cfg(test)]
mod tests {
    use super::testing::*;
    use super::*;

    #[test]
    fn json_files_png_only_groups_and_bar_textures_become_entries() {
        let root = scratch("scan");
        let kind = "assets/foo/polytone/item_modifiers";
        put(&root, &format!("{kind}/a.json"), "{}");
        put(&root, &format!("{kind}/a.png"), pixel(1, 1, 1));
        put(&root, &format!("{kind}/a_bar.png"), pixel(2, 2, 2));
        put(&root, &format!("{kind}/b_0.png"), pixel(3, 3, 3));
        put(&root, &format!("{kind}/b_1.png"), pixel(4, 4, 4));
        put(&root, &format!("{kind}/c_bar.png"), pixel(5, 5, 5));
        put(&root, &format!("{kind}/deep/d.png"), pixel(6, 6, 6));
        put(&root, &format!("{kind}/broken.json"), "{");
        put(&root, &format!("{kind}/notes.txt"), "x");
        let entries = scan(&root, "item_modifiers");
        let mut seen: Vec<_> = entries
            .iter()
            .map(|e| (e.id.clone().unwrap(), e.json.is_some()))
            .collect();
        seen.sort();
        assert_eq!(
            seen,
            [
                ("foo:a".to_owned(), true),
                ("foo:b".to_owned(), false),
                ("foo:c".to_owned(), false),
                ("foo:deep/d".to_owned(), false),
            ]
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn textures_pick_the_stem_and_its_tint_variants_only() {
        let root = scratch("textures");
        put(&root, "lush.png", pixel(1, 0, 0));
        put(&root, "lush_0.png", pixel(2, 0, 0));
        put(&root, "lush_3.png", pixel(3, 0, 0));
        put(&root, "lush_bar.png", pixel(4, 0, 0));
        put(&root, "lushy.png", pixel(5, 0, 0));
        put(&root, "other_1.png", pixel(6, 0, 0));
        let tints: Vec<_> = textures(&root, "lush").unwrap().into_keys().collect();
        assert_eq!(tints, [-1, 0, 3]);
        let bar = textures(&root, "lush_bar").unwrap();
        assert_eq!(bar.len(), 1);
        assert_eq!(bar[&ALL_TINTS].at(0, 0), 0x040000);
        assert!(textures(&root, "missing").unwrap().is_empty());
        put(&root, "bad.png", b"nope");
        assert!(textures(&root, "bad").is_err());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_folder_without_a_pack_scans_to_nothing() {
        let root = scratch("scan-empty");
        assert!(scan(&root, "block_modifiers").is_empty());
        assert!(scan(&root.join("missing"), "block_modifiers").is_empty());
        let _ = std::fs::remove_dir_all(&root);
    }
}
