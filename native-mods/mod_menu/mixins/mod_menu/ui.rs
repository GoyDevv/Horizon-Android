// Mod Menu's Mods screen. It is drawn in place of the Options screen while the title screen's
// Mods button has asked for it (`native_mod_menu_title`), so Horizon's
// own window, mouse, wheel, keys and Escape all reach it through the Options screen's hooks.
// The layout, the search and the wrapping are in `native_mod_menu`; this file draws them.
use super::native_mod_menu as m;
use super::Game;
use horizon_client::{
    Result,
    clipboard::EditShortcut,
    hud::{HudAtlas, HudBatch, rgba},
    options_ui::{Event, Page},
    text::Text,
    title::native_mod_menu_title as shared,
    ui,
};
use std::sync::{Mutex, MutexGuard};
use winit::keyboard::KeyCode;

/// What the screen remembers while it is open.
struct State {
    search: String,
    filter: m::Filter,
    /// The picked mod, as an index into `Mods::installed`.
    selected: Option<usize>,
    list_scroll: f32,
    detail_scroll: f32,
}

impl State {
    const fn new() -> Self {
        State {
            search: String::new(),
            filter: m::Filter::All,
            selected: None,
            list_scroll: 0.0,
            detail_scroll: 0.0,
        }
    }
}

static STATE: Mutex<State> = Mutex::new(State::new());

/// The state; a screen that has just been asked for starts over.
fn state() -> MutexGuard<'static, State> {
    let mut st = STATE.lock().unwrap_or_else(|e| e.into_inner());
    if shared::take_fresh() {
        *st = State::new();
    }
    st
}

fn color(rgb: u32) -> u32 {
    rgba(((rgb >> 16) & 0xFF) as u8, ((rgb >> 8) & 0xFF) as u8, (rgb & 0xFF) as u8, 255)
}

fn rgb_of(c: [u8; 3]) -> u32 {
    rgba(c[0], c[1], c[2], 255)
}

/// The installed mods as the screen lists them.
fn entries(game: &Game<'_>) -> Vec<m::Entry> {
    let mods = &game.settings.mods;
    mods.installed
        .iter()
        .enumerate()
        .map(|(i, e)| m::Entry {
            id: e.id.clone(),
            title: e.title.clone(),
            description: e.description.clone(),
            on: mods.is_on(i),
            native: e.native.is_some(),
            dev: e.dev,
            problem: e.problem.clone(),
            has_settings: e.native.is_some(),
        })
        .collect()
}

/// Leaves the Mods screen the way Options' Done does.
fn done(game: &mut Game<'_>) -> Result<()> {
    shared::close();
    game.handle_option_events(vec![Event::ButtonClick, Event::Close])
}

/// Mod Menu's own icon: the blue square with the white list, each of its 16 pixels 2 wide.
fn draw_menu_icon(batch: &mut HudBatch, atlas: &HudAtlas, x: f32, y: f32) {
    let unit = m::ICON / 16.0;
    batch.fill(atlas, x, y, m::ICON, m::ICON, rgb_of(m::MENU_ICON_BLUE));
    for (col, row, len) in m::menu_icon_runs() {
        batch.fill(atlas, x + f32::from(col) * unit, y + f32::from(row) * unit, f32::from(len) * unit, unit, u32::MAX);
    }
}

/// A mod's icon: a dark tile with a block in the mod's colour (grey while it is off); Mod Menu has
/// its own picture.
fn draw_icon(batch: &mut HudBatch, atlas: &HudAtlas, x: f32, y: f32, e: &m::Entry) {
    if m::is_self(e) {
        draw_menu_icon(batch, atlas, x, y);
        return;
    }
    let live = e.on && e.problem.is_none();
    let mut base = m::icon_rgb(&e.id);
    if !live {
        let grey = ((u32::from(base[0]) + u32::from(base[1]) + u32::from(base[2])) / 3) as u8;
        base = m::shade([grey, grey, grey], 0.8);
    }
    batch.fill(atlas, x, y, m::ICON, m::ICON, rgba(70, 70, 82, 255));
    batch.fill(atlas, x + 1.0, y + 1.0, m::ICON - 2.0, m::ICON - 2.0, rgba(26, 26, 32, 255));
    let shades = [rgb_of(m::shade(base, 1.1)), rgb_of(m::shade(base, 0.75)), rgb_of(m::shade(base, 0.5))];
    for (col, row, len, shade) in shared::cube_runs() {
        batch.fill(
            atlas,
            x + 2.0 + f32::from(col) * 2.0,
            y + 2.0 + f32::from(row) * 2.0,
            f32::from(len) * 2.0,
            2.0,
            shades[usize::from(shade) - 1],
        );
    }
}

