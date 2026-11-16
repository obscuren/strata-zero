//! CHROME on GEAR & INVENTORY: the tiles of the chrome in you, LOAD over the divider on
//! the right and the card under it, and the keys, which do what the ledger's did.

use super::card::{draw_lines, fit_card, piece_card};
use super::rules::{
    Here, Keys, Pulled, act, changing, chrome_mark, gauge, held, hint_line, install, load_after,
    pull,
};
use super::{Stop, chrome_layout, socket_kind, stops};
use crate::app::AppState;
use crate::screens::deck::{REVEAL_TICKS, cut};
use crate::screens::kit::body::{
    Dir, Ink, Lit, draw_joints, draw_label, draw_tile, header, perk_short, step,
};
use crate::screens::kit::{Kit, View};
use crate::ui::chrome::{DIM, FAINTER, RULE, put, reveal};
use crate::ui::hud::AMBER;
use crate::ui::implant_art;
use crossterm::event::KeyCode;
use game::{Item, Site};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::Line;

/// CHROME's words for walking and for leaving, in its hints.
const KEYS: Keys = Keys {
    select: "←↑↓→ select",
    back: "c meat/chrome",
};

/// Where on the body a stop is: a site (`None`), or a socket of what a site holds.
fn place(stop: Stop) -> (Site, Option<usize>) {
    match stop {
        Stop::Site(site) => (site, None),
        Stop::Socket(site, n) => (site, Some(n)),
    }
}

/// The stop the cursor stands on: `want` while it is still a stop, its site when a
/// socket went with its piece.
fn resolve(st: &AppState, want: Stop) -> Stop {
    let all = stops(st);
    if all.contains(&want) {
        return want;
    }
    let site = Stop::Site(place(want).0);
    if all.contains(&site) {
        site
    } else {
        all.first().copied().unwrap_or_default()
    }
}

/// A tile's label: the site's name as messages set it, or its socket's kind in
/// capitals.
fn label(st: &AppState, stop: Stop) -> String {
    match stop {
        Stop::Site(site) => site.label().to_string(),
        Stop::Socket(site, n) => {
            socket_kind(st, site, n).map_or_else(|| "SOCKET".into(), |k| k.label().to_uppercase())
        }
    }
}

/// What the piece `item` at `stop` gives, in a tile's `room` columns: `docked` for a
/// drone in its bay; else its first perk short, or the first ability it grants, or the
/// count of its sockets of a kind (`1 bay`); then, for a piece at a site, a pip for
/// each of its sockets (`■` fitted, `□` empty).
fn gives(st: &AppState, stop: Stop, item: &Item, room: usize) -> Vec<(String, Ink)> {
    if item.bot.is_some() {
        return vec![("docked".to_string(), Ink::Label)];
    }
    let pips: String = match stop {
        Stop::Site(site) => {
            let copy = st.character.chrome_copy(site);
            (0..item.sockets.len())
                .map(|i| {
                    let fitted = copy.is_some_and(|k| k.fitted.get(i).is_some_and(Option::is_some));
                    if fitted { '■' } else { '□' }
                })
                .collect()
        }
        Stop::Socket(..) => String::new(),
    };
    let room = match pips.chars().count() {
        0 => room,
        n => room.saturating_sub(n + 1),
    };
    let words = if let Some(perk) = item.perks.first() {
        perk_short(perk, room)
    } else if let Some(a) = item.grants.first() {
        st.content
            .ability(a)
            .map_or_else(|| a.to_string(), |a| a.name.clone())
    } else if let Some(&kind) = item.sockets.first() {
        let n = item.sockets.iter().filter(|&&k| k == kind).count();
        format!("{n} {}", kind.label())
    } else {
        String::new()
    };
    let mut out = vec![(cut(&words, room), Ink::Label)];
    if !pips.is_empty() {
        out.push((" ".to_string(), Ink::Label));
        out.push((pips, Ink::Pips));
    }
    out
}

impl Kit {
    /// Opens CHROME on `stop`, as the doc's chair does: FITS shut, no pull asked about,
    /// the card typing out.
    pub(crate) fn open_chrome(&mut self, stop: Stop, tick: u64) {
        self.state.view = View::Chrome;
        self.state.stop = stop;
        self.state.picker = None;
        self.pulling = None;
        self.changed = tick;
    }

    /// The stop the cursor is on, one that exists.
    fn stop(&self, st: &AppState) -> Stop {
        resolve(st, self.state.stop)
    }

    /// The cursor's place as it stands, with the pick while FITS is open on CHROME.
    fn here(&self, st: &AppState) -> Here {
        let picker = self
            .state
            .picker
            .filter(|_| self.state.view == View::Chrome);
        Here::of(st, place(self.stop(st)), picker)
    }

