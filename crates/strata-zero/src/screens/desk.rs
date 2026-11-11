//! The desk: the rig opened up at home. The map pane becomes the desk with the rack on
//! it (a dark wall, the rack on its base, the desk's edge under it), the BAYS down the
//! left (the frame first), the picked part outlined in the drawing with a dashed box,
//! and its card in the side panel over what in your pack (or your flat, for the frame)
//! fits there, each with the change it would make.

use crate::actions::perform;
use crate::app::{AppState, Screen, ScreenCmd, TransitionKind};
use crate::messages::Kind;
use crate::rig::{RigSpec, spec};
use crate::ui::chrome::{PANEL_BG, RED_TEXT, RULE};
use crate::ui::hardware::rgb;
use crate::ui::hardware::sled::tier;
use crate::ui::hud::GREEN;
use crate::ui::rack::{self, RackArt};
use crossterm::event::{KeyCode, KeyEvent};
use game::{Action, CopyId, Host, Item, ItemId, Mod, STOCK_FRAME, STOCK_SLEDS, SocketKind};
use ratatui::Frame;
use ratatui::buffer::{Buffer, Cell};
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};

/// Game minutes a fit, a pull or a frame change takes.
pub const FIT_MINUTES: u64 = 10;

const AMBER: Color = rgb(0xffb000);
const AMBER2: Color = rgb(0xe09b00);
const AMBER_DIM: Color = rgb(0x8c6014);
const AMBER_FAINT: Color = rgb(0x4a3410);
const WALL: Color = rgb(0x0a090c);
const DESK: Color = rgb(0x2b2119);
const DESK_EDGE: Color = rgb(0x3a2d22);
const DESK_FRONT: Color = rgb(0x16110d);
const SIDE: Color = rgb(0x040406);
const SIDE_RULE: Color = rgb(0x1a1c22);
const PICK_BG: Color = rgb(0x141a20);
const WHITE: Color = rgb(0xe6f0f5);
const CARD_TX: Color = rgb(0xb4bec8);
const DIM: Color = rgb(0x6b7686);
const FAINT: Color = rgb(0x3a414c);

/// The BAYS list's width, from the map pane's left edge; the rack stands right of it.
/// On a narrow pane the list names the kinds only, so the rack keeps room for its head
/// unit's line.
const LIST_W: u16 = 23;
const LIST_NARROW: u16 = 13;
/// The rack's stage at its narrowest beside the wide list.
const STAGE_MIN: u16 = 50;
/// Cells a typed-out card shows a tick.
const TYPE_RATE: u64 = 3;
/// What the bottom row says.
const STATUS: &str = " at your rig · ↑↓ bays · space what fits · x pull · esc stand up";

/// What can go in the picked row: another rack from your flat (row 0), or a sled in
/// your pack.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Pick {
    Frame(ItemId),
    Sled { item: ItemId, copy: CopyId },
}

pub struct Desk {
    /// 0 the frame, then the bays top to bottom.
    row: usize,
    /// The pick while the list of what fits is open.
    picker: Option<usize>,
    /// The tick the card last changed: it types out from there.
    card: u64,
}

impl Desk {
    /// The desk with the first bay picked.
    pub fn open(st: &AppState) -> Desk {
        let bays = st.character.rig_fitted(&st.content).len();
        Desk {
            row: usize::from(bays > 0),
            picker: None,
            card: st.tick,
        }
    }

    /// Rows: the frame and each bay.
    fn rows(st: &AppState) -> usize {
        1 + st.character.rig_fitted(&st.content).len()
    }

    fn walk(&mut self, st: &AppState, to: usize) {
        if to != self.row {
            self.row = to;
            self.card = st.tick;
        }
    }

    /// Space on a row: what fits here, or a word that nothing does.
    fn list(&mut self, st: &mut AppState) {
        if fits_here(st, self.row).is_empty() {
            st.warn(if self.row == 0 {
                "no other rack in your flat"
            } else {
                "nothing in your pack fits"
            });
        } else {
            self.picker = Some(0);
        }
    }

    /// Space in the list: the pick goes in, what was there to the pack, the clock runs
    /// and the list closes. A refusal is a warning, the list left open on the pick and
    /// no time gone, whether it is a rack or a sled.
    fn fit(&mut self, st: &mut AppState, pick: usize) {
        let Some(p) = fits_here(st, self.row).into_iter().nth(pick) else {
            self.picker = None;
            return;
        };
        let done = match p {
            Pick::Frame(frame) if st.content.item(&frame).is_none() => {
                Err(format!("`{frame}` is no rack"))
            }
            Pick::Frame(frame) => {
                let out = st.character.set_frame(&st.content, &frame);
                let mut line = format!(
                    "you move the rig into the {} · {FIT_MINUTES}m",
                    name(st, &frame)
                );
                if !out.is_empty() {
                    let names: Vec<String> = out.iter().map(|id| name(st, id)).collect();
                    line.push_str(&format!(" · the {} to your pack", names.join(", the ")));
                }
                Ok(line)
            }
            Pick::Sled { item, copy } => {
                let fit = Action::Fit {
                    host: Host::Rig,
                    socket: self.row - 1,
                    piece: copy,
                };
                perform(st, fit)
                    .map(|_| format!("you seat the {} · {FIT_MINUTES}m", name(st, &item)))
            }
        };
        match done {
            Ok(line) => {
                run_clock(st);
                st.say(line);
                self.row = self.row.min(Self::rows(st) - 1);
                self.picker = None;
                self.card = st.tick;
            }
            Err(e) => st.warn(e),
        }
    }

