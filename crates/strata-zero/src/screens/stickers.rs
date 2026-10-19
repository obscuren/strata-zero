//! The blueprint window's sticker mode: the item grey but where stickers may go, the
//! pack's stickers in a tray that rises over the drawing's foot, one held to move,
//! turn and stick, and the stickers already on it to point at and peel. It knows the
//! item only through its `Art` and the host's copy, so anything worn with a drawing
//! has it.

use crate::actions::{JACKED_IN, jacked_in, perform, report};
use crate::app::{AppState, ScreenCmd, TransitionKind};
use crate::screens::bench::readout;
use crate::ui::art::{Art, Dir, StickerArea, brackets, grey_outside, step};
use crate::ui::chrome::{DIM, PANEL_BG, put, solid};
use crate::ui::cyberdeck::Pen;
use crate::ui::stickers::{draw_at, size};
use crossterm::event::KeyCode;
use game::{Action, Host, ItemId, ItemKind, StickerArt, Turn};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};

/// The tray's label and counts, as WAREZ's on the deck.
const LABEL: Color = Color::Rgb(0x6b, 0x65, 0x75);
/// The tray's scroll marks, as WAREZ's.
const MARK: Color = Color::Rgb(0x8b, 0x85, 0x96);
/// The label's column from the drawing's left edge; the first tile's, 11 on.
const LABEL_X: u16 = 2;
const TILES_X: u16 = LABEL_X + 11;
/// Columns between two tiles: room for both tiles' brackets.
const GAP: u16 = 3;
/// The band's rows round the art: a bracket row above it, then the count row, a
/// bracket row and the readout under it.
const AROUND: u16 = 4;
/// The band when the pack holds no stickers, as modding's.
const EMPTY_BAND: u16 = 5;

/// What the sticker mode keeps between frames, and across switches to the other
/// modes and back.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct StickerBench {
    /// The pointed tile in the tray, kept while the pointer is elsewhere.
    pub tile: usize,
    pub at: At,
}

/// Where the pointer is.
#[derive(Clone, Debug, Default, PartialEq)]
pub enum At {
    /// On the tray's pointed tile.
    #[default]
    Tray,
    /// A sticker held over the drawing.
    Placing(Holding),
    /// On a stuck sticker: its place in the copy's stickers, and whether one more
    /// space peels it.
    Stuck { index: usize, armed: bool },
}

/// A sticker being placed: its top-left cell as turned, in drawing cells.
#[derive(Clone, Debug, PartialEq)]
pub struct Holding {
    pub sticker: ItemId,
    pub x: u16,
    pub y: u16,
    pub turn: Turn,
}

/// A sticker stack in the pack, as its tile shows it.
struct Tile {
    sticker: ItemId,
    name: String,
    qty: u32,
    art: StickerArt,
}

/// A sticker stuck on the host's copy, and where it lies on the drawing.
struct On {
    name: String,
    art: Option<StickerArt>,
    rect: Rect,
}

/// The pack's sticker stacks in pack order.
fn tray(st: &AppState) -> Vec<Tile> {
    st.character
        .inventory
        .iter()
        .filter(|s| s.copy.is_none() && s.qty > 0)
        .filter_map(|s| {
            let item = st.content.item(&s.item)?;
            let art = item
                .sticker
                .clone()
                .filter(|_| item.kind == ItemKind::Sticker)?;
            Some(Tile {
                sticker: s.item.clone(),
                name: item.name.clone(),
                qty: s.qty,
                art,
            })
        })
        .collect()
}

/// The stickers on the host's copy, one per entry, so a place in this list is its
/// place in the copy's; one the content no longer knows has no art and an empty rect.
fn stuck(st: &AppState, host: Option<Host>) -> Vec<On> {
    let Some(copy) = host.and_then(|h| st.character.host_copy(h)) else {
        return Vec::new();
    };
    copy.stickers
        .iter()
        .map(|s| {
            let item = st.content.item(&s.sticker);
            let art = item.and_then(|i| i.sticker.clone());
            let (w, h) = art.as_ref().map_or((0, 0), |a| size(a, s.turn));
            On {
                name: item.map_or_else(|| s.sticker.to_string(), |i| i.name.clone()),
                art,
                rect: Rect::new(s.x, s.y, w, h),
            }
        })
        .collect()
}

/// What kind of sticker it is, for the readout.
fn kind(art: &StickerArt) -> &'static str {
    match art {
        StickerArt::Lettered { .. } => "lettered",
        StickerArt::Picture { .. } => "picture",
    }
}

fn centre(r: Rect) -> (f32, f32) {
    (
        f32::from(r.x) + f32::from(r.width) / 2.0,
        f32::from(r.y) + f32::from(r.height) / 2.0,
    )
}

/// How far apart two points are, in square units: a row counts twice a column.
fn distance((ax, ay): (f32, f32), (bx, by): (f32, f32)) -> f32 {
    let (dx, dy) = (ax - bx, 2.0 * (ay - by));
    dx * dx + dy * dy
}

/// The stuck sticker whose centre lies nearest `to`; ties go to the newer, which lies
/// on top.
fn nearest_on(on: &[On], to: (f32, f32)) -> Option<usize> {
    let d = |o: &On| distance(centre(o.rect), to);
    (0..on.len())
        .rev()
        .min_by(|&a, &b| d(&on[a]).total_cmp(&d(&on[b])))
}

/// The stuck stickers as places for `step`, newest first: its ties go to the earlier
/// place, so of stickers stuck on one spot the arrows reach the one on top.
fn stops(on: &[On]) -> Vec<(usize, Rect)> {
    on.iter()
        .enumerate()
        .rev()
        .map(|(i, o)| (i, o.rect))
        .collect()
}

/// A tile's width: its sticker, level, or its count when that is wider.
fn width(tile: &Tile) -> u16 {
    let count = format!("×{}", tile.qty).chars().count() as u16;
    size(&tile.art, Turn::Level).0.max(count)
}

/// The tiles shown from `offset`: each with its column from the drawing's left edge
/// and its width, stopping once one would end past `end`.
fn layout(tiles: &[Tile], offset: usize, end: u16) -> Vec<(usize, u16, u16)> {
    let mut out = Vec::new();
    let mut x = TILES_X;
    for (k, tile) in tiles.iter().enumerate().skip(offset) {
        let w = width(tile);
        if x.saturating_add(w) > end {
            break;
        }
        out.push((k, x, w));
        x = x.saturating_add(w + GAP);
    }
    out
}

