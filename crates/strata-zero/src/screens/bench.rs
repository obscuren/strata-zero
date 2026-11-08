//! The blueprint window's modding mode: the item in grey but for its sockets (or,
//! for an item with a back, the back as it is with its sockets boxed), the
//! pointer on the sockets alone, a line on what the pointed one holds, and the
//! HARDWARE tray at the drawing's foot to fit, swap and take out pieces from. It
//! knows the item only through its `Art`, so any item with sockets and a drawing
//! has it.

use crate::actions::{JACKED_IN, jacked_in, perform, report};
use crate::app::{AppState, ScreenCmd, TransitionKind};
use crate::ui::art::{Art, Dir, SocketPlace, brackets, grey_area, step};
use crate::ui::chrome::{CYAN, DIM, PANEL_BG, put, solid};

use crate::ui::hardware::{EMPTY_TILE, Tile, tile};
use crossterm::event::KeyCode;
use game::{Action, CopyId, Hardware, Host, ItemId, Stack};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};

/// The readout's middle run.
const WHITE: Color = Color::Rgb(0xe8, 0xe8, 0xec);
/// The tray's label, as WAREZ's on the deck.
const LABEL: Color = Color::Rgb(0x6b, 0x65, 0x75);
/// The tray's scroll marks, as WAREZ's.
const MARK: Color = Color::Rgb(0x8b, 0x85, 0x96);
/// The band at the body's foot: a bracket row, two tile rows, a bracket row, then
/// the readout.
const BAND: u16 = 5;
/// The label's column from the drawing's left edge; the first tile's, 11 on.
const LABEL_X: u16 = 2;
const TILES_X: u16 = LABEL_X + 11;
/// Columns between two tiles: room for both tiles' brackets.
const GAP: u16 = 3;

/// What modding keeps between frames, and across switches to inspect and back.
/// Inspect keeps its own.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Bench {
    /// The pointed socket: its place in the art's sockets.
    pub point: usize,
    /// The socket a piece is being chosen for.
    pub choosing: Option<Choosing>,
}

/// A piece being chosen from the tray.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Choosing {
    /// The socket chosen for: its place in the art's sockets, as `point`.
    pub socket: usize,
    /// The pointed tile.
    pub tile: usize,
}

/// A tile in the tray: a piece in the pack, or EMPTY, which takes the socket's
/// piece out.
struct Offer {
    /// The piece's copy; `None` for EMPTY.
    piece: Option<CopyId>,
    /// In capitals: the tile's first row and the readout's head.
    name: String,
    /// The tile's second row: the piece's first mod line, or what EMPTY does.
    line: String,
    /// Every mod line, for the readout.
    mods: Vec<String>,
    look: Tile,
}