    /// `x` on a bay: its sled comes out to the pack, no asking.
    fn pull(&mut self, st: &mut AppState) {
        let Some(bay) = self.row.checked_sub(1) else {
            return;
        };
        let Some(Some(sled)) = st
            .character
            .rig_fitted(&st.content)
            .get(bay)
            .map(|b| b.1.clone())
        else {
            return;
        };
        match perform(
            st,
            Action::Unfit {
                host: Host::Rig,
                socket: bay,
            },
        ) {
            Ok(_) => {
                run_clock(st);
                st.say(format!("you pull the {} · {FIT_MINUTES}m", name(st, &sled)));
                self.card = st.tick;
            }
            Err(e) => st.warn(e),
        }
    }

    /// Stands you up from the desk.
    fn stand_up(st: &mut AppState) -> ScreenCmd {
        st.at_desk = false;
        ScreenCmd::Pop {
            fx: TransitionKind::None,
        }
    }
}

/// The clock runs while you work: `FIT_MINUTES` of the city's minutes.
fn run_clock(st: &mut AppState) {
    for _ in 0..FIT_MINUTES {
        crate::clock::run(
            st,
            Some(sim::TICKS_PER_MINUTE),
            crate::clock::PATHS_PER_FRAME,
        );
    }
}

fn name(st: &AppState, id: &ItemId) -> String {
    st.content
        .item(id)
        .map_or_else(|| id.0.clone(), |i| i.name.clone())
}

/// What fits `row`: for the frame, the racks in your flat but the fitted one; for a
/// bay, every sled copy in the pack of its kind, in pack order.
pub fn fits_here(st: &AppState, row: usize) -> Vec<Pick> {
    let (c, ch) = (&st.content, &st.character);
    if row == 0 {
        let fitted = ch.rig_frame();
        return ch
            .frames
            .iter()
            .filter(|f| **f != fitted)
            .map(|f| Pick::Frame(f.clone()))
            .collect();
    }
    let Some(kind) = ch.rig_fitted(c).get(row - 1).map(|(k, _)| *k) else {
        return Vec::new();
    };
    ch.inventory
        .iter()
        .filter(|s| s.qty > 0 && c.item(&s.item).and_then(Item::fits_socket) == Some(kind))
        .filter_map(|s| {
            Some(Pick::Sled {
                item: s.item.clone(),
                copy: s.copy_id()?,
            })
        })
        .collect()
}

/// The rig as it would stand with `pick` in `row`, and the sleds a frame change sends
/// to the pack.
fn after(st: &AppState, row: usize, pick: &Pick) -> (RigSpec, Vec<ItemId>) {
    let c = &st.content;
    let mut ch = st.character.clone();
    let out = match pick {
        Pick::Frame(frame) => ch.set_frame(c, frame),
        Pick::Sled { copy, .. } => {
            if let Some(i) = ch.inventory.iter().position(|s| s.copy_id() == Some(*copy)) {
                let stack = ch.inventory.remove(i);
                let rig = ch.rig_copy_mut(c);
                let bay = row - 1;
                if rig.fitted.len() <= bay {
                    rig.fitted.resize(bay + 1, None);
                }
                rig.fitted[bay] = Some(stack);
            }
            Vec::new()
        }
    };
    (spec(&ch, c), out)
}

fn firewall_word(n: u8) -> &'static str {
    match n {
        0 => "none",
        1 => "sandbag",
        2 => "tripwire",
        _ => "killswitch",
    }
}

fn speed_word(n: u16) -> String {
    if n.is_multiple_of(100) {
        format!("×{}", n / 100)
    } else {
        format!("×{}", f32::from(n) / 100.0)
    }
}

/// The change a pick makes, `seats 1 ▸ 2 · speed ×1 ▸ ×1.5`, coloured by whether it is
/// all for the better, all for the worse, or both.
fn change(before: RigSpec, after: RigSpec) -> Option<(String, Color)> {
    let mut parts = Vec::new();
    let (mut up, mut down) = (false, false);
    let mut add = |label: &str, b: u64, a: u64, show: &dyn Fn(u64) -> String| {
        if a != b {
            parts.push(format!("{label} {} ▸ {}", show(b), show(a)));
            up |= a > b;
            down |= a < b;
        }
    };
    let plain = |n: u64| n.to_string();
    add("seats", before.seats.into(), after.seats.into(), &plain);
    add("speed", before.speed.into(), after.speed.into(), &|n| {
        speed_word(n as u16)
    });
    add("lines", before.lines as u64, after.lines as u64, &plain);
    add("hold", before.hold_days, after.hold_days, &|n| {
        format!("{n}d")
    });
    add(
        "firewall",
        before.firewall.into(),
        after.firewall.into(),
        &|n| firewall_word(n as u8).to_string(),
    );
    if parts.is_empty() {
        return None;
    }
    let colour = match (up, down) {
        (true, false) => GREEN,
        (false, true) => RED_TEXT,
        _ => AMBER,
    };
    Some((parts.join(" · "), colour))
}

/// What a sled gives, a line a mod: `seats     2`.
fn gives(item: &Item) -> Vec<String> {
    let mods: &[Mod] = item.hardware.as_ref().map_or(&[], |h| h.mods.as_slice());
    mods.iter()
        .filter_map(|m| {
            let (label, value) = match *m {
                Mod::Seats(n) => ("seats", n.to_string()),
                Mod::Speed(n) => ("speed", speed_word(n)),
                Mod::Lines(n) => ("lines", n.to_string()),
                Mod::Hold(n) => ("hold", format!("{n} days")),
                Mod::Firewall(n) => ("firewall", firewall_word(n).to_string()),
                _ => return None,
            };
            Some(format!("{label:<10}{value}"))
        })
        .collect()
}

/// A rack's bays by kind: `core relay line firewall`.
fn bay_kinds(item: &Item) -> String {
    let kinds: Vec<&str> = item.sockets.iter().map(|k| k.label()).collect();
    kinds.join(" ")
}

/// The rack's name on the plate: its first word, upper case (`ZENBO`).
fn rack_word(st: &AppState, frame: &ItemId) -> String {
    name(st, frame)
        .split_whitespace()
        .next()
        .unwrap_or_default()
        .to_uppercase()
}

