//! Options > Skin Customization > Skin Changer: a grid of the launcher wardrobe's skins on a
//! translucent black panel, a draggable 3D preview beside it, and Apply / Remove / Arms stacked
//! under the preview. It reads and writes the same wardrobe the launcher's Skins tab uses
//! (`horizon-wardrobe.json` in the game folder points at it), and applies the pick to the local
//! player at once.
use super::Game;
#[cfg(target_os = "android")]
use super::Screen;
#[cfg(target_os = "android")]
use crate::skins::changer::{self, Changer, Link};
#[cfg(target_os = "android")]
use horizon_client::{hud::HudBatch, text::Text, ui};

#[derive(Default)]
pub(super) struct SkinChangerUi {
    #[cfg(target_os = "android")]
    state: Option<State>,
    #[cfg(target_os = "android")]
    error: Option<String>,
    #[cfg(target_os = "android")]
    from_title: bool,
    #[cfg(target_os = "android")]
    link_ok: Option<bool>,
    #[cfg(target_os = "android")]
    title_spin: f32,
    #[cfg(target_os = "android")]
    title_grab: Option<f32>,
}

#[cfg(target_os = "android")]
struct State {
    changer: Changer,
    link: Link,
    scroll: f32,
    grab: Grab,
    note: Option<(String, bool)>,
    spin: f32,
}

#[cfg(target_os = "android")]
#[derive(Clone, Copy)]
enum Grab {
    None,
    Stage {
        last: (f32, f32),
    },
    Grid {
        start: (f32, f32),
        scroll: f32,
        moved: bool,
    },
}

#[cfg(target_os = "android")]
struct ModelJob {
    rect: [f32; 4],
    clip: [f32; 4],
    skin: Option<(String, std::sync::Arc<Vec<u8>>)>,
    slim: bool,
    cape: Option<(String, std::sync::Arc<Vec<u8>>)>,
    yaw: f32,
    zoom: f32,
}

#[cfg(target_os = "android")]
fn overlap(a: [f32; 4], b: [f32; 4]) -> [f32; 4] {
    let x0 = a[0].max(b[0]);
    let y0 = a[1].max(b[1]);
    let x1 = (a[0] + a[2]).min(b[0] + b[2]);
    let y1 = (a[1] + a[3]).min(b[1] + b[3]);
    [x0, y0, (x1 - x0).max(0.0), (y1 - y0).max(0.0)]
}

#[cfg(target_os = "android")]
const HEADER: f32 = 24.0;
#[cfg(target_os = "android")]
const FOOTER: f32 = 30.0;
#[cfg(target_os = "android")]
const MARGIN: f32 = 8.0;
#[cfg(target_os = "android")]
const TILE: [f32; 2] = [58.0, 90.0];
#[cfg(target_os = "android")]
const TILE_GAP: f32 = 4.0;
#[cfg(target_os = "android")]
const THUMB_SCALE: f32 = 0.75;
#[cfg(target_os = "android")]
const BUTTON: f32 = 20.0;
#[cfg(target_os = "android")]
const BUTTON_GAP: f32 = 2.0;
#[cfg(target_os = "android")]
const PANEL: u32 = 0x8C00_0000;
#[cfg(target_os = "android")]
const PANEL_EDGE: u32 = 0xFF3A_3A3A;
#[cfg(target_os = "android")]
const MOVE_SLOP: f32 = 4.0;

#[cfg(target_os = "android")]
struct Layout {
    grid: [f32; 4],
    side: [f32; 4],
    stage: [f32; 4],
    buttons: [[f32; 4]; 3],
    done: [f32; 4],
    columns: usize,
}

#[cfg(target_os = "android")]
impl Layout {
    fn new(w: f32, h: f32) -> Self {
        let side_w = (w * 0.38).clamp(150.0, 210.0);
        let top = HEADER;
        let height = (h - HEADER - FOOTER).max(60.0);
        let side = [w - side_w - MARGIN, top, side_w, height];
        let grid = [MARGIN, top, (side[0] - 2.0 * MARGIN).max(TILE[0]), height];
        let stack = 3.0 * BUTTON + 2.0 * BUTTON_GAP;
        let button_x = side[0] + 6.0;
        let button_w = side_w - 12.0;
        let first = top + height - 6.0 - stack;
        let buttons = std::array::from_fn(|i| {
            [
                button_x,
                first + i as f32 * (BUTTON + BUTTON_GAP),
                button_w,
                BUTTON,
            ]
        });
        let stage = [
            side[0] + 6.0,
            top + 6.0,
            button_w,
            (first - top - 6.0 - 14.0).max(40.0),
        ];
        let done = [((w - 200.0) / 2.0).floor(), h - FOOTER + 5.0, 200.0, BUTTON];
        let columns = changer::columns(grid[2] - 10.0, TILE[0], TILE_GAP);
        Self {
            grid,
            side,
            stage,
            buttons,
            done,
            columns,
        }
    }