impl Bench {
    /// The hint row in modding.
    pub fn hint(&self) -> &'static str {
        if self.choosing.is_some() {
            "←→ hardware · space fit · esc back"
        } else {
            "↑↓←→ socket · space choose · m inspect · esc close"
        }
    }

    /// One key: the arrows and hjkl go to the nearest socket that way, space opens
    /// the tray on it; esc or q closes the window. While choosing, the keys work
    /// the tray instead.
    pub fn key(
        &mut self,
        st: &mut AppState,
        art: &dyn Art,
        host: Option<Host>,
        code: KeyCode,
    ) -> ScreenCmd {
        let places = art.sockets();
        if self.choosing.is_some() {
            self.pick(st, &places, host, code);
            return ScreenCmd::Stay;
        }
        let dir = match code {
            KeyCode::Up | KeyCode::Char('k') => Dir::Up,
            KeyCode::Down | KeyCode::Char('j') => Dir::Down,
            KeyCode::Left | KeyCode::Char('h') => Dir::Left,
            KeyCode::Right | KeyCode::Char('l') => Dir::Right,
            KeyCode::Char(' ') => {
                self.choose(st, &places, host);
                return ScreenCmd::Stay;
            }
            KeyCode::Esc | KeyCode::Char('q') => {
                return ScreenCmd::Pop {
                    fx: TransitionKind::None,
                };
            }
            _ => return ScreenCmd::Stay,
        };
        self.point = step(&stops(&places), self.point, dir, |_| true);
        ScreenCmd::Stay
    }

    /// Space on a socket: the tray opens on its first tile. Jacked in, the gear is
    /// locked, so it refuses; with no copy to fit into it opens nothing, and with
    /// nothing to offer it says so in the log.
    fn choose(&mut self, st: &mut AppState, places: &[SocketPlace], host: Option<Host>) {
        if jacked_in(st) {
            st.warn(JACKED_IN);
            return;
        }
        let Some(place) = places.get(self.point) else {
            return;
        };
        if host.is_none() {
            return;
        }
        if tray(st, host, places, Some(place)).is_empty() {
            st.say(format!("no {} in your pack", place.kind.takes()));
            return;
        }
        self.choosing = Some(Choosing {
            socket: self.point,
            tile: 0,
        });
    }

    /// A key while choosing: ←→ and h/l along the tray; space fits the pointed tile
    /// (or takes the piece out for EMPTY) and esc or q changes nothing, both putting
    /// the pointer back on the socket.
    fn pick(
        &mut self,
        st: &mut AppState,
        places: &[SocketPlace],
        host: Option<Host>,
        code: KeyCode,
    ) {
        let Some(Choosing { socket, tile }) = self.choosing else {
            return;
        };
        let Some(place) = places.get(socket) else {
            self.choosing = None;
            return;
        };
        let offers = tray(st, host, places, Some(place));
        let tile = match code {
            KeyCode::Left | KeyCode::Char('h') => tile.saturating_sub(1),
            KeyCode::Right | KeyCode::Char('l') => (tile + 1).min(offers.len().saturating_sub(1)),
            KeyCode::Char(' ') => {
                if let (Some(host), Some(offer)) = (host, offers.get(tile)) {
                    // The item's socket number, not its place in the art's list.
                    let socket = place.index;
                    let action = match offer.piece {
                        Some(piece) => Action::Fit {
                            host,
                            socket,
                            piece,
                        },
                        None => Action::Unfit { host, socket },
                    };
                    let result = perform(st, action);
                    report(st, result);
                }
                self.choosing = None;
                return;
            }
            KeyCode::Esc | KeyCode::Char('q') => {
                self.choosing = None;
                return;
            }
            _ => return,
        };
        self.choosing = Some(Choosing { socket, tile });
    }

    /// The item where inspect draws it, grey but for its sockets (cables and all,
    /// to the body's edges), the tray across the drawing's foot, the brackets on the
    /// pointed socket (or tile, while choosing) and the readout on the body's last
    /// row.
    pub fn draw(
        &self,
        st: &AppState,
        art: &dyn Art,
        host: Option<Host>,
        body: Rect,
        buf: &mut Buffer,
    ) {
        let origin = art.origin(body);
        let places = art.sockets();
        let rects: Vec<Rect> = places.iter().map(|s| on_screen(origin, s)).collect();
        if art.has_back() {
            // Opened up it shows as it is, every socket boxed.
            art.draw_back(origin, body, buf);
            for r in &rects {
                outline(buf, *r, body);
            }
        } else {
            art.draw(origin, body, buf);
            grey_area(buf, body, &rects, PANEL_BG);
        }
        // The tray's band spans only the drawing's own columns, so the cables out
        // either side still show.
        let top = body.bottom().saturating_sub(BAND);
        let band = Rect::new(origin.0, top, art.size().0, BAND).intersection(body);
        solid(buf, band);
        let choosing = self
            .choosing
            .and_then(|c| Some((c, *places.get(c.socket)?)));
        let offers = tray(st, host, &places, choosing.as_ref().map(|(_, p)| p));
        let pointed = choosing.map(|(c, _)| c.tile);
        let tile_at = draw_tray(buf, band, origin.0, art.size().0, &offers, pointed);
        if let Some((c, place)) = choosing {
            outline(buf, rects[c.socket], body);
            if let Some(r) = tile_at {
                brackets(buf, r, st.tick, band);
            }
            if let Some(offer) = offers.get(c.tile) {
                let mut rest = offer.mods.clone();
                if let Some(out) = fitted(st, host, &place) {
                    rest.push(format!("{} back to the pack", name(st, &out.item)));
                }
                let what = format!(" ▸ socket {}", place.name);
                readout(buf, body, &offer.name, &what, &rest);
            }
            return;
        }
        let Some(place) = places.get(self.point) else {
            return;
        };
        brackets(buf, rects[self.point], st.tick, body);
        let head = format!("SOCKET {}", place.name);
        match fitted(st, host, place) {
            Some(piece) => {
                let what = format!(" ▸ {}", name(st, &piece.item));
                readout(buf, body, &head, &what, &mods(st, &piece.item));
            }
            None => {
                let rest = [format!("takes {}", place.kind.takes())];
                readout(buf, body, &head, " ▸ empty", &rest);
            }
        }
    }
}