/// The tray scrolled as WAREZ scrolls, just far enough to show the pointed tile, and
/// the tiles it shows. A tile ends two short of the drawing's edge: its bracket, then
/// `›`.
fn shown(tiles: &[Tile], pointed: usize, art_w: u16) -> (usize, Vec<(usize, u16, u16)>) {
    let end = art_w.saturating_sub(2);
    let mut offset = 0;
    while offset < pointed
        && !layout(tiles, offset, end)
            .iter()
            .any(|(k, ..)| *k == pointed)
    {
        offset += 1;
    }
    (offset, layout(tiles, offset, end))
}

/// The band's height while the tray is up: the tallest sticker in it, level, and the
/// rows round it.
fn band_rows(tiles: &[Tile]) -> u16 {
    tiles
        .iter()
        .map(|t| size(&t.art, Turn::Level).1)
        .max()
        .map_or(EMPTY_BAND, |tallest| tallest + AROUND)
}

fn dir_of(code: KeyCode) -> Option<Dir> {
    match code {
        KeyCode::Up | KeyCode::Char('k') => Some(Dir::Up),
        KeyCode::Down | KeyCode::Char('j') => Some(Dir::Down),
        KeyCode::Left | KeyCode::Char('h') => Some(Dir::Left),
        KeyCode::Right | KeyCode::Char('l') => Some(Dir::Right),
        _ => None,
    }
}

fn close() -> ScreenCmd {
    ScreenCmd::Pop {
        fx: TransitionKind::None,
    }
}

impl StickerBench {
    /// Coming into the mode: the tile clamped to the tray, the pointer on the newest
    /// stuck sticker when the pack holds none, no peel armed.
    pub fn enter(&mut self, st: &AppState, host: Option<Host>) {
        let (tiles, on) = (tray(st), stuck(st, host));
        self.tile = self.tile.min(tiles.len().saturating_sub(1));
        self.at = match self.at {
            At::Stuck { index, .. } if index < on.len() => At::Stuck {
                index,
                armed: false,
            },
            _ if tiles.is_empty() && !on.is_empty() => At::Stuck {
                index: on.len() - 1,
                armed: false,
            },
            _ => At::Tray,
        };
    }

    /// A sticker is held or a peel armed: the mode keys go to the mode.
    pub fn busy(&self) -> bool {
        matches!(self.at, At::Placing(_) | At::Stuck { armed: true, .. })
    }

    /// The hint row, by where the pointer is.
    pub fn hint(&self, st: &AppState, host: Option<Host>) -> String {
        let (tiles, on) = (tray(st), stuck(st, host));
        let mut parts: Vec<&str> = Vec::new();
        match self.at {
            At::Tray => {
                if !tiles.is_empty() {
                    parts.extend(["←→ sticker", "space pick"]);
                }
                if !on.is_empty() {
                    parts.push("↑ stuck");
                }
            }
            At::Placing(_) => return "↑↓←→ move · r turn · space stick · esc back".to_string(),
            At::Stuck { .. } => {
                parts.extend(["↑↓←→ sticker", "space peel"]);
                if !tiles.is_empty() {
                    parts.push("↓ tray");
                }
            }
        }
        parts.extend(["s inspect", "esc close"]);
        parts.join(" · ")
    }

    /// One key, by where the pointer is. Esc or q puts a held sticker back and
    /// disarms a peel, else closes the window.
    pub fn key(
        &mut self,
        st: &mut AppState,
        art: &dyn Art,
        host: Option<Host>,
        code: KeyCode,
    ) -> ScreenCmd {
        let Some(area) = art.sticker_area() else {
            return match code {
                KeyCode::Esc | KeyCode::Char('q') => close(),
                _ => ScreenCmd::Stay,
            };
        };
        match self.at.clone() {
            At::Tray => self.in_tray(st, art, &area, host, code),
            At::Placing(h) => {
                self.placing(st, &area, host, h, code);
                ScreenCmd::Stay
            }
            At::Stuck { index, armed } => self.on_stuck(st, host, index, armed, code),
        }
    }

    /// ←→ along the tray, space picks, ↑ to the stickers on the drawing.
    fn in_tray(
        &mut self,
        st: &mut AppState,
        art: &dyn Art,
        area: &StickerArea,
        host: Option<Host>,
        code: KeyCode,
    ) -> ScreenCmd {
        let tiles = tray(st);
        match code {
            KeyCode::Left | KeyCode::Char('h') => self.tile = self.tile.saturating_sub(1),
            KeyCode::Right | KeyCode::Char('l') => {
                self.tile = (self.tile + 1).min(tiles.len().saturating_sub(1));
            }
            KeyCode::Char(' ') => self.pick(st, art, area, host, &tiles),
            KeyCode::Up | KeyCode::Char('k') => {
                // The stuck sticker nearest the pointed tile, the tray lying under the
                // drawing's foot.
                let (art_w, art_h) = art.size();
                let (_, layout) = shown(&tiles, self.tile, art_w);
                let x = layout
                    .iter()
                    .find(|(k, ..)| *k == self.tile)
                    .map_or(0.0, |&(_, x, w)| f32::from(x) + f32::from(w) / 2.0);
                if let Some(index) = nearest_on(&stuck(st, host), (x, f32::from(art_h))) {
                    self.at = At::Stuck {
                        index,
                        armed: false,
                    };
                }
            }
            KeyCode::Esc | KeyCode::Char('q') => return close(),
            _ => {}
        }
        ScreenCmd::Stay
    }

    /// Space on a tile: the sticker on the drawing where it starts, or a line in the
    /// log when it fits nowhere. Jacked in, the gear is locked.
    fn pick(
        &mut self,
        st: &mut AppState,
        art: &dyn Art,
        area: &StickerArea,
        host: Option<Host>,
        tiles: &[Tile],
    ) {
        if jacked_in(st) {
            st.warn(JACKED_IN);
            return;
        }
        let (Some(host), Some(tile)) = (host, tiles.get(self.tile)) else {
            return;
        };
        match start(area, &tile.art, art.size()) {
            Some((turn, x, y)) => {
                self.at = At::Placing(Holding {
                    sticker: tile.sticker.clone(),
                    x,
                    y,
                    turn,
                });
            }
            None => {
                let item = st
                    .character
                    .host_item(host)
                    .and_then(|i| st.content.item(i));
                let on = item.map_or_else(String::new, |i| i.name.clone());
                st.say(format!("`{}` doesn't fit on the `{on}`", tile.name));
            }
        }
    }

