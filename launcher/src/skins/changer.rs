//! The skin changer's state and layout, shared by the launcher's gallery and the in-game screen.
//! No frontend or GPU dependency: both draw the same rectangles from the same wardrobe on disk.
use super::{
    Model,
    library::{Entry, Kind, Library},
    preview::{self, Camera},
    read_cape, read_file, read_skin,
};
use image::RgbaImage;
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::Arc,
};

pub type Run = ([f32; 4], [u8; 4]);
pub type Runs = Arc<Vec<Run>>;

pub const THUMB: [u32; 2] = [64, 96];
pub const STAGE: [u32; 2] = [160, 240];
const THUMB_CAMERA: Camera = Camera {
    yaw: -25.0,
    pitch: -8.0,
    zoom: 1.0,
};

pub fn runs(image: &RgbaImage) -> Vec<Run> {
    let mut out = Vec::new();
    for y in 0..image.height() {
        let mut x = 0;
        while x < image.width() {
            let color = image.get_pixel(x, y).0;
            let start = x;
            x += 1;
            while x < image.width() && image.get_pixel(x, y).0 == color {
                x += 1;
            }
            if color[3] > 0 {
                out.push(([start as f32, y as f32, (x - start) as f32, 1.0], color));
            }
        }
    }
    out
}

pub fn thumbnail(skin: &RgbaImage, model: Model) -> Runs {
    Arc::new(runs(&preview::render(
        skin,
        None,
        model,
        THUMB_CAMERA,
        THUMB,
    )))
}

pub fn columns(width: f32, tile_w: f32, gap: f32) -> usize {
    if !(width.is_finite() && tile_w > 0.0) {
        return 1;
    }
    (((width + gap) / (tile_w + gap)).floor() as usize).max(1)
}

pub fn tile_rect(
    index: usize,
    columns: usize,
    tile: [f32; 2],
    gap: f32,
    origin: [f32; 2],
    scroll: f32,
) -> [f32; 4] {
    let columns = columns.max(1);
    let (row, column) = (index / columns, index % columns);
    [
        origin[0] + column as f32 * (tile[0] + gap),
        origin[1] + row as f32 * (tile[1] + gap) - scroll,
        tile[0],
        tile[1],
    ]
}

pub fn tile_at(
    point: [f32; 2],
    count: usize,
    columns: usize,
    tile: [f32; 2],
    gap: f32,
    origin: [f32; 2],
    scroll: f32,
) -> Option<usize> {
    let columns = columns.max(1);
    let x = point[0] - origin[0];
    let y = point[1] - origin[1] + scroll;
    if x < 0.0 || y < 0.0 {
        return None;
    }
    let (column, row) = (
        (x / (tile[0] + gap)).floor() as usize,
        (y / (tile[1] + gap)).floor() as usize,
    );
    let inside =
        x - column as f32 * (tile[0] + gap) < tile[0] && y - row as f32 * (tile[1] + gap) < tile[1];
    let index = row * columns + column;
    (column < columns && inside && index < count).then_some(index)
}

pub fn max_scroll(count: usize, columns: usize, tile_h: f32, gap: f32, viewport_h: f32) -> f32 {
    let rows = count.div_ceil(columns.max(1));
    if rows == 0 {
        return 0.0;
    }
    let content = rows as f32 * (tile_h + gap) - gap;
    (content - viewport_h).max(0.0)
}

pub const LINK_FILE: &str = "horizon-wardrobe.json";

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Link {
    pub schema: u32,
    pub root: PathBuf,
    pub key: String,
    pub skin: Option<PathBuf>,
    pub slim: bool,
    pub cape: Option<PathBuf>,
}

impl Link {
    pub fn new(root: &Path, key: &str, cloud: Option<(&Path, bool, Option<&Path>)>) -> Self {
        Self {
            schema: 1,
            root: root.to_owned(),
            key: key.to_owned(),
            skin: cloud.map(|(p, _, _)| p.to_owned()),
            slim: cloud.is_some_and(|(_, s, _)| s),
            cape: cloud.and_then(|(_, _, c)| c.map(Path::to_owned)),
        }
    }

