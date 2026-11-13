// What the title screen's Mods button shares with the Mods screen. This module is injected into
// the client library's `title.rs` (`horizon_client::title::native_mod_menu_title`), so the game
// binary, which draws the screen, can reach it too. It has no game types in it, so it also
// compiles and tests on its own.
use std::sync::atomic::{AtomicBool, Ordering};

/// The Mods button, or the key, asked for the Mods screen: the Options screen shows it while
/// this is set.
static OPEN: AtomicBool = AtomicBool::new(false);
/// The screen has just been asked for, so it starts with an empty search box.
static FRESH: AtomicBool = AtomicBool::new(false);

pub fn open() {
    OPEN.store(true, Ordering::SeqCst);
    FRESH.store(true, Ordering::SeqCst);
}

pub fn close() {
    OPEN.store(false, Ordering::SeqCst);
}

pub fn is_open() -> bool {
    OPEN.load(Ordering::SeqCst)
}

/// True once after [`open`]: the screen starts over.
pub fn take_fresh() -> bool {
    FRESH.swap(false, Ordering::SeqCst)
}

/// Lets a hook call the function it hooks: the first call in sets the guard and runs the original
/// through the hook again; that second call finds the guard set and lets the original run.
pub struct Guard(AtomicBool);

impl Guard {
    pub const fn new() -> Self {
        Self(AtomicBool::new(false))
    }

    /// True when the guard was already set (this is the call the hook made itself).
    pub fn enter(&self) -> bool {
        self.0.swap(true, Ordering::SeqCst)
    }

    pub fn leave(&self) {
        self.0.store(false, Ordering::SeqCst);
    }
}

pub static BUTTONS: Guard = Guard::new();
pub static DRAW: Guard = Guard::new();

/// The id of the Mods button among the title screen's buttons (0 to 7 are taken).
pub const BUTTON_ID: u8 = 8;
/// How far the icon row moves left to stay centred with a fourth button in it.
pub const ROW_SHIFT: f32 = 12.0;

/// The button's icon, three bars like a menu (☰), as `(x, y, width, height)` inside the 20 by 20
/// button.
pub const BARS: [(f32, f32, f32, f32); 3] = [(4.0, 5.0, 12.0, 2.0), (4.0, 9.0, 12.0, 2.0), (4.0, 13.0, 12.0, 2.0)];

/// The block shown for a mod without a picture of its own in the Mods screen: 14 by 14. 1 is the lit top, 2 the left side, 3 the right side.
pub const CUBE: [&str; 14] = [
    "......11......",
    "....111111....",
    "..1111111111..",
    "11111111111111",
    "22111111111133",
    "22221111113333",
    "22222211333333",
    "22222223333333",
    "22222223333333",
    "22222223333333",
    "22222223333333",
    "..2222233333..",
    "....222333....",
    "......23......",
];

/// The icon's runs of one shade, as `(column, row, length, shade)`, so a row is a few rectangles.
pub fn cube_runs() -> Vec<(u8, u8, u8, u8)> {
    let mut runs = Vec::new();
    for (row, line) in CUBE.iter().enumerate() {
        let bytes = line.as_bytes();
        let mut col = 0;
        while col < bytes.len() {
            let shade = bytes[col];
            let mut end = col;
            while end < bytes.len() && bytes[end] == shade {
                end += 1;
            }
            if shade != b'.' {
                runs.push((col as u8, row as u8, (end - col) as u8, shade - b'0'));
            }
            col = end;
        }
    }
    runs
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_flag_opens_closes_and_starts_fresh_once() {
        // One test: the flags are shared by the whole process.
        close();
        take_fresh();
        assert!(!is_open() && !take_fresh());
        open();
        assert!(is_open());
        assert!(take_fresh());
        assert!(!take_fresh(), "fresh only once");
        assert!(is_open());
        close();
        assert!(!is_open());
    }

    #[test]
    fn a_guard_lets_the_second_call_through() {
        let guard = Guard::new();
        assert!(!guard.enter(), "the first call is the hook's");
        assert!(guard.enter(), "the call the hook made finds it set");
        guard.leave();
        assert!(!guard.enter(), "and it is free again afterwards");
    }

    #[test]
    fn the_button_shows_three_bars_that_fit_and_are_centred() {
        for (x, y, w, h) in BARS {
            assert!(x >= 0.0 && y >= 0.0 && x + w <= 20.0 && y + h <= 20.0);
        }
        assert_eq!(BARS[0].2, BARS[2].2);
        assert_eq!(BARS[0].0 * 2.0 + BARS[0].2, 20.0, "centred sideways");
        assert_eq!(BARS[0].1 + (BARS[2].1 + BARS[2].3), 20.0, "centred up and down");
        assert_eq!(BARS[1].1 - (BARS[0].1 + BARS[0].3), BARS[2].1 - (BARS[1].1 + BARS[1].3), "even gaps");
    }

    #[test]
    fn the_block_is_a_square_of_three_shades() {
        assert!(CUBE.iter().all(|row| row.len() == 14));
        let runs = cube_runs();
        let mut shades = [0usize; 4];
        for (col, row, len, shade) in &runs {
            assert!(usize::from(*col) + usize::from(*len) <= 14 && *row < 14);
            shades[usize::from(*shade)] += usize::from(*len);
        }
        assert_eq!(shades[0], 0);
        assert!(shades[1] > 30 && shades[2] > 20 && shades[3] > 20);
        // Left and right sides are mirror images: the same number of pixels, give or take the
        // middle column.
        assert!(shades[2].abs_diff(shades[3]) <= 2, "{shades:?}");
        assert_eq!(shades.iter().sum::<usize>(), CUBE.iter().map(|r| r.bytes().filter(|b| *b != b'.').count()).sum::<usize>());
    }
}
