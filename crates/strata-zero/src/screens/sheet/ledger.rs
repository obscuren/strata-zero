//! The sheet's right pane: the ledger of the body's sites with a chrome limb's sockets
//! folded under it, the card of the row the cursor is on, the picker of what in the pack
//! fits a site or socket (swapping out what it holds), and the LOAD gauge over them.
//! The rules and the card are CHROME's on GEAR & INVENTORY (`kit::chrome`).

use super::{RULE_X, Sheet, SheetPane, marked_header};
use crate::app::AppState;
use crate::screens::deck::{REVEAL_TICKS, cut, lit};
use crate::screens::kit::chrome::card::{draw_lines, fit_card, piece_card};
use crate::screens::kit::chrome::rules::{
    Here, Keys, Pulled, act, chrome_mark, gauge, held, hint_line, install, load_after, pull,
    socket_word,
};
use crate::ui::chrome::{BAR_BG, CYAN, DIM, FAINTER, RULE, TEXT, put, reveal, solid};
use crate::ui::implant_art;
use crossterm::event::KeyCode;
use game::Site;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Line;

/// The right pane's first column, from the body's left edge: two past the rule.
const RIGHT_X: u16 = RULE_X + 2;
/// The ledger's width; on a body 120 columns wide and more, where each row names its
/// site, wider.
const LEDGER_W: u16 = 18;
const LEDGER_WIDE: u16 = 27;
/// A 120-column screen's body.
const WIDE_BODY: u16 = 118;
/// The wide ledger's site labels, before what each holds.
const LABEL_W: u16 = 8;
/// The LOAD gauge's segments.
const SEGMENTS: u32 = 12;
/// The ledger's walking and leaving words in its hints.
const KEYS: Keys = Keys {
    select: "↑↓ select",
    back: "← attributes",
};

/// A row of the ledger: a group's header, a site, or a socket of the piece a site holds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LedgerRow {
    Group(&'static str),
    Site(Site),
    Socket(Site, usize),
}

impl Default for LedgerRow {
    fn default() -> LedgerRow {
        LedgerRow::Site(Site::ALL[0])
    }
}

impl LedgerRow {
    /// Where on the body the row is: a site (`None`), or a socket of what a site holds.
    /// A group's header is nowhere.
    fn at(self) -> Option<(Site, Option<usize>)> {
        match self {
            LedgerRow::Group(_) => None,
            LedgerRow::Site(s) => Some((s, None)),
            LedgerRow::Socket(s, n) => Some((s, Some(n))),
        }
    }
}

/// The ledger's rows: every site in `Site::ALL` order under its group's header, and
/// under a piece with sockets one row for each.
pub fn ledger_rows(st: &AppState) -> Vec<LedgerRow> {
    let (c, ch) = (&st.content, &st.character);
    let mut rows = Vec::new();
    let mut group = None;
    for site in Site::ALL {
        if group != Some(site.group()) {
            group = Some(site.group());
            rows.push(LedgerRow::Group(site.group()));
        }
        rows.push(LedgerRow::Site(site));
        let sockets = ch.chrome_at(site).and_then(|id| c.item(id));
        let n = sockets.map_or(0, |i| i.sockets.len());
        rows.extend((0..n).map(|k| LedgerRow::Socket(site, k)));
    }
    rows
}

/// The row the cursor stands on: `want` while it is still a row, its site when a
/// socket went with its limb, else the first site.
fn resolve(rows: &[LedgerRow], want: LedgerRow) -> usize {
    let find = |r: LedgerRow| rows.iter().position(|&x| x == r);
    want.at()
        .and_then(|_| find(want))
        .or_else(|| match want {
            LedgerRow::Socket(site, _) => find(LedgerRow::Site(site)),
            _ => None,
        })
        .or_else(|| rows.iter().position(|r| r.at().is_some()))
        .unwrap_or(0)
}

/// The right pane as it stands.
struct View {
    rows: Vec<LedgerRow>,
    /// The cursor's row, an index into `rows`.
    row: usize,
    /// Its place, what it holds, what fits there and the pick.
    here: Here,
}

impl Sheet {
    fn view(&self, st: &AppState) -> View {
        let rows = ledger_rows(st);
        let row = resolve(&rows, self.state.row);
        let at = rows[row].at().unwrap_or((Site::ALL[0], None));
        let picker = self
            .state
            .picker
            .filter(|_| self.state.pane == SheetPane::Ledger);
        View {
            rows,
            row,
            here: Here::of(st, at, picker),
        }
    }

