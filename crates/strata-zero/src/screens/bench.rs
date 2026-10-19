//! The blueprint window's modding mode: the item in grey but for its sockets, the
//! pointer on the sockets alone, and a line on what the pointed one holds. It knows
//! the item only through its `Art`, so any item with sockets and a drawing has it.

use crate::app::{AppState, ScreenCmd, TransitionKind};
use crate::ui::art::{Art, Dir, SocketPlace, grey_area, step};
use crate::ui::chrome::{CYAN, DIM, PANEL_BG, put};
use crate::ui::cyberdeck::brackets;
use crossterm::event::KeyCode;
use game::{Host, Mod};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};

/// The readout's middle run.
const WHITE: Color = Color::Rgb(0xe8, 0xe8, 0xec);
/// The rows at the foot of the body that belong to the tray: four, then the readout.
const TRAY_ROWS: u16 = 5;

/// What modding keeps between frames. Inspect keeps its own.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Bench {
    /// The pointed socket: its place in the art's sockets.
    pub point: usize,
    /// The socket a piece is being chosen for.
    #[allow(dead_code)] // until the tray chooses
    pub choosing: Option<Choosing>,
    /// The tray's first tile shown.
    #[allow(dead_code)] // until the tray scrolls
    pub offset: usize,
}

/// A piece being chosen from the tray.
#[allow(dead_code)] // until the tray chooses
#[derive(Clone, Debug, PartialEq)]
pub struct Choosing;

impl Bench {
    /// The hint row in modding.
    pub fn hint(&self) -> &'static str {
        "↑↓←→ socket · space choose · m inspect · esc close"
    }

    /// One key: the arrows and hjkl go to the nearest socket that way; esc or q
    /// closes the window.
    pub fn key(&mut self, art: &dyn Art, code: KeyCode) -> ScreenCmd {
        let dir = match code {
            KeyCode::Up | KeyCode::Char('k') => Dir::Up,
            KeyCode::Down | KeyCode::Char('j') => Dir::Down,
            KeyCode::Left | KeyCode::Char('h') => Dir::Left,
            KeyCode::Right | KeyCode::Char('l') => Dir::Right,
            KeyCode::Esc | KeyCode::Char('q') => {
                return ScreenCmd::Pop {
                    fx: TransitionKind::None,
                };
            }
            _ => return ScreenCmd::Stay,
        };
        self.point = step(&stops(&art.sockets()), self.point, dir, |_| true);
        ScreenCmd::Stay
    }

    /// The item where inspect draws it, grey but for its sockets, the brackets on the
    /// pointed one and the readout on the body's last row. The tray's rows, from the
    /// drawing's left edge on, are left as drawn: the tray paints them.
    pub fn draw(
        &self,
        st: &AppState,
        art: &dyn Art,
        host: Option<Host>,
        body: Rect,
        buf: &mut Buffer,
    ) {
        let origin = art.origin(body);
        art.draw(origin, body, buf);
        let places = art.sockets();
        let rects: Vec<Rect> = places.iter().map(|s| on_screen(origin, s)).collect();
        let tray = Rect::new(
            origin.0,
            body.bottom().saturating_sub(TRAY_ROWS),
            body.right().saturating_sub(origin.0),
            TRAY_ROWS,
        )
        .intersection(body);
        let keep: Vec<Rect> = rects.iter().copied().chain([tray]).collect();
        grey_area(buf, body, &keep, PANEL_BG);
        let Some(place) = places.get(self.point) else {
            return;
        };
        brackets(buf, rects[self.point], st.tick, body);
        readout(st, host, place, body, buf);
    }
}

/// The line above the status row: the pointed socket, then what is fitted there and
/// what it does, or that it is empty and what it takes.
fn readout(st: &AppState, host: Option<Host>, place: &SocketPlace, body: Rect, buf: &mut Buffer) {
    let fitted = host
        .and_then(|h| st.character.host_copy(h))
        .and_then(|copy| copy.fitted.get(place.index)?.as_ref());
    let (what, does) = match fitted {
        Some(piece) => {
            let item = st.content.item(&piece.item);
            let name = item.map_or_else(|| piece.item.to_string(), |i| i.name.clone());
            let mods = item.and_then(|i| i.hardware.as_ref());
            let lines: Vec<String> = mods.iter().flat_map(|h| &h.mods).map(Mod::line).collect();
            (name, lines.join(" · "))
        }
        None => ("empty".to_string(), format!("takes {}", place.kind.takes())),
    };
    let tail = if does.is_empty() {
        String::new()
    } else {
        format!(" · {does}")
    };
    let runs = [
        (
            format!("SOCKET {}", place.name),
            Style::default().fg(CYAN).add_modifier(Modifier::BOLD),
        ),
        (format!(" ▸ {what}"), Style::default().fg(WHITE)),
        (tail, Style::default().fg(DIM)),
    ];
    let y = body.bottom().saturating_sub(1);
    let mut x = body.x + 1;
    for (text, style) in runs {
        put(buf, x, y, &text, style, body.right());
        x = x.saturating_add(text.chars().count() as u16);
    }
}

/// A screen coordinate, cut to what a cell can be.
fn cut(v: i32) -> u16 {
    u16::try_from(v.max(0)).unwrap_or(u16::MAX)
}

/// A socket's rectangle on the screen, from the drawing's origin; what would fall
/// past the screen's top or left edge is cut off.
fn on_screen((ox, oy): (u16, u16), s: &SocketPlace) -> Rect {
    let (x, y) = (i32::from(ox) + s.x, i32::from(oy) + s.y);
    let (left, top) = (cut(x), cut(y));
    let (right, bottom) = (cut(x + i32::from(s.w)), cut(y + i32::from(s.h)));
    Rect::new(left, top, right - left, bottom - top)
}

/// The sockets as places for `step`, by their place in the list. A socket may sit
/// left of the drawing, so all of them shift right alike: `step` reads only how far
/// apart they are.
fn stops(places: &[SocketPlace]) -> Vec<(usize, Rect)> {
    let (left, top) = places
        .iter()
        .fold((0, 0), |(l, t), s| (l.min(s.x), t.min(s.y)));
    places
        .iter()
        .enumerate()
        .map(|(i, s)| (i, Rect::new(cut(s.x - left), cut(s.y - top), s.w, s.h)))
        .collect()
}
