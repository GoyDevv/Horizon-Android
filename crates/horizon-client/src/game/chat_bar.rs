//! The chat's touch bar: ↑ ↓ Tab ← → and Paste, at the bottom right of the chat screen.
//! Each button is a key tap (or the paste shortcut) for a touch screen with no keyboard keys.

use super::chat::chat_tree;
use super::*;
use horizon_client::chat::Commands;
use horizon_client::screen_draw::Measure;
use std::time::Duration;
use winit::event::{Touch, TouchPhase};

/// The bar's keys, left to right; the sixth button is Paste.
const CHAT_KEYS: [KeyCode; 5] = [
    KeyCode::ArrowUp,
    KeyCode::ArrowDown,
    KeyCode::Tab,
    KeyCode::ArrowLeft,
    KeyCode::ArrowRight,
];

/// A pixel arrow centred on `c`; `dir`: 0 up, 1 down, 2 left, 3 right.
fn arrow(batch: &mut HudBatch, atlas: &horizon_client::hud::HudAtlas, c: [f32; 2], dir: u8, ink: u32) {
    let [cx, cy] = c;
    for k in 0..4 {
        let (k, w) = (k as f32, 2.0 * k as f32 + 1.0);
        let [x, y, w, h] = match dir {
            0 => [cx - k, cy - 3.0 + k, w, 1.0],
            1 => [cx - k, cy + 3.0 - k, w, 1.0],
            2 => [cx - 3.0 + k, cy - k, 1.0, w],
            _ => [cx + 3.0 - k, cy - k, 1.0, w],
        };
        batch.fill(atlas, x, y, w, h, ink);
    }
    let [x, y] = match dir {
        0 => [cx - 1.0, cy + 1.0],
        1 => [cx - 1.0, cy - 3.0],
        2 => [cx + 1.0, cy - 1.0],
        _ => [cx - 3.0, cy - 1.0],
    };
    batch.fill(atlas, x, y, 3.0, 3.0, ink);
}

/// A pixel clipboard centred on `c`, for Paste.
fn clipboard_icon(batch: &mut HudBatch, atlas: &horizon_client::hud::HudAtlas, c: [f32; 2], ink: u32) {
    let [cx, cy] = c;
    let (left, top) = (cx - 4.0, cy - 4.0);
    batch.fill(atlas, left, top, 1.0, 10.0, ink);
    batch.fill(atlas, left + 8.0, top, 1.0, 10.0, ink);
    batch.fill(atlas, left, top + 9.0, 9.0, 1.0, ink);
    batch.fill(atlas, left, top, 2.0, 1.0, ink);
    batch.fill(atlas, left + 7.0, top, 2.0, 1.0, ink);
    batch.fill(atlas, cx - 2.0, top - 1.0, 5.0, 2.0, ink);
    batch.fill(atlas, cx - 2.0, cy, 5.0, 1.0, ink);
    batch.fill(atlas, cx - 2.0, cy + 2.0, 5.0, 1.0, ink);
}

impl Game<'_> {
    /// The bar shows with the chat open on Android.
    fn chat_bar_shown(&self) -> bool {
        cfg!(target_os = "android") && self.screen == Screen::Chat && self.title.is_none()
    }

    /// The buttons in GUI pixels (x, y, w, h), at the right just above the text field.
    /// `height` is the chat's visible height, so the bar rises with the keyboard.
    fn chat_bar(width: f32, height: f32) -> [[f32; 4]; 6] {
        let (w, h, gap) = (24.0, 16.0, 2.0);
        let y = height - 14.0 - h - 2.0;
        let left = width - 2.0 - 6.0 * w - 5.0 * gap;
        std::array::from_fn(|i| [left + i as f32 * (w + gap), y, w, h])
    }

    /// The button under window pixel `p`.
    fn chat_bar_at(&self, p: [f32; 2]) -> Option<usize> {
        if !self.chat_bar_shown() {
            return None;
        }
        let batch = self.gui_scale()?;
        let height = Self::visible_chat_height(&batch);
        let (x, y) = (p[0] / batch.scale, p[1] / batch.scale);
        Self::chat_bar(batch.width, height)
            .iter()
            .position(|r| x >= r[0] && x < r[0] + r[2] && y >= r[1] && y < r[1] + r[3])
    }

    /// A finger on the chat bar: a tap is the button, and the finger stays the bar's until it
    /// lifts. True when the touch was the bar's.
    pub(super) fn chat_bar_touch(&mut self, t: &Touch) -> bool {
        if t.phase == TouchPhase::Started
            && let Some(i) = self.chat_bar_at([t.location.x as f32, t.location.y as f32])
        {
            self.chat_bar_lit = Some((i, Instant::now()));
            self.chat_bar_finger = Some(t.id);
            self.audio.play_button_click();
            self.android_touch_haptic(10);
            match CHAT_KEYS.get(i) {
                Some(&key) => self.chat_bar_key(key),
                None => self.chat_paste(),
            }
            return true;
        }
        if self.chat_bar_finger == Some(t.id) {
            if matches!(t.phase, TouchPhase::Ended | TouchPhase::Cancelled) {
                self.chat_bar_finger = None;
            }
            return true;
        }
        false
    }

    fn chat_bar_key(&mut self, key: KeyCode) {
        self.chat_key_code(key, None);
    }

    /// Paste the clipboard into the chat box.
    pub(super) fn chat_paste(&mut self) {
        let m = Measure(&self.gui.atlas.font);
        let c = Commands {
            dispatcher: chat_tree(&self.remote, &self.mods),
            registries: &self.ui.registries,
        };
        if let Some(screen) = self.ui.chat_screen.as_mut() {
            let paste = horizon_client::clipboard::EditShortcut::Paste;
            screen.shortcut(&m, &c, paste, &mut self.clipboard);
        }
    }

    /// Draw the bar with vanilla buttons, the one just tapped lit. `batch.height` is the
    /// chat's visible height here.
    pub(super) fn draw_chat_bar(&self, batch: &mut HudBatch) {
        if !self.chat_bar_shown() {
            return;
        }
        let atlas = &self.gui.atlas;
        let lit = self
            .chat_bar_lit
            .filter(|(_, at)| at.elapsed() < Duration::from_millis(150))
            .map(|(i, _)| i);
        let ink = horizon_client::hud::rgba(255, 255, 255, 255);
        for (i, r) in Self::chat_bar(batch.width, batch.height).iter().enumerate() {
            let sprite = if lit == Some(i) {
                "gui/sprites/widget/button_highlighted"
            } else {
                "gui/sprites/widget/button"
            };
            horizon_client::ui::nine_slice(batch, atlas, sprite, r[0], r[1], r[2], r[3], u32::MAX);
            let c = [(r[0] + r[2] / 2.0).floor(), (r[1] + r[3] / 2.0).floor()];
            match i {
                2 => {
                    let tw = HudBatch::text_width(atlas, "Tab");
                    batch.text(atlas, "Tab", (c[0] - tw / 2.0).floor() + 1.0, c[1] - 4.0, ink, true);
                }
                0 => arrow(batch, atlas, c, 0, ink),
                1 => arrow(batch, atlas, c, 1, ink),
                3 => arrow(batch, atlas, c, 2, ink),
                4 => arrow(batch, atlas, c, 3, ink),
                _ => clipboard_icon(batch, atlas, c, ink),
            }
        }
    }
}