    /// Puts the cursor back on a row that exists (a socket's limb gone, its site) and
    /// closes a picker that has nothing left to pick.
    pub fn reselect(&mut self, st: &AppState) {
        let v = self.view(st);
        let row = v.rows[v.row];
        if row != self.state.row {
            self.state.row = row;
            self.card = st.tick;
        }
        if self.state.picker.is_some() && v.here.pick != self.state.picker {
            if v.here.pick.is_none() {
                self.card = st.tick;
            }
            self.state.picker = v.here.pick;
        }
    }

    /// The ledger's keys, and the picker's while it is open; true when it was the
    /// sheet's.
    pub(super) fn ledger_key(&mut self, st: &mut AppState, code: KeyCode) -> bool {
        let tick = st.tick;
        // Anything but a second `x` lets the asked-about pull go; esc only that.
        if self.pulling.is_some() && code != KeyCode::Char('x') {
            self.pulling = None;
            if code == KeyCode::Esc {
                return true;
            }
        }
        let v = self.view(st);
        let here = &v.here;
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
                    if install(st, here, pick) {
                        self.state.picker = None;
                        self.card = st.tick;
                    }
                }
                // Back on a piece, its own card types out again.
                KeyCode::Esc => {
                    self.state.picker = None;
                    if here.held.is_some() {
                        self.card = tick;
                    }
                }
                // The picker keeps the cursor until it closes.
                KeyCode::Left | KeyCode::Char('h') | KeyCode::Right | KeyCode::Char('l') => {}
                _ => return false,
            }
            return true;
        }
        match code {
            KeyCode::Left | KeyCode::Char('h') => self.state.pane = SheetPane::Left,
            KeyCode::Up | KeyCode::Char('k') => {
                let up = v.rows[..v.row].iter().rposition(|r| r.at().is_some());
                self.walk(&v, up, tick);
            }
            KeyCode::Down | KeyCode::Char('j') => {
                let down = v.rows[v.row + 1..].iter().position(|r| r.at().is_some());
                self.walk(&v, down.map(|i| v.row + 1 + i), tick);
            }
            KeyCode::Right | KeyCode::Char('l') => {}
            // On a piece the FITS card takes the place of its own and types out.
            KeyCode::Char(' ') | KeyCode::Enter => {
                if act(st, here) {
                    self.state.picker = Some(0);
                    if here.held.is_some() {
                        self.card = st.tick;
                    }
                }
            }
            KeyCode::Char('x') => {
                let row = v.rows[v.row];
                match pull(st, here, self.pulling == Some(row)) {
                    Pulled::Nothing => {}
                    Pulled::Asked => self.pulling = Some(row),
                    Pulled::Tried(ok) => {
                        self.pulling = None;
                        if ok {
                            self.card = st.tick;
                        }
                    }
                }
            }
            _ => return false,
        }
        true
    }

    fn walk(&mut self, v: &View, to: Option<usize>, tick: u64) {
        if let Some(i) = to {
            self.state.row = v.rows[i];
            self.card = tick;
        }
    }

    /// The ledger's hint row: what space does, and on chrome what `x` does, dimmer
    /// while the lock is on.
    pub(super) fn ledger_hints(&self, st: &AppState) -> Line<'static> {
        let v = self.view(st);
        let asked = self.pulling == Some(v.rows[v.row]);
        hint_line(st, &v.here, asked, KEYS)
    }

    /// Draws the right pane into `body`'s columns past the rule: the CHROME header with
    /// its LOAD gauge, the ledger, its rule and the card.
    pub(super) fn draw_right(&self, st: &AppState, body: Rect, buf: &mut Buffer) {
        let x0 = body.x + RIGHT_X;
        let x1 = body.right().saturating_sub(1);
        if x0 + 2 >= x1 || body.height < 2 {
            return;
        }
        // Solid, as the left pane is: the map's ghost stays out of the gaps.
        let pane = body.x + RULE_X + 1;
        solid(
            buf,
            Rect::new(pane, body.y, body.right() - pane, body.height),
        );
        let v = self.view(st);
        let focused = self.state.pane == SheetPane::Ledger;
        let wide = body.width >= WIDE_BODY;
        let rule = (x0 + if wide { LEDGER_WIDE } else { LEDGER_W }).min(x1);
        let after = v
            .here
            .pick
            .map(|p| load_after(st, v.here.at, &v.here.picks[p]));
        chrome_header(st, (x0, x1), body.y, focused, after, buf);
        let rows = Rect::new(x0, body.y + 1, rule - x0, body.height - 1);
        draw_ledger(st, &v, rows, focused && v.here.pick.is_none(), wide, buf);
        for y in body.y + 1..body.bottom() {
            buf[(rule, y)].set_char('│').set_fg(RULE);
        }
        let cx = rule + 2;
        if cx < x1 {
            let card = Rect::new(cx, body.y + 1, x1 - cx, body.height - 1);
            self.draw_card(st, &v.here, card, focused, buf);
        }
    }

    /// The card of the cursor's row, typing out: a piece's drawing and what it is, or
    /// what in the pack fits an empty place, or a held one while the picker is open.
    fn draw_card(&self, st: &AppState, v: &Here, area: Rect, focused: bool, buf: &mut Buffer) {
        let p = st.tick.saturating_sub(self.card) as f32 / REVEAL_TICKS as f32;
        let cursor_on = (st.tick / 4).is_multiple_of(2);
        let w = area.width as usize;
        let mut text = area;
        let card = match (&v.held, v.pick) {
            (Some(id), None) => {
                // The drawing scans in over the first half of the reveal.
                if let Some(art) = st.content.implant_art(id) {
                    let scan = ((2.0 * p).min(1.0) * f32::from(implant_art::ROWS)).ceil() as u16;
                    implant_art::draw(buf, area.x, area.y, area, art, scan);
                    let drop = (implant_art::ROWS + 1).min(area.height);
                    text.y += drop;
                    text.height -= drop;
                }
                piece_card(st, v.at, id, w)
            }
            _ => fit_card(st, v, focused, w),
        };
        let card = card.fit(text.height as usize, w);
        draw_lines(buf, text, &reveal(&card.lines, p, cursor_on), &card.bars);
    }
}

