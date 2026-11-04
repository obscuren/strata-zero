//! The daze (morning news spec §8): a few seconds where the map swims, then settles.

use crate::app::AppState;
use crate::ui::chrome::PANEL_BG;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Color;

/// Frame ticks a second.
const FRAMES_PER_SECOND: f32 = 30.0;

/// The most a row slides sideways, in cells.
pub const DRIFT: i32 = 2;

/// A daze running.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Daze {
    /// The frame tick it began.
    pub start: u64,
    /// The frame ticks it lasts.
    pub ticks: u64,
}

/// Starts a daze of `seconds`, in place of any running.
pub fn start(st: &mut AppState, seconds: f32) {
    st.missions.daze = Some(Daze {
        start: st.tick,
        ticks: (seconds * FRAMES_PER_SECOND).round() as u64,
    });
}

/// How strong the daze is now: 1 at the start, easing out to 0 at the end and after,
/// and 0 with none running.
pub fn strength(st: &AppState) -> f32 {
    let Some(d) = st.missions.daze else {
        return 0.0;
    };
    if d.ticks == 0 {
        return 0.0;
    }
    let t = (st.tick.saturating_sub(d.start) as f32 / d.ticks as f32).min(1.0);
    (1.0 - t) * (1.0 - t)
}

/// The map rect `map` swimming: each row slides sideways on a slow wave, and the colours
/// bleed into a magenta and a cyan fringe. Nothing outside `map` is touched.
pub fn draw(buf: &mut Buffer, map: Rect, st: &AppState) {
    let s = strength(st);
    let map = map.intersection(buf.area);
    if s <= 0.0 || map.is_empty() {
        return;
    }
    let tick = st.tick as f32;
    let before = buf.clone();
    for y in map.y..map.bottom() {
        let wave = (0.7 * f32::from(y) + 0.25 * tick).sin();
        let shift = (DRIFT as f32 * s * wave).round() as i32;
        for x in map.x..map.right() {
            let from = i32::from(x) - shift;
            let mut cell = if from >= i32::from(map.x) && from < i32::from(map.right()) {
                before[(from as u16, y)].clone()
            } else {
                let mut blank = before[(x, y)].clone();
                blank.reset();
                blank.set_bg(PANEL_BG).set_fg(PANEL_BG);
                blank
            };
            let toward = match (u64::from(x) + u64::from(y) + st.tick / 4) % 3 {
                0 => Some((255, 0, 200)),
                1 => Some((0, 229, 255)),
                _ => None,
            };
            if let (Some(to), Color::Rgb(r, g, b)) = (toward, cell.fg) {
                cell.set_fg(bleed((r, g, b), to, 0.5 * s));
            }
            buf[(x, y)] = cell;
        }
    }
}

/// `from` mixed toward `to` by `by`.
fn bleed(from: (u8, u8, u8), to: (u8, u8, u8), by: f32) -> Color {
    let mix = |a: u8, b: u8| (f32::from(a) * (1.0 - by) + f32::from(b) * by).round() as u8;
    Color::Rgb(mix(from.0, to.0), mix(from.1, to.1), mix(from.2, to.2))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::Screen;
    use crate::jobs::testutil::state_home;
    use crate::screens::explore::MapScreen;
    use crate::screens::shots::draw_stack;

    fn started(at: u64, ticks: u64) -> AppState {
        let mut st = AppState::new();
        st.tick = at;
        st.missions.daze = Some(Daze { start: at, ticks });
        st
    }

    /// Letters and greys that differ cell to cell, in a frame bigger than the map.
    fn picture() -> (Buffer, Rect) {
        let full = Rect::new(0, 0, 100, 30);
        let map = Rect::new(2, 3, 60, 20);
        let mut buf = Buffer::empty(full);
        for y in 0..full.height {
            for x in 0..full.width {
                let c = &mut buf[(x, y)];
                c.set_char((b'a' + ((x + 3 * y) % 26) as u8) as char);
                c.set_fg(Color::Rgb(120, 120, 120));
            }
        }
        (buf, map)
    }

    #[test]
    fn it_fades_out() {
        let mut st = started(100, 120);
        assert_eq!(strength(&st), 1.0);
        st.tick = 160;
        assert!((strength(&st) - 0.25).abs() < 1e-6, "{}", strength(&st));
        st.tick = 220;
        assert_eq!(strength(&st), 0.0);
        st.tick = 900;
        assert_eq!(strength(&st), 0.0);
        st.missions.daze = None;
        assert_eq!(strength(&st), 0.0);
    }

    #[test]
    fn rows_slide_and_colours_split_early() {
        let st = started(100, 120);
        let (mut buf, map) = picture();
        let before = buf.clone();
        draw(&mut buf, map, &st);
        let mut moved = false;
        for y in map.y..map.bottom() {
            // The shift of the row: where the middle cell's letter came from.
            let x = map.x + 30;
            let shift = (-DRIFT..=DRIFT)
                .find(|d| before[((x as i32 - d) as u16, y)].symbol() == buf[(x, y)].symbol());
            let shift = shift.unwrap_or_else(|| panic!("row {y} slid more than {DRIFT}"));
            moved |= shift != 0;
        }
        assert!(moved, "some row slid");
        let mut magenta = false;
        let mut cyan = false;
        for y in map.y..map.bottom() {
            for x in map.x + 3..map.right() - 3 {
                let Color::Rgb(r, g, b) = buf[(x, y)].fg else {
                    panic!("rgb")
                };
                magenta |= r > 120 && g < 120;
                cyan |= r < 120 && g > 120;
                assert_ne!((r, g, b), (0, 0, 0));
            }
        }
        assert!(magenta && cyan, "both fringes");
    }

    #[test]
    fn nothing_changes_after() {
        let mut st = started(0, 120);
        st.tick = 120;
        let (mut buf, map) = picture();
        let before = buf.clone();
        draw(&mut buf, map, &st);
        assert_eq!(buf, before);
    }

    #[test]
    fn only_the_map() {
        let st = started(100, 120);
        let (mut buf, map) = picture();
        let before = buf.clone();
        draw(&mut buf, map, &st);
        for y in 0..buf.area.height {
            for x in 0..buf.area.width {
                if !map.contains((x, y).into()) {
                    assert_eq!(buf[(x, y)], before[(x, y)], "{x},{y}");
                }
            }
        }
        assert_ne!(buf, before, "the map moved");
        // A map bigger than the buffer stays inside it.
        let mut small = Buffer::empty(Rect::new(0, 0, 20, 10));
        draw(&mut small, Rect::new(0, 0, 60, 30), &st);
    }

    #[test]
    fn the_daze_ends_and_is_cleared() {
        let mut st = state_home();
        let mut map = MapScreen::walking(&mut st);
        start(&mut st, 1.0);
        map.update(&mut st, &[]);
        assert!(st.missions.daze.is_some(), "still dizzy");
        let drawn = draw_stack(&st, &[&map], (100, 30));
        assert_eq!(drawn.area.width, 100);
        st.tick += 31;
        map.update(&mut st, &[]);
        assert!(st.missions.daze.is_none(), "cleared once over");
    }
}