    fn origin(&self) -> [f32; 2] {
        [self.grid[0] + 4.0, self.grid[1] + 4.0]
    }

    fn view_height(&self) -> f32 {
        self.grid[3] - 8.0
    }
}

#[cfg(target_os = "android")]
fn inside(point: (f32, f32), rect: [f32; 4]) -> bool {
    ui::hovered(point, rect[0], rect[1], rect[2], rect[3])
}

#[cfg(target_os = "android")]
fn frame(
    batch: &mut HudBatch,
    atlas: &horizon_client::hud::HudAtlas,
    rect: [f32; 4],
    fill: u32,
    edge: u32,
) {
    batch.fill(atlas, rect[0], rect[1], rect[2], rect[3], edge);
    batch.fill(
        atlas,
        rect[0] + 1.0,
        rect[1] + 1.0,
        rect[2] - 2.0,
        rect[3] - 2.0,
        fill,
    );
}

#[cfg(target_os = "android")]
fn fit(atlas: &horizon_client::hud::HudAtlas, text: &str, width: f32) -> Text {
    let mut shown = text.to_owned();
    while shown.chars().count() > 1 && atlas.font.width_text(&Text::literal(shown.as_str())) > width
    {
        shown.pop();
    }
    if shown.len() < text.len() {
        shown.pop();
        shown.push('…');
    }
    Text::literal(shown)
}