/// `CHROME ──── LOAD ▰▰▰▱ 20/24`: the load's share of the budget in twelve segments,
/// rounded up, with the load `after` a pick goes in (`rules::gauge`).
fn chrome_header(
    st: &AppState,
    (x0, x1): (u16, u16),
    y: u16,
    focused: bool,
    after: Option<u32>,
    buf: &mut Buffer,
) {
    let mut tail = vec![("LOAD ".to_string(), Style::default().fg(DIM))];
    tail.extend(gauge(st, after, SEGMENTS, focused));
    marked_header(buf, (x0, x1), y, "CHROME", &chrome_mark(st), &tail, focused);
}

/// the piece. The cursor's row has the mark, and the bar while the ledger has focus.
fn draw_ledger(st: &AppState, v: &View, area: Rect, focused: bool, wide: bool, buf: &mut Buffer) {
    let right = area.right();
    let height = area.height as usize;
    let first = match v.rows.len().checked_sub(height) {
        Some(over) if over > 0 => v.row.saturating_sub(height / 2).min(over),
        _ => 0,
    };
    let name_x = area.x + 2 + if wide { LABEL_W } else { 0 };
    let fg = |c: Color| Style::default().fg(c);
    for (k, (i, &row)) in v
        .rows
        .iter()
        .enumerate()
        .skip(first)
        .take(height)
        .enumerate()
    {
        let y = area.y + k as u16;
        let Some(at) = row.at() else {
            if let LedgerRow::Group(g) = row {
                put(
                    buf,
                    area.x + 1,
                    y,
                    &format!("── {g} ──"),
                    fg(FAINTER),
                    right,
                );
            }
            continue;
        };
        let on = i == v.row;
        let bar = on && focused;
        let bg = |s: Style| if bar { s.bg(BAR_BG) } else { s };
        if bar {
            for x in area.x..right {
                buf[(x, y)].set_bg(BAR_BG);
            }
        }
        if on {
            let mark = fg(if focused { lit(st.tick) } else { DIM });
            put(
                buf,
                area.x,
                y,
                "▸",
                bg(mark).add_modifier(Modifier::BOLD),
                right,
            );
        }
        let (x, empty) = match row {
            LedgerRow::Socket(site, n) => {
                let last = !matches!(v.rows.get(i + 1), Some(LedgerRow::Socket(..)));
                let branch = if last { "└ " } else { "├ " };
                put(buf, name_x + 1, y, branch, bg(fg(RULE)), right);
                (name_x + 3, format!("□ {}", socket_word(st, site, n)))
            }
            LedgerRow::Site(site) if wide => {
                put(buf, area.x + 2, y, site.label(), bg(fg(FAINTER)), right);
                (name_x, "· meat".to_string())
            }
            LedgerRow::Site(site) => (name_x, format!("· {}", site.lower())),
            LedgerRow::Group(_) => continue,
        };
        let room = right.saturating_sub(x) as usize;
        let (text, style) = match held(st, at).and_then(|id| st.content.item(id)) {
            Some(item) => {
                let colour = match (on, focused) {
                    (true, true) => CYAN,
                    (true, false) => TEXT,
                    _ if at.1.is_none() => TEXT,
                    _ => DIM,
                };
                let style = fg(colour);
                (
                    item.name.clone(),
                    if on {
                        style.add_modifier(Modifier::BOLD)
                    } else {
                        style
                    },
                )
            }
            None if bar => (empty, fg(lit(st.tick)).add_modifier(Modifier::BOLD)),
            None => (empty, fg(FAINTER)),
        };
        put(buf, x, y, &cut(&text, room), bg(style), right);
    }
}

