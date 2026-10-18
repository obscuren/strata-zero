//! A popup of verbs over whatever opened it: hotkey letters, a selected row, a card view.

use crate::actions::{perform, report};
use crate::anim::effects::{pulse, typewriter};
use crate::app::{AppState, Screen, ScreenCmd, TransitionKind};
use crate::ui::chrome::{
    BAR_BG, CYAN, CYAN_RGB, DIM, MAGENTA, PANEL_BG, TEXT, frame, put, put_paragraph, reveal, solid,
};
use crate::ui::sidebar::Section;
use crate::walker::jacked_in_line;
use crossterm::event::{KeyCode, KeyEvent};
use game::Action;
use ratatui::Frame;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Line;

/// Something the popup does to the player in the world.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WorldAct {
    JackIn,
    JackOut,
    Up,
    Down,
    /// A lift ride to a floor of the building the player is in.
    Lift(i32),
    /// The stairwell, one floor up or down.
    Climb(bool),
}

/// What a row does when fired.
#[derive(Clone, Debug)]
pub enum Pick {
    /// Runs through `actions::perform`; the result lands in the status strip.
    Act(Action),
    /// Moves the player: jacks or stairs. Jacks pop with a glitch.
    World(WorldAct),
    /// Shows these lines in place of the rows; Esc returns to the rows.
    Card(Vec<Line<'static>>),
    /// Toggles a sidebar section and closes.
    View(Section),
    /// Starts a program against a target in the construct; the result lands in the strip.
    Run(game::ItemId, game::HackTarget),
    /// Opens a flat's keypad in place of the popup.
    Keypad {
        building: citygen::BuildingId,
        level: i32,
        number: u16,
    },
    /// Opens the access terminal in place of the popup, keyed by the machine's tile.
    Terminal {
        origin: crate::helix::shell::HostId,
        key: (citygen::Place, citygen::Pos),
    },
    Close,
}

/// What the top border names: the window's one subject, or the list it holds.
#[derive(Clone, Debug)]
pub struct Header {
    /// The glyph the map draws for the subject, in its colour.
    pub glyph: Option<(char, Color)>,
    pub name: String,
    pub fg: Color,
}

impl Header {
    /// A plain title: a menu named by what it lists.
    pub fn title(name: &str) -> Header {
        Header {
            glyph: None,
            name: name.to_string(),
            fg: CYAN,
        }
    }

    /// Columns the header takes on the border, its padding included.
    fn width(&self) -> usize {
        self.name.chars().count() + 2 + if self.glyph.is_some() { 2 } else { 0 }
    }
}

#[derive(Clone, Debug)]
pub struct Choice {
    pub key: char,
    /// The verb, capitalised.
    pub label: String,
    /// What the verb acts on, drawn after ": " in its kind's colour.
    pub thing: Option<(String, Color)>,
    /// Plain text after the thing: odds, a lock's time left.
    pub tail: String,
    /// A second key that fires the row.
    pub alias: Option<char>,
    pub pick: Pick,
    pub enabled: bool,
}

impl Choice {
    pub fn new(key: char, label: impl Into<String>, pick: Pick) -> Self {
        Choice {
            key,
            label: label.into(),
            thing: None,
            tail: String::new(),
            alias: None,
            pick,
            enabled: true,
        }
    }

    /// `Verb: Thing`, the thing in its kind's colour.
    pub fn on(
        key: char,
        verb: impl Into<String>,
        thing: impl Into<String>,
        colour: Color,
        pick: Pick,
    ) -> Self {
        Choice {
            thing: Some((thing.into(), colour)),
            ..Choice::new(key, verb, pick)
        }
    }

    pub fn tail(mut self, tail: impl Into<String>) -> Self {
        self.tail = tail.into();
        self
    }

    pub fn alias(mut self, key: char) -> Self {
        self.alias = Some(key);
        self
    }

    /// A greyed row that cannot be fired; shown so the player knows it exists.
    pub fn disabled(key: char, label: impl Into<String>) -> Self {
        Choice {
            enabled: false,
            ..Choice::new(key, label, Pick::Close)
        }
    }