    pub fn write(&self, home: &Path) -> Result<(), String> {
        let bytes = serde_json::to_vec(self).map_err(|_| "Could not encode the wardrobe link.")?;
        std::fs::write(home.join(LINK_FILE), bytes)
            .map_err(|_| "Could not write the wardrobe link.".to_string())
    }

    pub fn read(home: &Path) -> Option<Self> {
        use std::io::Read;
        let mut bytes = Vec::new();
        std::fs::File::open(home.join(LINK_FILE))
            .ok()?
            .take(65537)
            .read_to_end(&mut bytes)
            .ok()?;
        if bytes.len() > 65536 {
            return None;
        }
        let link: Self = serde_json::from_slice(&bytes).ok()?;
        (link.schema == 1 && link.root.is_absolute() && link.root.join("wardrobe.json").is_file())
            .then_some(link)
    }

    pub fn cloud(&self) -> Option<(&Path, bool, Option<&Path>)> {
        self.skin
            .as_deref()
            .map(|skin| (skin, self.slim, self.cape.as_deref()))
    }

    pub fn changer(&self) -> Result<Changer, String> {
        Changer::new(&self.root, &self.key, self.skin.as_deref())
    }

    pub fn publish(&self, changer: &Changer, home: &Path) -> Result<(), String> {
        changer.prepare_game(home, self.cloud())
    }
}

type StageKey = (usize, bool, [u32; 3], Option<String>, bool);

pub struct Changer {
    root: PathBuf,
    key: String,
    library: Library,
    fallback: RgbaImage,
    pub camera: Camera,
    pub selected: usize,
    thumbs: HashMap<(String, bool), Runs>,
    stage: Option<(StageKey, Runs)>,
}

impl Changer {
    pub fn new(root: &Path, key: &str, default_skin: Option<&Path>) -> Result<Self, String> {
        let library = Library::load(root)?;
        let fallback = default_skin
            .and_then(|path| read_file(path).ok())
            .and_then(|bytes| read_skin(&bytes).ok())
            .unwrap_or_else(preview::mannequin);
        let mut value = Self {
            root: root.to_owned(),
            key: key.to_owned(),
            library,
            fallback,
            camera: Camera::default(),
            selected: 0,
            thumbs: HashMap::new(),
            stage: None,
        };
        value.selected = value.active();
        Ok(value)
    }

    pub fn reload(&mut self) -> Result<(), String> {
        let id = self.entry(self.selected).map(|e| e.id.clone());
        self.library = Library::load(&self.root)?;
        self.thumbs.clear();
        self.stage = None;
        self.selected = id
            .and_then(|id| self.skins().position(|e| e.id == id))
            .map_or(0, |i| i + 1);
        Ok(())
    }

    pub fn skins(&self) -> impl Iterator<Item = &Entry> {
        self.library.entries.iter().filter(|e| e.kind == Kind::Skin)
    }

    pub fn count(&self) -> usize {
        1 + self.skins().count()
    }

    pub fn entry(&self, index: usize) -> Option<&Entry> {
        self.skins().nth(index.checked_sub(1)?)
    }

    pub fn title(&self, index: usize) -> &str {
        match index {
            0 => "Default",
            _ => self.entry(index).map_or("", |e| e.title.as_str()),
        }
    }

    pub fn model(&self) -> Model {
        self.library
            .selections
            .get(&self.key)
            .map_or(Model::Classic, |s| s.model)
    }

    pub fn active(&self) -> usize {
        self.library
            .selections
            .get(&self.key)
            .and_then(|s| s.skin.as_deref())
            .and_then(|id| self.skins().position(|e| e.id == id))
            .map_or(0, |i| i + 1)
    }

    pub fn can_remove(&self) -> bool {
        self.selected > 0 && self.entry(self.selected).is_some()
    }

    pub fn select_tile(&mut self, index: usize) {
        if index < self.count() {
            self.selected = index;
        }
    }

    pub fn apply(&mut self) -> Result<(), String> {
        match self.entry(self.selected).map(|e| e.id.clone()) {
            Some(id) => self
                .library
                .select(&self.root, &self.key, &id, Kind::Skin)?,
            None => {
                let selection = self.library.selections.entry(self.key.clone()).or_default();
                selection.skin = None;
                self.library.save(&self.root)?;
            }
        }
        self.stage = None;
        Ok(())
    }