#[cfg(test)]
mod tests {
    //! The ledger's own rows, walk and look; the rules it shares with CHROME are tested
    //! there (`kit::chrome`).
    use super::*;
    use crate::app::{Screen, ScreenCmd};
    use crate::screens::deck::{Deck, REVEAL_TICKS};
    use crate::screens::testutil::key;
    use crate::ui::chrome::{BAR_BG, CYAN, DIM, FAINTER, RULE, TEXT};
    use crate::ui::hud::AMBER;
    use game::{CopyId, ItemId, Stack};
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use ratatui::style::Color;

    /// The deck's body at 100×30.
    const BODY: Rect = Rect::new(1, 5, 98, 22);
    /// The right pane's first column at both sizes; at 100×30 the ledger's rule and
    /// the card's first column.
    const R0: u16 = 48;
    const RULE_100: u16 = 66;
    const CARD_100: u16 = 68;

    fn id(s: &str) -> ItemId {
        ItemId::new(s)
    }

    fn last(st: &AppState) -> Option<&str> {
        st.log.latest().map(|e| e.text.as_str())
    }

    /// One of `item` into the pack; its copy when it is held per copy.
    fn give(st: &mut AppState, item: &str) -> Option<CopyId> {
        st.character.give_item(&st.content, id(item), 1);
        st.character
            .inventory
            .iter()
            .rev()
            .find(|s| s.item == id(item))
            .and_then(|s| s.copy_id())
    }

    /// Installs `item` at `site`, as a copy when it is held per copy, with `fitted` in
    /// its sockets.
    fn install(st: &mut AppState, site: Site, item: &str, fitted: &[(usize, &str)]) {
        let per_copy = st.content.item(&id(item)).unwrap().per_copy();
        let mut copy = None;
        if per_copy {
            let mut k = st.character.new_copy(&st.content, &id(item));
            for &(n, piece) in fitted {
                let pc = st.character.new_copy(&st.content, &id(piece));
                k.fitted[n] = Some(Stack {
                    item: id(piece),
                    qty: 1,
                    copy: Some(pc),
                });
            }
            copy = Some(k);
        }
        st.character.install(site, id(item), copy);
    }

    /// The spec's loadout: a piece at every site but EARS, R ARM and LEGS, the Scout
    /// drone in the cradle's bay, and the Piston arm at L ARM with the Arc hand in its
    /// hand.
    fn loadout(st: &mut AppState) {
        for (site, item) in [
            (Site::Neural, "neural_port"),
            (Site::Eyes, "target_lens"),
            (Site::Jaw, "bulldog_jaw"),
            (Site::Spine, "redline_spine"),
            (Site::Organs, "adrenal_pump"),
            (Site::Skin, "subdermal_mesh"),
        ] {
            install(st, site, item, &[]);
        }
        install(st, Site::Back, "drone_cradle", &[(0, "scout_drone")]);
        install(st, Site::LeftArm, "piston_arm", &[(0, "arc_hand")]);
    }

    /// The deck on ATTRIBUTES with the cursor on the ledger's `row`, free chrome on: these
    /// test the ledger, the chair has its own.
    fn on_ledger(st: &mut AppState, row: LedgerRow) -> Deck {
        st.free_chrome = true;
        let mut d = Deck::new(st.tick);
        d.update(st, &[key(KeyCode::Char('2'))]);
        d.sheet.state.pane = SheetPane::Ledger;
        d.sheet.state.row = row;
        d
    }

    fn press(d: &mut Deck, st: &mut AppState, codes: &[KeyCode]) {
        let keys: Vec<_> = codes.iter().map(|&c| key(c)).collect();
        d.update(st, &keys);
    }

    /// The deck at `w`×`h` once its cards and the strip have typed out, clear of the
    /// deck's signal jitter.
    fn screen(d: &Deck, st: &mut AppState, (w, h): (u16, u16)) -> Buffer {
        let typed = d.sheet.card.max(d.sheet.changed) + REVEAL_TICKS + 1;
        st.tick = (st.tick + 25).max(typed);
        if st.tick % 90 < 3 {
            st.tick += 3;
        }
        let mut t = Terminal::new(TestBackend::new(w, h)).unwrap();
        t.draw(|f| d.draw(st, f)).unwrap();
        t.backend().buffer().clone()
    }

    /// `n` cells of row `y` from column `x`.
    fn cells(buf: &Buffer, x: u16, y: u16, n: u16) -> String {
        (x..x + n).map(|cx| buf[(cx, y)].symbol()).collect()
    }

