//! A container's panel: what a locked one offers (jack in, the Keyjack, force it, a card)
//! and what an open one holds (take a line, take all). Drawn in the verb popup's chrome;
//! the look is the agreed mock `docs/superpowers/specs/2026-11-18-containers-mock/gen.py`.

use crate::app::{AppState, Screen, ScreenCmd, TransitionKind};
use crate::containers::state::{Loot, Opened};
use crate::containers::{self, Container, ContainerAt, Lock, Status, ways};
use crate::render::tileset::{
    CONTAINER_NET, container_body, container_detail, container_open_shape,
};
use crate::screens::lockjack::LockJack;
use crate::ui::chrome::{
    BAR_BG, CYAN, DIM, FAINT, FAINTER, MAGENTA, PANEL_BG, TEXT, frame, put, solid,
};
use crossterm::event::{KeyCode, KeyEvent};
use game::Attribute;
use ratatui::Frame;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};

/// The mark of a container on the Net.
const NET_MARK: Color = Color::Rgb(0, 230, 210);
const MIN_W: usize = 30;

/// One row of a panel.
#[derive(Clone, Debug, PartialEq)]
pub struct PanelRow {
    pub key: char,
    pub label: String,
    pub right: String,
    pub right_fg: Color,
    pub enabled: bool,
}

impl PanelRow {
    fn new(key: char, label: impl Into<String>, right: impl Into<String>, enabled: bool) -> Self {
        PanelRow {
            key,
            label: label.into(),
            right: right.into(),
            right_fg: DIM,
            enabled,
        }
    }
}

/// The rows of a locked container's panel.
fn locked_rows(st: &AppState, c: &Container, lock: &Lock) -> Vec<PanelRow> {
    let mut rows = Vec::new();
    if lock.node.is_some() {
        rows.push(PanelRow::new('j', "jack in", "", true));
    }
    let fused = matches!(
        crate::keyjack::carried(st),
        crate::keyjack::Carried::Fused(_)
    );
    let (_, out) = ways::keyjack_now(st, c.at);
    rows.push(if let Some(until) = out {
        let left = until.saturating_sub(st.net.now(st)).max(1);
        PanelRow::new('k', "Keyjack", format!("locked {left}m"), false)
    } else if fused {
        let n = crate::keyjack::fuses(st);
        let word = if n == 1 { "fuse" } else { "fuses" };
        PanelRow::new('k', "Keyjack", format!("{n} {word}"), true)
    } else {
        PanelRow::new('k', "Keyjack", "none", false)
    });
    let need = containers::force::need(lock.grade);
    let body = st.character.attribute(&st.content, Attribute::Body);
    rows.push(PanelRow::new(
        'f',
        "force it",
        format!("Body {need}"),
        body >= need,
    ));
    if let Some((_, zone)) = ways::card_zone(st, c) {
        rows.push(PanelRow::new('c', format!("card · {zone}"), "none", false));
    }
    rows
}

/// The letters of an open panel's lines: `a` on, `t` kept for take all.
fn line_keys() -> impl Iterator<Item = char> {
    ('a'..='z').filter(|c| *c != 't')
}

/// The rows of an open container's panel, one a line left in it.
fn open_rows(st: &AppState, c: &Container, lines: &[Loot]) -> Vec<PanelRow> {
    lines
        .iter()
        .zip(line_keys())
        .map(|(l, key)| {
            let kg = format!("{:.1} kg", containers::line_weight(st, l));
            match l {
                Loot::Cash(n) => PanelRow::new(key, "cash", containers::cr(*n), true),
                Loot::Chip { credits, locked } => {
                    let mut r = PanelRow::new(key, "credchip", containers::cr(*credits), true);
                    if *locked {
                        r.right = "LOCKED".to_string();
                        r.right_fg = MAGENTA;
                    }
                    r
                }
                Loot::Item { item, qty } => {
                    let name = st.content.item(item).map_or(item.0.as_str(), |i| &i.name);
                    let label = if *qty > 1 {
                        format!("{name} ×{qty}")
                    } else {
                        name.to_string()
                    };
                    PanelRow::new(key, label, kg, true)
                }
                Loot::Card(z) => {
                    let zone = ways::zone_name(st, c.at.building, *z).unwrap_or_default();
                    PanelRow::new(key, format!("Keycard · {zone}"), kg, true)
                }
            }
        })
        .collect()
}