    /// A held sticker: the arrows move it, r turns it, space sticks it, esc puts it
    /// back.
    fn placing(
        &mut self,
        st: &mut AppState,
        area: &StickerArea,
        host: Option<Host>,
        mut h: Holding,
        code: KeyCode,
    ) {
        let Some(art) = st.content.item(&h.sticker).and_then(|i| i.sticker.clone()) else {
            self.at = At::Tray;
            return;
        };
        if let Some(dir) = dir_of(code) {
            (h.x, h.y) = slide(area, &art, &h, dir);
            self.at = At::Placing(h);
            return;
        }
        match code {
            KeyCode::Char('r') => {
                if let Some(t) = turned(area, &art, &h) {
                    self.at = At::Placing(t);
                }
            }
            KeyCode::Char(' ') => {
                if let Some(host) = host {
                    let stick = Action::Stick {
                        host,
                        sticker: h.sticker.clone(),
                        x: h.x,
                        y: h.y,
                        turn: h.turn,
                    };
                    let result = perform(st, stick);
                    report(st, result);
                }
                let (tiles, on) = (tray(st), stuck(st, host));
                self.tile = self.tile.min(tiles.len().saturating_sub(1));
                self.at = match on.len() {
                    n if tiles.is_empty() && n > 0 => At::Stuck {
                        index: n - 1,
                        armed: false,
                    },
                    _ => At::Tray,
                };
            }
            KeyCode::Esc | KeyCode::Char('q') => self.at = At::Tray,
            _ => {}
        }
    }

    /// On a stuck sticker: the arrows step between them, ↓ with none below to the
    /// tray, space arms a peel and a second space peels it; any other key disarms and
    /// then does what it does, esc only disarms.
    fn on_stuck(
        &mut self,
        st: &mut AppState,
        host: Option<Host>,
        index: usize,
        armed: bool,
        code: KeyCode,
    ) -> ScreenCmd {
        let on = stuck(st, host);
        let Some(here) = on.get(index) else {
            self.at = At::Tray;
            return ScreenCmd::Stay;
        };
        if armed {
            self.at = At::Stuck {
                index,
                armed: false,
            };
            match code {
                KeyCode::Char(' ') => {
                    if let Some(host) = host {
                        let result = perform(st, Action::Peel { host, index });
                        report(st, result);
                    }
                    let left = stuck(st, host);
                    self.at = match nearest_on(&left, centre(here.rect)) {
                        Some(index) => At::Stuck {
                            index,
                            armed: false,
                        },
                        None => At::Tray,
                    };
                    return ScreenCmd::Stay;
                }
                KeyCode::Esc | KeyCode::Char('q') => return ScreenCmd::Stay,
                _ => {}
            }
        }
        if let Some(dir) = dir_of(code) {
            let next = step(&stops(&on), index, dir, |_| true);
            if next != index {
                self.at = At::Stuck {
                    index: next,
                    armed: false,
                };
            } else if dir == Dir::Down && !tray(st).is_empty() {
                self.at = At::Tray;
            }
            return ScreenCmd::Stay;
        }
        match code {
            KeyCode::Char(' ') if jacked_in(st) => st.warn(JACKED_IN),
            KeyCode::Char(' ') => self.at = At::Stuck { index, armed: true },
            KeyCode::Esc | KeyCode::Char('q') => return close(),
            _ => {}
        }
        ScreenCmd::Stay
    }

    /// The item where inspect draws it, grey off its sticker area (cables and all, to
    /// the body's edges), the held sticker over it, the band across the drawing's
    /// columns at the body's foot (the tray risen while the pointer is in it, else the
    /// readout row alone), the brackets and the readout on the body's last row.
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
        let Some(area) = art.sticker_area() else {
            return;
        };
        grey_outside(buf, body, origin, &area, PANEL_BG);
        let on_screen = |r: Rect| Rect::new(origin.0 + r.x, origin.1 + r.y, r.width, r.height);
        let held = match &self.at {
            At::Placing(h) => st.content.item(&h.sticker).and_then(|i| {
                let sticker = i.sticker.clone()?;
                Some((h, i.name.to_uppercase(), sticker))
            }),
            _ => None,
        };
        if let Some((h, _, sticker)) = &held {
            let mut pen = Pen {
                clip: body.intersection(buf.area),
                buf: &mut *buf,
                ox: origin.0,
                oy: origin.1,
            };
            draw_at(sticker, h.turn, h.x, h.y, &mut pen);
        }
        let tiles = tray(st);
        let art_w = art.size().0;
        let rows = match self.at {
            At::Tray => band_rows(&tiles),
            _ => 1,
        };
        let band = Rect::new(origin.0, body.bottom().saturating_sub(rows), art_w, rows);
        let band = band.intersection(body);
        solid(buf, band);
        match &self.at {
            At::Tray => {
                if band.height == rows {
                    self.draw_tray(st, buf, band, origin.0, art_w, &tiles);
                }
                if let Some(tile) = tiles.get(self.tile) {
                    let what = format!(" ▸ {} in your pack", tile.qty);
                    let rest = [kind(&tile.art).to_string()];
                    readout(buf, body, &tile.name.to_uppercase(), &what, &rest);
                }
            }
            At::Placing(_) => {
                let Some((h, name, sticker)) = &held else {
                    return;
                };
                let (w, hh) = size(sticker, h.turn);
                brackets(buf, on_screen(Rect::new(h.x, h.y, w, hh)), st.tick, body);
                let what = match h.turn {
                    Turn::Level => " ▸ level".to_string(),
                    t => format!(" ▸ turned {}°", t.degrees()),
                };
                let rest = ["space sticks it", "it tears if peeled"].map(String::from);
                readout(buf, body, name, &what, &rest);
            }
            At::Stuck { index, armed } => {
                let on = stuck(st, host);
                let Some(o) = on.get(*index) else {
                    return;
                };
                brackets(buf, on_screen(o.rect), st.tick, body);
                let head = o.name.to_uppercase();
                if *armed {
                    let rest = ["it tears", "any other key keeps it"].map(String::from);
                    readout(buf, body, &head, " ▸ space again peels it", &rest);
                } else {
                    let rest: Vec<String> = o.art.iter().map(|a| kind(a).to_string()).collect();
                    readout(buf, body, &head, " ▸ stuck", &rest);
                }
            }
        }
    }

    /// The tray in `band`, from the drawing's left edge `left`: the label, the tiles
    /// drawn as they are (bottom-aligned on the count row) with their counts, `‹` and
    /// `›` where more lie off either side, the brackets on the pointed one; or that the
    /// pack holds none.
    fn draw_tray(
        &self,
        st: &AppState,
        buf: &mut Buffer,
        band: Rect,
        left: u16,
        art_w: u16,
        tiles: &[Tile],
    ) {
        let y = band.y + 1;
        let at = |x: u16| left.saturating_add(x);
        let max = band.right();
        let bold = |fg: Color| Style::default().fg(fg).add_modifier(Modifier::BOLD);
        put(buf, at(LABEL_X), y, "STICKERS", bold(LABEL), max);
        if tiles.is_empty() {
            let dim = Style::default().fg(DIM);
            put(buf, at(TILES_X), y, "no stickers in your pack", dim, max);
            return;
        }
        let tallest = band.height - AROUND;
        let (offset, layout) = shown(tiles, self.tile, art_w);
        for &(k, x, w) in &layout {
            let tile = &tiles[k];
            let h = size(&tile.art, Turn::Level).1;
            let mut pen = Pen {
                clip: band,
                buf: &mut *buf,
                ox: at(x),
                oy: y + tallest - h,
            };
            draw_at(&tile.art, Turn::Level, 0, 0, &mut pen);
            let count = Style::default().fg(LABEL);
            put(
                buf,
                at(x),
                y + tallest,
                &format!("×{}", tile.qty),
                count,
                max,
            );
            if k == self.tile {
                brackets(buf, Rect::new(at(x), y, w, tallest + 1), st.tick, band);
            }
        }
        if offset > 0 {
            put(buf, at(TILES_X - 2), y, "‹", bold(MARK), max);
        }
        if offset + layout.len() < tiles.len() {
            put(buf, at(art_w.saturating_sub(1)), y, "›", bold(MARK), max);
        }
    }
}