    /// Columns `x0..x1` of rows `y0..y1`, a line each.
    fn region(buf: &Buffer, (x0, x1): (u16, u16), (y0, y1): (u16, u16)) -> String {
        (y0..y1)
            .map(|y| cells(buf, x0, y, x1 - x0).trim_end().to_string() + "\n")
            .collect()
    }

    /// The column in row `y`, from `x0`, where `needle` starts.
    fn col_of(buf: &Buffer, y: u16, x0: u16, needle: &str) -> Option<u16> {
        let want: Vec<String> = needle.chars().map(String::from).collect();
        let n = want.len() as u16;
        (x0..buf.area.right().saturating_sub(n))
            .find(|&x| (0..n).all(|k| buf[(x + k, y)].symbol() == want[k as usize]))
    }

    /// The row, `y0..y1`, whose cells from `x0` hold `needle`, and where it starts.
    fn find(buf: &Buffer, x0: u16, (y0, y1): (u16, u16), needle: &str) -> Option<(u16, u16)> {
        (y0..y1).find_map(|y| col_of(buf, y, x0, needle).map(|x| (x, y)))
    }

    #[test]
    fn ledger_lists_sites_and_sockets() {
        let mut st = AppState::new();
        loadout(&mut st);
        use LedgerRow::{Group, Socket};
        let s = LedgerRow::Site;
        assert_eq!(
            ledger_rows(&st),
            [
                Group("head"),
                s(Site::Neural),
                s(Site::Eyes),
                s(Site::Ears),
                s(Site::Jaw),
                Group("body"),
                s(Site::Spine),
                s(Site::Back),
                Socket(Site::Back, 0),
                s(Site::Organs),
                s(Site::Skin),
                Group("limbs"),
                s(Site::LeftArm),
                Socket(Site::LeftArm, 0),
                Socket(Site::LeftArm, 1),
                s(Site::RightArm),
                s(Site::Legs),
            ]
        );
        let d = on_ledger(&mut st, s(Site::LeftArm));
        let buf = screen(&d, &mut st, (100, 30));
        let ledger = region(&buf, (R0, RULE_100), (6, 27));
        for want in [
            "── head ──",
            "Neural port",
            "· ears",
            "└ Scout drone",
            "── limbs ──",
            "Piston arm",
            "├ Arc hand",
            "└ □ forearm",
            "· r arm",
            "· legs",
        ] {
            assert!(ledger.contains(want), "{want}:\n{ledger}");
        }
        // The mock's columns: groups from R0 + 1, names from R0 + 2, sockets' rules at
        // R0 + 3 and their names at R0 + 5; the cursor's mark at R0 on the bar.
        assert_eq!(cells(&buf, R0 + 1, 6, 10), "── head ──");
        assert_eq!(buf[(R0 + 1, 6)].fg, FAINTER);
        assert_eq!(cells(&buf, R0 + 2, 7, 11), "Neural port");
        assert_eq!(buf[(R0 + 2, 7)].fg, TEXT);
        assert_eq!(cells(&buf, R0 + 2, 9, 6), "· ears");
        assert_eq!(buf[(R0 + 2, 9)].fg, FAINTER);
        assert_eq!(buf[(R0, 18)].symbol(), "▸");
        assert_eq!(cells(&buf, R0 + 2, 18, 10), "Piston arm");
        assert_eq!(buf[(R0 + 2, 18)].fg, CYAN);
        for x in R0..RULE_100 {
            assert_eq!(buf[(x, 18)].bg, BAR_BG, "the cursor's bar at {x}");
        }
        assert_eq!(cells(&buf, R0 + 3, 19, 10), "├ Arc hand");
        assert_eq!(buf[(R0 + 3, 19)].fg, RULE);
        assert_eq!(buf[(R0 + 5, 19)].fg, DIM);
        assert_eq!(cells(&buf, R0 + 3, 20, 11), "└ □ forearm");
        assert_eq!(buf[(R0 + 5, 20)].fg, FAINTER);
        for y in 6..BODY.bottom() {
            assert_eq!(buf[(RULE_100, y)].symbol(), "│", "the ledger's rule at {y}");
        }
        // The header: CHROME, and LOAD with the numbers from content.
        let (load, max) = (
            st.character.chrome_load(&st.content),
            st.character.load_max(),
        );
        assert_eq!(cells(&buf, R0, 5, 6), "CHROME");
        assert_eq!(buf[(R0, 5)].fg, CYAN);
        let header = cells(&buf, R0, 5, 50);
        assert!(header.contains("LOAD"), "{header}");
        let total = format!("{load}/{max}");
        assert!(header.trim_end().ends_with(&total), "{header}");
        assert_eq!(
            cells(&buf, 97, 5, 1),
            max.to_string().chars().last().unwrap().to_string()
        );
        // Twelve segments, the load's share rounded up.
        let filled = (load * 12).div_ceil(max) as usize;
        assert_eq!(header.matches('▰').count(), filled, "{header}");
        assert_eq!(header.matches('▱').count(), 12 - filled, "{header}");
    }