    /// The row's whole text after the key: verb, thing, tail.
    fn text(&self) -> String {
        match &self.thing {
            Some((t, _)) => format!("{}: {t}{}", self.label, self.tail),
            None => format!("{}{}", self.label, self.tail),
        }
    }
}

const REVEAL_TICKS: u64 = 10;

/// Rows a line takes at `width` under the paragraph's word wrap: greedy, words
/// joined by single spaces, a word longer than a row spilling into the next.
fn wrapped_rows(line: &Line<'_>, width: usize) -> usize {
    let width = width.max(1);
    let text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
    let mut rows = 1;
    let mut used = 0usize;
    for word in text.split(' ') {
        let w = word.chars().count();
        if w == 0 {
            continue;
        }
        if used == 0 {
            used = w;
        } else if used + 1 + w <= width {
            used += 1 + w;
        } else {
            rows += 1;
            used = w;
        }
        while used > width {
            rows += 1;
            used -= width;
        }
    }
    rows
}
pub(crate) const MIN_W: u16 = 40;
const MAX_W: u16 = 60;

pub struct Popup {
    /// None: the top border is blank.
    header: Option<Header>,
    /// A dim line under the header, before the rows.
    subtitle: Option<String>,
    choices: Vec<Choice>,
    pub selected: usize,
    anchor: Option<(u16, u16)>,
    /// Tick the rows (or the card) appeared; drives the reveal.
    opened: u64,
    /// When set, the card's lines replace the rows; closing it closes the popup.
    card: Option<Vec<Line<'static>>>,
}

impl Popup {
    /// A list under a plain title; an empty title leaves the border blank.
    pub fn verbs(title: &str, choices: Vec<Choice>, tick: u64) -> Self {
        let header = (!title.is_empty()).then(|| Header::title(title));
        Popup::about(header, None, choices, tick)
    }

    /// A window about one subject: its header, a dim subtitle, its rows.
    pub fn about(
        header: Option<Header>,
        subtitle: Option<String>,
        choices: Vec<Choice>,
        tick: u64,
    ) -> Self {
        let selected = choices.iter().position(|c| c.enabled).unwrap_or(0);
        Popup {
            header,
            subtitle,
            choices,
            selected,
            anchor: None,
            opened: tick,
            card: None,
        }
    }

    /// Rows above the choices: the subtitle and the blank row under it.
    fn lead(&self) -> u16 {
        if self.subtitle.is_some() { 2 } else { 0 }
    }

    /// Applies a world act to the player and returns the transition to pop with.
    fn world(&self, st: &mut AppState, act: WorldAct) -> TransitionKind {
        if let WorldAct::Climb(up) = act {
            match crate::interiors::climb(st, up) {
                Ok(msg) => st.say(msg),
                Err(e) => st.warn(e),
            }
            return TransitionKind::None;
        }
        if let WorldAct::Lift(level) = act {
            let building = match st.player.as_ref().map(|p| p.place) {
                Some(citygen::Place::Floor { building, .. }) => Some(building),
                _ => None,
            };
            let result = match building {
                Some(b) => crate::interiors::ride(st, b, level),
                None => Err("no lift here".to_string()),
            };
            match result {
                Ok(msg) => st.say(msg),
                Err(e) => st.warn(e),
            }
            return TransitionKind::None;
        }
        if act == WorldAct::JackIn
            && let Some(why) = crate::net::jack_in_refusal(st)
        {
            st.warn(why);
            return TransitionKind::None;
        }
        if act == WorldAct::JackOut {
            return match crate::net::jack_out(st) {
                Ok(()) => {
                    st.say("jacked out");
                    TransitionKind::Glitch
                }
                Err(e) => {
                    st.warn(e);
                    TransitionKind::None
                }
            };
        }
        let bonus = st.sight_bonus();
        let home = st.home;
        let result: Result<(String, TransitionKind), String> =
            match (st.city.as_ref(), st.player.as_mut()) {
                (Some(city), Some(player)) => match act {
                    WorldAct::JackIn => {
                        let from = player.place;
                        player
                            .jack_in(city, home.as_ref(), bonus)
                            .map(|port| (jacked_in_line(city, from, port), TransitionKind::Glitch))
                            .map_err(|e| e.to_string())
                    }
                    // Taken above, with the floor seen again.
                    WorldAct::JackOut => Err("not jacked in".to_string()),
                    WorldAct::Lift(_) => Err("no lift here".to_string()),
                    WorldAct::Climb(_) => Err("no stairs here".to_string()),
                    WorldAct::Up | WorldAct::Down => {
                        let up = act == WorldAct::Up;
                        match player.use_connector(city, up) {
                            Some(dest) => {
                                player.update_fov(
                                    &crate::render::map::MapRef::stratum_of(city, dest),
                                    bonus,
                                );
                                Ok((format!("now on `{}`", dest.label()), TransitionKind::None))
                            }
                            None => Err(if up {
                                "nothing leads up from here".to_string()
                            } else {
                                "nothing leads down from here".to_string()
                            }),
                        }
                    }
                },
                _ => Err("no body in the city".to_string()),
            };
        match result {
            Ok((msg, fx)) => {
                st.say(msg);
                fx
            }
            Err(e) => {
                st.warn(e);
                TransitionKind::None
            }
        }
    }