impl Screen for Desk {
    fn update(&mut self, st: &mut AppState, keys: &[KeyEvent]) -> ScreenCmd {
        st.at_desk = true;
        for k in keys {
            let rows = Self::rows(st);
            if let Some(pick) = self.picker {
                let n = fits_here(st, self.row).len();
                if n == 0 {
                    self.picker = None;
                    continue;
                }
                match k.code {
                    KeyCode::Up | KeyCode::Char('k') => {
                        self.picker = Some(pick.min(n - 1).saturating_sub(1))
                    }
                    KeyCode::Down | KeyCode::Char('j') => self.picker = Some((pick + 1).min(n - 1)),
                    KeyCode::Char(' ') => self.fit(st, pick.min(n - 1)),
                    KeyCode::Esc => self.picker = None,
                    _ => {}
                }
                continue;
            }
            match k.code {
                KeyCode::Up | KeyCode::Char('k') => self.walk(st, self.row.saturating_sub(1)),
                KeyCode::Down | KeyCode::Char('j') => self.walk(st, (self.row + 1).min(rows - 1)),
                KeyCode::Char(' ') => self.list(st),
                KeyCode::Char('x') => self.pull(st),
                KeyCode::Esc => return Self::stand_up(st),
                _ => {}
            }
        }
        ScreenCmd::Stay
    }

    /// The desk over the map pane and the side panel, the bottom row its keys.
    fn draw(&self, st: &AppState, f: &mut Frame) {
        let area = f.area();
        let r = super::explore::layout(area, false);
        let buf = f.buffer_mut();
        let picks = fits_here(st, self.row);
        let pick = self
            .picker
            .filter(|_| !picks.is_empty())
            .map(|p| p.min(picks.len() - 1));
        draw_desk(self, st, buf, r.map);
        // The rule between the panes, as the map draws it.
        for y in r.map.y..r.map.bottom() {
            if r.vrule_x < buf.area.right() {
                buf[(r.vrule_x, y)]
                    .set_char('│')
                    .set_fg(RULE)
                    .set_bg(PANEL_BG);
            }
        }
        draw_side(self, st, buf, r.side, &picks, pick);
        let status = r.footer[0];
        fill(buf, status, PANEL_BG);
        put(
            buf,
            status.x,
            status.y,
            STATUS,
            DIM,
            PANEL_BG,
            false,
            status.right(),
        );
    }
}

fn fill(buf: &mut Buffer, rect: Rect, bg: Color) {
    let rect = rect.intersection(buf.area);
    for y in rect.y..rect.bottom() {
        for x in rect.x..rect.right() {
            buf[(x, y)].reset();
            buf[(x, y)].set_bg(bg);
        }
    }
}

/// `text` from (x, y), no further than `max_x`.
#[allow(clippy::too_many_arguments)]
fn put(buf: &mut Buffer, x: u16, y: u16, text: &str, fg: Color, bg: Color, bold: bool, max_x: u16) {
    if y >= buf.area.bottom() {
        return;
    }
    // Whatever the map left in the cell (a dimmed tile) goes.
    let mut style = Style::default()
        .fg(fg)
        .bg(bg)
        .remove_modifier(Modifier::all());
    if bold {
        style = style.add_modifier(Modifier::BOLD);
    }
    let end = max_x.min(buf.area.right());
    for (cx, ch) in (x..end).zip(text.chars()) {
        buf[(cx, y)].set_char(ch).set_style(style);
    }
}

/// Where the desk put things: the rack's stage, the rack's rects (`rack::draw`'s), the
/// row each line of the BAYS list stands on (FRAME first) and the column the list ends
/// before.
#[derive(Debug, Default)]
#[cfg_attr(not(test), allow(dead_code))] // the tests read where things went
struct Drawn {
    stage: Rect,
    rects: Vec<Rect>,
    rows: Vec<u16>,
    list_end: u16,
}