    #[test]
    fn full_body_fits_at_100x30() {
        let mut st = AppState::new();
        for (site, item) in [
            (Site::Neural, "neural_port"),
            (Site::Eyes, "target_lens"),
            (Site::Ears, "damper_cochlea"),
            (Site::Jaw, "bulldog_jaw"),
            (Site::Spine, "redline_spine"),
            (Site::Organs, "adrenal_pump"),
            (Site::Skin, "subdermal_mesh"),
            (Site::Legs, "dog_legs"),
        ] {
            install(&mut st, site, item, &[]);
        }
        install(&mut st, Site::Back, "drone_cradle", &[(0, "scout_drone")]);
        install(
            &mut st,
            Site::LeftArm,
            "piston_arm",
            &[(0, "arc_hand"), (1, "monowire_spool")],
        );
        install(
            &mut st,
            Site::RightArm,
            "grip_arm",
            &[(0, "toolfingers"), (1, "launcher_sleeve")],
        );
        let rows = ledger_rows(&st);
        // Three groups, eleven sites, four arm sockets and the cradle's bay.
        assert_eq!(rows.len(), 3 + 11 + 5);
        // Every row on its own line inside the body, whichever the cursor is on.
        for (i, &row) in rows.iter().enumerate() {
            let Some(at) = (match row {
                LedgerRow::Group(_) => None,
                LedgerRow::Site(s) => Some((s, None)),
                LedgerRow::Socket(s, n) => Some((s, Some(n))),
            }) else {
                continue;
            };
            let d = on_ledger(&mut st, row);
            let buf = screen(&d, &mut st, (100, 30));
            let y = BODY.y + 1 + i as u16;
            assert!(y < BODY.bottom());
            assert_eq!(buf[(R0, y)].symbol(), "▸", "{row:?}");
            let name = match at {
                (s, None) => st.character.chrome_at(s).cloned(),
                (s, Some(n)) => st
                    .character
                    .chrome_copy(s)
                    .and_then(|k| k.fitted[n].as_ref().map(|f| f.item.clone())),
            };
            let name = st.content.item(&name.unwrap()).unwrap().name.clone();
            let shown = cells(&buf, R0, y, RULE_100 - R0);
            let lead: String = name.chars().take(8).collect();
            assert!(shown.contains(&lead), "{row:?}: {shown}");
            // The frame's edge right of the body is the deck's.
            for y in BODY.y..BODY.bottom() {
                assert_eq!(buf[(BODY.right(), y)].symbol(), "│", "{row:?} y {y}");
            }
        }
        // Nothing of the sheet outside its body, whichever row.
        for &row in &rows {
            let mut s = Sheet::new(0);
            s.state.pane = SheetPane::Ledger;
            s.state.row = row;
            let mut st2 = AppState::new();
            st2.character = st.character.clone();
            st2.tick = REVEAL_TICKS + 1;
            let area = Rect::new(0, 0, 100, 30);
            let mut buf = Buffer::empty(area);
            for c in &mut buf.content {
                c.set_char('X');
            }
            s.draw(&st2, BODY, &mut buf);
            for y in area.y..area.bottom() {
                for x in area.x..area.right() {
                    if !BODY.contains((x, y).into()) {
                        assert_eq!(buf[(x, y)].symbol(), "X", "{row:?} ({x},{y})");
                    }
                }
            }
        }
    }

