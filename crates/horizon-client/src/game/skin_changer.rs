//! Options > Skin Customization > Skin Changer: a grid of the launcher wardrobe's skins on a
//! translucent black panel, a draggable 3D preview beside it, and Apply / Remove / Arms stacked
//! under the preview. It reads and writes the same wardrobe the launcher's Skins tab uses
//! (`horizon-wardrobe.json` in the game folder points at it), and applies the pick to the local
//! player at once.
use super::Game;
#[cfg(target_os = "android")]
use super::Screen;
#[cfg(target_os = "android")]
use crate::skins::changer::{self, Changer, Link, Runs};
#[cfg(target_os = "android")]
use horizon_client::{hud::HudBatch, text::Text, ui};

#[derive(Default)]
pub(super) struct SkinChangerUi {
    #[cfg(target_os = "android")]
    state: Option<State>,
    #[cfg(target_os = "android")]
    error: Option<String>,
}

#[cfg(target_os = "android")]
struct State {
    changer: Changer,
    link: Link,
    scroll: f32,
    grab: Grab,
    note: Option<(String, bool)>,
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
    zoom: [[f32; 4]; 2],
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
        let zoom = [
            [stage[0] + stage[2] - 30.0, stage[1] + 2.0, 14.0, 14.0],
            [stage[0] + stage[2] - 15.0, stage[1] + 2.0, 14.0, 14.0],
        ];
        let done = [((w - 200.0) / 2.0).floor(), h - FOOTER + 5.0, 200.0, BUTTON];
        let columns = changer::columns(grid[2] - 10.0, TILE[0], TILE_GAP);
        Self {
            grid,
            side,
            stage,
            buttons,
            zoom,
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
fn argb(color: [u8; 4]) -> u32 {
    (u32::from(color[3]) << 24)
        | (u32::from(color[0]) << 16)
        | (u32::from(color[1]) << 8)
        | u32::from(color[2])
}

#[cfg(target_os = "android")]
fn inside(point: (f32, f32), rect: [f32; 4]) -> bool {
    ui::hovered(point, rect[0], rect[1], rect[2], rect[3])
}

#[cfg(target_os = "android")]
fn draw_runs(
    batch: &mut HudBatch,
    atlas: &horizon_client::hud::HudAtlas,
    runs: &Runs,
    origin: (f32, f32),
    scale: f32,
) {
    for (rect, color) in runs.iter() {
        let x0 = (origin.0 + rect[0] * scale).floor();
        let y0 = (origin.1 + rect[1] * scale).floor();
        let x1 = (origin.0 + (rect[0] + rect[2]) * scale).ceil();
        let y1 = (origin.1 + (rect[1] + rect[3]) * scale).ceil();
        batch.fill(atlas, x0, y0, x1 - x0, y1 - y0, argb(*color));
    }
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
                });
            }
            Err(error) => {
                log::warn!("Horizon: skin changer: {error}");
                self.skin_changer_ui.state = None;
                self.skin_changer_ui.error = Some(error);
            }
        }
        self.dragging = false;
        self.screen = Screen::SkinChanger;
    }

    pub(super) fn close_skin_changer(&mut self) {
        self.skin_changer_ui.state = None;
        self.skin_changer_ui.error = None;
        self.screen = Screen::Options;
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
            let runs = state.changer.thumb(index);
            draw_runs(
                batch,
                atlas,
                &runs,
                (
                    rect[0] + (TILE[0] - 64.0 * THUMB_SCALE) / 2.0,
                    rect[1] + 3.0,
                ),
                THUMB_SCALE,
            );
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
        let runs = state.changer.stage();
        let scale = ((layout.stage[2] - 4.0) / 160.0).min((layout.stage[3] - 16.0) / 240.0);
        let (sw, sh) = (160.0 * scale, 240.0 * scale);
        let first = batch.vertices.len();
        draw_runs(
            batch,
            atlas,
            &runs,
            (
                (layout.stage[0] + (layout.stage[2] - sw) / 2.0).floor(),
                (layout.stage[1] + 2.0 + (layout.stage[3] - 16.0 - sh) / 2.0).floor(),
            ),
            scale,
        );
        batch.clip_since(
            first,
            layout.stage[0] + 1.0,
            layout.stage[1] + 1.0,
            layout.stage[0] + layout.stage[2] - 1.0,
            layout.stage[1] + layout.stage[3] - 1.0,
        );
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
        for (rect, label) in layout.zoom.iter().zip(["-", "+"]) {
            ui::button(
                batch,
                atlas,
                rect[0],
                rect[1],
                rect[2],
                rect[3],
                &Text::literal(label),
                true,
                inside(cursor, *rect),
            );
        }

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
        for (rect, factor) in layout.zoom.iter().zip([1.0 / 1.2, 1.2]) {
            if inside(cursor, *rect) {
                state.changer.camera.zoom(factor);
                self.audio.play_button_click();
                return;
            }
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
                state
                    .changer
                    .camera
                    .drag((cursor.0 - last.0) * 1.5, (cursor.1 - last.1) * 1.5);
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
            state.changer.camera.zoom(1.0 + delta * 0.1);
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
    pub(super) fn close_skin_changer(&mut self) {
        self.screen = super::Screen::Options;
    }
}
