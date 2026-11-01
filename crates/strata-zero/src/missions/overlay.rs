//! What a scene draws over the map in every look: open doors with the walkway's light
//! falling in, and the marker over whoever speaks (scenes spec §5, §8). Also what both
//! looks share to write a line: a glyph in its whole style, the typed part of each row.

use super::scene::{self, Line};
use super::{Mark, YOU, marks, visitors};
use crate::app::AppState;
use crate::render::viewport::tile_to_screen;
use crate::ui::chrome::DIM;
use citygen::{Place, Pos};
use ratatui::buffer::{Buffer, Cell};
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};

/// An open door's amber.
const AMBER: (u8, u8, u8) = (255, 200, 80);
/// How much amber an open door's cell takes as its background.
const DOOR_BG: f32 = 0.25;
/// How much amber each cell inward takes as its background, by depth.
const LIGHT: [f32; 3] = [0.12, 0.09, 0.06];
pub(super) const WHITE: Color = Color::Rgb(255, 255, 255);

/// The speaker's label upper case, and its colour: `DIM` when the line is spoken `as`
/// someone else, white for you, else the role's tint.
pub fn speaker(st: &AppState, line: &Line) -> (String, Color) {
    let colour = if line.hidden {
        DIM
    } else if line.who == YOU {
        WHITE
    } else {
        visitors::colour(&st.book, &line.who)
    };
    (line.label.to_uppercase(), colour)
}

/// Draws the scene's open doors and the speaker's marker over the map view in `map`.
/// The marker never goes on a tile someone stands on.
pub fn draw(buf: &mut Buffer, map: Rect, origin: Pos, st: &AppState) {
    let Some(run) = scene::running(st) else {
        return;
    };
    let Some(me) = st.player.as_ref() else {
        return;
    };
    for &(place, door) in run.doors() {
        if place == me.place {
            door_open(buf, map, origin, st, door);
        }
    }
    if let Some(line) = run.line()
        && let Some((place, at)) = line.anchor
        && place == me.place
        && !actor_on(st, place, at.offset(0, -1))
        && let Some((sx, sy)) = tile_to_screen(map, origin, at.offset(0, -1))
    {
        let style = Style::default()
            .fg(speaker(st, line).1)
            .add_modifier(Modifier::BOLD);
        ink(&mut buf[(sx, sy)], '▾', style);
    }
}

/// You or a visitor stands on the tile.
fn actor_on(st: &AppState, place: Place, pos: Pos) -> bool {
    st.player
        .as_ref()
        .is_some_and(|p| p.place == place && p.pos == pos)
        || visitors::at(st, place, pos).is_some()
}

/// Writes `ch` in `style`, its modifiers exactly the style's: nothing of the map cell
/// under it (bold, dim) shows through. A colour the style leaves out stays the cell's.
pub(super) fn ink(cell: &mut Cell, ch: char, style: Style) {
    cell.set_char(ch).set_style(style);
    cell.modifier = style.add_modifier;
}

/// How much of each row of `rows` (`text` word-wrapped whole) shows once the first
/// `typed` characters of `text` have typed. A row break drops the space it fell on, or
/// none where a word longer than a row was split; the text's own spaces count as they
/// type.
pub(super) fn typed_rows(text: &str, rows: &[String], typed: usize) -> Vec<usize> {
    let chars: Vec<char> = text.chars().collect();
    // Where in the text the row's next character is.
    let mut at = 0;
    rows.iter()
        .map(|row| {
            let mut shown = 0;
            for c in row.chars() {
                // The spaces the wrap dropped before it.
                while chars.get(at).is_some_and(|t| *t == ' ' && c != ' ') {
                    at += 1;
                }
                if at < typed {
                    shown += 1;
                }
                at += 1;
            }
            shown
        })
        .collect()
}

fn amber(f: f32) -> Color {
    let s = |v: u8| (f32::from(v) * f).round() as u8;
    Color::Rgb(s(AMBER.0), s(AMBER.1), s(AMBER.2))
}