/// The map pane as the desk: the header, the BAYS, the rack with the pick outlined, the
/// desk's edge under it and the last word on its front.
fn draw_desk(d: &Desk, st: &AppState, buf: &mut Buffer, map: Rect) -> Drawn {
    fill(buf, map, WALL);
    if map.width < 8 || map.height < 6 {
        return Drawn::default();
    }
    let art = RackArt::rig(st, d.row.checked_sub(1));
    let (frame, bays) = (art.frame.clone(), art.bays.clone());
    let (x, right) = (map.x, map.right());
    // The header: the rack's name, then whose it is.
    let rack_name = name(st, &frame).to_uppercase();
    put(buf, x + 2, map.y + 1, &rack_name, AMBER, WALL, true, right);
    let whose = format!(" · your rig · {} bays", bays.len());
    let after = x + 2 + rack_name.chars().count() as u16;
    put(buf, after, map.y + 1, &whose, AMBER_DIM, WALL, false, right);
    // The desk under it all: an edge, then its front.
    let desk_h = (map.height.saturating_sub(14) / 2).clamp(2, 6);
    let desk_y = map.bottom() - desk_h;
    fill(buf, Rect::new(x, desk_y, map.width, desk_h), DESK_FRONT);
    for cx in x..right {
        buf[(cx, desk_y)]
            .set_char('▀')
            .set_fg(DESK_EDGE)
            .set_bg(DESK);
    }
    if let Some(e) = st.log.recent(st.tick) {
        let fg = match e.kind {
            Kind::Warn => RED_TEXT,
            Kind::Story => AMBER,
            _ => AMBER2,
        };
        let y = desk_y + 1 + u16::from(desk_h > 3);
        put(buf, x + 2, y, &e.text, fg, DESK_FRONT, false, right - 1);
    }
    // The list keeps its own columns, a gap, then the rack's stage: the two never share
    // a cell.
    let wide = map.width >= LIST_W + 1 + STAGE_MIN;
    let list_end = x + if wide { LIST_W } else { LIST_NARROW }.min(map.width);
    let top = map.y + 3;
    let stage = Rect::new(
        list_end + 1,
        top,
        right.saturating_sub(list_end + 1),
        desk_y.saturating_sub(top),
    );
    let rects = if stage.width > 0 && stage.height > 0 {
        rack::draw(&art, st, stage, buf)
    } else {
        Vec::new()
    };
    // Each line of the list stands on its part's row in the drawing: FRAME on the head
    // unit's LCD, a bay on its plate's writing.
    let shown = |r: &Rect| r.width > 0 && r.height > 0;
    let rows: Vec<u16> = if rects.len() == bays.len() + 1 && rects.iter().all(shown) {
        let head = rects.get(1).map_or(rects[0].height, |b| b.y - rects[0].y);
        std::iter::once(rects[0].y + u16::from(head >= 3))
            .chain(rects[1..].iter().map(|r| r.y + u16::from(r.height >= 3)))
            .collect()
    } else {
        (0..=bays.len() as u16).map(|i| top + 2 + i).collect()
    };
    let label_y = rows[0].saturating_sub(2).max(map.y + 2);
    if label_y < rows[0] {
        put(buf, x + 2, label_y, "BAYS", AMBER_DIM, WALL, true, list_end);
    }
    let mut lines: Vec<(String, String, bool)> =
        vec![("FRAME".into(), rack_word(st, &frame), true)];
    for (kind, sled) in &bays {
        let model = sled
            .as_ref()
            .map(|id| rack::model(&name(st, id)))
            .unwrap_or_else(|| "empty".to_string());
        lines.push((kind.label().to_uppercase(), model, sled.is_some()));
    }
    let mut end = label_y;
    for (i, (kind, model, held)) in lines.iter().enumerate() {
        let y = rows[i];
        if y >= desk_y {
            break;
        }
        end = y;
        let picked = i == d.row;
        let bg = if picked { AMBER_FAINT } else { WALL };
        fill(buf, Rect::new(x + 1, y, list_end - x - 1, 1), bg);
        let mark = if picked { "▸ " } else { "  " };
        let fg = if picked { AMBER } else { AMBER2 };
        let label = format!("{mark}{kind}");
        put(buf, x + 2, y, &label, fg, bg, picked, list_end);
        let mfg = match (picked, held) {
            (true, _) => AMBER,
            (false, true) => AMBER_DIM,
            (false, false) => AMBER_FAINT,
        };
        if wide {
            put(buf, x + 13, y, model, mfg, bg, false, list_end);
        }
    }
    // No firewall bay: the list says so under its rows, where there is room.
    if !bays.iter().any(|(k, _)| *k == SocketKind::Firewall) && end + 4 < desk_y {
        let rule = "─".repeat(usize::from(list_end - x).saturating_sub(4));
        put(
            buf,
            x + 2,
            end + 2,
            &rule,
            AMBER_FAINT,
            WALL,
            false,
            list_end,
        );
        put(
            buf,
            x + 4,
            end + 3,
            "FIREWALL",
            AMBER_FAINT,
            WALL,
            false,
            list_end,
        );
        put(
            buf,
            x + 4,
            end + 4,
            "no bay",
            AMBER_FAINT,
            WALL,
            false,
            list_end,
        );
    }
    // The pick outlined: the frame boxed round the whole rack on the wall's empty
    // cells; a bay marked along the rails beside its plate, its corners on the rails
    // above and below while the rails go on, so no plate, lamp or base is covered.
    let clip = Rect::new(
        stage.x.saturating_sub(1),
        map.y + 2,
        right.saturating_sub(stage.x.saturating_sub(1)),
        desk_y.saturating_sub(map.y + 2),
    );
    match (d.row, rects.get(d.row)) {
        (0, Some(r)) if shown(r) => frame_box(buf, *r, clip),
        (_, Some(r)) if shown(r) => {
            let rails = (rects[0].y, rects.last().map_or(r.bottom(), |b| b.bottom()));
            bay_marks(buf, *r, rails, clip);
        }
        _ => {}
    }
    Drawn {
        stage,
        rects,
        rows,
        list_end,
    }
}

/// Puts a piece of the outline at (x, y) inside `clip`, when `ok` takes the cell.
fn mark(buf: &mut Buffer, clip: Rect, x: i32, y: i32, ch: char, ok: impl Fn(&Cell) -> bool) {
    let (Ok(x), Ok(y)) = (u16::try_from(x), u16::try_from(y)) else {
        return;
    };
    let clip = clip.intersection(buf.area);
    if clip.contains((x, y).into()) && ok(&buf[(x, y)]) {
        buf[(x, y)].set_char(ch).set_fg(AMBER);
    }
}

/// A cell nothing is drawn on: the bare wall.
fn bare(c: &Cell) -> bool {
    c.symbol() == " " && c.bg == WALL
}

/// A dashed amber box one cell outside the rack `r`, on bare wall only: where the loom
/// or the desk is, it breaks off.
fn frame_box(buf: &mut Buffer, r: Rect, clip: Rect) {
    let (x0, x1) = (i32::from(r.x) - 1, i32::from(r.right()));
    let (y0, y1) = (i32::from(r.y) - 1, i32::from(r.bottom()));
    for y in y0 + 1..y1 {
        mark(buf, clip, x0, y, '╎', bare);
        mark(buf, clip, x1, y, '╎', bare);
    }
    for (y, l, rt) in [(y0, '┌', '┐'), (y1, '└', '┘')] {
        for x in x0 + 1..x1 {
            mark(buf, clip, x, y, '╌', bare);
        }
        mark(buf, clip, x0, y, l, bare);
        mark(buf, clip, x1, y, rt, bare);
    }
}