    /// Keeps CHROME's cursor on a stop that exists (a socket gone with its piece, its
    /// site, FITS shut: what it was open on is gone) and shuts FITS when nothing is
    /// left to pick.
    pub(crate) fn reselect_chrome(&mut self, st: &AppState) {
        let on = self.state.view == View::Chrome;
        let stop = self.stop(st);
        if stop != self.state.stop {
            self.state.stop = stop;
            self.state.picker = None;
            if on {
                self.changed = st.tick;
            }
        }
        if self.state.picker.is_some() {
            let pick = if on { self.here(st).pick } else { None };
            if pick != self.state.picker {
                if pick.is_none() {
                    self.changed = st.tick;
                }
                self.state.picker = pick;
            }
        }
    }

    /// CHROME's keys, and the picker's while FITS is open; true when it was the
    /// kit's. `c` switches back to MEAT from anywhere.
    pub(crate) fn chrome_key(&mut self, st: &mut AppState, code: KeyCode) -> bool {
        let tick = st.tick;
        // Anything but a second `x` lets the asked-about pull go; esc only that.
        if self.pulling.is_some() && code != KeyCode::Char('x') {
            self.pulling = None;
            if code == KeyCode::Esc {
                return true;
            }
        }
        if code == KeyCode::Char('c') {
            self.switch(tick);
            return true;
        }
        let here = self.here(st);
        if let Some(pick) = here.pick {
            match code {
                // The picker works inside the place's card: moving the pick changes
                // lines in place and never types the card out again. Closing it does
                // only on a held place, whose own card comes back (esc, below).
                KeyCode::Up | KeyCode::Char('k') => {
                    self.state.picker = Some(pick.saturating_sub(1))
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    self.state.picker = Some((pick + 1).min(here.picks.len() - 1))
                }
                KeyCode::Char(' ') | KeyCode::Enter => {
                    if install(st, &here, pick) {
                        self.state.picker = None;
                        self.changed = st.tick;
                    }
                }
                // Back on a piece, its own card types out again.
                KeyCode::Esc => {
                    self.state.picker = None;
                    if here.held.is_some() {
                        self.changed = tick;
                    }
                }
                // The picker keeps the cursor until it closes.
                KeyCode::Left | KeyCode::Char('h') | KeyCode::Right | KeyCode::Char('l') => {}
                _ => return false,
            }
            return true;
        }
        let dir = match code {
            KeyCode::Up | KeyCode::Char('k') => Dir::Up,
            KeyCode::Down | KeyCode::Char('j') => Dir::Down,
            KeyCode::Left | KeyCode::Char('h') => Dir::Left,
            KeyCode::Right | KeyCode::Char('l') => Dir::Right,
            // On a piece the FITS card takes the place of its own and types out.
            KeyCode::Char(' ') | KeyCode::Enter => {
                if act(st, &here) {
                    self.state.picker = Some(0);
                    if here.held.is_some() {
                        self.changed = st.tick;
                    }
                }
                return true;
            }
            KeyCode::Char('x') => {
                let stop = self.stop(st);
                match pull(st, &here, self.pulling == Some(stop)) {
                    Pulled::Nothing => {}
                    Pulled::Asked => self.pulling = Some(stop),
                    Pulled::Tried(ok) => {
                        self.pulling = None;
                        if ok {
                            self.changed = st.tick;
                        }
                    }
                }
                return true;
            }
            _ => return false,
        };
        let l = chrome_layout(self.area(), st);
        if let Some(to) = step(&l, self.stop(st), dir) {
            self.state.stop = to;
            self.changed = tick;
        }
        true
    }