/// The door as `·` on amber, and the three cells across and three deep on its inside
/// (away from the walkway outside) lit by it.
fn door_open(buf: &mut Buffer, map: Rect, origin: Pos, st: &AppState, door: Pos) {
    if let Some((sx, sy)) = tile_to_screen(map, origin, door) {
        let cell = &mut buf[(sx, sy)];
        // Whoever stands in the doorway stays drawn.
        if cell.symbol() != "@" {
            ink(cell, '·', Style::default().fg(amber(1.0)));
        }
        cell.bg = amber(DOOR_BG);
    }
    let Some((_, out)) = marks::resolve(st, &Mark::HomeOutside, None) else {
        return;
    };
    if door.manhattan(out) != 1 {
        return;
    }
    let (dx, dy) = (door.x - out.x, door.y - out.y);
    for (depth, level) in LIGHT.iter().enumerate() {
        let k = depth as i32 + 1;
        for side in -1..=1 {
            // Across the light is the other axis.
            let p = door.offset(dx * k + dy.abs() * side, dy * k + dx.abs() * side);
            if let Some((sx, sy)) = tile_to_screen(map, origin, p) {
                buf[(sx, sy)].bg = amber(*level);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::marks;
    use super::super::scene::tests::{SLOW, playing, queued};
    use super::*;
    use crate::walker::Player;
    use ratatui::style::Style;

    /// A map of grey cells, bold and dim, as a lit wall or a sign can be.
    fn map_buf(area: Rect) -> Buffer {
        let mut buf = Buffer::empty(area);
        let style = Style::default()
            .fg(Color::Rgb(90, 90, 90))
            .bg(Color::Rgb(20, 20, 20))
            .add_modifier(Modifier::BOLD | Modifier::DIM);
        buf.set_style(area, style);
        buf
    }

    const AREA: Rect = Rect::new(0, 0, 20, 10);

    #[test]
    fn the_marker_never_covers_an_actor() {
        let mut st = queued(SLOW, r#"Say(who: "castellan", text: "Hi.")"#);
        let (place, pos) = marks::resolve(&st, &Mark::HomeInside(2), None).unwrap();
        visitors::arrive(&mut st, "castellan", (place, pos));
        super::super::tick(&mut st);
        // He is at (10, 5); the marker's cell is above him.
        let origin = pos.offset(-10, -5);
        let above = |st: &AppState| {
            let mut buf = map_buf(AREA);
            draw(&mut buf, AREA, origin, st);
            buf[(10, 4)].clone()
        };
        st.player = Some(Player::at(place, pos.offset(4, 0)));
        let cell = above(&st);
        assert_eq!(cell.symbol(), "▾");
        assert_eq!(cell.modifier, Modifier::BOLD, "the map's dim doesn't show");
        // You on that cell: no marker over you.
        st.player = Some(Player::at(place, pos.offset(0, -1)));
        assert_eq!(above(&st).symbol(), " ");
        // Nor over a visitor there.
        st.player = Some(Player::at(place, pos.offset(4, 0)));
        visitors::arrive(&mut st, "seren", (place, pos.offset(0, -1)));
        assert_eq!(above(&st).symbol(), " ");
    }

    #[test]
    fn an_open_door_sets_its_whole_style() {
        let mut st = playing(SLOW, "Door(at: HomeDoor, open: true), Wait(1.0)");
        let (_, door) = marks::resolve(&st, &Mark::HomeDoor, None).unwrap();
        // You stand well off the door.
        let (place, inside) = marks::resolve(&st, &Mark::HomeInside(2), None).unwrap();
        st.player = Some(Player::at(place, inside));
        let mut buf = map_buf(AREA);
        draw(&mut buf, AREA, door.offset(-10, -5), &st);
        let cell = &buf[(10, 5)];
        assert_eq!(cell.symbol(), "·");
        assert!(cell.modifier.is_empty(), "{:?}", cell.modifier);
    }

    #[test]
    fn typed_rows_follow_the_breaks() {
        let text = "abcdefghijklmnop qr st";
        let rows = crate::ui::sidebar::wrap_text(text, 5);
        assert_eq!(rows, ["abcde", "fghij", "klmno", "p qr", "st"]);
        let at = |n| typed_rows(text, &rows, n);
        assert_eq!(at(0), [0, 0, 0, 0, 0]);
        // The long word is split with no space dropped.
        assert_eq!(at(6), [5, 1, 0, 0, 0]);
        assert_eq!(at(16), [5, 5, 5, 1, 0]);
        assert_eq!(at(17), [5, 5, 5, 2, 0]);
        // The space a break fell on types, and shows nowhere.
        assert_eq!(at(20), [5, 5, 5, 4, 0]);
        assert_eq!(at(21), [5, 5, 5, 4, 1]);
        assert_eq!(at(99), [5, 5, 5, 4, 2]);
    }
}