/// An item's name, or its id when the content does not know it.
fn name(st: &AppState, item: &ItemId) -> String {
    st.content
        .item(item)
        .map_or_else(|| item.to_string(), |i| i.name.clone())
}

/// What a piece of hardware does, a line per mod, or that it has no bonus.
fn mods(st: &AppState, item: &ItemId) -> Vec<String> {
    let hardware = st.content.item(item).and_then(|i| i.hardware.as_ref());
    hardware.map(Hardware::lines).unwrap_or_default()
}

/// The piece in a socket of the host, if any.
fn fitted<'a>(st: &'a AppState, host: Option<Host>, place: &SocketPlace) -> Option<&'a Stack> {
    let copy = st.character.host_copy(host?)?;
    copy.fitted.get(place.index)?.as_ref()
}

/// The tray: the pack's hardware that fits any of the item's sockets, in pack order;
/// while choosing, only what fits the pointed socket, and EMPTY first when it is
/// filled.
fn tray(
    st: &AppState,
    host: Option<Host>,
    places: &[SocketPlace],
    choosing: Option<&SocketPlace>,
) -> Vec<Offer> {
    let empty = choosing
        .filter(|place| fitted(st, host, place).is_some())
        .map(|_| Offer {
            piece: None,
            name: "EMPTY".to_string(),
            line: "take it out".to_string(),
            mods: Vec::new(),
            look: EMPTY_TILE,
        });
    let pieces = st.character.inventory.iter().filter_map(|s| {
        let piece = s.copy_id()?;
        let item = st.content.item(&s.item)?;
        let hardware = item.hardware.as_ref()?;
        let fits = match choosing {
            Some(place) => place.kind == hardware.fits,
            None => places.iter().any(|p| p.kind == hardware.fits),
        };
        if !fits {
            return None;
        }
        let mods = hardware.lines();
        Some(Offer {
            piece: Some(piece),
            name: item.name.to_uppercase(),
            line: mods.first().cloned().unwrap_or_default(),
            mods,
            look: tile(hardware.look),
        })
    });
    empty.into_iter().chain(pieces).collect()
}

/// A tile's width: the swatch, then the longer of its two rows with a space each side.
fn width(offer: &Offer) -> u16 {
    let text = offer.name.chars().count().max(offer.line.chars().count());
    4 + u16::try_from(text).unwrap_or(u16::MAX - 4)
}

/// The tiles shown from `offset`: each with its column from the drawing's left edge
/// and its width, stopping once one would end past `end`.
fn layout(offers: &[Offer], offset: usize, end: u16) -> Vec<(usize, u16, u16)> {
    let mut out = Vec::new();
    let mut x = TILES_X;
    for (k, offer) in offers.iter().enumerate().skip(offset) {
        let w = width(offer);
        if x.saturating_add(w) > end {
            break;
        }
        out.push((k, x, w));
        x = x.saturating_add(w + GAP);
    }
    out
}