    /// Pins the top-left corner near `(x, y)`; clamped so the whole panel stays on screen.
    pub fn anchored(mut self, x: u16, y: u16) -> Self {
        self.anchor = Some((x, y));
        self
    }

    /// Wraps at both ends, landing on the next enabled row; none enabled, no move.
    fn move_sel(&mut self, delta: i32) {
        let n = self.choices.len() as i32;
        let mut i = self.selected as i32;
        for _ in 0..n {
            i = (i + delta).rem_euclid(n.max(1));
            if self.choices[i as usize].enabled {
                self.selected = i as usize;
                return;
            }
        }
    }

    /// Where the panel goes inside `area`.
    pub(crate) fn rect(&self, area: Rect) -> Rect {
        let title_w = self.header.as_ref().map_or(0, |h| h.width());
        let (w, h) = match &self.card {
            Some(lines) => {
                let longest = lines.iter().map(|l| l.width()).max().unwrap_or(0);
                let w = (longest.max(title_w) as u16 + 6).clamp(MIN_W, MAX_W);
                let inner_w = w.saturating_sub(4).max(1) as usize;
                let rows: usize = lines.iter().map(|l| wrapped_rows(l, inner_w)).sum();
                (w, rows as u16 + 4)
            }
            None => {
                let longest = self
                    .choices
                    .iter()
                    .map(|c| c.text().chars().count() + 5)
                    .chain(self.subtitle.iter().map(|s| s.chars().count()))
                    .max()
                    .unwrap_or(0);
                let w = (longest.max(title_w) as u16 + 6).clamp(MIN_W, MAX_W);
                (w, self.choices.len() as u16 + 4 + self.lead())
            }
        };
        let w = w.min(area.width);
        let h = h.min(area.height);
        let (x, y) = match self.anchor {
            Some((ax, ay)) => (
                ax.clamp(area.x, area.right().saturating_sub(w).max(area.x)),
                ay.clamp(area.y, area.bottom().saturating_sub(h).max(area.y)),
            ),
            None => (
                area.x + area.width.saturating_sub(w) / 2,
                area.y + area.height.saturating_sub(h) / 2,
            ),
        };
        Rect::new(x, y, w, h)
    }

    fn fire(&mut self, st: &mut AppState, i: usize) -> ScreenCmd {
        let Some(choice) = self.choices.get(i) else {
            return ScreenCmd::Stay;
        };
        if !choice.enabled {
            return ScreenCmd::Stay;
        }
        match choice.pick.clone() {
            Pick::Act(action) => {
                let result = perform(st, action);
                report(st, result);
                ScreenCmd::Pop {
                    fx: TransitionKind::None,
                }
            }
            Pick::World(act) => {
                let fx = self.world(st, act);
                ScreenCmd::Pop { fx }
            }
            Pick::Run(program, target) => {
                let result = crate::net::start_run(st, program, target);
                report(st, result);
                ScreenCmd::Pop {
                    fx: TransitionKind::None,
                }
            }
            Pick::Card(lines) => {
                self.card = Some(lines);
                self.opened = st.tick;
                ScreenCmd::Stay
            }
            Pick::View(section) => {
                st.sidebar.toggle(section);
                ScreenCmd::Pop {
                    fx: TransitionKind::None,
                }
            }
            Pick::Keypad {
                building,
                level,
                number,
            } => ScreenCmd::Replace {
                to: Box::new(crate::screens::keypad::Keypad::open(
                    st, building, level, number,
                )),
                fx: TransitionKind::None,
            },
            Pick::Terminal { origin, key } => ScreenCmd::Replace {
                to: Box::new(crate::screens::terminal::Terminal::open(st, origin, key)),
                fx: TransitionKind::None,
            },
            Pick::Close => ScreenCmd::Pop {
                fx: TransitionKind::None,
            },
        }
    }