    #[test]
    fn picker_installs() {
        let mut st = AppState::new();
        loadout(&mut st);
        let grip = give(&mut st, "grip_arm");
        let (load, max) = (
            st.character.chrome_load(&st.content),
            st.character.load_max(),
        );
        let mut d = on_ledger(&mut st, LedgerRow::Site(Site::RightArm));
        press(&mut d, &mut st, &[KeyCode::Char(' ')]);
        assert_eq!(d.sheet.state.picker, Some(0));
        let buf = screen(&d, &mut st, (100, 30));
        let card = region(&buf, (CARD_100, 98), (6, 27));
        for want in ["RIGHT ARM", "FITS", "from your bag", "Grip arm", "L3"] {
            assert!(card.contains(want), "{want}:\n{card}");
        }
        // The pick on the bar; the ledger's cursor dim.
        let (x, y) = find(&buf, CARD_100, (6, 27), "Grip arm").unwrap();
        assert_eq!(buf[(x, y)].bg, BAR_BG);
        assert_eq!(buf[(R0, 21)].symbol(), "▸");
        assert_eq!(buf[(R0, 21)].fg, DIM);
        assert_ne!(buf[(R0 + 2, 21)].bg, BAR_BG);
        // LOAD shows what it takes, in amber.
        let after = format!(" {}/{max}", load + 3);
        let x = col_of(&buf, 5, R0, &after).unwrap_or_else(|| panic!("{}", cells(&buf, R0, 5, 50)));
        assert_eq!(buf[(x + 1, 5)].fg, AMBER);
        assert!((R0..98).any(|x| buf[(x, 5)].symbol() == "▰" && buf[(x, 5)].fg == AMBER));
        // Space installs it, and the picker closes on the arm.
        press(&mut d, &mut st, &[KeyCode::Char(' ')]);
        assert_eq!(
            st.character.chrome_at(Site::RightArm),
            Some(&id("grip_arm"))
        );
        assert_eq!(st.character.chrome_copy(Site::RightArm).map(|k| k.id), grip);
        assert_eq!(last(&st), Some("installed Grip arm · R ARM"));
        assert_eq!(d.sheet.state.picker, None);
        assert_eq!(d.sheet.state.row, LedgerRow::Site(Site::RightArm));

        // Esc closes the picker and changes nothing; esc again closes the deck.
        let mut st = AppState::new();
        loadout(&mut st);
        give(&mut st, "grip_arm");
        let mut d = on_ledger(&mut st, LedgerRow::Site(Site::RightArm));
        press(&mut d, &mut st, &[KeyCode::Char(' ')]);
        assert_eq!(d.sheet.state.picker, Some(0));
        let before = st.character.clone();
        assert!(matches!(
            d.update(&mut st, &[key(KeyCode::Esc)]),
            ScreenCmd::Stay
        ));
        assert_eq!(d.sheet.state.picker, None);
        assert_eq!(st.character, before);
        assert_eq!(d.sheet.state.row, LedgerRow::Site(Site::RightArm));
        assert!(matches!(
            d.update(&mut st, &[key(KeyCode::Esc)]),
            ScreenCmd::Pop { .. }
        ));
    }

    #[test]
    fn wide_ledger_names_sites() {
        let mut st = AppState::new();
        loadout(&mut st);
        let d = on_ledger(&mut st, LedgerRow::Site(Site::LeftArm));
        let buf = screen(&d, &mut st, (120, 40));
        // The body is (1, 10, 118, 27): the ledger 27 wide, its rule at 75, the card
        // from 77.
        let ledger = region(&buf, (R0, 75), (11, 37));
        for want in [
            "NEURAL  Neural port",
            "EARS    · meat",
            "L ARM   Piston arm",
        ] {
            assert!(ledger.contains(want), "{want}:\n{ledger}");
        }
        assert_eq!(cells(&buf, R0 + 2, 12, 19), "NEURAL  Neural port");
        assert_eq!(buf[(R0 + 2, 12)].fg, FAINTER);
        assert_eq!(buf[(R0 + 10, 12)].fg, TEXT);
        // A socket's row sits under its piece's name.
        let (x, _) = find(&buf, R0, (11, 37), "├ Arc hand").unwrap();
        assert_eq!(x, R0 + 11);
        for y in 11..37 {
            assert_eq!(buf[(75, y)].symbol(), "│", "y {y}");
            assert_eq!(buf[(76, y)].symbol(), " ", "the gap at {y}");
        }
        assert_eq!(cells(&buf, 77, 18, 10), "PISTON ARM");
        let elbow = Color::Rgb(0, 229, 255);
        let lit =
            (11..17).any(|y| (77..118).any(|x| buf[(x, y)].fg == elbow || buf[(x, y)].bg == elbow));
        assert!(lit, "the art right of the rule");
        let card = region(&buf, (77, 118), (11, 37));
        assert!(card.contains("L ARM · load 3 · EMP −2"), "{card}");
        // The header still spans the pane.
        assert_eq!(cells(&buf, R0, 10, 6), "CHROME");
        assert_eq!(
            cells(&buf, 115, 10, 3),
            format!("/{}", st.character.load_max())
        );
    }