/// Where a picked sticker starts: the first turn, level first, at which it fits
/// anywhere, on the fitting spot nearest the drawing's centre.
fn start(area: &StickerArea, art: &StickerArt, (w, h): (u16, u16)) -> Option<(Turn, u16, u16)> {
    let middle = (f32::from(w) / 2.0, f32::from(h) / 2.0);
    Turn::ALL
        .into_iter()
        .find_map(|turn| area.nearest(art, turn, middle).map(|(x, y)| (turn, x, y)))
}

/// The spot an arrow takes the held sticker to: a cell that way, or on to the first
/// spot that way where it fits; where it was when it reaches the edge first.
fn slide(area: &StickerArea, art: &StickerArt, h: &Holding, dir: Dir) -> (u16, u16) {
    let (dx, dy) = match dir {
        Dir::Up => (0, -1),
        Dir::Down => (0, 1),
        Dir::Left => (-1, 0),
        Dir::Right => (1, 0),
    };
    let (w, hh) = area.size();
    let (mut x, mut y) = (i32::from(h.x), i32::from(h.y));
    loop {
        (x, y) = (x + dx, y + dy);
        let (Ok(cx), Ok(cy)) = (u16::try_from(x), u16::try_from(y)) else {
            return (h.x, h.y);
        };
        if cx >= w || cy >= hh {
            return (h.x, h.y);
        }
        if area.fits(art, h.turn, x, y) {
            return (cx, cy);
        }
    }
}