    pub fn remove(&mut self) -> Result<(), String> {
        let Some(id) = self.entry(self.selected).map(|e| e.id.clone()) else {
            return Err("The default skin cannot be removed.".into());
        };
        self.library.remove(&self.root, &id, Kind::Skin)?;
        self.thumbs.retain(|(thumb, _), _| *thumb != id);
        self.stage = None;
        self.selected = self.selected.min(self.count() - 1);
        Ok(())
    }

    pub fn toggle_model(&mut self) -> Result<(), String> {
        let selection = self.library.selections.entry(self.key.clone()).or_default();
        selection.model = match selection.model {
            Model::Classic => Model::Slim,
            Model::Slim => Model::Classic,
        };
        self.library.save(&self.root)?;
        self.stage = None;
        Ok(())
    }

    pub fn prepare_game(
        &self,
        game_home: &Path,
        cloud: Option<(&Path, bool, Option<&Path>)>,
    ) -> Result<(), String> {
        self.library
            .prepare_game(&self.root, &self.key, game_home, cloud)
    }

    fn skin_image(&self, index: usize) -> RgbaImage {
        self.entry(index)
            .and_then(|e| Library::image(&self.root, &e.id).ok())
            .and_then(|path| read_file(&path).ok())
            .and_then(|bytes| read_skin(&bytes).ok())
            .unwrap_or_else(|| self.fallback.clone())
    }

    pub fn thumb(&mut self, index: usize) -> Runs {
        let model = self.model();
        let id = self.entry(index).map(|e| e.id.clone()).unwrap_or_default();
        let slim = model == Model::Slim;
        if let Some(found) = self.thumbs.get(&(id.clone(), slim)) {
            return found.clone();
        }
        let value = thumbnail(&self.skin_image(index), model);
        self.thumbs.insert((id, slim), value.clone());
        value
    }