fn pop() -> ScreenCmd {
    ScreenCmd::Pop {
        fx: TransitionKind::None,
    }
}

/// A container's panel on the screen stack.
#[derive(Clone, Debug)]
pub struct ContainerPanel {
    at: ContainerAt,
    started: bool,
    /// The row selected (an index into the rows shown).
    sel: usize,
}

impl ContainerPanel {
    pub fn open(_st: &AppState, at: ContainerAt) -> ContainerPanel {
        ContainerPanel {
            at,
            started: false,
            sel: 0,
        }
    }

    /// The selected row, moved to an enabled one when it isn't.
    fn selected(&self, rows: &[PanelRow]) -> usize {
        let sel = self.sel.min(rows.len().saturating_sub(1));
        if rows.get(sel).is_some_and(|r| r.enabled) {
            sel
        } else {
            rows.iter().position(|r| r.enabled).unwrap_or(0)
        }
    }

    fn step(&mut self, rows: &[PanelRow], by: isize) {
        let n = rows.len() as isize;
        let mut i = self.selected(rows) as isize;
        for _ in 0..n {
            i = (i + by).rem_euclid(n.max(1));
            if rows.get(i as usize).is_some_and(|r| r.enabled) {
                self.sel = i as usize;
                return;
            }
        }
    }

    /// What happens as it opens: an empty one says so, a free or card-held one opens.
    fn start(&mut self, st: &mut AppState, c: &Container) -> Option<ScreenCmd> {
        match containers::status(st, c) {
            Status::Empty => {
                st.say("empty");
                return Some(pop());
            }
            Status::Shut => containers::open(st, c, Opened::Free),
            Status::Locked(lock) => {
                if let Some(way) = ways::free_way(st, c, &lock) {
                    containers::open(st, c, way);
                }
            }
            Status::Open(_) => {}
        }
        if containers::status(st, c) == Status::Empty {
            st.say("empty");
            return Some(pop());
        }
        None
    }

    /// A locked panel's row fired: the Keyjack replaces the panel, jacking in pops it; forcing
    /// it asks the map to start the bar.
    fn fire_locked(&mut self, st: &mut AppState, c: &Container, key: char) -> Option<ScreenCmd> {
        match key {
            'k' if st.net.engaged.is_some() => st.warn("not now"),
            'k' => {
                return LockJack::open(st, self.at).map(|jack| ScreenCmd::Replace {
                    to: Box::new(jack),
                    fx: TransitionKind::None,
                });
            }
            'j' => match ways::jack_in(st, c) {
                Ok(said) => {
                    st.say(said);
                    return Some(ScreenCmd::Pop {
                        fx: TransitionKind::Glitch,
                    });
                }
                Err(why) => st.warn(why),
            },
            'f' => match containers::force::can(st, c) {
                Ok(_) => {
                    st.force_at = Some(self.at);
                    return Some(pop());
                }
                Err(why) => st.warn(why),
            },
            _ => {}
        }
        None
    }

    fn take(&mut self, st: &mut AppState, c: &Container, line: usize) {
        match containers::take(st, c, line) {
            Ok(said) => st.say(said),
            Err(why) => st.warn(why),
        }
    }

    /// A key on the locked panel: what it asks of the stack, if anything.
    fn keys_locked(
        &mut self,
        st: &mut AppState,
        c: &Container,
        lock: &Lock,
        k: KeyEvent,
    ) -> Option<ScreenCmd> {
        let rows = locked_rows(st, c, lock);
        match k.code {
            KeyCode::Esc => return Some(pop()),
            KeyCode::Up => self.step(&rows, -1),
            KeyCode::Down => self.step(&rows, 1),
            KeyCode::Enter | KeyCode::Char(' ') => {
                if let Some(r) = rows.get(self.selected(&rows)).filter(|r| r.enabled) {
                    return self.fire_locked(st, c, r.key);
                }
            }
            KeyCode::Char(ch) => {
                if let Some(r) = rows.iter().find(|r| r.key == ch && r.enabled) {
                    return self.fire_locked(st, c, r.key);
                }
            }
            _ => {}
        }
        None
    }