    #[test]
    fn hints_follow_the_focus() {
        let mut st = AppState::new();
        // Free chrome: today's hints, the chair's have their own test.
        st.free_chrome = true;
        loadout(&mut st);
        give(&mut st, "grip_arm");
        let hint_row = |d: &Deck, st: &mut AppState| cells(&screen(d, st, (100, 30)), 2, 28, 90);
        let mut d = Deck::new(0);
        d.update(&mut st, &[key(KeyCode::Char('2'))]);
        let left =
            "←↑↓→ select · space allocates attribute · → over to the chrome · Q/E tabs · tab close";
        assert_eq!(d.sheet.hints(&st), left);
        assert!(hint_row(&d, &mut st).starts_with(left));
        d.sheet.state.pane = SheetPane::Ledger;
        d.sheet.state.row = LedgerRow::Site(Site::LeftArm);
        let chrome =
            "↑↓ select · space what fits · x pull it out · ← attributes · Q/E tabs · tab close";
        assert_eq!(d.sheet.hints(&st), chrome);
        assert!(hint_row(&d, &mut st).starts_with(chrome));
        d.sheet.state.row = LedgerRow::Socket(Site::LeftArm, 0);
        assert_eq!(d.sheet.hints(&st), chrome, "a socket's piece is chrome too");
        d.sheet.state.row = LedgerRow::Site(Site::RightArm);
        let empty = "↑↓ select · space what fits · ← attributes · Q/E tabs · tab close";
        assert_eq!(d.sheet.hints(&st), empty);
        assert!(hint_row(&d, &mut st).starts_with(empty));
        press(&mut d, &mut st, &[KeyCode::Char(' ')]);
        let picker = "↑↓ choose · space install · esc back · Q/E tabs · tab close";
        assert_eq!(d.sheet.hints(&st), picker);
        let row = hint_row(&d, &mut st);
        assert!(row.starts_with(picker), "{row}");
        let x = col_of(&screen(&d, &mut st, (100, 30)), 28, 2, "space install").unwrap();
        assert_eq!(screen(&d, &mut st, (100, 30))[(x, 28)].fg, DIM);
        // On a place that holds a piece, space swaps, and the hint says so.
        press(&mut d, &mut st, &[KeyCode::Esc]);
        d.sheet.state.row = LedgerRow::Site(Site::LeftArm);
        press(&mut d, &mut st, &[KeyCode::Char(' ')]);
        assert!(d.sheet.state.picker.is_some());
        let swap = "↑↓ choose · space swap · esc back · Q/E tabs · tab close";
        assert_eq!(d.sheet.hints(&st), swap);
        let row = hint_row(&d, &mut st);
        assert!(row.starts_with(swap), "{row}");
    }

    #[test]
    fn ledger_keys_skip_the_groups() {
        let mut st = AppState::new();
        loadout(&mut st);
        let mut d = on_ledger(&mut st, LedgerRow::Site(Site::Neural));
        let walk = |d: &mut Deck, st: &mut AppState, code: KeyCode| {
            st.tick += 5;
            press(d, st, &[code]);
            d.sheet.state.row
        };
        let s = LedgerRow::Site;
        assert_eq!(
            walk(&mut d, &mut st, KeyCode::Up),
            s(Site::Neural),
            "the first stops"
        );
        for want in [Site::Eyes, Site::Ears, Site::Jaw, Site::Spine] {
            assert_eq!(walk(&mut d, &mut st, KeyCode::Down), s(want));
        }
        assert_eq!(d.sheet.card, st.tick, "the card types out afresh");
        assert_eq!(
            walk(&mut d, &mut st, KeyCode::Char('k')),
            s(Site::Jaw),
            "over the body's header"
        );
        d.sheet.state.row = s(Site::Skin);
        for want in [
            s(Site::LeftArm),
            LedgerRow::Socket(Site::LeftArm, 0),
            LedgerRow::Socket(Site::LeftArm, 1),
            s(Site::RightArm),
            s(Site::Legs),
            s(Site::Legs),
        ] {
            assert_eq!(walk(&mut d, &mut st, KeyCode::Char('j')), want);
        }
        // Right does nothing; left goes back to the tile the left pane had.
        let tile = d.sheet.state.tile;
        walk(&mut d, &mut st, KeyCode::Right);
        assert_eq!(d.sheet.state.pane, SheetPane::Ledger);
        walk(&mut d, &mut st, KeyCode::Left);
        assert_eq!(
            (d.sheet.state.pane, d.sheet.state.tile),
            (SheetPane::Left, tile)
        );
        assert_eq!(d.sheet.state.row, s(Site::Legs), "the ledger keeps its row");
    }

    #[test]
    fn rows_scroll_with_the_cursor() {
        let mut st = AppState::new();
        loadout(&mut st);
        let mut s = Sheet::new(0);
        s.state.pane = SheetPane::Ledger;
        s.state.row = LedgerRow::Site(Site::Legs);
        st.tick = REVEAL_TICKS + 1;
        let body = Rect::new(1, 5, 98, 10);
        let mut buf = Buffer::empty(Rect::new(0, 0, 100, 30));
        s.draw(&st, body, &mut buf);
        let ledger = region(&buf, (R0, RULE_100), (body.y + 1, body.bottom()));
        assert!(ledger.contains("· legs"), "{ledger}");
        assert!(!ledger.contains("── head ──"), "{ledger}");
        let (_, y) = find(&buf, R0, (body.y + 1, body.bottom()), "· legs").unwrap();
        assert_eq!(buf[(R0, y)].symbol(), "▸");
    }
}