/// A coloured tag; returns its width.
fn draw_badge(batch: &mut HudBatch, atlas: &HudAtlas, x: f32, y: f32, badge: &m::Badge) -> f32 {
    let w = HudBatch::text_width(atlas, badge.text).ceil() + 6.0;
    let base = [(badge.rgb >> 16) as u8, (badge.rgb >> 8) as u8, badge.rgb as u8];
    batch.fill(atlas, x, y, w, 10.0, rgb_of(m::shade(base, 0.6)));
    batch.fill(atlas, x + 1.0, y + 1.0, w - 2.0, 8.0, rgb_of(base));
    batch.text(atlas, badge.text, x + 3.0, y + 1.0, u32::MAX, false);
    w
}

fn tone_color(tone: m::Tone) -> u32 {
    match tone {
        m::Tone::Text => color(0xD8D8D8),
        m::Tone::Dim => color(0x8C8C8C),
        m::Tone::Bad => color(0xFF7070),
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_row(
    batch: &mut HudBatch,
    atlas: &HudAtlas,
    e: &m::Entry,
    x: f32,
    y: f32,
    w: f32,
    selected: bool,
    hover: bool,
) {
    let measure = |s: &str| HudBatch::text_width(atlas, s);
    if selected {
        batch.fill(atlas, x + 2.0, y, w - 4.0, m::ROW, u32::MAX);
        batch.fill(atlas, x + 3.0, y + 1.0, w - 6.0, m::ROW - 2.0, rgba(0, 0, 0, 255));
    } else if hover {
        batch.fill(atlas, x + 2.0, y, w - 4.0, m::ROW, rgba(255, 255, 255, 40));
    }
    draw_icon(batch, atlas, x + 4.0, y + 2.0, e);
    let text_x = x + 4.0 + m::ICON + 4.0;
    let live = e.on && e.problem.is_none();
    let badges = m::screen_badges(e);
    let badge_w: f32 = badges.iter().map(|b| HudBatch::text_width(atlas, b.text).ceil() + 6.0 + 3.0).sum();
    let name = m::name_and_version(&e.title).0;
    let name = m::ellipsize(name, (w - (text_x - x) - 10.0 - badge_w).max(30.0), &measure);
    let name_w = batch.text(atlas, &name, text_x, y + 4.0, if live { u32::MAX } else { color(0xA0A0A0) }, true);
    let mut bx = text_x + name_w + 4.0;
    for b in &badges {
        if bx + 20.0 > x + w - 8.0 {
            break;
        }
        bx += draw_badge(batch, atlas, bx, y + 3.0, b) + 3.0;
    }
    let (about, about_color) = match (&e.problem, e.description.trim()) {
        (Some(p), _) => (p.as_str(), color(0xFF8080)),
        (None, "") => ("No description.", color(0x707070)),
        (None, d) => (d, color(0xAAAAAA)),
    };
    let avail = w - (text_x - x) - 10.0;
    let mut lines = m::wrap(about, avail, &measure);
    if lines.len() > 2 {
        let more = format!("{}...", lines[1]);
        lines[1] = m::ellipsize(&more, avail, &measure);
    }
    for (i, line) in lines.iter().take(2).enumerate() {
        batch.text(atlas, line, text_x, y + 15.0 + 10.0 * i as f32, about_color, true);
    }
}

pub fn draw(game: &mut Game<'_>, batch: &mut HudBatch) {
    let entries = entries(game);
    let mut st = state();
    let (w, h) = (batch.width, batch.height);
    let l = m::layout(w, h);
    let shown = m::visible(&entries, &st.search, st.filter);
    if st.selected.is_none_or(|s| !shown.contains(&s)) {
        st.selected = shown.first().copied();
        st.detail_scroll = 0.0;
    }
    st.list_scroll = st.list_scroll.clamp(0.0, m::max_scroll(shown.len(), l.list.h));
    let atlas = &game.gui.atlas;
    let cursor = game.cursor;
    let in_world = game.title.is_none();
    let measure = |s: &str| HudBatch::text_width(atlas, s);

    ui::menu_background(batch, atlas, 0.0, 0.0, w, h, in_world);
    ui::list_background(batch, atlas, l.list.y, l.list.y + l.list.h, in_world);

    // The search box, always ready for typing, and the filter beside it.
    ui::nine_slice(
        batch,
        atlas,
        "gui/sprites/widget/text_field_highlighted",
        l.search.x,
        l.search.y,
        l.search.w,
        l.search.h,
        u32::MAX,
    );
    let (tx, ty) = (l.search.x + 4.0, l.search.y + 6.0);
    let mut caret = tx;
    if st.search.is_empty() {
        batch.text(atlas, "Search...", tx, ty, color(0x6C6C6C), true);
    } else {
        let tail = m::tail_fit(&st.search, l.search.w - 10.0, &measure);
        caret += batch.text(atlas, &tail, tx, ty, color(0xE0E0E0), true);
    }
    let blink = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(true, |d| (d.as_millis() / 500) % 2 == 0);
    if blink {
        batch.fill(atlas, caret, ty - 1.0, 1.0, 10.0, color(0xE0E0E0));
    }
    let filter_hover = l.filter.contains(cursor.0, cursor.1);
    ui::button(batch, atlas, l.filter.x, l.filter.y, l.filter.w, l.filter.h, &Text::literal(""), true, filter_hover);
    let funnel = if st.filter == m::Filter::All { u32::MAX } else { color(0xFFD35A) };
    for (dx, dy, fw, fh) in [(4.0, 5.0, 12.0, 2.0), (6.0, 8.0, 8.0, 2.0), (8.0, 11.0, 4.0, 2.0), (9.0, 13.0, 2.0, 3.0)] {
        batch.fill(atlas, l.filter.x + dx, l.filter.y + dy, fw, fh, funnel);
    }
    batch.text(atlas, &m::showing(shown.len()), l.showing[0], l.showing[1], color(0xA0A0A0), true);

    // The list.
    let first = batch.vertices.len();
    for (row, &index) in shown.iter().enumerate() {
        let y = l.list.y + row as f32 * m::ROW - st.list_scroll;
        if y + m::ROW < l.list.y || y > l.list.y + l.list.h {
            continue;
        }
        let hover = l.list.contains(cursor.0, cursor.1) && cursor.1 >= y && cursor.1 < y + m::ROW && cursor.0 < l.list.w - 6.0;
        draw_row(batch, atlas, &entries[index], l.list.x, y, l.list.w - 6.0, st.selected == Some(index), hover);
    }
    if entries.is_empty() {
        let note = "No mods installed";
        let nx = l.list.x + ((l.list.w - HudBatch::text_width(atlas, note)) / 2.0).floor();
        batch.text(atlas, note, nx, l.list.y + 20.0, color(0x8C8C8C), true);
    } else if shown.is_empty() {
        let note = "Nothing matches";
        let nx = l.list.x + ((l.list.w - HudBatch::text_width(atlas, note)) / 2.0).floor();
        batch.text(atlas, note, nx, l.list.y + 20.0, color(0x8C8C8C), true);
    }
    batch.clip_since(first, l.list.x, l.list.y, l.list.x + l.list.w, l.list.y + l.list.h);
    ui::scrollbar(
        batch,
        atlas,
        l.list.x + l.list.w - 6.0,
        l.list.y,
        l.list.y + l.list.h,
        shown.len() as f32 * m::ROW,
        st.list_scroll.trunc(),
    );

    // The detail pane.
    if let Some(e) = st.selected.and_then(|s| entries.get(s)) {
        let buttons = !m::is_self(e);
        let lines = m::body_lines(e, m::detail(&l, 0.0, buttons).width, &measure);
        let content = m::detail_height(lines.len(), buttons);
        st.detail_scroll = st.detail_scroll.clamp(0.0, (content - l.detail.h).max(0.0));
        let d = m::detail(&l, st.detail_scroll, buttons);
        let first = batch.vertices.len();
        draw_icon(batch, atlas, d.icon.x, d.icon.y, e);
        let name = m::name_and_version(&e.title).0;
        let name_w = batch.text(atlas, name, d.name[0], d.name[1], u32::MAX, true);
        let mut bx = d.name[0] + name_w + 5.0;
        for b in &m::screen_badges(e) {
            bx += draw_badge(batch, atlas, bx, d.name[1] - 1.0, b) + 3.0;
        }
        let info = m::info_pair(e);
        for (line, at) in info.iter().zip(d.info) {
            batch.text(atlas, line, at[0], at[1], color(0xA0A0A0), true);
        }
        if buttons {
            let inside = l.detail.contains(cursor.0, cursor.1);
            let can_toggle = !e.dev && !e.id.is_empty();
            let label = if e.on { "Turn off" } else { "Turn on" };
            let t = d.toggle;
            ui::button(batch, atlas, t.x, t.y, t.w, t.h, &Text::literal(label), can_toggle, inside && can_toggle && t.contains(cursor.0, cursor.1));
            let s = d.settings;
            ui::button(batch, atlas, s.x, s.y, s.w, s.h, &Text::literal("Settings"), e.has_settings, inside && e.has_settings && s.contains(cursor.0, cursor.1));
        }
        for (i, line) in lines.iter().enumerate() {
            batch.text(atlas, &line.text, d.body[0], d.body[1] + m::LINE * i as f32, tone_color(line.tone), true);
        }
        batch.clip_since(first, l.detail.x, l.detail.y, l.detail.x + l.detail.w, l.detail.y + l.detail.h);
        ui::scrollbar(
            batch,
            atlas,
            l.detail.x + l.detail.w - 6.0,
            l.detail.y,
            l.detail.y + l.detail.h,
            content,
            st.detail_scroll.trunc(),
        );
    }

    ui::separators(batch, atlas, l.list.y, l.list.y + l.list.h, in_world);

    // The footer.
    let folder_hover = l.folder.contains(cursor.0, cursor.1);
    let folder = Text::translate("horizon.mods.open_folder", &[]);
    ui::button(batch, atlas, l.folder.x, l.folder.y, l.folder.w, l.folder.h, &folder, true, folder_hover);
    let done_hover = l.done.contains(cursor.0, cursor.1);
    ui::button(batch, atlas, l.done.x, l.done.y, l.done.w, l.done.h, &Text::translate("gui.done", &[]), true, done_hover);
    let hint = m::ellipsize("Add mods with the Horizon launcher or the mods folder", w - 12.0, &measure);
    let hx = ((w - HudBatch::text_width(atlas, &hint)) / 2.0).floor();
    batch.text(atlas, &hint, hx, l.hint_y, color(0x808080), true);

    if filter_hover {
        let tip = [Text::literal(format!("Filter: {}", st.filter.label()))];
        ui::tooltip(batch, atlas, &tip, cursor);
    }
}

/// A press on the screen (a release or a right click does nothing).
pub fn mouse(game: &mut Game<'_>, pressed: bool, right: bool) -> Result<()> {
    if !pressed || right {
        game.dragging = false;
        return Ok(());
    }
    let (w, h) = game.gui_size();
    let l = m::layout(w, h);
    let (x, y) = game.cursor;
    let entries = entries(game);
    let mut events: Vec<Event> = Vec::new();
    let mut leave = false;
    let mut settings = None;
    {
        let mut st = state();
        let shown = m::visible(&entries, &st.search, st.filter);
        if l.done.contains(x, y) {
            leave = true;
        } else if l.folder.contains(x, y) {
            events.extend([Event::ButtonClick, Event::OpenModFolder]);
        } else if l.filter.contains(x, y) {
            st.filter = st.filter.next();
            st.list_scroll = 0.0;
            events.push(Event::ButtonClick);
        } else if let Some(row) = m::row_at(&l, st.list_scroll, shown.len(), x, y) {
            st.selected = Some(shown[row]);
            st.detail_scroll = 0.0;
        } else if l.detail.contains(x, y) {
            if let Some(index) = st.selected
                && let Some(e) = entries.get(index)
            {
                let d = m::detail(&l, st.detail_scroll, !m::is_self(e));
                if m::is_self(e) {
                    // No buttons here: Mod Menu can't be turned off and has no settings.
                } else if d.toggle.contains(x, y) && !e.dev && !e.id.is_empty() {
                    game.settings.mods.toggle(index);
                    events.extend([Event::ButtonClick, Event::Changed]);
                } else if d.settings.contains(x, y) && e.has_settings {
                    settings = Some(index);
                }
            }
        }
    }
    if leave {
        return done(game);
    }
    game.handle_option_events(events)?;
    if let Some(index) = settings {
        // The mod's own settings page, the way Options > Mods opens it.
        shared::close();
        let (from_title, to_game) = (game.options_from_title, game.options_to_game);
        game.open_options_at(from_title, Page::NativeMod(index));
        game.options_to_game = to_game;
        game.handle_option_events(vec![Event::ButtonClick, Event::NativeSettings(index)])?;
    }
    Ok(())
}

/// The wheel: the detail pane when the pointer is over it, the list otherwise.
pub fn scroll(game: &mut Game<'_>, amount: f32) -> Result<()> {
    let (w, h) = game.gui_size();
    let l = m::layout(w, h);
    let (x, y) = game.cursor;
    let entries = entries(game);
    let mut st = state();
    if l.detail.contains(x, y) {
        // The most it can scroll is known when it is drawn.
        st.detail_scroll = (st.detail_scroll - amount * m::SCROLL_STEP).max(0.0);
    } else {
        let count = m::visible(&entries, &st.search, st.filter).len();
        st.list_scroll = (st.list_scroll - amount * m::SCROLL_STEP).clamp(0.0, m::max_scroll(count, l.list.h));
    }
    Ok(())
}

/// Keys: typing searches, the arrows pick a mod, Escape leaves.
pub fn key(game: &mut Game<'_>, key: KeyCode, text: Option<&str>) -> Result<()> {
    let ctrl = game.has_control_down();
    if key == KeyCode::Escape {
        return done(game);
    }
    let (w, h) = game.gui_size();
    let l = m::layout(w, h);
    let entries = entries(game);
    let mut st = state();
    let shown = m::visible(&entries, &st.search, st.filter);
    let at = shown.iter().position(|&i| Some(i) == st.selected);
    let page = ((l.list.h / m::ROW).floor() as i32 - 1).max(1);
    let moved = match key {
        KeyCode::ArrowDown => m::step(shown.len(), at, 1),
        KeyCode::ArrowUp => m::step(shown.len(), at, -1),
        KeyCode::PageDown => m::step(shown.len(), at, page),
        KeyCode::PageUp => m::step(shown.len(), at, -page),
        KeyCode::Home => m::step(shown.len(), None, 1),
        KeyCode::End => m::step(shown.len(), None, -1),
        _ => None,
    };
    if let Some(row) = moved {
        st.selected = Some(shown[row]);
        st.detail_scroll = 0.0;
        st.list_scroll = m::scroll_into_view(st.list_scroll, row, l.list.h);
    } else if key == KeyCode::Backspace {
        m::backspace(&mut st.search, ctrl);
        st.list_scroll = 0.0;
    } else if !ctrl && let Some(typed) = text {
        m::type_text(&mut st.search, typed);
        st.list_scroll = 0.0;
    }
    Ok(())
}

/// Ctrl+A, C, V and X in the search box.
pub fn shortcut(game: &mut Game<'_>, shortcut: EditShortcut) -> bool {
    use horizon_client::clipboard::Clipboard;
    let mut st = state();
    match shortcut {
        EditShortcut::Paste => {
            let pasted = game.clipboard.get().replace(['\r', '\n'], " ");
            m::type_text(&mut st.search, &pasted);
            st.list_scroll = 0.0;
        }
        EditShortcut::Copy => {
            let text = st.search.clone();
            game.clipboard.set(&text);
        }
        EditShortcut::Cut => {
            let text = std::mem::take(&mut st.search);
            game.clipboard.set(&text);
            st.list_scroll = 0.0;
        }
        EditShortcut::SelectAll => {}
    }
    true
}