    /// A key on the open panel; true when it closes.
    fn keys_open(&mut self, st: &mut AppState, c: &Container, lines: &[Loot], k: KeyEvent) -> bool {
        let rows = open_rows(st, c, lines);
        match k.code {
            KeyCode::Esc => return true,
            KeyCode::Up => self.step(&rows, -1),
            KeyCode::Down => self.step(&rows, 1),
            KeyCode::Enter | KeyCode::Char(' ') => {
                let i = self.selected(&rows);
                self.take(st, c, i);
            }
            KeyCode::Char('t') => match containers::take_all(st, c) {
                Ok(said) => st.say(said),
                Err(why) => st.warn(why),
            },
            KeyCode::Char(ch) => {
                if let Some(i) = rows.iter().position(|r| r.key == ch) {
                    self.take(st, c, i);
                }
            }
            _ => {}
        }
        false
    }
}

impl Screen for ContainerPanel {
    fn update(&mut self, st: &mut AppState, keys: &[KeyEvent]) -> ScreenCmd {
        let Some(c) = containers::find(st, self.at) else {
            return pop();
        };
        if !self.started {
            self.started = true;
            if let Some(cmd) = self.start(st, &c) {
                return cmd;
            }
        }
        for k in keys {
            let cmd = match containers::status(st, &c) {
                Status::Locked(lock) => self.keys_locked(st, &c, &lock, *k),
                Status::Open(lines) => self.keys_open(st, &c, &lines, *k).then(pop),
                Status::Shut | Status::Empty => Some(pop()),
            };
            if let Some(cmd) = cmd {
                return cmd;
            }
        }
        // The last line taken closes it.
        match containers::status(st, &c) {
            Status::Empty | Status::Shut => pop(),
            _ => ScreenCmd::Stay,
        }
    }

    fn draw(&self, st: &AppState, f: &mut Frame) {
        let Some(c) = containers::find(st, self.at) else {
            return;
        };
        match containers::status(st, &c) {
            Status::Locked(lock) => {
                let rows = locked_rows(st, &c, &lock);
                self.draw_locked(f, &c, &lock, &rows);
            }
            Status::Open(lines) => {
                let rows = open_rows(st, &c, &lines);
                self.draw_open(st, f, &c, &rows);
            }
            Status::Shut | Status::Empty => {}
        }
    }
}

/// The panel's rect: `w` by `h`, centred.
fn centred(area: Rect, w: usize, h: u16) -> Rect {
    let w = (w as u16).min(area.width);
    let h = h.min(area.height);
    Rect::new(
        area.x + area.width.saturating_sub(w) / 2,
        area.y + area.height.saturating_sub(h) / 2,
        w,
        h,
    )
}

/// The width the rows ask: the longest `label` + `right`, and the other lines' lengths.
fn width_for(rows: &[PanelRow], other: &[usize]) -> usize {
    let longest = rows
        .iter()
        .map(|r| r.label.chars().count() + r.right.chars().count())
        .max()
        .unwrap_or(0);
    let others = other.iter().map(|n| n + 4).max().unwrap_or(0);
    MIN_W.max(longest + 8).max(others)
}

impl ContainerPanel {
    /// The header, ` {glyph} {TITLE} `, at the top rule's left: the glyph as the map
    /// draws it (its detail on its body), the title in cyan.
    fn header(buf: &mut Buffer, r: Rect, glyph: (char, Color), title: &str, c: &Container) {
        let max = r.right() - 1;
        let bold = Style::default().fg(CYAN).add_modifier(Modifier::BOLD);
        put(buf, r.x + 2, r.y, " ", bold, max);
        let g = Style::default()
            .fg(glyph.1)
            .bg(container_body(c.kind).to_color())
            .add_modifier(Modifier::BOLD);
        put(buf, r.x + 3, r.y, &glyph.0.to_string(), g, max);
        put(buf, r.x + 4, r.y, &format!(" {title} "), bold, max);
    }