    /// CHROME's hint row: the ledger's, with `c meat/chrome` and the four arrows.
    pub(crate) fn chrome_hints(&self, st: &AppState) -> Line<'static> {
        let asked = self.pulling.is_some() && self.pulling == Some(self.stop(st));
        hint_line(st, &self.here(st), asked, KEYS)
    }

    /// Draws CHROME into the tab's `area`: the header with the ledger's mark, the tiles
    /// and their lines, the divider (amber while chrome can change), and right of it
    /// LOAD over a divider of its own with the card under it.
    pub(crate) fn draw_chrome(&self, st: &AppState, area: Rect, buf: &mut Buffer) {
        let l = chrome_layout(area, st);
        let stop = self.stop(st);
        let here = self.here(st);
        header(st, &l, true, true, &chrome_mark(st), buf);
        let rows = if l.full { 2 } else { 1 };
        let light = |s: Stop| match (s == stop, here.pick) {
            (false, _) => Lit::Off,
            (true, None) => Lit::On,
            (true, Some(_)) => Lit::Dim,
        };
        for &(s, r) in &l.tiles {
            let room = usize::from(r.width.saturating_sub(2));
            let piece = held(st, place(s)).and_then(|id| st.content.item(id));
            let holds = piece.map(|item| (item.name.as_str(), gives(st, s, item, room)));
            draw_tile(st, r, &label(st, s), holds, light(s), rows, buf);
        }
        // A line meets a tile at its middle column, where the label sits: the label
        // stays whole and the line runs up to it.
        draw_joints(&l, buf);
        for &(s, r) in &l.tiles {
            draw_label(st, r, &label(st, s), light(s), buf);
        }

        let divider = l.left.right();
        if divider >= area.right() {
            return;
        }
        let edge = Style::default().fg(if changing(st) { AMBER } else { RULE });
        for y in area.y..area.bottom() {
            buf[(divider, y)].set_char('║').set_style(edge);
        }
        let (x0, x1) = (divider + 2, area.right().saturating_sub(1));
        if x0 + 2 >= x1 || area.height < 4 {
            return;
        }
        self.draw_load(st, &here, (x0, x1), area.y, buf);
        let rule = area.y + 2;
        buf[(divider, rule)].set_char('╠').set_style(edge);
        for x in divider + 1..area.right() {
            buf[(x, rule)].set_char('═').set_fg(RULE);
        }
        let card = Rect::new(x0, rule + 1, x1 - x0, area.bottom() - (rule + 1));
        self.draw_chrome_card(st, &here, card, buf);
    }

    /// `LOAD ▰▰▰▱ 19/24` across the pane's columns `x0..x1` on row `y`, the count at its
    /// right end, with the load after the pick while FITS is open (the ledger's
    /// colours); under it, dim, what's left (or how far over) and the pieces in you.
    fn draw_load(
        &self,
        st: &AppState,
        here: &Here,
        (x0, x1): (u16, u16),
        y: u16,
        buf: &mut Buffer,
    ) {
        let ch = &st.character;
        let (load, max) = (ch.chrome_load(&st.content), ch.load_max());
        let after = here.pick.map(|p| load_after(st, here.at, &here.picks[p]));
        let total = after.unwrap_or(load);
        // The count takes the room of `total/max` with the total as wide as the
        // budget, so the gauge keeps its length as the pick moves.
        let digits = |n: u32| n.to_string().chars().count() as u16;
        let count = 2 + digits(total).max(digits(max)) + digits(max);
        let label = "LOAD ";
        let segments = (x1 - x0).saturating_sub(label.len() as u16 + count);
        put(
            buf,
            x0,
            y,
            label,
            Style::default().fg(DIM).add_modifier(Modifier::BOLD),
            x1,
        );
        let runs = gauge(st, after, u32::from(segments), true);
        let (bar, tail) = runs.split_at(3);
        let mut x = x0 + label.len() as u16;
        for (text, style) in bar {
            put(buf, x, y, text, *style, x1);
            x += text.chars().count() as u16;
        }
        let tail_w: u16 = tail.iter().map(|(t, _)| t.chars().count() as u16).sum();
        let mut x = x1.saturating_sub(tail_w).max(x);
        for (text, style) in tail {
            put(buf, x, y, text, *style, x1);
            x += text.chars().count() as u16;
        }
        let left = if total > max {
            format!("over by {}", total - max)
        } else {
            format!("{} left", max - total)
        };
        let pieces = ch.pieces(&st.content);
        let noun = if pieces == 1 { "piece" } else { "pieces" };
        put(
            buf,
            x0,
            y + 1,
            &format!("{left} · {pieces} {noun} in you"),
            Style::default().fg(FAINTER),
            x1,
        );
    }

    /// The card of the cursor's stop, typing out: a piece's drawing and what it is, or
    /// FITS, what in the pack fits an empty place, or a held one while the picker is
    /// open.
    fn draw_chrome_card(&self, st: &AppState, here: &Here, area: Rect, buf: &mut Buffer) {
        let p = st.tick.saturating_sub(self.changed) as f32 / REVEAL_TICKS as f32;
        let cursor_on = (st.tick / 4).is_multiple_of(2);
        let w = area.width as usize;
        let mut text = area;
        let card = match (&here.held, here.pick) {
            (Some(id), None) => {
                // The drawing scans in over the first half of the reveal.
                if let Some(art) = st.content.implant_art(id) {
                    let scan = ((2.0 * p).min(1.0) * f32::from(implant_art::ROWS)).ceil() as u16;
                    implant_art::draw(buf, area.x, area.y, area, art, scan);
                    let drop = (implant_art::ROWS + 1).min(area.height);
                    text.y += drop;
                    text.height -= drop;
                }
                piece_card(st, here.at, id, w)
            }
            _ => fit_card(st, here, true, w),
        };
        let card = card.fit(text.height as usize, w);
        draw_lines(buf, text, &reveal(&card.lines, p, cursor_on), &card.bars);
    }
}