/// The held sticker turned a quarter further clockwise about its centre, onto the
/// fitting spot nearest it; a turn at which it fits nowhere is skipped, and `None`
/// when no other turn fits.
fn turned(area: &StickerArea, art: &StickerArt, h: &Holding) -> Option<Holding> {
    let (w, hh) = size(art, h.turn);
    let middle = centre(Rect::new(h.x, h.y, w, hh));
    let mut turn = h.turn;
    for _ in 0..3 {
        turn = turn.cw();
        if let Some((x, y)) = area.nearest(art, turn, middle) {
            return Some(Holding {
                sticker: h.sticker.clone(),
                x,
                y,
                turn,
            });
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::Screen;
    use crate::screens::blueprint::{Blueprint, Mode};
    use crate::screens::deck::Deck;
    use crate::screens::testutil::{buffer_text, key};
    use crate::ui::stickers::size;
    use citygen::{GenParams, Generator, SizePreset};
    use game::{Character, ItemKind, Slot, Stuck};
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use ratatui::style::{Color, Modifier};

    const LABEL: Color = Color::Rgb(0x6b, 0x65, 0x75);

    fn id(s: &str) -> ItemId {
        ItemId::new(s)
    }

    /// D'SHAWN in a small city, his worn items' blueprints at hand.
    fn dshawn() -> AppState {
        let mut st = AppState::new();
        st.city = Some(
            Generator::run(GenParams {
                seed: 6,
                size: SizePreset::Custom { w: 120, h: 60 },
                districts: 5,
                ..Default::default()
            })
            .unwrap(),
        );
        st.character = Character::debug_runner(&st.content);
        st.tick = 40;
        st
    }

    /// The blueprint on what a slot wears, opened from GEAR.
    fn worn(st: &AppState, item: &str, slot: Slot) -> Blueprint {
        let copy = st.character.worn_copy(slot).map(|k| k.id);
        Blueprint::open(st, id(item), true, copy)
    }

    fn press(bp: &mut Blueprint, st: &mut AppState, codes: &[KeyCode]) -> ScreenCmd {
        let keys: Vec<_> = codes.iter().map(|c| key(*c)).collect();
        bp.update(st, &keys)
    }

    fn times(bp: &mut Blueprint, st: &mut AppState, code: KeyCode, n: usize) {
        press(bp, st, &vec![code; n]);
    }

    fn screen(bp: &Blueprint, st: &AppState, w: u16, h: u16) -> Buffer {
        let mut t = Terminal::new(TestBackend::new(w, h)).unwrap();
        t.draw(|f| {
            Deck::new(0).draw(st, f);
            crate::ui::dim::wash(f.buffer_mut());
            bp.draw(st, f);
        })
        .unwrap();
        t.backend().buffer().clone()
    }

    /// The text inside the window's frame on row `dy` from its top edge, trimmed.
    fn inside(buf: &Buffer, dy: u16) -> String {
        let r = Blueprint::rect(buf.area);
        let line: String = (r.x + 1..r.right() - 1)
            .map(|x| buf[(x, r.y + dy)].symbol())
            .collect();
        line.trim().to_string()
    }

    fn title(buf: &Buffer) -> String {
        inside(buf, 0)
    }

    fn readout(buf: &Buffer) -> String {
        inside(buf, Blueprint::rect(buf.area).height - 4)
    }

    fn hint(buf: &Buffer) -> String {
        inside(buf, Blueprint::rect(buf.area).height - 2)
    }

    fn text_at(buf: &Buffer, x: u16, y: u16, n: u16) -> String {
        (x..x + n).map(|cx| buf[(cx, y)].symbol()).collect()
    }

    /// The drawing's sticker area and its size, as the window draws it.
    fn area_of(bp: &Blueprint, st: &AppState) -> (StickerArea, (u16, u16)) {
        let art = bp.drawing(st).expect("a drawing");
        (art.sticker_area().expect("takes stickers"), art.size())
    }

    fn art_of(st: &AppState, sticker: &str) -> StickerArt {
        st.content
            .item(&id(sticker))
            .unwrap()
            .sticker
            .clone()
            .unwrap()
    }

    fn holding(bp: &Blueprint) -> Holding {
        match &bp.stickers.at {
            At::Placing(h) => h.clone(),
            at => panic!("not placing: {at:?}"),
        }
    }

    fn stuck(st: &AppState, slot: Slot) -> Vec<Stuck> {
        st.character.worn_copy(slot).unwrap().stickers.clone()
    }

    fn last_log(st: &AppState) -> String {
        st.log.latest().map(|e| e.text.clone()).unwrap_or_default()
    }

    #[test]
    fn s_opens_the_mode_on_a_drawn_worn_item() {
        let mut st = dshawn();
        let mut bp = worn(&st, "rebuilt_deck", Slot::Deck);
        let buf = screen(&bp, &st, 100, 30);
        assert!(
            hint(&buf).ends_with("space act · m mod · s stickers · esc close"),
            "{}",
            hint(&buf)
        );
        press(&mut bp, &mut st, &[KeyCode::Char('s')]);
        assert_eq!(bp.mode, Mode::Stickers);
        let buf = screen(&bp, &st, 100, 30);
        assert!(
            title(&buf).contains(" STICKERS · REBUILT PAWN DECK "),
            "{}",
            title(&buf)
        );
        assert_eq!(
            hint(&buf),
            "←→ sticker · space pick · ↑ stuck · s inspect · esc close"
        );
        press(&mut bp, &mut st, &[KeyCode::Char('s')]);
        assert_eq!(bp.mode, Mode::Inspect);
        assert!(title(&screen(&bp, &st, 100, 30)).contains(" BLUEPRINT · REBUILT PAWN DECK "));
        // Straight between the two modes.
        press(&mut bp, &mut st, &[KeyCode::Char('s'), KeyCode::Char('m')]);
        assert_eq!(bp.mode, Mode::Mod);
        press(&mut bp, &mut st, &[KeyCode::Char('s')]);
        assert_eq!(bp.mode, Mode::Stickers);

        // The Ratchet has it too.
        let mut gun = worn(&st, "ratchet_smg", Slot::Primary);
        press(&mut gun, &mut st, &[KeyCode::Char('s')]);
        assert_eq!(gun.mode, Mode::Stickers);

        // Hardware, an item with no drawing, and a deck held without a copy: no `s`.
        let piece = st
            .character
            .inventory
            .iter()
            .find(|s| s.item == id("coldfin_2"))
            .and_then(|s| s.copy_id());
        for mut other in [
            Blueprint::open(&st, id("coldfin_2"), false, piece),
            Blueprint::open(&st, id("salvage_jacket"), false, None),
            Blueprint::open(&st, id("pawnshop_deck"), false, None),
        ] {
            let before = buffer_text(&screen(&other, &st, 100, 30));
            assert!(!hint(&screen(&other, &st, 100, 30)).contains("s stickers"));
            assert!(matches!(
                press(&mut other, &mut st, &[KeyCode::Char('s')]),
                ScreenCmd::Stay
            ));
            assert_eq!(other.mode, Mode::Inspect);
            assert_eq!(buffer_text(&screen(&other, &st, 100, 30)), before);
        }
    }

    #[test]
    fn the_tray_rises_with_the_stickers_drawn_as_they_are() {
        let mut st = dshawn();
        let mut bp = worn(&st, "rebuilt_deck", Slot::Deck);
        press(&mut bp, &mut st, &[KeyCode::Char('s')]);
        let buf = screen(&bp, &st, 100, 30);
        // The body is rows 2..26 and the deck's origin (8, 3) at 100×30. The tallest
        // sticker in his pack is six rows, so the band is ten: rows 16 to 25.
        let (ox, top) = (8, 16);
        assert_eq!(text_at(&buf, ox + 2, top + 1, 8), "STICKERS");
        let label = &buf[(ox + 2, top + 1)];
        assert_eq!(label.fg, LABEL);
        assert!(label.modifier.contains(Modifier::BOLD));
        // The first tile, his first stack: NO FUTURE, its count under it.
        assert_eq!(text_at(&buf, ox + 13, top + 6, 11), " NO FUTURE ");
        assert_eq!(buf[(ox + 14, top + 6)].bg, Color::Rgb(0xff, 0x00, 0xc8));
        assert_eq!(text_at(&buf, ox + 13, top + 7, 2), "×2");
        assert_eq!(buf[(ox + 13, top + 7)].fg, LABEL);
        // Brackets one cell outside its art and count.
        for (x, y, corner) in [
            (ox + 12, top, "┏"),
            (ox + 24, top, "┓"),
            (ox + 12, top + 8, "┗"),
            (ox + 24, top + 8, "┛"),
        ] {
            assert_eq!(buf[(x, y)].symbol(), corner, "({x},{y})");
        }
        assert_eq!(readout(&buf), "NO FUTURE ▸ 2 in your pack · lettered");
        // Above the band the deck is grey off the sticker area: the maker plate...
        let plate = &buf[(ox + 5, 3 + 5)];
        assert_eq!(
            plate.fg,
            crate::ui::art::grey(Color::Rgb(0x8b, 0x85, 0x96), crate::ui::chrome::PANEL_BG)
        );
        assert!(!plate.modifier.contains(Modifier::BOLD));
        // ...and in colour on it: the factory NO FUTURE stuck at (50, 10).
        let sticker = &buf[(ox + 51, 3 + 10)];
        assert_eq!(
            (sticker.symbol(), sticker.fg, sticker.bg),
            (
                "N",
                Color::Rgb(0xe8, 0xe8, 0xec),
                Color::Rgb(0xff, 0x00, 0xc8)
            )
        );
        // Under the band nothing of the deck shows.
        assert_eq!(buf[(ox + 40, top + 2)].symbol(), " ");
    }

    #[test]
    fn the_tray_scrolls_to_the_pointed_sticker() {
        let mut st = dshawn();
        for (w, h) in [(100, 30), (120, 40)] {
            let mut bp = worn(&st, "rebuilt_deck", Slot::Deck);
            press(&mut bp, &mut st, &[KeyCode::Char('s')]);
            times(&mut bp, &mut st, KeyCode::Right, 28);
            assert_eq!(bp.stickers.tile, 28);
            times(&mut bp, &mut st, KeyCode::Right, 3);
            assert_eq!(bp.stickers.tile, 28, "stops at the last");
            let buf = screen(&bp, &st, w, h);
            assert_eq!(readout(&buf), "CORP SEAL ▸ 2 in your pack · picture");
            let r = Blueprint::rect(buf.area);
            let body = Rect::new(r.x + 1, r.y + 1, r.width - 2, r.height - 4);
            let ox = body.x + (body.width - 84) / 2;
            let top = body.bottom() - 10;
            assert!(top >= body.y, "the band stays in the body");
            assert_eq!(buf[(ox + 11, top + 1)].symbol(), "‹");
            assert_ne!(buf[(ox + 83, top + 1)].symbol(), "›");
            // The pointed tile shows whole, its brackets inside the drawing's columns.
            let corners: Vec<u16> = (ox..ox + 84)
                .filter(|&x| buf[(x, top)].symbol() == "┓")
                .collect();
            assert_eq!(corners.len(), 1);
            assert!(corners[0] < ox + 83);
            times(&mut bp, &mut st, KeyCode::Left, 28);
            let buf = screen(&bp, &st, w, h);
            assert_eq!(buf[(ox + 83, top + 1)].symbol(), "›");
            assert_ne!(buf[(ox + 11, top + 1)].symbol(), "‹");
        }
    }

    #[test]
    fn picking_places_the_sticker_where_it_fits_nearest_the_centre() {
        let mut st = dshawn();
        let mut bp = worn(&st, "rebuilt_deck", Slot::Deck);
        press(&mut bp, &mut st, &[KeyCode::Char('s'), KeyCode::Char(' ')]);
        let (area, (w, h)) = area_of(&bp, &st);
        let art = art_of(&st, "sticker_no_future");
        let at = area.nearest(&art, Turn::Level, (f32::from(w) / 2.0, f32::from(h) / 2.0));
        let held = holding(&bp);
        assert_eq!(
            (held.sticker.clone(), held.turn),
            (id("sticker_no_future"), Turn::Level)
        );
        assert_eq!(Some((held.x, held.y)), at);
        let buf = screen(&bp, &st, 100, 30);
        assert_eq!(
            readout(&buf),
            "NO FUTURE ▸ level · space sticks it · it tears if peeled"
        );
        assert_eq!(hint(&buf), "↑↓←→ move · r turn · space stick · esc back");
        // The tray has sunk: the keys show again, in colour.
        assert_eq!(buf[(8 + 40, 3 + 15)].symbol(), "▆");
        // Mode keys do nothing while it is held.
        press(&mut bp, &mut st, &[KeyCode::Char('s'), KeyCode::Char('m')]);
        assert_eq!(bp.mode, Mode::Stickers);
        assert_eq!(holding(&bp), held);
    }

    #[test]
    fn arrows_slide_over_spots_where_it_does_not_fit() {
        let mut st = dshawn();
        let mut gun = worn(&st, "ratchet_smg", Slot::Primary);
        press(&mut gun, &mut st, &[KeyCode::Char('s')]);
        // FLATLINE, his sixth stack: ten cells, one row.
        times(&mut gun, &mut st, KeyCode::Right, 5);
        press(&mut gun, &mut st, &[KeyCode::Char(' ')]);
        let held = holding(&gun);
        assert_eq!(
            (held.sticker.clone(), held.x, held.y),
            (id("sticker_flatline"), 39, 7)
        );
        press(&mut gun, &mut st, &[KeyCode::Right]);
        assert_eq!((holding(&gun).x, holding(&gun).y), (40, 7));
        // Left as far as it goes: the receiver's left edge, never off the area.
        times(&mut gun, &mut st, KeyCode::Left, 30);
        assert_eq!((holding(&gun).x, holding(&gun).y), (24, 7));
        press(&mut gun, &mut st, &[KeyCode::Down]);
        assert_eq!((holding(&gun).x, holding(&gun).y), (24, 8));
        // Nothing below fits ten wide at x 24: it stays.
        press(&mut gun, &mut st, &[KeyCode::Char('j')]);
        assert_eq!((holding(&gun).x, holding(&gun).y), (24, 8));
        // Over the grip, ten wide, it goes down onto it.
        times(&mut gun, &mut st, KeyCode::Char('l'), 14);
        press(&mut gun, &mut st, &[KeyCode::Down]);
        assert_eq!((holding(&gun).x, holding(&gun).y), (38, 9));
        let (area, _) = area_of(&gun, &st);
        let art = art_of(&st, "sticker_flatline");
        assert!(area.fits(&art, Turn::Level, 38, 9));
    }

    #[test]
    fn r_turns_about_the_centre_and_skips_turns_that_fit_nowhere() {
        let mut st = dshawn();
        let mut bp = worn(&st, "rebuilt_deck", Slot::Deck);
        press(
            &mut bp,
            &mut st,
            &[
                KeyCode::Char('s'),
                KeyCode::Right,
                KeyCode::Right,
                KeyCode::Char(' '),
            ],
        );
        let before = holding(&bp);
        assert_eq!(before.sticker, id("sticker_arasaka"));
        let (area, _) = area_of(&bp, &st);
        let art = art_of(&st, "sticker_arasaka");
        let (w, h) = size(&art, Turn::Level);
        let centre = (
            f32::from(before.x) + f32::from(w) / 2.0,
            f32::from(before.y) + f32::from(h) / 2.0,
        );
        press(&mut bp, &mut st, &[KeyCode::Char('r')]);
        let after = holding(&bp);
        assert_eq!(after.turn, Turn::Quarter);
        assert_eq!(
            Some((after.x, after.y)),
            area.nearest(&art, Turn::Quarter, centre)
        );
        assert!(readout(&screen(&bp, &st, 100, 30)).contains("▸ turned 90°"));

        // Stood up, CHROME > FLESH is sixteen rows: the gun has no room, so r skips to 180°.
        let mut gun = worn(&st, "ratchet_smg", Slot::Primary);
        press(&mut gun, &mut st, &[KeyCode::Char('s')]);
        times(&mut gun, &mut st, KeyCode::Right, 7);
        press(&mut gun, &mut st, &[KeyCode::Char(' '), KeyCode::Char('r')]);
        let held = holding(&gun);
        assert_eq!(
            (held.sticker.clone(), held.turn),
            (id("sticker_chrome_flesh"), Turn::Half)
        );
        assert!(readout(&screen(&gun, &st, 100, 30)).contains("▸ turned 180°"));

        // A sticker that fits only level does not turn at all.
        let one = StickerArea::new(1, 1, &[(0, 0, 0, 0)], &[]);
        let dot = StickerArt::Picture {
            rows: vec!["a..".into(), "...".into()],
            colours: [('a', (1, 1, 1))].into_iter().collect(),
        };
        let h = Holding {
            sticker: id("x"),
            x: 0,
            y: 0,
            turn: Turn::Level,
        };
        assert!(one.fits(&dot, Turn::Level, 0, 0));
        assert_eq!(turned(&one, &dot, &h), None);
        assert_eq!(slide(&one, &dot, &h, Dir::Right), (0, 0));
        assert_eq!(start(&one, &dot, (1, 1)), Some((Turn::Level, 0, 0)));
    }

    #[test]
    fn space_sticks_it_and_esc_puts_it_back() {
        let mut st = dshawn();
        let mut bp = worn(&st, "rebuilt_deck", Slot::Deck);
        let before = (
            stuck(&st, Slot::Deck),
            st.character.count(&id("sticker_arasaka")),
        );
        press(
            &mut bp,
            &mut st,
            &[
                KeyCode::Char('s'),
                KeyCode::Right,
                KeyCode::Right,
                KeyCode::Char(' '),
            ],
        );
        press(&mut bp, &mut st, &[KeyCode::Esc]);
        assert_eq!(bp.stickers.at, At::Tray);
        assert_eq!(bp.stickers.tile, 2);
        assert_eq!(
            (
                stuck(&st, Slot::Deck),
                st.character.count(&id("sticker_arasaka"))
            ),
            before
        );
        press(&mut bp, &mut st, &[KeyCode::Char(' ')]);
        let held = holding(&bp);
        press(&mut bp, &mut st, &[KeyCode::Char(' ')]);
        let now = stuck(&st, Slot::Deck);
        assert_eq!(now.len(), before.0.len() + 1);
        assert_eq!(
            now.last(),
            Some(&Stuck {
                sticker: id("sticker_arasaka"),
                x: held.x,
                y: held.y,
                turn: held.turn
            })
        );
        assert_eq!(st.character.count(&id("sticker_arasaka")), before.1 - 1);
        // The log lights the name the message quotes.
        assert_eq!(last_log(&st), "Arasaka stuck");
        assert_eq!((bp.stickers.at.clone(), bp.stickers.tile), (At::Tray, 2));
    }

    #[test]
    fn sticking_the_last_one_points_at_it() {
        let mut st = dshawn();
        let content = st.content.clone();
        st.character.inventory.retain(|s| {
            content
                .item(&s.item)
                .is_none_or(|i| i.kind != ItemKind::Sticker)
        });
        st.character.give(id("sticker_neko"), 1);
        let mut bp = worn(&st, "rebuilt_deck", Slot::Deck);
        press(
            &mut bp,
            &mut st,
            &[KeyCode::Char('s'), KeyCode::Char(' '), KeyCode::Char(' ')],
        );
        assert_eq!(st.character.count(&id("sticker_neko")), 0);
        assert_eq!(
            bp.stickers.at,
            At::Stuck {
                index: 2,
                armed: false
            }
        );
        let buf = screen(&bp, &st, 100, 30);
        assert!(!hint(&buf).contains("↓ tray"), "{}", hint(&buf));
        assert_eq!(readout(&buf), "NEKO ▸ stuck · picture");
        // With nothing in the pack, ↓ never leaves the stuck stickers for the tray.
        times(&mut bp, &mut st, KeyCode::Down, 5);
        assert!(matches!(bp.stickers.at, At::Stuck { .. }));
        // Back in the mode later with an empty pack: the tray says so.
        press(&mut bp, &mut st, &[KeyCode::Char('s'), KeyCode::Char('s')]);
        assert!(matches!(bp.stickers.at, At::Stuck { .. }));
    }

    #[test]
    fn a_sticker_that_fits_nowhere_is_not_picked() {
        let mut st = dshawn();
        let mut gun = worn(&st, "ratchet_smg", Slot::Primary);
        press(&mut gun, &mut st, &[KeyCode::Char('s')]);
        times(&mut gun, &mut st, KeyCode::Right, 27);
        press(&mut gun, &mut st, &[KeyCode::Char(' ')]);
        assert_eq!(gun.stickers.at, At::Tray);
        assert_eq!(last_log(&st), "Scan me doesn't fit on the Ratchet SMG");
    }
    /// Jacks the player into the construct of the first building, body on the street.
    fn jack_in(st: &mut AppState) {
        use citygen::{Place, Pos, StratumKind};
        let b = st.city.as_ref().unwrap().buildings[0].id;
        let mut p = crate::walker::Player::at(Place::Construct(b), Pos::new(1, 1));
        p.body = Some((Place::Stratum(StratumKind::Street), Pos::new(1, 1)));
        st.player = Some(p);
    }

    fn at(bp: &Blueprint) -> At {
        bp.stickers.at.clone()
    }

    fn on(index: usize) -> At {
        At::Stuck {
            index,
            armed: false,
        }
    }

    #[test]
    fn up_from_the_tray_points_at_the_stuck_stickers() {
        let mut st = dshawn();
        let mut bp = worn(&st, "rebuilt_deck", Slot::Deck);
        press(&mut bp, &mut st, &[KeyCode::Char('s'), KeyCode::Up]);
        // From the first tile, the nearer factory sticker is x‿x at (45, 13).
        assert_eq!(at(&bp), on(1));
        let buf = screen(&bp, &st, 100, 30);
        let (ox, oy) = (8, 3);
        for (x, y, corner) in [
            (ox + 44, oy + 12, "┏"),
            (ox + 50, oy + 12, "┓"),
            (ox + 44, oy + 14, "┗"),
            (ox + 50, oy + 14, "┛"),
        ] {
            assert_eq!(buf[(x, y)].symbol(), corner, "({x},{y})");
        }
        // The tray has sunk to the readout row: the keys show.
        assert_eq!(buf[(ox + 40, oy + 15)].symbol(), "▆");
        assert_eq!(readout(&buf), "DEAD FACE ▸ stuck · lettered");
        assert_eq!(
            hint(&buf),
            "↑↓←→ sticker · space peel · ↓ tray · s inspect · esc close"
        );
        // Right to NO FUTURE, down again to x‿x, down once more to the tray.
        press(&mut bp, &mut st, &[KeyCode::Right]);
        assert_eq!(at(&bp), on(0));
        assert_eq!(
            readout(&screen(&bp, &st, 100, 30)),
            "NO FUTURE ▸ stuck · lettered"
        );
        press(&mut bp, &mut st, &[KeyCode::Char('j')]);
        assert_eq!(at(&bp), on(1));
        press(&mut bp, &mut st, &[KeyCode::Down]);
        assert_eq!((at(&bp), bp.stickers.tile), (At::Tray, 0));
    }

    #[test]
    fn peeling_takes_two_spaces_and_tears_it() {
        let mut st = dshawn();
        let mut bp = worn(&st, "rebuilt_deck", Slot::Deck);
        let faces = st.character.count(&id("sticker_dead_face"));
        press(
            &mut bp,
            &mut st,
            &[KeyCode::Char('s'), KeyCode::Up, KeyCode::Char(' ')],
        );
        assert_eq!(
            at(&bp),
            At::Stuck {
                index: 1,
                armed: true
            }
        );
        assert_eq!(stuck(&st, Slot::Deck).len(), 2, "armed is not peeled");
        assert_eq!(
            readout(&screen(&bp, &st, 100, 30)),
            "DEAD FACE ▸ space again peels it · it tears · any other key keeps it"
        );
        // Mode keys go to the mode while armed.
        press(&mut bp, &mut st, &[KeyCode::Char('s')]);
        assert_eq!(bp.mode, Mode::Stickers);
        // Any other key keeps it, and does what it does: → moves on.
        assert_eq!(at(&bp), on(1), "s disarmed it");
        press(&mut bp, &mut st, &[KeyCode::Char(' '), KeyCode::Right]);
        assert_eq!(at(&bp), on(0));
        // Esc only disarms; the window stays.
        press(&mut bp, &mut st, &[KeyCode::Char(' ')]);
        assert!(matches!(
            press(&mut bp, &mut st, &[KeyCode::Esc]),
            ScreenCmd::Stay
        ));
        assert_eq!(at(&bp), on(0));
        assert_eq!(stuck(&st, Slot::Deck).len(), 2);
        // Two spaces on x‿x: it is gone, and not back in the pack.
        press(
            &mut bp,
            &mut st,
            &[KeyCode::Down, KeyCode::Char(' '), KeyCode::Char(' ')],
        );
        let left: Vec<ItemId> = stuck(&st, Slot::Deck)
            .into_iter()
            .map(|s| s.sticker)
            .collect();
        assert_eq!(left, [id("sticker_no_future")]);
        assert_eq!(st.character.count(&id("sticker_dead_face")), faces);
        assert_eq!(last_log(&st), "Dead face peeled · it tore");
        assert_eq!(at(&bp), on(0), "on the sticker left");
        assert!(!buffer_text(&screen(&bp, &st, 100, 30)).contains("x‿x"));
    }

    #[test]
    fn jacked_in_browses_but_never_changes() {
        let mut st = dshawn();
        let mut bp = worn(&st, "rebuilt_deck", Slot::Deck);
        jack_in(&mut st);
        let before = st.character.clone();
        press(&mut bp, &mut st, &[KeyCode::Char('s'), KeyCode::Char(' ')]);
        assert_eq!(at(&bp), At::Tray);
        assert_eq!(last_log(&st), crate::actions::JACKED_IN);
        press(&mut bp, &mut st, &[KeyCode::Right]);
        assert_eq!(bp.stickers.tile, 1, "the tray still browses");
        press(&mut bp, &mut st, &[KeyCode::Up, KeyCode::Char(' ')]);
        assert!(
            matches!(at(&bp), At::Stuck { armed: false, .. }),
            "not armed"
        );
        assert_eq!(last_log(&st), crate::actions::JACKED_IN);
        let was = at(&bp);
        press(&mut bp, &mut st, &[KeyCode::Right]);
        assert_ne!(at(&bp), was, "the arrows still move");
        assert_eq!(st.character, before);
    }

    #[test]
    fn a_stuck_sticker_off_the_area_still_draws_and_peels() {
        let mut st = dshawn();
        let mut bp = worn(&st, "rebuilt_deck", Slot::Deck);
        let plain = screen(&bp, &st, 100, 30);
        // Over the glass, where the mode would never put it.
        let over = Stuck {
            sticker: id("sticker_neko"),
            x: 10,
            y: 8,
            turn: Turn::Level,
        };
        let copy = st.character.worn.get_mut(&Slot::Deck).unwrap();
        copy.stickers.push(over);
        let buf = screen(&bp, &st, 100, 30);
        let (ox, oy) = (8, 3);
        let neko = Color::Rgb(0xff, 0x00, 0xc8);
        let cells: Vec<(u16, u16)> = (oy + 8..oy + 13)
            .flat_map(|y| (ox + 10..ox + 22).map(move |x| (x, y)))
            .collect();
        assert!(
            cells
                .iter()
                .any(|&(x, y)| buf[(x, y)].fg == neko || buf[(x, y)].bg == neko)
        );
        assert!(cells.iter().any(|&(x, y)| buf[(x, y)] != plain[(x, y)]));
        // The mode points at it and peels it like any other.
        press(
            &mut bp,
            &mut st,
            &[KeyCode::Char('s'), KeyCode::Up, KeyCode::Left],
        );
        assert_eq!(at(&bp), on(2));
        assert_eq!(
            readout(&screen(&bp, &st, 100, 30)),
            "NEKO ▸ stuck · picture"
        );
        press(&mut bp, &mut st, &[KeyCode::Char(' '), KeyCode::Char(' ')]);
        assert_eq!(stuck(&st, Slot::Deck).len(), 2);
        assert!(
            stuck(&st, Slot::Deck)
                .iter()
                .all(|s| s.sticker != id("sticker_neko"))
        );
    }
    /// The deck worn with only the given stickers, then NO FUTURE and ARASAKA picked
    /// and stuck: both land on the same start spot, ARASAKA on top.
    fn stacked(first: &[Stuck]) -> (AppState, Blueprint) {
        let mut st = dshawn();
        st.character.worn.get_mut(&Slot::Deck).unwrap().stickers = first.to_vec();
        let mut bp = worn(&st, "rebuilt_deck", Slot::Deck);
        let (space, right) = (KeyCode::Char(' '), KeyCode::Right);
        press(&mut bp, &mut st, &[KeyCode::Char('s'), space, space]);
        press(&mut bp, &mut st, &[right, right, space, space]);
        let now = stuck(&st, Slot::Deck);
        let n = first.len();
        assert_eq!(now[n].sticker, id("sticker_no_future"));
        assert_eq!(now[n + 1].sticker, id("sticker_arasaka"));
        assert_eq!(
            (now[n].x, now[n].y),
            (now[n + 1].x, now[n + 1].y),
            "stacked"
        );
        (st, bp)
    }

    #[test]
    fn stickers_on_one_spot_point_at_the_top_one_first() {
        let (mut st, mut bp) = stacked(&[]);
        press(&mut bp, &mut st, &[KeyCode::Up]);
        assert_eq!(at(&bp), on(1), "the one on top");
        assert_eq!(
            readout(&screen(&bp, &st, 100, 30)),
            "ARASAKA ▸ stuck · lettered"
        );
        // Peeled from the top down, as vinyl comes off.
        press(&mut bp, &mut st, &[KeyCode::Char(' '), KeyCode::Char(' ')]);
        let left: Vec<ItemId> = stuck(&st, Slot::Deck)
            .into_iter()
            .map(|s| s.sticker)
            .collect();
        assert_eq!(left, [id("sticker_no_future")]);
        assert_eq!(at(&bp), on(0));
        assert_eq!(
            readout(&screen(&bp, &st, 100, 30)),
            "NO FUTURE ▸ stuck · lettered"
        );

        // The arrows reach the top one too: from x‿x, left onto the stack.
        let face = Stuck {
            sticker: id("sticker_dead_face"),
            x: 45,
            y: 13,
            turn: Turn::Level,
        };
        let (mut st, mut bp) = stacked(&[face]);
        bp.stickers.at = on(0);
        press(&mut bp, &mut st, &[KeyCode::Left]);
        assert_eq!(at(&bp), on(2));
    }
}