/// A bay's marks, all on the rails either side of its plate `r`: a dashed line down
/// each beside the plate, and corners on the rows above and below while those are still
/// rails (`rails`, the rows they run), not the base.
fn bay_marks(buf: &mut Buffer, r: Rect, (rail_top, rail_end): (u16, u16), clip: Rect) {
    let (x0, x1) = (i32::from(r.x) - 1, i32::from(r.right()));
    let any = |_: &Cell| true;
    for y in r.y..r.bottom() {
        mark(buf, clip, x0, i32::from(y), '╎', any);
        mark(buf, clip, x1, i32::from(y), '╎', any);
    }
    if r.y > rail_top {
        let y = i32::from(r.y) - 1;
        mark(buf, clip, x0, y, '┌', any);
        mark(buf, clip, x1, y, '┐', any);
    }
    if r.bottom() < rail_end {
        let y = i32::from(r.bottom());
        mark(buf, clip, x0, y, '└', any);
        mark(buf, clip, x1, y, '┘', any);
    }
}

/// The side panel: the picked part's card, then what fits it with the change each
/// makes, and the keys at its foot.
fn draw_side(
    d: &Desk,
    st: &AppState,
    buf: &mut Buffer,
    side: Rect,
    picks: &[Pick],
    pick: Option<usize>,
) {
    fill(buf, side, SIDE);
    if side.width < 8 || side.height < 6 {
        return;
    }
    let (c, ch) = (&st.content, &st.character);
    let (x, right) = (side.x + 2, side.right().saturating_sub(1));
    let bays = ch.rig_fitted(c);
    // The card: what the row holds and what it gives.
    let (title, sub, lines): (String, Option<String>, Vec<(String, Color)>) = if d.row == 0 {
        let frame = ch.rig_frame();
        let item = c.item(&frame);
        let sub = if frame.0 == STOCK_FRAME {
            "came with the box".to_string()
        } else {
            format!("{} bays", bays.len())
        };
        let kinds = item.map(bay_kinds).unwrap_or_default();
        (
            format!("FRAME · {}", name(st, &frame).to_uppercase()),
            Some(sub),
            vec![(format!("{:<10}{kinds}", "bays"), CARD_TX)],
        )
    } else {
        match bays.get(d.row - 1) {
            Some((kind, Some(id))) => {
                let item = c.item(id);
                let model = rack::model(&name(st, id));
                let sub = if STOCK_SLEDS.contains(&id.0.as_str()) {
                    "came with the box".to_string()
                } else {
                    let mods = item
                        .and_then(|i| i.hardware.as_ref())
                        .map_or(&[][..], |h| h.mods.as_slice());
                    format!("tier {} sled", tier(mods))
                };
                (
                    format!("{} · {model}", kind.label().to_uppercase()),
                    Some(sub),
                    item.map(gives)
                        .unwrap_or_default()
                        .into_iter()
                        .map(|l| (l, CARD_TX))
                        .collect(),
                )
            }
            Some((kind, None)) => (
                format!("{} · empty", kind.label().to_uppercase()),
                Some(format!("takes {}", kind.takes())),
                vec![("nothing fitted".to_string(), FAINT)],
            ),
            None => (String::new(), None, Vec::new()),
        }
    };
    let typed = (st.tick.saturating_sub(d.card) * TYPE_RATE) as usize;
    let title_shown: String = title.chars().take(typed).collect();
    let mut y = side.y + 1;
    put(buf, x, y, &title_shown, AMBER, SIDE, true, right);
    if let Some(sub) = sub {
        let left = typed.saturating_sub(title.chars().count());
        let sub_shown: String = sub.chars().take(left).collect();
        put(buf, x, y + 1, &sub_shown, AMBER_DIM, SIDE, false, right);
    }
    let foot = side.bottom() - 1;
    let tall = side.height >= 26;
    y += 2 + u16::from(tall);
    let rule = |buf: &mut Buffer, y: u16| {
        if y < foot {
            put(
                buf,
                side.x + 1,
                y,
                &"─".repeat(side.width as usize - 2),
                SIDE_RULE,
                SIDE,
                false,
                side.right() - 1,
            );
        }
    };
    rule(buf, y);
    y += 1;
    for (l, fg) in &lines {
        if y + 1 >= foot {
            break;
        }
        put(buf, x, y, l, *fg, SIDE, false, right);
        y += 1;
    }
    y += u16::from(tall);
    rule(buf, y);
    y += 1;
    let head = if d.row == 0 {
        "IN YOUR FLAT"
    } else {
        "FITS HERE · IN YOUR PACK"
    };
    if y < foot {
        put(buf, x, y, head, DIM, SIDE, true, right);
    }
    y += 2;
    // The list: each pick's name, then the change it makes.
    let before = spec(ch, c);
    let entries: Vec<Vec<(String, Color, bool)>> =
        picks.iter().map(|p| entry(st, d.row, p, before)).collect();
    let room = foot.saturating_sub(1).saturating_sub(y);
    if entries.is_empty() {
        let none = if d.row == 0 {
            "no other rack in your flat"
        } else {
            "nothing in your pack fits"
        };
        if y < foot {
            put(buf, x, y, none, FAINT, SIDE, false, right);
        }
    }
    // Scrolled so the pick shows.
    let height = |e: &Vec<(String, Color, bool)>| e.len() as u16 + 1;
    let mut start = 0;
    if let Some(p) = pick {
        while start < p && entries[start..=p].iter().map(height).sum::<u16>() > room {
            start += 1;
        }
    }
    for (i, e) in entries.iter().enumerate().skip(start) {
        if y + height(e) - 1 > foot.saturating_sub(1) {
            break;
        }
        let on = pick == Some(i);
        let bg = if on { PICK_BG } else { SIDE };
        if on {
            fill(
                buf,
                Rect::new(side.x + 1, y, side.width - 2, e.len() as u16),
                bg,
            );
        }
        for (k, (text, fg, bold)) in e.iter().enumerate() {
            let text = if k == 0 {
                format!("{}{text}", if on { "▸ " } else { "  " })
            } else {
                format!("    {text}")
            };
            let fg = if k == 0 && on { WHITE } else { *fg };
            put(buf, x, y + k as u16, &text, fg, bg, *bold, right);
        }
        y += height(e);
    }
    rule(buf, foot - 1);
    let keys = match (pick.is_some(), d.row) {
        (true, _) => "space fit · esc back",
        (false, 0) => "space what fits",
        (false, row) if matches!(bays.get(row - 1), Some((_, Some(_)))) => {
            "space what fits · x pull"
        }
        _ => "space what fits",
    };
    put(buf, x, foot, keys, DIM, SIDE, false, right);
}

