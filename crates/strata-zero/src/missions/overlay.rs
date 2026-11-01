//! What a scene draws over the map in every look: open doors with the walkway's light
//! falling in, and the marker over whoever speaks (scenes spec §5, §8).

use super::scene::{self, Line};
use super::{Mark, YOU, marks, visitors};
use crate::app::AppState;
use crate::render::viewport::tile_to_screen;
use crate::ui::chrome::DIM;
use citygen::Pos;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier};

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
        && let Some((sx, sy)) = tile_to_screen(map, origin, at.offset(0, -1))
    {
        let cell = &mut buf[(sx, sy)];
        cell.set_char('▾').fg = speaker(st, line).1;
        cell.modifier.insert(Modifier::BOLD);
    }
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
            cell.set_char('·').fg = amber(1.0);
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