    fn row(buf: &mut Buffer, r: Rect, y: u16, row: &PanelRow, sel: bool) {
        let bg = if sel { BAR_BG } else { PANEL_BG };
        let max = r.right() - 1;
        if sel {
            for x in r.x + 1..max {
                buf[(x, y)].set_bg(BAR_BG);
            }
        }
        let key_fg = if !row.enabled {
            FAINTER
        } else if sel {
            Color::White
        } else {
            CYAN
        };
        let key = Style::default()
            .fg(key_fg)
            .bg(bg)
            .add_modifier(Modifier::BOLD);
        put(buf, r.x + 2, y, &row.key.to_string(), key, max);
        let mut label = Style::default()
            .fg(if row.enabled { TEXT } else { FAINTER })
            .bg(bg);
        if sel {
            label = label.add_modifier(Modifier::BOLD);
        }
        put(buf, r.x + 5, y, &row.label, label, max);
        if !row.right.is_empty() {
            let fg = if row.enabled { row.right_fg } else { FAINTER };
            let x = r.x + r.width - 2 - row.right.chars().count() as u16;
            put(buf, x, y, &row.right, Style::default().fg(fg).bg(bg), max);
        }
    }

    fn draw_locked(&self, f: &mut Frame, c: &Container, lock: &Lock, rows: &[PanelRow]) {
        let line = match &lock.maker {
            Some(m) => format!("{m} lock · grade {}", lock.grade),
            None => format!("lock · grade {}", lock.grade),
        };
        let hint = "esc leave it";
        let title = c.kind.word().to_uppercase();
        let w = width_for(rows, &[line.chars().count(), hint.len(), title.len() + 12]);
        let r = centred(f.area(), w, rows.len() as u16 + 7);
        if r.width < 6 || r.height < 4 {
            return;
        }
        let buf = f.buffer_mut();
        solid(buf, r);
        frame(buf, r);
        let (glyph, fg) = container_detail(c.kind);
        let fg = if lock.node.is_some() {
            CONTAINER_NET
        } else {
            fg
        };
        Self::header(buf, r, (glyph, fg.to_color()), &title, c);
        if lock.node.is_some() {
            let tag = Style::default().fg(NET_MARK).add_modifier(Modifier::BOLD);
            put(buf, r.x + r.width - 8, r.y, " ● NET ", tag, r.right());
        }
        let max = r.right() - 1;
        put(buf, r.x + 2, r.y + 2, &line, Style::default().fg(DIM), max);
        let sel = self.selected(rows);
        for (i, row) in rows.iter().enumerate() {
            Self::row(buf, r, r.y + 4 + i as u16, row, i == sel);
        }
        let y = r.y + 4 + rows.len() as u16 + 1;
        put(buf, r.x + 2, y, hint, Style::default().fg(FAINT), max);
    }