    fn draw_rows(&self, st: &AppState, inner: Rect, bar: Rect, buf: &mut Buffer) {
        let p = st.tick.saturating_sub(self.opened) as f32 / REVEAL_TICKS as f32;
        let n = self.choices.len().max(1) as f32;
        let glow = 0.7 + 0.3 * pulse(st.tick, 30);
        for (i, c) in self.choices.iter().enumerate() {
            let ry = inner.y + i as u16;
            if ry >= inner.bottom() {
                break;
            }
            let selected = i == self.selected && c.enabled;
            let bg = if selected { BAR_BG } else { PANEL_BG };
            if selected {
                for cx in bar.x..bar.right() {
                    buf[(cx, ry)].set_bg(BAR_BG);
                }
            }
            let label_style = if selected {
                Style::default()
                    .fg(CYAN_RGB.scale(glow).to_color())
                    .add_modifier(Modifier::BOLD)
            } else if c.enabled {
                Style::default().fg(TEXT)
            } else {
                Style::default().fg(DIM)
            };
            let key_style = if c.enabled {
                Style::default().fg(MAGENTA).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(DIM)
            };
            let frac = (p * n - i as f32).clamp(0.0, 1.0);
            let key = if c.enabled {
                c.key.to_string()
            } else {
                " ".to_string()
            };
            put(
                buf,
                inner.x,
                ry,
                if selected { "▸ " } else { "  " },
                label_style.bg(bg),
                inner.right(),
            );
            put(buf, inner.x + 2, ry, &key, key_style.bg(bg), inner.right());
            put(buf, inner.x + 3, ry, "  ", label_style.bg(bg), inner.right());
            // The revealed text in three runs: verb and colon, the thing in its
            // colour, the tail.
            let text = c.text();
            let shown: Vec<char> = typewriter(&text, frac).chars().collect();
            let verb = c.label.chars().count() + if c.thing.is_some() { 2 } else { 0 };
            let thing = c.thing.as_ref().map_or(0, |(t, _)| t.chars().count());
            let thing_style = match &c.thing {
                Some((_, colour)) if c.enabled => Style::default()
                    .fg(*colour)
                    .add_modifier(Modifier::BOLD),
                _ => label_style,
            };
            for (j, ch) in shown.iter().enumerate() {
                let x = inner.x + 5 + j as u16;
                if x >= inner.right() {
                    break;
                }
                let style = if (verb..verb + thing).contains(&j) {
                    thing_style
                } else {
                    label_style
                };
                buf[(x, ry)].set_char(*ch).set_style(style.bg(bg));
            }
        }
    }
}

impl Screen for Popup {
    fn update(&mut self, st: &mut AppState, keys: &[KeyEvent]) -> ScreenCmd {
        for k in keys {
            // A card closes the whole popup: back to whatever lies under it.
            if self.card.is_some() {
                if matches!(
                    k.code,
                    KeyCode::Esc | KeyCode::Char('q') | KeyCode::Enter | KeyCode::Char(' ')
                ) {
                    return ScreenCmd::Pop {
                        fx: TransitionKind::None,
                    };
                }
                continue;
            }
            // A row's letter wins over j/k navigation; the arrows always navigate.
            if let KeyCode::Char(c) = k.code
                && let Some(i) = self
                    .choices
                    .iter()
                    .position(|ch| ch.enabled && (ch.key == c || ch.alias == Some(c)))
            {
                return self.fire(st, i);
            }
            match k.code {
                KeyCode::Up | KeyCode::Char('k') => self.move_sel(-1),
                KeyCode::Down | KeyCode::Char('j') => self.move_sel(1),
                KeyCode::Esc | KeyCode::Char('q') => {
                    return ScreenCmd::Pop {
                        fx: TransitionKind::None,
                    };
                }
                KeyCode::Enter | KeyCode::Char(' ') => return self.fire(st, self.selected),
                _ => {}
            }
        }
        ScreenCmd::Stay
    }