#[cfg(target_os = "android")]
impl Game<'_> {
    pub(super) fn open_skin_changer(&mut self) {
        let home = std::path::Path::new(".");
        let opened = Link::read(home)
            .ok_or_else(|| {
                "Open the game from the launcher once so it can find your skins.".to_owned()
            })
            .and_then(|link| link.changer().map(|changer| (link, changer)));
        match opened {
            Ok((link, changer)) => {
                self.skin_changer_ui.error = None;
                self.skin_changer_ui.state = Some(State {
                    changer,
                    link,
                    scroll: 0.0,
                    grab: Grab::None,
                    note: None,
                    spin: 0.0,
                });
            }
            Err(error) => {
                log::warn!("Horizon: skin changer: {error}");
                self.skin_changer_ui.state = None;
                self.skin_changer_ui.error = Some(error);
            }
        }
        self.dragging = false;
        self.skin_changer_ui.from_title = self.title.is_some();
        self.screen = Screen::SkinChanger;
    }

    pub(super) fn close_skin_changer(&mut self) {
        self.skin_changer_ui.state = None;
        self.skin_changer_ui.error = None;
        if let Some(title) = &mut self.title {
            title.skin_stage = None;
        }
        self.screen = if self.skin_changer_ui.from_title {
            Screen::Playing
        } else {
            Screen::Options
        };
    }

    pub(super) fn sync_title_skin_preview(&mut self) {
        if self.screen == Screen::SkinChanger {
            return;
        }
        let Some(title) = &self.title else {
            return;
        };
        if title.skin_stage.is_some() {
            return;
        }
        let ok = *self
            .skin_changer_ui
            .link_ok
            .get_or_insert_with(|| Link::read(std::path::Path::new(".")).is_some());
        if !ok {
            return;
        }
        let name = self.options.player_name();
        if let Some(title) = &mut self.title {
            title.skin_stage = Some(name);
        }
    }

    pub(super) fn draw_title_player(&mut self, batch: &mut HudBatch, w: f32, h: f32) {
        let Some(layout) = self
            .title
            .as_ref()
            .filter(|t| {
                t.skin_stage.is_some() && matches!(t.mode, horizon_client::title::Mode::Main)
            })
            .and_then(|_| horizon_client::title::skin_layout(w, h))
        else {
            return;
        };
        self.flush_skin_slots();
        const SKIN: &str = "horizon/offline_skin";
        const CAPE: &str = "capes/local";
        let (mut skin, mut slim, mut cape) = (None, false, None);
        if let Some(appearance) = horizon_client::skins::appearance::current() {
            if let Some((rgba, is_slim)) = &appearance.skin {
                if !self.skin_slots.contains(SKIN) || !self.gui.atlas.sprites.contains_key(SKIN) {
                    self.store_skin(SKIN.into(), rgba.clone());
                }
                skin = Some(SKIN.to_owned());
                slim = *is_slim;
            }
            if !appearance.hide_cape
                && let Some(rgba) = &appearance.cape
            {
                if !self.skin_slots.contains(CAPE) || !self.gui.atlas.sprites.contains_key(CAPE) {
                    self.store_skin(CAPE.into(), rgba.clone());
                }
                cape = Some(CAPE.to_owned());
            }
        }
        let seconds = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0.0, |d| d.as_secs_f32());
        let yaw = 25.0 + self.skin_changer_ui.title_spin;
        let [x0, y0, x1, y1] = layout.model;
        let model = [x0, y0, x1 - x0, y1 - y0];
        self.draw_model(
            batch,
            model,
            model,
            skin.as_deref(),
            slim,
            cape.as_deref(),
            yaw,
            1.0,
        );
    }

    fn model_sprite(&mut self, name: &str, rgba: &[u8]) -> bool {
        if !self.skin_slots.contains(name) || !self.gui.atlas.sprites.contains_key(name) {
            self.store_skin(name.to_owned(), rgba.to_vec());
        }
        if self.gui.atlas.sprites.contains_key(name) {
            self.skin_downloads.borrow_mut().drawn(name);
            true
        } else {
            false
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn draw_model(
        &mut self,
        batch: &mut HudBatch,
        rect: [f32; 4],
        clip: [f32; 4],
        skin: Option<&str>,
        slim: bool,
        cape: Option<&str>,
        yaw: f32,
        zoom: f32,
    ) {
        use horizon_client::mobs::{Mob, render};
        const STEVE: &str = "entity/player/wide/steve";
        let Some(kind) = self.res.player_type().clone() else {
            return;
        };
        let skin = match skin {
            Some(name) => Some(name.to_owned()),
            None if self.gui.atlas.sprites.contains_key(STEVE) => Some(STEVE.to_owned()),
            None => None,
        };
        let mut m = Mob::new(0, kind, horizon_core::geometry::Vec3::ZERO, 0.0);
        m.slim = slim && skin.is_some();
        m.skin = skin;
        m.cape = cape.map(str::to_owned);
        m.extra.model_scale = 0.0;
        let seconds = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0.0, |d| d.as_secs_f32());
        let look = (8.0 * (seconds * 0.45).sin() / 40.0).atan();
        m.age = (seconds * 20.0) as u32;
        m.body_yaw = 180.0 + look * 20.0 + yaw;
        m.head_yaw = m.body_yaw + look * 20.0;
        m.head_pitch = 0.0;
        m.previous = m.position;
        m.previous_body_yaw = m.body_yaw;
        m.previous_head_yaw = m.head_yaw;
        m.previous_head_pitch = m.head_pitch;
        let camera = glam::DVec3::new(m.position.x, m.position.y, m.position.z);
        let atlas = &self.gui.atlas;
        let lookup = |name: &str| atlas.sprites.get(name).map(|r| [r.u0, r.v0, r.u1, r.v1]);
        let mut verts = Vec::new();
        render::render_with(
            &m,
            1.0,
            camera,
            &lookup,
            240 | 240 << 8,
            self.res.mob_data(),
            &self.player_armor_layers(),
            &mut verts,
        );
        self.player_cape_geometry(&m, None, camera, 1.0, 240 | 240 << 8, &mut verts);
        let [x, y, rw, rh] = rect;
        let (cx, cy) = (x + rw / 2.0, y + rh / 2.0);
        let size = ((rh * 0.94 / 1.8).min(rw / 0.9) * zoom).max(1.0);
        let gui = glam::Mat4::from_translation(glam::Vec3::new(cx, cy, 0.0))
            * glam::Mat4::from_scale(glam::Vec3::new(size, size, -size))
            * glam::Mat4::from_translation(glam::Vec3::new(0.0, 0.9 + 0.0625, 0.0))
            * glam::Mat4::from_rotation_z(std::f32::consts::PI);
        let quads = super::perspective::player_preview_quads(verts, gui, None, &atlas.sprites);
        let start = batch.vertices.len();
        for (_, p, uv, color) in quads {
            batch.quad_3d(p, uv, color);
        }
        batch.clip_since(
            start,
            clip[0],
            clip[1],
            clip[0] + clip[2],
            clip[1] + clip[3],
        );
        batch.depth_since(start, [x, y, x + rw, y + rh]);
    }

    fn draw_model_job(&mut self, batch: &mut HudBatch, job: ModelJob) {
        let skin = job
            .skin
            .as_ref()
            .filter(|(name, pixels)| self.model_sprite(name, pixels))
            .map(|(name, _)| name.as_str());
        let cape = job
            .cape
            .as_ref()
            .filter(|(name, pixels)| self.model_sprite(name, pixels))
            .map(|(name, _)| name.as_str());
        self.draw_model(
            batch, job.rect, job.clip, skin, job.slim, cape, job.yaw, job.zoom,
        );
    }

    pub(super) fn rotate_zone_at(&self, p: (f32, f32)) -> bool {
        let (w, h) = self.gui_size();
        if self.screen == Screen::SkinChanger {
            return self.skin_changer_ui.state.is_some() && inside(p, Layout::new(w, h).stage);
        }
        self.title
            .as_ref()
            .filter(|t| {
                t.skin_stage.is_some() && matches!(t.mode, horizon_client::title::Mode::Main)
            })
            .and_then(|_| horizon_client::title::skin_layout(w, h))
            .is_some_and(|layout| {
                let [x0, y0, x1, y1] = layout.model;
                inside(p, [x0, y0, x1 - x0, y1 - y0])
            })
    }

    pub(super) fn title_spin_press(&mut self) -> bool {
        let (w, h) = self.gui_size();
        let over = self
            .title
            .as_ref()
            .filter(|t| {
                t.skin_stage.is_some() && matches!(t.mode, horizon_client::title::Mode::Main)
            })
            .and_then(|_| horizon_client::title::skin_layout(w, h))
            .is_some_and(|layout| {
                let [x0, y0, x1, y1] = layout.model;
                inside(self.cursor, [x0, y0, x1 - x0, y1 - y0])
            });
        if over {
            self.skin_changer_ui.title_grab = Some(self.cursor.0);
        }
        over
    }

    pub(super) fn title_spin_drag(&mut self) {
        if let Some(last) = self.skin_changer_ui.title_grab {
            self.skin_changer_ui.title_spin =
                (self.skin_changer_ui.title_spin - (self.cursor.0 - last) * 2.0).rem_euclid(360.0);
            self.skin_changer_ui.title_grab = Some(self.cursor.0);
        }
    }

    pub(super) fn title_spin_release(&mut self) {
        self.skin_changer_ui.title_grab = None;
    }

    fn publish_skin_changer(&mut self) {
        let Some(state) = &self.skin_changer_ui.state else {
            return;
        };
        let home = std::path::Path::new(".");
        let result = state
            .link
            .publish(&state.changer, home)
            .and_then(|()| horizon_client::skins::appearance::configure(home));
        if let Err(error) = result {
            if let Some(state) = &mut self.skin_changer_ui.state {
                state.note = Some((error, true));
            }
            return;
        }
        if let Some(appearance) = horizon_client::skins::appearance::current() {
            if let Some((rgba, _)) = &appearance.skin
                && self.skin_slots.contains("horizon/offline_skin")
            {
                self.store_skin("horizon/offline_skin".into(), rgba.clone());
            }
            if let Some(rgba) = &appearance.cape
                && self.skin_slots.contains("capes/local")
            {
                self.store_skin("capes/local".into(), rgba.clone());
            }
        }
        self.update_remote_skins();
    }

    fn skin_changer_note(&mut self, note: Result<String, String>) {
        if let Some(state) = &mut self.skin_changer_ui.state {
            state.note = Some(match note {
                Ok(said) => (said, false),
                Err(error) => (error, true),
            });
        }
    }

    pub(super) fn draw_skin_changer(&mut self, batch: &mut HudBatch) {
        self.flush_skin_slots();
        self.update_remote_skins();
        let (w, h) = (batch.width, batch.height);
        let layout = Layout::new(w, h);
        let cursor = self.cursor;
        let atlas = &self.gui.atlas;
        ui::menu_background(batch, atlas, 0.0, 0.0, w, h, true);
        let title = Text::literal("Skin Changer");
        let tw = atlas.font.width_text(&title);
        batch.text_styled(atlas, &title, ((w - tw) / 2.0).floor(), 8.0, u32::MAX, true);
        let done_hover = inside(cursor, layout.done);
        let Some(state) = &mut self.skin_changer_ui.state else {
            let message = self.skin_changer_ui.error.clone().unwrap_or_default();
            let line = fit(atlas, &message, w - 2.0 * MARGIN);
            let lw = atlas.font.width_text(&line);
            batch.text_styled(
                atlas,
                &line,
                ((w - lw) / 2.0).floor(),
                (h / 2.0).floor(),
                0xFFFF_5555,
                true,
            );
            ui::button(
                batch,
                atlas,
                layout.done[0],
                layout.done[1],
                layout.done[2],
                layout.done[3],
                &Text::translate("gui.done", &[]),
                true,
                done_hover,
            );
            return;
        };
        let count = state.changer.count();
        let limit = changer::max_scroll(
            count,
            layout.columns,
            TILE[1],
            TILE_GAP,
            layout.view_height(),
        );
        state.scroll = state.scroll.clamp(0.0, limit);
        let mut jobs: Vec<ModelJob> = Vec::new();
        let grid_inner = [
            layout.grid[0] + 1.0,
            layout.grid[1] + 1.0,
            layout.grid[2] - 2.0,
            layout.grid[3] - 2.0,
        ];

        frame(batch, atlas, layout.grid, PANEL, PANEL_EDGE);
        let first = batch.vertices.len();
        let active = state.changer.active();
        let origin = layout.origin();
        for index in 0..count {
            let rect =
                changer::tile_rect(index, layout.columns, TILE, TILE_GAP, origin, state.scroll);
            if rect[1] + rect[3] < layout.grid[1] || rect[1] > layout.grid[1] + layout.grid[3] {
                continue;
            }
            let selected = index == state.changer.selected;
            let hover = inside(cursor, rect) && inside(cursor, layout.grid);
            let fill = if selected {
                0xB020_2A3A
            } else if hover {
                0x9028_2828
            } else {
                0x7000_0000
            };
            let edge = if selected { 0xFFFF_FFFF } else { 0xFF2A_2A2A };
            frame(batch, atlas, rect, fill, edge);
            let model_rect = [rect[0] + 3.0, rect[1] + 3.0, TILE[0] - 6.0, TILE[1] - 18.0];
            let pixels = if index == 0 {
                None
            } else {
                state.changer.tile_skin(index)
            };
            jobs.push(ModelJob {
                rect: model_rect,
                clip: overlap(model_rect, grid_inner),
                slim: pixels.as_ref().is_some_and(|p| p.2),
                skin: pixels.map(|(id, px, _)| (format!("changer/tile/{id}"), px)),
                cape: None,
                yaw: 25.0,
                zoom: 1.0,
            });
            let name = fit(atlas, state.changer.title(index), TILE[0] - 6.0);
            let nw = atlas.font.width_text(&name);
            batch.text_styled(
                atlas,
                &name,
                rect[0] + ((TILE[0] - nw) / 2.0).floor(),
                rect[1] + TILE[1] - 12.0,
                if index == active {
                    0xFF55_FF55
                } else {
                    0xFFFF_FFFF
                },
                true,
            );
            if index == active {
                batch.fill(
                    atlas,
                    rect[0] + 1.0,
                    rect[1] + TILE[1] - 3.0,
                    TILE[0] - 2.0,
                    2.0,
                    0xFF55_FF55,
                );
            }
        }
        batch.clip_since(
            first,
            layout.grid[0] + 1.0,
            layout.grid[1] + 1.0,
            layout.grid[0] + layout.grid[2] - 1.0,
            layout.grid[1] + layout.grid[3] - 1.0,
        );
        ui::scrollbar(
            batch,
            atlas,
            layout.grid[0] + layout.grid[2] - 7.0,
            layout.grid[1] + 4.0,
            layout.grid[1] + layout.grid[3] - 4.0,
            layout.view_height() + limit,
            state.scroll,
        );

        frame(batch, atlas, layout.side, PANEL, PANEL_EDGE);
        frame(batch, atlas, layout.stage, 0x6000_0000, 0xFF2A_2A2A);
        let stage_rect = [
            layout.stage[0] + 1.0,
            layout.stage[1] + 16.0,
            layout.stage[2] - 2.0,
            (layout.stage[3] - 30.0).max(10.0),
        ];
        let selected_pixels = if state.changer.selected == 0 {
            None
        } else {
            state.changer.tile_skin(state.changer.selected)
        };
        jobs.push(ModelJob {
            rect: stage_rect,
            clip: stage_rect,
            slim: selected_pixels.as_ref().is_some_and(|p| p.2),
            skin: selected_pixels.map(|(id, px, _)| (format!("changer/tile/{id}"), px)),
            cape: state
                .changer
                .stage_cape()
                .map(|(id, px)| (format!("capes/changer/{id}"), px)),
            yaw: 25.0 + state.spin,
            zoom: 1.0,
        });
        let selected_name = fit(
            atlas,
            state.changer.title(state.changer.selected),
            layout.stage[2] - 4.0,
        );
        let nw = atlas.font.width_text(&selected_name);
        batch.text_styled(
            atlas,
            &selected_name,
            layout.stage[0] + ((layout.stage[2] - nw) / 2.0).floor(),
            layout.stage[1] + layout.stage[3] - 11.0,
            u32::MAX,
            true,
        );
        let hint = Text::literal("Drag to rotate");
        let hw = atlas.font.width_text(&hint);
        batch.text_styled(
            atlas,
            &hint,
            layout.stage[0] + ((layout.stage[2] - hw) / 2.0).floor(),
            layout.stage[1] + 4.0,
            0xFF99_9999,
            true,
        );

        let applied = state.changer.selected == active;
        let labels = [
            Text::literal(if applied { "Applied" } else { "Apply" }),
            Text::literal("Remove"),
            Text::literal(match state.changer.model() {
                crate::skins::Model::Classic => "Arms: Classic",
                crate::skins::Model::Slim => "Arms: Slim",
            }),
        ];
        let enabled = [!applied, state.changer.can_remove(), true];
        for ((rect, label), on) in layout.buttons.iter().zip(&labels).zip(enabled) {
            ui::button(
                batch,
                atlas,
                rect[0],
                rect[1],
                rect[2],
                rect[3],
                label,
                on,
                on && inside(cursor, *rect),
            );
        }
        if let Some((note, error)) = &state.note {
            let line = fit(atlas, note, w - 2.0 * MARGIN);
            let lw = atlas.font.width_text(&line);
            batch.text_styled(
                atlas,
                &line,
                ((w - lw) / 2.0).floor(),
                layout.done[1] - 11.0,
                if *error { 0xFFFF_5555 } else { 0xFFAA_AAAA },
                true,
            );
        }
        ui::button(
            batch,
            atlas,
            layout.done[0],
            layout.done[1],
            layout.done[2],
            layout.done[3],
            &Text::translate("gui.done", &[]),
            true,
            inside(cursor, layout.done),
        );
        for job in jobs {
            self.draw_model_job(batch, job);
        }
    }

    pub(super) fn skin_changer_mouse(&mut self, pressed: bool) {
        let (w, h) = self.gui_size();
        let layout = Layout::new(w, h);
        let cursor = self.cursor;
        let Some(state) = &mut self.skin_changer_ui.state else {
            if pressed && inside(cursor, layout.done) {
                self.audio.play_button_click();
                self.close_skin_changer();
            }
            return;
        };
        if !pressed {
            let grab = std::mem::replace(&mut state.grab, Grab::None);
            if let Grab::Grid {
                start,
                moved: false,
                ..
            } = grab
            {
                if let Some(index) = changer::tile_at(
                    [start.0, start.1],
                    state.changer.count(),
                    layout.columns,
                    TILE,
                    TILE_GAP,
                    layout.origin(),
                    state.scroll,
                ) {
                    state.changer.select_tile(index);
                    state.note = None;
                    self.audio.play_button_click();
                }
            }
            return;
        }
        if inside(cursor, layout.done) {
            self.audio.play_button_click();
            self.close_skin_changer();
            return;
        }
        let active = state.changer.active();
        if inside(cursor, layout.buttons[0]) {
            if state.changer.selected != active {
                self.audio.play_button_click();
                let note = state.changer.apply().map(|()| "Skin applied.".to_owned());
                self.skin_changer_note(note);
                self.publish_skin_changer();
            }
            return;
        }
        if inside(cursor, layout.buttons[1]) {
            if state.changer.can_remove() {
                self.audio.play_button_click();
                let note = state.changer.remove().map(|()| "Skin removed.".to_owned());
                self.skin_changer_note(note);
                self.publish_skin_changer();
            }
            return;
        }
        if inside(cursor, layout.buttons[2]) {
            self.audio.play_button_click();
            let note = state.changer.toggle_model().map(|()| String::new());
            self.skin_changer_note(note);
            self.publish_skin_changer();
            return;
        }
        if inside(cursor, layout.stage) {
            state.grab = Grab::Stage { last: cursor };
        } else if inside(cursor, layout.grid) {
            state.grab = Grab::Grid {
                start: cursor,
                scroll: state.scroll,
                moved: false,
            };
        }
    }

    pub(super) fn skin_changer_drag(&mut self) {
        let (w, h) = self.gui_size();
        let layout = Layout::new(w, h);
        let cursor = self.cursor;
        let Some(state) = &mut self.skin_changer_ui.state else {
            return;
        };
        match state.grab {
            Grab::Stage { last } => {
                state.spin = (state.spin - (cursor.0 - last.0) * 2.0).rem_euclid(360.0);
                state.grab = Grab::Stage { last: cursor };
            }
            Grab::Grid {
                start,
                scroll,
                moved,
            } => {
                let travelled = (cursor.1 - start.1).abs() + (cursor.0 - start.0).abs();
                let moved = moved || travelled > MOVE_SLOP;
                if moved {
                    let limit = changer::max_scroll(
                        state.changer.count(),
                        layout.columns,
                        TILE[1],
                        TILE_GAP,
                        layout.view_height(),
                    );
                    state.scroll = (scroll - (cursor.1 - start.1)).clamp(0.0, limit);
                }
                state.grab = Grab::Grid {
                    start,
                    scroll,
                    moved,
                };
            }
            Grab::None => {}
        }
    }

    pub(super) fn skin_changer_scroll(&mut self, delta: f32) {
        let (w, h) = self.gui_size();
        let layout = Layout::new(w, h);
        let cursor = self.cursor;
        let Some(state) = &mut self.skin_changer_ui.state else {
            return;
        };
        if inside(cursor, layout.stage) || inside(cursor, layout.side) {
            state.spin = (state.spin - delta * 15.0).rem_euclid(360.0);
            return;
        }
        let limit = changer::max_scroll(
            state.changer.count(),
            layout.columns,
            TILE[1],
            TILE_GAP,
            layout.view_height(),
        );
        state.scroll = (state.scroll - delta * 20.0).clamp(0.0, limit);
    }
}

#[cfg(not(target_os = "android"))]
impl Game<'_> {
    pub(super) fn draw_skin_changer(&mut self, _batch: &mut horizon_client::hud::HudBatch) {}
    pub(super) fn skin_changer_mouse(&mut self, _pressed: bool) {}
    pub(super) fn skin_changer_drag(&mut self) {}
    pub(super) fn skin_changer_scroll(&mut self, _delta: f32) {}
    pub(super) fn open_skin_changer(&mut self) {}
    pub(super) fn sync_title_skin_preview(&mut self) {}
    pub(super) fn title_spin_press(&mut self) -> bool {
        false
    }
    pub(super) fn title_spin_drag(&mut self) {}
    pub(super) fn rotate_zone_at(&self, _p: (f32, f32)) -> bool {
        false
    }
    pub(super) fn title_spin_release(&mut self) {}
    pub(super) fn draw_title_player(
        &mut self,
        _batch: &mut horizon_client::hud::HudBatch,
        _w: f32,
        _h: f32,
    ) {
    }
    pub(super) fn close_skin_changer(&mut self) {
        self.screen = super::Screen::Options;
    }
}