    pub fn stage(&mut self) -> Runs {
        let camera = self.camera;
        let model = self.model();
        let selection = self.library.selections.get(&self.key);
        let cape_id = selection
            .filter(|s| !s.hide_cape)
            .and_then(|s| s.cape.clone());
        let key: StageKey = (
            self.selected,
            model == Model::Slim,
            [camera.yaw, camera.pitch, camera.zoom].map(f32::to_bits),
            cape_id.clone(),
            self.entry(self.selected).is_some(),
        );
        if let Some((cached, value)) = &self.stage {
            if *cached == key {
                return value.clone();
            }
        }
        let cape = cape_id
            .and_then(|id| Library::image(&self.root, &id).ok())
            .and_then(|path| read_file(&path).ok())
            .and_then(|bytes| read_cape(&bytes).ok());
        let image = preview::render(
            &self.skin_image(self.selected),
            cape.as_ref(),
            model,
            camera,
            STAGE,
        );
        let value = Arc::new(runs(&image));
        self.stage = Some((key, value.clone()));
        value
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn png(color: [u8; 4]) -> Vec<u8> {
        let mut out = std::io::Cursor::new(vec![]);
        RgbaImage::from_pixel(64, 64, image::Rgba(color))
            .write_to(&mut out, image::ImageFormat::Png)
            .unwrap();
        out.into_inner()
    }

    fn scratch(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("horizon-changer-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    fn changer(root: &Path) -> (Changer, Vec<String>) {
        let mut library = Library::default();
        let ids = [[200, 30, 30, 255], [30, 30, 200, 255], [30, 200, 30, 255]]
            .iter()
            .enumerate()
            .map(|(i, c)| {
                library
                    .import(root, &format!("Skin {i}"), Kind::Skin, &png(*c))
                    .unwrap()
            })
            .collect();
        (Changer::new(root, "offline:me", None).unwrap(), ids)
    }

    #[test]
    fn the_default_tile_comes_first_and_cannot_be_removed() {
        let root = scratch("default");
        let (mut changer, _) = changer(&root);
        assert_eq!(changer.count(), 4);
        assert_eq!(changer.title(0), "Default");
        assert_eq!(changer.title(2), "Skin 1");
        assert_eq!(changer.selected, 0);
        assert!(!changer.can_remove());
        assert!(changer.remove().is_err());
        assert_eq!(changer.count(), 4);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn applying_a_tile_reaches_the_games_handoff_and_the_default_tile_clears_it() {
        let root = scratch("apply");
        let (mut changer, ids) = changer(&root);
        let game = root.join("game");
        let handoff = |changer: &Changer| {
            changer.prepare_game(&game, None).unwrap();
            serde_json::from_slice::<serde_json::Value>(
                &std::fs::read(game.join("horizon-appearance.json")).unwrap(),
            )
            .unwrap()
        };
        assert!(handoff(&changer)["skin"].is_null());

        changer.select_tile(2);
        assert_eq!(changer.active(), 0);
        changer.apply().unwrap();
        assert_eq!(changer.active(), 2);
        let expected = Library::image(&root, &ids[1]).unwrap();
        assert_eq!(handoff(&changer)["skin"].as_str(), expected.to_str(),);
        let reopened = Changer::new(&root, "offline:me", None).unwrap();
        assert_eq!(reopened.active(), 2);
        assert_eq!(reopened.selected, 2);
        let other = Changer::new(&root, "offline:someone", None).unwrap();
        assert_eq!(other.active(), 0);

        changer.select_tile(0);
        changer.apply().unwrap();
        assert_eq!(changer.active(), 0);
        assert!(handoff(&changer)["skin"].is_null());
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn removing_the_applied_skin_falls_back_to_default_and_keeps_a_valid_selection() {
        let root = scratch("remove");
        let (mut changer, ids) = changer(&root);
        changer.select_tile(3);
        changer.apply().unwrap();
        assert!(changer.can_remove());
        changer.remove().unwrap();
        assert_eq!(changer.count(), 3);
        assert_eq!(changer.active(), 0);
        assert_eq!(changer.selected, 2);
        assert_eq!(changer.title(2), "Skin 1");
        assert!(!Library::image(&root, &ids[2]).unwrap().exists());
        changer.remove().unwrap();
        changer.remove().unwrap();
        assert_eq!((changer.count(), changer.selected), (1, 0));
        assert!(changer.remove().is_err());
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn thumbnails_show_each_skin_and_the_stage_follows_the_camera_and_model() {
        let root = scratch("preview");
        let (mut changer, _) = changer(&root);
        let color = |runs: &Runs| runs.iter().map(|r| r.1).find(|c| c[3] == 255).unwrap();
        let (red, blue) = (changer.thumb(1), changer.thumb(2));
        assert!(!red.is_empty() && !blue.is_empty());
        assert_ne!(color(&red), color(&blue));
        assert!(Arc::ptr_eq(&red, &changer.thumb(1)));
        assert!(
            red.iter()
                .all(|(r, _)| r[0] >= 0.0 && r[0] + r[2] <= THUMB[0] as f32)
        );

        changer.select_tile(1);
        let front = changer.stage();
        assert!(Arc::ptr_eq(&front, &changer.stage()));
        changer.camera.rotate(90.0, 0.0);
        let turned = changer.stage();
        assert_ne!(*front, *turned);

        let classic = changer.thumb(1);
        changer.toggle_model().unwrap();
        assert_eq!(changer.model(), Model::Slim);
        assert_ne!(*classic, *changer.thumb(1));
        changer.toggle_model().unwrap();
        assert_eq!(changer.model(), Model::Classic);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn reload_keeps_the_same_skin_selected_when_the_list_changed_underneath() {
        let root = scratch("reload");
        let (mut changer, ids) = changer(&root);
        changer.select_tile(3);
        let mut other = Library::load(&root).unwrap();
        other.remove(&root, &ids[0], Kind::Skin).unwrap();
        changer.reload().unwrap();
        assert_eq!(changer.selected, 2);
        assert_eq!(changer.entry(2).unwrap().id, ids[2]);
        other.remove(&root, &ids[2], Kind::Skin).unwrap();
        changer.reload().unwrap();
        assert_eq!(changer.selected, 0);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn the_grid_places_tiles_by_column_and_hits_only_their_pixels() {
        let (tile, gap, origin) = ([50.0, 70.0], 6.0, [10.0, 20.0]);
        assert_eq!(columns(10.0, 50.0, 6.0), 1);
        assert_eq!(columns(162.0, 50.0, 6.0), 3);
        assert_eq!(columns(161.5, 50.0, 6.0), 2);
        assert_eq!(columns(f32::NAN, 50.0, 6.0), 1);
        assert_eq!(
            tile_rect(4, 3, tile, gap, origin, 0.0),
            [66.0, 96.0, 50.0, 70.0]
        );
        assert_eq!(tile_rect(4, 3, tile, gap, origin, 30.0)[1], 66.0);

        let at = |p: [f32; 2], count, scroll| tile_at(p, count, 3, tile, gap, origin, scroll);
        assert_eq!(at([11.0, 21.0], 7, 0.0), Some(0));
        assert_eq!(at([66.0, 96.0], 7, 0.0), Some(4));
        assert_eq!(at([115.9, 165.9], 7, 0.0), Some(4));
        assert_eq!(at([60.0, 30.0], 7, 0.0), None);
        assert_eq!(at([66.0, 91.0], 7, 0.0), None);
        assert_eq!(at([5.0, 30.0], 7, 0.0), None);
        assert_eq!(at([180.0, 30.0], 7, 0.0), None);
        assert_eq!(at([10.0, 20.0 + 2.0 * 76.0], 7, 0.0), Some(6));
        assert_eq!(at([66.0, 20.0 + 2.0 * 76.0], 7, 0.0), None);
        assert_eq!(at([66.0, 66.0], 7, 30.0), Some(4));
        for index in 0..7 {
            let r = tile_rect(index, 3, tile, gap, origin, 13.0);
            let centre = [r[0] + r[2] / 2.0, r[1] + r[3] / 2.0];
            assert_eq!(at(centre, 7, 13.0), Some(index));
        }
    }

    #[test]
    fn scrolling_stops_at_the_last_row() {
        assert_eq!(max_scroll(0, 3, 70.0, 6.0, 100.0), 0.0);
        assert_eq!(max_scroll(3, 3, 70.0, 6.0, 100.0), 0.0);
        assert_eq!(max_scroll(7, 3, 70.0, 6.0, 100.0), 3.0 * 76.0 - 6.0 - 100.0);
        assert_eq!(max_scroll(10, 3, 70.0, 6.0, 1000.0), 0.0);
    }

    #[test]
    fn the_link_round_trips_and_ignores_a_missing_or_foreign_wardrobe() {
        let root = scratch("link");
        let (changer, ids) = changer(&root);
        let home = root.join("game");
        std::fs::create_dir_all(&home).unwrap();
        assert!(Link::read(&home).is_none());
        let cloud = root.join("cloud.png");
        let link = Link::new(&root, "offline:me", Some((&cloud, true, None)));
        link.write(&home).unwrap();
        assert_eq!(Link::read(&home), Some(link.clone()));
        assert_eq!(link.cloud(), Some((cloud.as_path(), true, None)));
        assert!(Link::new(&root, "offline:me", None).cloud().is_none());

        let mut changed = link.changer().unwrap();
        changed.select_tile(1);
        changed.apply().unwrap();
        link.publish(&changed, &home).unwrap();
        let handoff: serde_json::Value =
            serde_json::from_slice(&std::fs::read(home.join("horizon-appearance.json")).unwrap())
                .unwrap();
        assert_eq!(
            handoff["skin"].as_str(),
            Library::image(&root, &ids[0]).unwrap().to_str()
        );
        let _ = changer;

        std::fs::write(home.join(LINK_FILE), b"{\"schema\":2}").unwrap();
        assert!(Link::read(&home).is_none());
        let mut relative = link.clone();
        relative.root = PathBuf::from("wardrobe");
        relative.write(&home).unwrap();
        assert!(Link::read(&home).is_none());
        let mut missing = link;
        missing.root = root.join("absent");
        missing.write(&home).unwrap();
        assert!(Link::read(&home).is_none());
        std::fs::remove_dir_all(root).unwrap();
    }
}