/// The tray in `band`, from the drawing's left edge `left`, `art_w` wide: the label,
/// the tiles (scrolled as WAREZ scrolls, just far enough to show the pointed one),
/// `‹` and `›` where more lie off either side, or that nothing fits. Returns the
/// pointed tile's rectangle when it shows.
fn draw_tray(
    buf: &mut Buffer,
    band: Rect,
    left: u16,
    art_w: u16,
    offers: &[Offer],
    pointed: Option<usize>,
) -> Option<Rect> {
    if band.height < BAND {
        return None;
    }
    let y = band.y + 1;
    let at = |x: u16| left.saturating_add(x);
    let bold = |fg: Color| Style::default().fg(fg).add_modifier(Modifier::BOLD);
    let max = band.right();
    put(buf, at(LABEL_X), y, "HARDWARE", bold(LABEL), max);
    if offers.is_empty() {
        let dim = Style::default().fg(DIM);
        put(buf, at(TILES_X), y, "nothing in your pack fits", dim, max);
        return None;
    }
    // A tile ends two short of the drawing's edge: its bracket, then `›`.
    let end = art_w.saturating_sub(2);
    let mut offset = 0;
    if let Some(k) = pointed.filter(|k| *k < offers.len()) {
        while offset < k && !layout(offers, offset, end).iter().any(|(i, ..)| *i == k) {
            offset += 1;
        }
    }
    let shown = layout(offers, offset, end);
    let mut lit = None;
    for &(k, x, w) in &shown {
        let offer = &offers[k];
        let swatch = Style::default()
            .fg(offer.look.swatch.0)
            .bg(offer.look.swatch.1);
        let face = Style::default().fg(offer.look.ink).bg(offer.look.bg);
        let pad = usize::from(w - 3);
        let rows = [
            (&offer.name, face.add_modifier(Modifier::BOLD)),
            (&offer.line, face),
        ];
        for (dy, (text, style)) in (0..).zip(rows) {
            put(buf, at(x), y + dy, "▀▀", swatch, max);
            put(
                buf,
                at(x + 2),
                y + dy,
                &format!(" {text:<pad$}"),
                style,
                max,
            );
        }
        if pointed == Some(k) {
            lit = Some(Rect::new(at(x), y, w, 2));
        }
    }
    if offset > 0 {
        put(buf, at(TILES_X - 2), y, "‹", bold(MARK), max);
    }
    if offset + shown.len() < offers.len() {
        put(buf, at(art_w.saturating_sub(1)), y, "›", bold(MARK), max);
    }
    lit
}

/// A dashed cyan outline one cell outside `r`, nothing written outside `clip`: the
/// socket a piece is being chosen for.
fn outline(buf: &mut Buffer, r: Rect, clip: Rect) {
    let (left, top) = (i32::from(r.x) - 1, i32::from(r.y) - 1);
    let (right, bottom) = (i32::from(r.right()), i32::from(r.bottom()));
    for y in top..=bottom {
        for x in left..=right {
            let glyph = match (x == left, x == right, y == top, y == bottom) {
                (true, _, true, _) => '┌',
                (_, true, true, _) => '┐',
                (true, _, _, true) => '└',
                (_, true, _, true) => '┘',
                (_, _, true, _) | (_, _, _, true) => '╌',
                (true, _, _, _) | (_, true, _, _) => '╎',
                _ => continue,
            };
            let (Ok(x), Ok(y)) = (u16::try_from(x), u16::try_from(y)) else {
                continue;
            };
            if clip.contains((x, y).into()) {
                buf[(x, y)].set_char(glyph).set_fg(CYAN);
            }
        }
    }
}

/// The line above the status row, from the inside's second column: what is pointed
/// at, bold cyan; where it is or what it holds, white; then the rest, each part
/// after a ` · `, dim.
pub(crate) fn readout(buf: &mut Buffer, body: Rect, head: &str, what: &str, rest: &[String]) {
    let tail: String = rest.iter().map(|part| format!(" · {part}")).collect();
    let runs = [
        (head, Style::default().fg(CYAN).add_modifier(Modifier::BOLD)),
        (what, Style::default().fg(WHITE)),
        (&tail, Style::default().fg(DIM)),
    ];
    let y = body.bottom().saturating_sub(1);
    let mut x = body.x + 1;
    for (text, style) in runs {
        put(buf, x, y, text, style, body.right());
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