    fn draw(&self, st: &AppState, f: &mut Frame) {
        let r = self.rect(f.area());
        if r.width < 6 || r.height < 4 {
            return;
        }
        let buf = f.buffer_mut();
        solid(buf, r);
        frame(buf, r);
        if let Some(h) = &self.header {
            let max = r.right() - 1;
            let name_style = Style::default().fg(h.fg).add_modifier(Modifier::BOLD);
            put(buf, r.x + 2, r.y, " ", name_style, max);
            let mut x = r.x + 3;
            if let Some((g, colour)) = h.glyph {
                put(
                    buf,
                    x,
                    r.y,
                    &format!("{g} "),
                    Style::default().fg(colour).add_modifier(Modifier::BOLD),
                    max,
                );
                x += 2;
            }
            put(buf, x, r.y, &format!("{} ", h.name), name_style, max);
        }
        let inner = Rect::new(r.x + 2, r.y + 2, r.width - 4, r.height - 4);
        match &self.card {
            Some(lines) => {
                let p = st.tick.saturating_sub(self.opened) as f32 / REVEAL_TICKS as f32;
                let shown = reveal(lines, p, (st.tick / 4).is_multiple_of(2));
                put_paragraph(buf, inner, shown);
            }
            None => {
                if let Some(s) = &self.subtitle {
                    put(
                        buf,
                        inner.x,
                        inner.y,
                        s,
                        Style::default().fg(DIM),
                        inner.right(),
                    );
                }
                let lead = self.lead().min(inner.height);
                let rows = Rect::new(inner.x, inner.y + lead, inner.width, inner.height - lead);
                let bar = Rect::new(r.x + 1, rows.y, r.width - 2, rows.height);
                self.draw_rows(st, rows, bar, buf);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::messages::Kind;

    fn last(st: &AppState) -> Option<&str> {
        st.log.latest().map(|e| e.text.as_str())
    }

    fn last_is_warning(st: &AppState) -> bool {
        st.log.latest().is_some_and(|e| e.kind == Kind::Warn)
    }
    use crate::screens::testutil::{buffer_text, key};
    use citygen::{GenParams, Generator, SizePreset};
    use game::ItemId;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    fn state() -> AppState {
        let mut st = AppState::new();
        let city = Generator::run(GenParams {
            seed: 6,
            size: SizePreset::Custom { w: 120, h: 60 },
            districts: 5,
            ..Default::default()
        })
        .unwrap();
        st.player = Some(crate::walker::Player::spawn(&city, 1));
        st.city = Some(city);
        st
    }

    fn draw(p: &Popup, st: &AppState, w: u16, h: u16) -> String {
        let mut t = Terminal::new(TestBackend::new(w, h)).unwrap();
        t.draw(|f| p.draw(st, f)).unwrap();
        buffer_text(t.backend().buffer())
    }

    #[test]
    fn popup_lists_verbs_and_fires_by_letter() {
        let mut st = state();
        st.character.hp = 20;
        let mut p = Popup::verbs(
            "STIM PATCH",
            vec![
                Choice::new(
                    'u',
                    "use",
                    Pick::Act(Action::Use(ItemId::new("stim_patch"))),
                ),
                Choice::disabled('x', "nothing"),
                Choice::new(
                    'd',
                    "drop one",
                    Pick::Act(Action::Drop {
                        item: ItemId::new("stim_patch"),
                        qty: 1,
                    }),
                ),
            ],
            0,
        );
        st.tick = 40;
        let text = draw(&p, &st, 120, 35);
        assert!(
            text.contains("STIM PATCH") && text.contains("u  use") && text.contains("d  drop one"),
            "{text}"
        );
        p.update(&mut st, &[key(KeyCode::Down)]);
        assert_eq!(p.selected, 2, "skips the disabled row");
        p.update(&mut st, &[key(KeyCode::Up)]);
        assert_eq!(p.selected, 0);
        assert!(matches!(
            p.update(&mut st, &[key(KeyCode::Char('u'))]),
            ScreenCmd::Pop { .. }
        ));
        assert_eq!(st.character.hp, 35);
        assert_eq!(last(&st), Some("used Stim patch · +15 hp"));
        assert!(!last_is_warning(&st));
    }

    #[test]
    fn popup_refusal_warns_and_cards_open_and_close() {
        let mut st = state();
        let mut p = Popup::verbs(
            "COIL",
            vec![Choice::new(
                'e',
                "equip",
                Pick::Act(Action::Equip(ItemId::new("copper_coil"))),
            )],
            0,
        );
        assert!(matches!(
            p.update(&mut st, &[key(KeyCode::Enter)]),
            ScreenCmd::Pop { .. }
        ));
        assert!(last_is_warning(&st));
        let mut p = Popup::verbs(
            "COIL",
            vec![Choice::new(
                'i',
                "inspect",
                Pick::Card(vec![Line::from("a spool")]),
            )],
            0,
        );
        assert!(matches!(
            p.update(&mut st, &[key(KeyCode::Char('i'))]),
            ScreenCmd::Stay
        ));
        st.tick = 40;
        assert!(draw(&p, &st, 120, 35).contains("a spool"));
        assert!(
            matches!(
                p.update(&mut st, &[key(KeyCode::Esc)]),
                ScreenCmd::Pop { .. }
            ),
            "esc on the card closes the popup"
        );
    }

    #[test]
    fn popup_clamps_inside_the_minimum_terminal() {
        let mut st = state();
        let p = Popup::verbs(
            "EDGE",
            vec![Choice::new(
                'a',
                "a very long label that pushes the width",
                Pick::Close,
            )],
            0,
        )
        .anchored(95, 28);
        let r = p.rect(Rect::new(0, 0, 100, 30));
        assert!(
            r.right() <= 100 && r.bottom() <= 30 && r.width >= 24,
            "{r:?}"
        );
        st.tick = 40;
        let text = draw(&p, &st, 100, 30);
        assert!(
            text.contains("EDGE") && text.contains("a very long"),
            "{text}"
        );
        for c in ['┍', '┑', '┕', '┙', '▸', '▮'] {
            assert_eq!(unicode_width::UnicodeWidthChar::width(c), Some(1));
        }
    }

    #[test]
    fn card_fits_word_wrapped_lines() {
        let mut st = state();
        let line = format!("{} {} {}", "x".repeat(23), "y".repeat(23), "z".repeat(23));
        let mut p = Popup::verbs(
            "WRAP",
            vec![Choice::new('l', "look", Pick::Card(vec![Line::from(line)]))],
            0,
        );
        p.update(&mut st, &[key(KeyCode::Char('l'))]);
        st.tick = 40;
        let text = draw(&p, &st, 120, 35);
        assert!(
            text.contains(&"z".repeat(23)),
            "last wrapped row cut: {text}"
        );
    }

    #[test]
    fn a_row_lettered_j_fires_instead_of_moving() {
        let mut st = state();
        let mut p = Popup::verbs(
            "HERE",
            vec![
                Choice::new('a', "first", Pick::Close),
                Choice::new('j', "jack in", Pick::World(WorldAct::JackIn)),
            ],
            0,
        );
        assert!(matches!(
            p.update(&mut st, &[key(KeyCode::Char('j'))]),
            ScreenCmd::Pop { .. }
        ));
        assert!(last(&st).is_some(), "the jack was attempted");
        let mut p = Popup::verbs(
            "HERE",
            vec![
                Choice::new('a', "first", Pick::Close),
                Choice::new('b', "second", Pick::Close),
            ],
            0,
        );
        p.update(&mut st, &[key(KeyCode::Char('j'))]);
        assert_eq!(p.selected, 1, "j still moves when no row uses it");
    }

    /// The cell where `needle` starts in the drawn buffer.
    fn find(b: &Buffer, needle: &str) -> (u16, u16) {
        let text = buffer_text(b);
        for (y, line) in text.lines().enumerate() {
            let chars: Vec<char> = line.chars().collect();
            let n: Vec<char> = needle.chars().collect();
            if let Some(x) = chars.windows(n.len()).position(|w| w == n.as_slice()) {
                return (x as u16, y as u16);
            }
        }
        panic!("{needle} not drawn: {text}")
    }

    #[test]
    fn a_header_names_the_subject_and_a_blank_one_draws_nothing() {
        let mut st = state();
        st.tick = 40;
        let p = Popup::about(
            Some(Header {
                glyph: Some(('@', CYAN)),
                name: "Mara Chen".into(),
                fg: CYAN,
            }),
            Some("door muscle · on the door".into()),
            vec![
                Choice::new('x', "Look", Pick::Close),
                Choice::new('t', "Talk", Pick::Close),
            ],
            0,
        );
        let text = draw(&p, &st, 120, 35);
        assert!(text.contains("@ Mara Chen"), "{text}");
        assert!(text.contains("door muscle · on the door"), "{text}");
        assert!(
            text.contains("x  Look") && text.contains("t  Talk"),
            "{text}"
        );
        let blank = Popup::verbs("", vec![Choice::new('x', "Look", Pick::Close)], 0);
        let r = blank.rect(Rect::new(0, 0, 120, 35));
        assert!(r.width >= 40, "{r:?}");
        let text = draw(&blank, &st, 120, 35);
        let top = text.lines().nth(r.y as usize).unwrap();
        assert!(
            !top.chars().any(|c| c.is_alphabetic()),
            "blank header: {top}"
        );
    }

    #[test]
    fn a_row_draws_verb_colon_thing_and_tail() {
        let mut st = state();
        st.tick = 40;
        let p = Popup::verbs(
            "",
            vec![
                Choice::on('i', "Interact", "Keypad", crate::ui::hud::AMBER, Pick::Close)
                    .tail(" · locked 4m"),
            ],
            0,
        );
        let mut t = Terminal::new(TestBackend::new(120, 35)).unwrap();
        t.draw(|f| p.draw(&st, f)).unwrap();
        let text = buffer_text(t.backend().buffer());
        assert!(text.contains("i  Interact: Keypad · locked 4m"), "{text}");
        let (x, y) = find(t.backend().buffer(), "Keypad");
        assert_eq!(t.backend().buffer()[(x, y)].fg, crate::ui::hud::AMBER);
    }

    #[test]
    fn a_card_closes_the_whole_popup() {
        let mut st = state();
        let mut p = Popup::verbs(
            "",
            vec![Choice::new(
                'i',
                "Inspect",
                Pick::Card(vec![Line::from("a spool")]),
            )],
            0,
        );
        assert!(matches!(
            p.update(&mut st, &[key(KeyCode::Char('i'))]),
            ScreenCmd::Stay
        ));
        assert!(matches!(
            p.update(&mut st, &[key(KeyCode::Esc)]),
            ScreenCmd::Pop { .. }
        ));
    }

    #[test]
    fn an_alias_fires_its_row() {
        let mut st = state();
        let mut p = Popup::verbs(
            "Jack out?",
            vec![
                Choice::new('y', "Jack out", Pick::Close).alias('J'),
                Choice::new('n', "Stay in", Pick::Close),
            ],
            0,
        );
        assert!(matches!(
            p.update(&mut st, &[key(KeyCode::Char('J'))]),
            ScreenCmd::Pop { .. }
        ));
    }

    #[test]
    fn rows_wrap_and_skip_disabled() {
        let mut st = state();
        let mut p = Popup::verbs(
            "T",
            vec![
                Choice::disabled('a', "a"),
                Choice::new('b', "b", Pick::Close),
                Choice::new('c', "c", Pick::Close),
            ],
            0,
        );
        assert_eq!(p.selected, 1);
        p.update(&mut st, &[key(KeyCode::Up)]);
        assert_eq!(
            p.selected, 2,
            "up from the first enabled row wraps to the last"
        );
        p.update(&mut st, &[key(KeyCode::Down)]);
        assert_eq!(p.selected, 1);
        let mut dead = Popup::verbs(
            "T",
            vec![Choice::disabled('a', "a"), Choice::disabled('b', "b")],
            0,
        );
        dead.update(&mut st, &[key(KeyCode::Down), key(KeyCode::Up)]);
        assert_eq!(dead.selected, 0, "nothing to land on: no change, no hang");
    }
}