/// A pick's lines in the list: its name, then what changes, and for a rack its bays
/// and what goes to the pack.
fn entry(st: &AppState, row: usize, p: &Pick, before: RigSpec) -> Vec<(String, Color, bool)> {
    let (after, out) = after(st, row, p);
    let mut lines = Vec::new();
    match p {
        Pick::Frame(frame) => {
            lines.push((name(st, frame).to_uppercase(), CARD_TX, true));
            let item = st.content.item(frame);
            let n = item.map_or(0, |i| i.sockets.len());
            let kinds = item.map(bay_kinds).unwrap_or_default();
            lines.push((format!("{n} bays · {kinds}"), CARD_TX, false));
        }
        Pick::Sled { item, .. } => {
            let kind = st
                .content
                .item(item)
                .and_then(Item::fits_socket)
                .map_or("", SocketKind::label);
            let model = rack::model(&name(st, item));
            lines.push((format!("{model} {kind}"), CARD_TX, true));
        }
    }
    match change(before, after) {
        Some((text, colour)) => lines.push((text, colour, false)),
        None if matches!(p, Pick::Sled { .. }) => lines.push(("no change".into(), DIM, false)),
        None => {}
    }
    if !out.is_empty() {
        let models: Vec<String> = out.iter().map(|id| rack::model(&name(st, id))).collect();
        lines.push((format!("{} to your pack", models.join(", ")), AMBER, false));
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::interact::gather;
    use crate::jobs::testutil::state_home;
    use crate::screens::popup::{Choice, Pick as PopupPick};
    use crate::screens::testutil::{buffer_text, key};
    use crossterm::event::KeyCode;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    fn press(d: &mut Desk, st: &mut AppState, code: KeyCode) -> ScreenCmd {
        d.update(st, &[key(code)])
    }

    /// A copy of `id` put in the pack.
    fn give(st: &mut AppState, id: &str) -> CopyId {
        st.character.give_item(&st.content, ItemId::new(id), 1);
        st.character
            .inventory
            .iter()
            .rev()
            .find(|s| s.item.0 == id)
            .and_then(|s| s.copy_id())
            .unwrap()
    }

    fn bay(st: &AppState, i: usize) -> Option<String> {
        st.character.rig_fitted(&st.content)[i]
            .1
            .as_ref()
            .map(|id| id.0.clone())
    }

    fn drawn(d: &Desk, st: &AppState, w: u16, h: u16) -> String {
        let mut term = Terminal::new(TestBackend::new(w, h)).unwrap();
        term.draw(|f| d.draw(st, f)).unwrap();
        buffer_text(term.backend().buffer())
    }

    fn last(st: &AppState) -> String {
        st.log.latest().map(|e| e.text.clone()).unwrap_or_default()
    }

    #[test]
    fn o_opens_the_desk_at_home() {
        let mut st = state_home();
        crate::screens::explore::MapScreen::walking(&mut st);
        let home = st.home.unwrap().home;
        let floor = st
            .interiors
            .floor(home.building, home.level)
            .map(|(_, f)| f.clone())
            .unwrap();
        st.player.as_mut().unwrap().pos = home
            .rig
            .neighbors4()
            .find(|p| {
                home.rect.contains(*p) && floor.tiles.get(*p).is_some_and(|t| t.is_walkable())
            })
            .expect("a floor tile by the rig");
        let o_row = |st: &AppState| -> Choice {
            gather(st, &|_, _| None)
                .into_iter()
                .find(|s| s.header.as_ref().is_some_and(|h| h.name == "Access rig"))
                .expect("the rig")
                .choices
                .into_iter()
                .find(|c| c.key == 'o')
                .expect("an o row")
        };
        let row = o_row(&st);
        assert!(row.enabled);
        assert_eq!(row.label, "Open up");
        assert!(matches!(row.pick, PopupPick::Desk));
        assert!(!crate::interact::FREE_LETTERS.contains('o'));
        // Fired, the desk takes the popup's place and you are at it.
        let mut window = crate::screens::popup::Popup::about(None, None, vec![row], 0);
        st.tick += 200;
        let cmd = crate::app::Screen::update(&mut window, &mut st, &[key(KeyCode::Char('o'))]);
        assert!(matches!(cmd, ScreenCmd::Replace { .. }));
        assert!(st.at_desk);
        // Seized: greyed, with why.
        let now = st.net.now(&st);
        st.backdoors.rig_seized_until = Some(now + 60);
        let row = o_row(&st);
        assert!(!row.enabled);
        assert!(row.tail.contains("held as evidence"), "{}", row.tail);
    }

    #[test]
    fn space_lists_what_fits() {
        let mut st = AppState::new();
        give(&mut st, "sled_tinman");
        give(&mut st, "sled_longwire");
        let mut d = Desk::open(&st);
        d.row = 1;
        let picks = fits_here(&st, 1);
        assert_eq!(picks.len(), 1);
        assert!(matches!(&picks[0], Pick::Sled { item, .. } if item.0 == "sled_tinman"));
        assert_eq!(
            fits_here(&st, 3).len(),
            1,
            "the line bay takes the Longwire"
        );
        assert!(fits_here(&st, 0).is_empty(), "no other rack in the flat");
        press(&mut d, &mut st, KeyCode::Char(' '));
        assert_eq!(d.picker, Some(0));
        let text = drawn(&d, &st, 120, 40);
        assert!(text.contains("FITS HERE · IN YOUR PACK"), "{text}");
        assert!(text.contains("seats 1 ▸ 2"), "{text}");
        // Esc closes the list, a second stands you up.
        press(&mut d, &mut st, KeyCode::Esc);
        assert_eq!(d.picker, None);
        assert!(st.at_desk);
        assert!(matches!(
            press(&mut d, &mut st, KeyCode::Esc),
            ScreenCmd::Pop { .. }
        ));
        assert!(!st.at_desk);
    }

    #[test]
    fn fitting_swaps_and_runs_the_clock() {
        let mut st = state_home();
        give(&mut st, "sled_tinman");
        let mut d = Desk::open(&st);
        d.row = 1;
        let from = st.net.now(&st);
        press(&mut d, &mut st, KeyCode::Char(' '));
        press(&mut d, &mut st, KeyCode::Char(' '));
        assert_eq!(bay(&st, 0).as_deref(), Some("sled_tinman"));
        assert_eq!(
            st.character.count(&ItemId::new("sled_zb8")),
            1,
            "ZB-8 in the pack"
        );
        assert_eq!(st.character.count(&ItemId::new("sled_tinman")), 0);
        assert_eq!(st.net.now(&st), from + FIT_MINUTES);
        assert_eq!(last(&st), "you seat the Tinman 2X core · 10m");
        assert_eq!(d.picker, None, "the list closes");
    }

    #[test]
    fn x_pulls_without_asking() {
        let mut st = state_home();
        let mut d = Desk::open(&st);
        d.row = 2;
        let from = st.net.now(&st);
        press(&mut d, &mut st, KeyCode::Char('x'));
        assert_eq!(bay(&st, 1), None);
        assert_eq!(st.character.count(&ItemId::new("sled_r3")), 1);
        assert_eq!(st.net.now(&st), from + FIT_MINUTES);
        assert_eq!(last(&st), "you pull the R-3 relay · 10m");
        // An empty bay has nothing to pull, and takes no time.
        press(&mut d, &mut st, KeyCode::Char('x'));
        assert_eq!(st.net.now(&st), from + FIT_MINUTES);
    }

    #[test]
    fn rig_fits_are_refused_off_the_desk() {
        let mut st = AppState::new();
        let piece = give(&mut st, "sled_tinman");
        let fit = Action::Fit {
            host: Host::Rig,
            socket: 0,
            piece,
        };
        assert!(perform(&mut st, fit.clone()).is_err());
        let unfit = Action::Unfit {
            host: Host::Rig,
            socket: 0,
        };
        assert!(perform(&mut st, unfit.clone()).is_err());
        assert_eq!(bay(&st, 0).as_deref(), Some("sled_zb8"));
        st.at_desk = true;
        assert!(perform(&mut st, fit).is_ok());
        assert_eq!(bay(&st, 0).as_deref(), Some("sled_tinman"));
        assert!(perform(&mut st, unfit).is_ok());
    }

    #[test]
    fn changing_frame_sends_extras_to_the_pack() {
        let mut st = state_home();
        let lotus = ItemId::new("rack_lotus");
        st.character.set_frame(&st.content, &lotus);
        let piece = give(&mut st, "sled_tinman");
        st.at_desk = true;
        perform(
            &mut st,
            Action::Fit {
                host: Host::Rig,
                socket: 1,
                piece,
            },
        )
        .unwrap();
        let mut d = Desk::open(&st);
        d.row = 0;
        assert_eq!(
            fits_here(&st, 0),
            vec![Pick::Frame(ItemId::new("rack_zenbo"))]
        );
        let from = st.net.now(&st);
        press(&mut d, &mut st, KeyCode::Char(' '));
        let text = drawn(&d, &st, 120, 40);
        assert!(text.contains("IN YOUR FLAT"), "{text}");
        assert!(text.contains("TINMAN 2X to your pack"), "{text}");
        press(&mut d, &mut st, KeyCode::Char(' '));
        assert_eq!(st.character.rig_frame(), ItemId::new("rack_zenbo"));
        assert_eq!(st.character.count(&ItemId::new("sled_tinman")), 1);
        assert!(st.character.frames.contains(&lotus), "the old rack stays");
        assert_eq!(st.net.now(&st), from + FIT_MINUTES);
        let said = last(&st);
        assert!(
            said.contains("Zenbo half-rack") && said.contains("Tinman 2X core"),
            "{said}"
        );
    }

    /// The three racks the shots show: the stock Zenbo, the Civitas with its firewall
    /// bay empty, and the Halcyon with every bay fitted.
    fn racks() -> Vec<(&'static str, AppState)> {
        let stock = AppState::new();
        let mut civitas = AppState::new();
        civitas
            .character
            .set_frame(&civitas.content, &ItemId::new("rack_civitas"));
        let mut halcyon = AppState::new();
        let c = halcyon.content.clone();
        halcyon
            .character
            .set_frame(&c, &ItemId::new("rack_halcyon"));
        let sleds = [
            "sled_qstack",
            "sled_tinman",
            "sled_hydra",
            "sled_darkfibre",
            "sled_killswitch",
        ];
        for (bay, sled) in sleds.into_iter().enumerate() {
            let id = ItemId::new(sled);
            let copy = halcyon.character.new_copy(&c, &id);
            halcyon.character.rig_copy_mut(&c).fitted[bay] = Some(game::Stack {
                item: id,
                qty: 1,
                copy: Some(copy),
            });
        }
        vec![("zenbo", stock), ("civitas", civitas), ("halcyon", halcyon)]
    }

    /// The desk's map pane on a `w` by `h` screen with `row` picked, and where it put
    /// things.
    fn pane(st: &AppState, row: usize, (w, h): (u16, u16)) -> (Buffer, Drawn, Rect) {
        let map = crate::screens::explore::layout(Rect::new(0, 0, w, h), false).map;
        let mut buf = Buffer::empty(Rect::new(0, 0, w, h));
        let d = Desk {
            row,
            picker: None,
            card: 0,
        };
        let drawn = draw_desk(&d, st, &mut buf, map);
        (buf, drawn, map)
    }

    const OUTLINE: [&str; 6] = ["╎", "╌", "┌", "┐", "└", "┘"];

    #[test]
    fn the_outline_covers_no_part_of_the_rack() {
        for (frame, st) in racks() {
            let rows = st.character.rig_fitted(&st.content).len() + 1;
            for size in [(100, 30), (120, 40)] {
                for row in 0..rows {
                    let at = format!("{frame} {}x{} row {row}", size.0, size.1);
                    let (picked, drawn, map) = pane(&st, row, size);
                    // The rack alone on the bare wall, as the desk draws it.
                    let mut bare_rack = Buffer::empty(picked.area);
                    fill(&mut bare_rack, map, WALL);
                    rack::draw(
                        &RackArt::rig(&st, row.checked_sub(1)),
                        &st,
                        drawn.stage,
                        &mut bare_rack,
                    );
                    let rects = &drawn.rects;
                    let rails = |x: u16, y: u16| {
                        let b = rects[1];
                        (x + 1 == b.x || x == b.right())
                            && y >= rects[0].y
                            && y < rects.last().unwrap().bottom()
                    };
                    let mut marks = 0;
                    for y in map.y..map.bottom() {
                        for x in drawn.list_end..map.right() {
                            let (a, b) = (&picked[(x, y)], &bare_rack[(x, y)]);
                            if !OUTLINE.contains(&a.symbol()) || OUTLINE.contains(&b.symbol()) {
                                continue;
                            }
                            marks += 1;
                            let wall = b.symbol() == " " && b.bg == WALL;
                            assert!(
                                wall || (row > 0 && rails(x, y)),
                                "{at}: outline on ({x},{y}) {:?} over {:?}:\n{}",
                                a.symbol(),
                                b.symbol(),
                                buffer_text(&picked)
                            );
                        }
                    }
                    assert!(marks >= 4, "{at}: the pick is outlined");
                }
            }
        }
    }

    #[test]
    fn the_list_lines_up_with_its_sleds_and_keeps_off_the_rack() {
        for (frame, st) in racks() {
            let bays = st.character.rig_fitted(&st.content);
            for size in [(100, 30), (120, 40)] {
                let at = format!("{frame} {}x{}", size.0, size.1);
                let (buf, drawn, map) = pane(&st, 1, size);
                let rects = &drawn.rects;
                assert!(rects[0].x > drawn.list_end, "{at}: a gap before the rack");
                for (i, (kind, _)) in bays.iter().enumerate() {
                    let (y, plate) = (drawn.rows[i + 1], rects[i + 1]);
                    assert!(
                        plate.y <= y && y < plate.bottom(),
                        "{at}: {kind:?} row {y} by {plate:?}"
                    );
                    let line: String = (map.x..drawn.list_end)
                        .map(|x| buf[(x, y)].symbol())
                        .collect();
                    assert!(line.contains(&kind.label().to_uppercase()), "{at}: {line}");
                }
                // Nothing of the list in its gap, under the header: only the wall, the
                // desk, or the frame's outline.
                for y in map.y + 2..map.bottom() {
                    let c = &buf[(drawn.list_end, y)];
                    assert!(
                        c.symbol() == " " || c.symbol() == "▀" || OUTLINE.contains(&c.symbol()),
                        "{at}: {:?} in the gap at row {y}\n{}",
                        c.symbol(),
                        buffer_text(&buf)
                    );
                }
            }
        }
    }

    #[test]
    fn a_refused_fit_leaves_the_list_open() {
        let mut st = state_home();
        st.character.rig_copy_mut(&st.content.clone());
        st.character.frames.push(ItemId::new("rack_nonesuch"));
        let mut d = Desk::open(&st);
        d.row = 0;
        let from = st.net.now(&st);
        press(&mut d, &mut st, KeyCode::Char(' '));
        press(&mut d, &mut st, KeyCode::Char(' '));
        assert_eq!(d.picker, Some(0), "the list stays open");
        assert_eq!(st.net.now(&st), from, "no time gone");
        assert_eq!(st.character.rig_frame(), ItemId::new("rack_zenbo"));
        assert_eq!(st.log.latest().map(|e| e.kind), Some(Kind::Warn));
    }

    #[test]
    fn the_map_on_top_gets_you_up_from_the_desk() {
        let mut st = state_home();
        let mut map = crate::screens::explore::MapScreen::walking(&mut st);
        st.at_desk = true;
        map.update(&mut st, &[]);
        assert!(!st.at_desk);
    }

    #[test]
    fn the_desk_fits_100x30() {
        let mut st = AppState::new();
        for frame in ["rack_zenbo", "rack_halcyon"] {
            st.character.set_frame(&st.content, &ItemId::new(frame));
            let d = Desk::open(&st);
            for (w, h) in [(100, 30), (120, 40)] {
                let text = drawn(&d, &st, w, h);
                for kind in ["FRAME", "CORE", "RELAY", "LINE"] {
                    assert!(text.contains(kind), "{frame} {w}x{h} {kind}:\n{text}");
                }
                if frame == "rack_halcyon" {
                    assert!(text.contains("FIREWALL"), "{w}x{h}:\n{text}");
                    assert!(text.contains("HALCYON NODE 9"), "{w}x{h}:\n{text}");
                }
                assert!(text.contains("BAYS"), "{frame} {w}x{h}");
                assert!(
                    text.contains(
                        "at your rig · ↑↓ bays · space what fits · x pull · esc stand up"
                    ),
                    "{frame} {w}x{h}:\n{text}"
                );
            }
        }
    }
}