    fn draw_open(&self, st: &AppState, f: &mut Frame, c: &Container, rows: &[PanelRow]) {
        let title = format!("{} · OPEN", c.kind.word().to_uppercase());
        let pack = format!(
            "pack {:.1}/{:.0}",
            st.character.weight(&st.content),
            st.character.carry_cap(&st.content)
        );
        let hint = "space take · esc close";
        let all = "t take all";
        let w = width_for(
            rows,
            &[hint.len(), title.len() + 6, all.len() + 2 + pack.len()],
        );
        let r = centred(f.area(), w, rows.len() as u16 + 7);
        if r.width < 6 || r.height < 4 {
            return;
        }
        let buf = f.buffer_mut();
        solid(buf, r);
        frame(buf, r);
        let (_, fg) = container_detail(c.kind);
        let open = container_open_shape(c.kind);
        Self::header(buf, r, (open, fg.to_color()), &title, c);
        let max = r.right() - 1;
        let sel = self.selected(rows);
        for (i, row) in rows.iter().enumerate() {
            Self::row(buf, r, r.y + 2 + i as u16, row, i == sel);
        }
        let y = r.y + 2 + rows.len() as u16 + 1;
        put(buf, r.x + 2, y, all, Style::default().fg(CYAN), max);
        let x = r.x + r.width - 1 - pack.chars().count() as u16;
        put(buf, x, y, &pack, Style::default().fg(DIM), max);
        put(buf, r.x + 2, y + 2, hint, Style::default().fg(FAINT), max);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::containers::testutil::*;
    use crate::containers::{state_mut, status};
    use crate::screens::testutil::{buffer_text, key};
    use citygen::tile::ContainerKind;
    use citygen::{Feature, Furniture};
    use crossterm::event::KeyCode;
    use game::ItemId;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use ratatui::buffer::Buffer;

    fn draw(st: &AppState, panel: &ContainerPanel) -> Buffer {
        let mut t = Terminal::new(TestBackend::new(100, 30)).unwrap();
        t.draw(|f| panel.draw(st, f)).unwrap();
        t.backend().buffer().clone()
    }

    fn popped(cmd: &ScreenCmd) -> bool {
        matches!(cmd, ScreenCmd::Pop { .. })
    }

    /// The container re-kinded on its floor.
    fn as_kind(st: &mut AppState, c: &Container, kind: ContainerKind) -> Container {
        let (b, level, at) = (c.at.building, c.at.level, c.at.at);
        let mut floor = st.interiors.floor(b, level).unwrap().1.clone();
        floor.tiles.get_mut(at).unwrap().feature = Some(Feature::Furniture(kind.furniture()));
        st.interiors.put_floor(b, floor);
        crate::containers::find(st, c.at).expect("still a container")
    }

    /// Which row of the drawing holds `text`.
    fn row_of(buf: &Buffer, text: &str) -> u16 {
        let s = buffer_text(buf);
        let y = s.lines().position(|l| l.contains(text)).expect(text);
        y as u16
    }

    fn open_with(st: &mut AppState, c: &Container, left: Vec<Loot>) {
        let s = state_mut(st, c.at);
        s.opened = Some(Opened::Free);
        s.left = Some(left);
    }

    #[test]
    fn a_locked_safe_on_the_net_shows_the_mock() {
        let (mut st, c) = copy_with_spot(150);
        let c = as_kind(&mut st, &c, ContainerKind::Safe);
        beside(&mut st, &c);
        let mut p = ContainerPanel::open(&st, c.at);
        assert!(!popped(&p.update(&mut st, &[])));
        let buf = draw(&st, &p);
        let text = buffer_text(&buf);
        for want in [
            "◉ SAFE",
            "● NET",
            "lock · grade",
            "jack in",
            "Keyjack",
            "force it",
            "Body ",
            "card · ",
            "esc leave it",
        ] {
            assert!(text.contains(want), "{want} missing:\n{text}");
        }
        let y = row_of(&buf, "jack in");
        let x = text
            .lines()
            .nth(y as usize)
            .unwrap()
            .find("jack in")
            .unwrap();
        assert_eq!(buf[(x as u16 - 2, y)].bg, BAR_BG, "jack in is selected");
    }

    #[test]
    fn the_j_row_jacks_in_and_pops_with_a_glitch_or_warns_and_stays() {
        let (mut st, c) = copy_with_spot(150);
        let c = as_kind(&mut st, &c, ContainerKind::Safe);
        beside(&mut st, &c);
        let mut p = ContainerPanel::open(&st, c.at);
        let cmd = p.update(&mut st, &[key(KeyCode::Char('j'))]);
        assert!(matches!(
            cmd,
            ScreenCmd::Pop {
                fx: TransitionKind::Glitch
            }
        ));
        assert!(st.player.as_ref().unwrap().jacked_in());
        // Alarmed, it stays.
        let (mut st, c) = copy_with_spot(150);
        let c = as_kind(&mut st, &c, ContainerKind::Safe);
        beside(&mut st, &c);
        crate::net::lock_down(&mut st, c.at.building);
        let mut p = ContainerPanel::open(&st, c.at);
        assert!(!popped(&p.update(&mut st, &[key(KeyCode::Char('j'))])));
        assert!(!st.player.as_ref().unwrap().jacked_in());
    }

    #[test]
    fn off_the_net_there_is_no_mark_and_no_jack_in_row() {
        // A locker the seed locks, in a plain site: off the Net.
        let mut found = None;
        for seed in 0..12 {
            let (mut st, b) = plain_site(citygen::BuildingKind::Hotel, seed);
            let lockers = all_in(&mut st, b);
            if let Some(c) = lockers.into_iter().find(|c| {
                c.kind == ContainerKind::Locker
                    && matches!(crate::containers::status(&st, c), Status::Locked(_))
            }) {
                found = Some((st, c));
                break;
            }
        }
        let (mut st, c) = found.expect("a locked locker");
        beside(&mut st, &c);
        let mut p = ContainerPanel::open(&st, c.at);
        assert!(!popped(&p.update(&mut st, &[])));
        let buf = draw(&st, &p);
        let text = buffer_text(&buf);
        assert!(
            !text.contains("● NET") && !text.contains("jack in"),
            "{text}"
        );
        assert!(text.contains("lock · grade 1"), "{text}");
        let fused = crate::keyjack::fuses(&st) > 0;
        let want = if fused { "Keyjack" } else { "force it" };
        let y = row_of(&buf, want);
        let x = buffer_text(&buf)
            .lines()
            .nth(y as usize)
            .unwrap()
            .find(want)
            .unwrap();
        assert_eq!(buf[(x as u16 - 2, y)].bg, BAR_BG, "{want} is selected");
    }

    #[test]
    fn the_card_opens_it_with_no_locked_panel() {
        let (mut st, c) = copy_with_spot(150);
        let c = as_kind(&mut st, &c, ContainerKind::Safe);
        beside(&mut st, &c);
        let b = c.at.building;
        crate::credentials::stock_zone_cards(&mut st, b);
        let need = c.zone.unwrap_or(0).max(1);
        st.character
            .give_item(&st.content, crate::credentials::zone_card_id(b, need), 1);
        let mut p = ContainerPanel::open(&st, c.at);
        let cmd = p.update(&mut st, &[]);
        assert_eq!(
            crate::containers::state(&st, c.at).and_then(|s| s.opened),
            Some(Opened::Card)
        );
        if !popped(&cmd) {
            assert!(matches!(status(&st, &c), Status::Open(_)));
            assert!(buffer_text(&draw(&st, &p)).contains(" · OPEN"));
        } else {
            assert_eq!(status(&st, &c), Status::Empty);
        }
    }

    #[test]
    fn a_live_crack_opens_it_with_no_prompt() {
        let (mut st, c) = copy_with_spot(150);
        beside(&mut st, &c);
        let node = crate::containers::lock(&st, &c)
            .unwrap()
            .node
            .expect("a node");
        crate::sitenet::testutil::hack(
            &mut st,
            node,
            citygen::NodeKind::Container,
            game::Op::Crack,
        );
        let mut p = ContainerPanel::open(&st, c.at);
        p.update(&mut st, &[]);
        assert_eq!(
            crate::containers::state(&st, c.at).and_then(|s| s.opened),
            Some(Opened::Net)
        );
    }

    #[test]
    fn letters_take_lines_and_t_takes_all() {
        let (mut st, c) = plain_site_with(ContainerKind::Crate);
        beside(&mut st, &c);
        st.character.dev = true;
        let left = vec![
            Loot::Cash(1250),
            Loot::Chip {
                credits: 400,
                locked: false,
            },
            Loot::Chip {
                credits: 900,
                locked: true,
            },
            Loot::Item {
                item: ItemId::new("frag"),
                qty: 2,
            },
        ];
        open_with(&mut st, &c, left);
        let mut p = ContainerPanel::open(&st, c.at);
        assert!(!popped(&p.update(&mut st, &[])));
        let text = buffer_text(&draw(&st, &p));
        for want in [
            "1,250 cr",
            "400 cr",
            "LOCKED",
            "Frag ×2",
            "t take all",
            "pack ",
        ] {
            assert!(text.contains(want), "{want} missing:\n{text}");
        }
        let credits = st.character.credits;
        assert!(!popped(&p.update(&mut st, &[key(KeyCode::Char('a'))])));
        assert_eq!(st.character.credits, credits + 1250);
        assert_eq!(crate::containers::contents(&st, &c).len(), 3);
        assert!(popped(&p.update(&mut st, &[key(KeyCode::Char('t'))])));
        assert!(crate::containers::contents(&st, &c).is_empty());
    }

    #[test]
    fn an_empty_one_says_so() {
        let (mut st, c) = plain_site_with(ContainerKind::Crate);
        beside(&mut st, &c);
        open_with(&mut st, &c, Vec::new());
        let mut p = ContainerPanel::open(&st, c.at);
        assert!(popped(&p.update(&mut st, &[])));
        assert_eq!(st.log.latest().unwrap().text, "empty");
    }

    #[test]
    fn a_crate_opens_at_once() {
        let (mut st, c) = plain_site_with(ContainerKind::Crate);
        beside(&mut st, &c);
        let mut p = ContainerPanel::open(&st, c.at);
        let cmd = p.update(&mut st, &[]);
        assert_eq!(
            crate::containers::state(&st, c.at).and_then(|s| s.opened),
            Some(Opened::Free)
        );
        assert!(!popped(&cmd), "a crate holds something");
        assert!(buffer_text(&draw(&st, &p)).contains("CRATE · OPEN"));
        let _ = Furniture::Crate;
    }
}
