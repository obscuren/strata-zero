//! A popup of verbs over whatever opened it: hotkey letters, a selected row, a card view.

use crate::actions::{perform, report};
use crate::anim::effects::{pulse, typewriter};
use crate::app::{AppState, Screen, ScreenCmd, TransitionKind};
use crate::ui::chrome::{
    BAR_BG, CYAN, CYAN_RGB, DIM, MAGENTA, PANEL_BG, TEXT, frame, put, put_paragraph, reveal, solid,
};
use crate::ui::sidebar::Section;
use crossterm::event::{KeyCode, KeyEvent};
use game::Action;
use ratatui::Frame;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};

/// Something the popup does to the player in the world.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WorldAct {
    JackIn,
    /// Jack in from the rig and land at this building's gate.
    Land(citygen::BuildingId),
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
    /// Opens an item's blueprint window in place of the popup: from the rack the item
    /// worn, from the pack one carried; `copy` names which, for an item held per copy.
    Blueprint {
        item: game::ItemId,
        equipped: bool,
        copy: Option<game::CopyId>,
    },
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
    /// Closes and asks the map for its look cursor on this tile.
    Look(citygen::Pos),
    /// Shows what someone says as a card, and logs their line.
    Talk {
        lines: Vec<Line<'static>>,
        said: String,
        /// Who speaks, when it is a person a story may listen for.
        who: Option<sim::PersonId>,
    },
    /// Asks before jacking out, in place of the popup.
    JackOutAsk,
    /// Hops from a server to another backdoor's gate.
    Hop(citygen::BuildingId),
    /// Uses the verb on this key of the held node nearest you.
    Verb(char),
    /// Puts a line on a held ad board's building's ads.
    Hijack {
        node: citygen::NodeId,
        line: crate::holds::Line,
    },
    /// A fixer's offer, in place of their window.
    Offer(crate::jobs::JobId),
    /// Takes the offered job.
    Take(crate::jobs::JobId),
    /// Hands a finished job in to its fixer, for the pay.
    HandIn(crate::jobs::JobId),
    /// Asks before dropping a job, in place of the window.
    DropAsk(crate::jobs::JobId),
    Drop(crate::jobs::JobId),
    /// Puts a Deliver's package into the person's hands.
    Hand(crate::jobs::JobId),
    /// Tracks this, or stops when it is the track.
    Track(crate::track::Source),
    /// Closes and asks the deck for its MAP with the cursor on this listing.
    Map(crate::track::ListingId),
    /// Shows these mails, newest first, as a card, and marks them read.
    Mail(Vec<usize>),
    /// Sits in a ripperdoc's chair on this floor: the deck opens on the ledger.
    Chair {
        building: citygen::BuildingId,
        level: i32,
    },
    /// Opens the Rest window for a sleep in place of the popup.
    Sleep,
    /// Switches your holo set on and opens the news broadcast in place of the popup.
    Watch,
    /// Opens the card of an item's reading at the rig or the bench in place of the popup.
    Read {
        item: game::ItemId,
        at: crate::missions::ReadAt,
    },
    /// Takes the story's tray's piece at this index, on the tab; held out from a
    /// chair, then sits in it.
    Tray {
        piece: usize,
        chair: Option<(citygen::BuildingId, i32)>,
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

/// ` · neural · grants Ping, Static`: where a piece goes, lower case, and the abilities
/// it grants by name, when it grants any.
fn piece_tail(st: &AppState, item: &game::Item) -> String {
    let mut tail = item
        .implant
        .map(|i| format!(" · {}", i.fits.label().to_lowercase()))
        .unwrap_or_default();
    let grants: Vec<String> = item
        .grants
        .iter()
        .map(|a| {
            st.content
                .ability(a)
                .map_or_else(|| a.to_string(), |a| a.name.clone())
        })
        .collect();
    if !grants.is_empty() {
        tail.push_str(&format!(" · grants {}", grants.join(", ")));
    }
    tail
}

const REVEAL_TICKS: u64 = 10;

/// Rows a line takes at `width` under the paragraph's word wrap: greedy, words
/// joined by single spaces, a word longer than a row spilling into the next.
pub(crate) fn wrapped_rows(line: &Line<'_>, width: usize) -> usize {
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
const MAX_W: u16 = 68;

pub struct Popup {
    /// None: the top border is blank.
    header: Option<Header>,
    /// A dim line under the header, before the rows.
    subtitle: Option<String>,
    /// A paragraph between the subtitle and the rows, word-wrapped.
    body: Vec<String>,
    choices: Vec<Choice>,
    pub selected: usize,
    anchor: Option<(u16, u16)>,
    /// Tick the rows (or the card) appeared; drives the reveal.
    opened: u64,
    /// When set, the card's lines replace the rows; closing it closes the popup.
    card: Option<Vec<Line<'static>>>,
    /// Logged when esc puts the window down.
    esc_line: Option<String>,
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
            body: Vec::new(),
            choices,
            selected,
            anchor: None,
            opened: tick,
            card: None,
            esc_line: None,
        }
    }

    /// A paragraph between the subtitle and the rows.
    pub fn with_body(mut self, lines: Vec<String>) -> Self {
        self.body = lines;
        self
    }

    /// A line logged when esc puts the window down.
    fn on_esc(mut self, line: String) -> Self {
        self.esc_line = Some(line);
        self
    }

    /// Rows the body takes at this inner width, its blank row after included.
    fn body_rows(&self, inner_w: usize) -> u16 {
        if self.body.is_empty() {
            return 0;
        }
        let rows: usize = self
            .body
            .iter()
            .map(|l| wrapped_rows(&Line::from(l.as_str()), inner_w))
            .sum();
        rows as u16 + 1
    }

    /// The confirm before leaving the Net: Jack out starts selected, so `J` then
    /// enter or space leaves at once; `J` again confirms too.
    pub fn jack_out_confirm(tick: u64) -> Self {
        Popup::verbs(
            "Jack out?",
            vec![
                Choice::new('y', "Jack out", Pick::World(WorldAct::JackOut)).alias('J'),
                Choice::new('n', "Stay in", Pick::Close),
            ],
            tick,
        )
    }

    /// The story's tray, while one waits (doctor spec §8): `<holder> · <title>` over its
    /// line, a `Take` row a piece on keys 1 to 9, each with where it goes and what it
    /// grants. Esc puts it down and says how to get it back.
    pub fn tray(st: &AppState) -> Option<Self> {
        Popup::tray_then(st, None)
    }

    /// The tray, held out from the chair on this floor when `chair` names one: a piece
    /// taken, you sit in it.
    fn tray_then(st: &AppState, chair: Option<(citygen::BuildingId, i32)>) -> Option<Self> {
        let t = st.missions.tray.as_ref()?;
        let rows = t
            .items
            .iter()
            .zip('1'..='9')
            .enumerate()
            .map(|(i, (id, key))| {
                let item = st.content.item(id);
                let name = item.map_or_else(|| id.to_string(), |it| it.name.clone());
                let tail = item.map(|it| piece_tail(st, it)).unwrap_or_default();
                Choice::on(key, "Take", name, CYAN, Pick::Tray { piece: i, chair }).tail(tail)
            })
            .collect();
        let from = crate::missions::role_name(st, &t.from);
        let title = format!("{from} · {}", t.title);
        Some(
            Popup::verbs(&title, rows, st.tick)
                .with_body(vec![t.line.clone()])
                .on_esc(format!("`{from}` · the tray waits · talk to take a piece")),
        )
    }

    #[cfg(test)]
    pub fn choices(&self) -> &[Choice] {
        &self.choices
    }

    /// Rows above the choices: the subtitle and the blank row under it, then the body.
    fn lead(&self, inner_w: usize) -> u16 {
        (if self.subtitle.is_some() { 2 } else { 0 }) + self.body_rows(inner_w)
    }

    /// Whether this is the landing list a rig offers before it jacks in.
    pub fn is_landing(&self) -> bool {
        self.header.as_ref().is_some_and(|h| h.name == "JACK IN")
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
        // The two jacks from here: the port in reach, or a backdoor's gate from the rig.
        let jacked = match act {
            WorldAct::JackIn => Some(crate::net::jack_in_here(st)),
            WorldAct::Land(b) => Some(crate::net::land_at(st, b)),
            _ => None,
        };
        if let Some(result) = jacked {
            return match result {
                Ok(line) => {
                    st.say(line);
                    TransitionKind::Glitch
                }
                Err(e) => {
                    st.warn(e);
                    TransitionKind::None
                }
            };
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
        let minute = st.minute_of_day();
        let result: Result<(String, TransitionKind), String> =
            match (st.city.as_ref(), st.player.as_mut()) {
                (Some(city), Some(player)) => match act {
                    // Taken above.
                    WorldAct::JackIn => Err("no terminal within reach".to_string()),
                    WorldAct::Land(_) => Err("not from here".to_string()),
                    WorldAct::JackOut => Err("not jacked in".to_string()),
                    WorldAct::Lift(_) => Err("no lift here".to_string()),
                    WorldAct::Climb(_) => Err("no stairs here".to_string()),
                    WorldAct::Up | WorldAct::Down => {
                        let up = act == WorldAct::Up;
                        match player.use_connector(city, up) {
                            Some(dest) => {
                                player.update_fov(
                                    &crate::render::map::MapRef::stratum_at(city, dest, minute),
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

    /// The rows' labels, for tests.
    #[cfg(test)]
    pub(crate) fn labels(&self) -> Vec<String> {
        self.choices.iter().map(|c| c.label.clone()).collect()
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
                    .chain(self.body.iter().map(|s| s.chars().count()))
                    .max()
                    .unwrap_or(0);
                let w = (longest.max(title_w) as u16 + 6).clamp(MIN_W, MAX_W);
                let inner_w = w.saturating_sub(4).max(1) as usize;
                (w, self.choices.len() as u16 + 4 + self.lead(inner_w))
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
                let done = result.is_ok();
                report(st, result);
                // Over the deck too, what it changed moves the story at once.
                if done {
                    crate::missions::settle(st);
                }
                ScreenCmd::Pop {
                    fx: TransitionKind::None,
                }
            }
            Pick::World(WorldAct::JackIn)
                if !self.is_landing() && crate::net::wants_landing_list(st) =>
            {
                ScreenCmd::Replace {
                    to: Box::new(crate::net::landing_popup(st)),
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
            Pick::View(section) => {
                st.sidebar.toggle(section);
                ScreenCmd::Pop {
                    fx: TransitionKind::None,
                }
            }
            Pick::Blueprint {
                item,
                equipped,
                copy,
            } => ScreenCmd::Replace {
                to: Box::new(crate::screens::blueprint::Blueprint::open(
                    st, item, equipped, copy,
                )),
                fx: TransitionKind::None,
            },
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
            Pick::Verb(key) => match verb_opens(st, key) {
                Some(to) => ScreenCmd::Replace {
                    to,
                    fx: TransitionKind::None,
                },
                None => ScreenCmd::Pop {
                    fx: TransitionKind::None,
                },
            },
            Pick::Hijack { node, line } => {
                let said = crate::holds::hijack(st, node, line);
                st.say(said);
                ScreenCmd::Pop {
                    fx: TransitionKind::None,
                }
            }
            Pick::Terminal { origin, key } => ScreenCmd::Replace {
                to: Box::new(crate::screens::terminal::Terminal::open(st, origin, key)),
                fx: TransitionKind::None,
            },
            Pick::Sleep => ScreenCmd::Replace {
                to: Box::new(crate::screens::rest::RestScreen::open(
                    st,
                    crate::rest::Kind::Sleep,
                )),
                fx: TransitionKind::None,
            },
            Pick::Watch => {
                crate::missions::news::switch_on(st);
                ScreenCmd::Replace {
                    to: Box::new(crate::missions::broadcast::Broadcast::new()),
                    fx: TransitionKind::Glitch,
                }
            }
            // Offered on a state gone stale, it only closes.
            Pick::Read { item, at } => {
                match crate::missions::reading::ReadCard::open(st, &item, at) {
                    // One that tells moves the story under its card, at the bench too.
                    Some(card) => {
                        crate::missions::settle(st);
                        ScreenCmd::Replace {
                            to: Box::new(card),
                            fx: TransitionKind::None,
                        }
                    }
                    None => ScreenCmd::Pop {
                        fx: TransitionKind::None,
                    },
                }
            }
            Pick::Chair { building, level } => sit(st, building, level),
            Pick::Look(pos) => {
                st.look_at = Some(pos);
                ScreenCmd::Pop {
                    fx: TransitionKind::None,
                }
            }
            Pick::Talk { lines, said, who } => {
                st.say(said);
                let heard = who.filter(|w| crate::missions::listens(st, *w));
                if let Some(who) = heard {
                    crate::missions::send(st, crate::missions::Event::Talked(who));
                }
                // Whoever holds out a tray holds it out again.
                if who.is_some()
                    && who == crate::missions::tray_from(st)
                    && let Some(tray) = Popup::tray(st)
                {
                    if let Some(t) = st.missions.tray.as_mut() {
                        t.shown = true;
                    }
                    return ScreenCmd::Replace {
                        to: Box::new(tray),
                        fx: TransitionKind::None,
                    };
                }
                if heard.is_some() {
                    return ScreenCmd::Pop {
                        fx: TransitionKind::None,
                    };
                }
                self.card = Some(lines);
                self.opened = st.tick;
                ScreenCmd::Stay
            }
            Pick::Tray { piece, chair } => {
                crate::missions::take_from_tray(st, piece);
                match chair {
                    Some((building, level)) => sit(st, building, level),
                    None => ScreenCmd::Pop {
                        fx: TransitionKind::None,
                    },
                }
            }
            Pick::JackOutAsk => ScreenCmd::Replace {
                to: Box::new(Popup::jack_out_confirm(st.tick)),
                fx: TransitionKind::None,
            },
            Pick::Hop(b) => {
                let result = crate::net::hop(st, b);
                report(st, result);
                ScreenCmd::Pop {
                    fx: TransitionKind::Glitch,
                }
            }
            Pick::Offer(id) => ScreenCmd::Replace {
                to: Box::new(crate::jobs::ui::offer_popup(st, id, st.tick)),
                fx: TransitionKind::None,
            },
            Pick::Take(id) => {
                if let Err(e) = crate::jobs::take_job(st, id) {
                    st.warn(e);
                }
                ScreenCmd::Pop {
                    fx: TransitionKind::None,
                }
            }
            Pick::HandIn(id) => {
                if let Err(e) = crate::jobs::ui::hand_in(st, id) {
                    st.warn(e);
                }
                ScreenCmd::Pop {
                    fx: TransitionKind::None,
                }
            }
            Pick::DropAsk(id) => {
                let title = st.jobs.get(id).map(|j| j.title.clone()).unwrap_or_default();
                ScreenCmd::Replace {
                    to: Box::new(crate::jobs::ui::drop_confirm(id, &title, st.tick)),
                    fx: TransitionKind::None,
                }
            }
            Pick::Drop(id) => {
                if let Err(e) = crate::jobs::ui::drop(st, id) {
                    st.warn(e);
                }
                ScreenCmd::Pop {
                    fx: TransitionKind::None,
                }
            }
            Pick::Hand(id) => {
                if let Err(e) = crate::jobs::ui::hand_over(st, id) {
                    st.warn(e);
                }
                ScreenCmd::Pop {
                    fx: TransitionKind::None,
                }
            }
            Pick::Track(source) => {
                if let Err(e) = crate::track::toggle(st, source) {
                    st.warn(e);
                }
                crate::track::tick(st);
                ScreenCmd::Pop {
                    fx: TransitionKind::None,
                }
            }
            Pick::Map(id) => {
                st.deck_map_to = Some(id);
                ScreenCmd::Pop {
                    fx: TransitionKind::None,
                }
            }
            Pick::Mail(mails) => {
                self.card = Some(crate::jobs::ui::mail_card(st, &mails));
                self.opened = st.tick;
                ScreenCmd::Stay
            }
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
            put(
                buf,
                inner.x + 3,
                ry,
                "  ",
                label_style.bg(bg),
                inner.right(),
            );
            // The revealed text in three runs: verb and colon, the thing in its
            // colour, the tail.
            let text = c.text();
            let shown: Vec<char> = typewriter(&text, frac).chars().collect();
            let verb = c.label.chars().count() + if c.thing.is_some() { 2 } else { 0 };
            let thing = c.thing.as_ref().map_or(0, |(t, _)| t.chars().count());
            let thing_style = match &c.thing {
                Some((_, colour)) if c.enabled => {
                    Style::default().fg(*colour).add_modifier(Modifier::BOLD)
                }
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

/// Sits you in the doc's chair on this floor and opens the deck on it. A tray its doc
/// holds out waits first: it comes up in the chair's place, and a piece taken sits you
/// down.
fn sit(st: &mut AppState, building: citygen::BuildingId, level: i32) -> ScreenCmd {
    let Some(doc) = crate::clinic::doc_at_work(st, building) else {
        st.warn("the doc is out");
        return ScreenCmd::Pop {
            fx: TransitionKind::None,
        };
    };
    if crate::missions::tray_from(st) == Some(doc)
        && let Some(tray) = Popup::tray_then(st, Some((building, level)))
    {
        if let Some(t) = st.missions.tray.as_mut() {
            t.shown = true;
        }
        return ScreenCmd::Replace {
            to: Box::new(tray),
            fx: TransitionKind::None,
        };
    }
    st.chair = Some(crate::clinic::Chair {
        building,
        level,
        doc,
    });
    let line = crate::clinic::doc_line(st, doc);
    st.say(format!("{}: “{line}”", crate::clinic::doc_name(st, doc)));
    ScreenCmd::Replace {
        to: Box::new(crate::screens::deck::Deck::chair(st)),
        fx: TransitionKind::Glitch,
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
            // A row's letter wins over j/k navigation; the arrows always navigate. Space
            // is the confirm key, never a letter: rows past the letters carry a blank.
            if let KeyCode::Char(c) = k.code
                && c != ' '
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
                    if let Some(line) = self.esc_line.take() {
                        st.story(line);
                    }
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
                if !self.body.is_empty() {
                    let top = if self.subtitle.is_some() { 2 } else { 0 };
                    let body_h = self.body_rows(inner.width as usize).saturating_sub(1);
                    let area = Rect::new(
                        inner.x,
                        inner.y + top,
                        inner.width,
                        body_h.min(inner.height.saturating_sub(top)),
                    );
                    let lines: Vec<Line<'static>> = self
                        .body
                        .iter()
                        .map(|l| Line::from(Span::styled(l.clone(), Style::default().fg(TEXT))))
                        .collect();
                    put_paragraph(buf, area, lines);
                }
                let lead = self.lead(inner.width as usize).min(inner.height);
                let rows = Rect::new(inner.x, inner.y + lead, inner.width, inner.height - lead);
                let bar = Rect::new(r.x + 1, rows.y, r.width - 2, rows.height);
                self.draw_rows(st, rows, bar, buf);
            }
        }
    }
}

/// A held node's verb, by its key: a line logged, or the screen it opens (a held
/// terminal's shell, an ad board's hijack picker). A refusal is warned.
pub fn verb_opens(st: &mut AppState, key: char) -> Option<Box<dyn crate::app::Screen>> {
    use crate::holds::VerbDone;
    match crate::holds::use_verb(st, key) {
        Ok(VerbDone::Said(line)) => {
            st.say(line);
            None
        }
        Ok(VerbDone::Shell(host, at)) => Some(Box::new(crate::screens::terminal::Terminal::open(
            st, host, at,
        ))),
        Ok(VerbDone::Pick(node)) => {
            let b = st
                .city
                .as_ref()
                .and_then(|c| c.net.nodes.get(node.index()))
                .and_then(|n| n.building)?;
            let choices = crate::holds::hijack_lines(st, b)
                .into_iter()
                .map(|(line, text)| {
                    Choice::on(
                        line.key(),
                        line.label(),
                        text,
                        crate::ui::chrome::MAGENTA,
                        Pick::Hijack { node, line },
                    )
                })
                .collect();
            Some(Box::new(Popup::verbs("HIJACK", choices, st.tick)))
        }
        Err(why) => {
            st.warn(why);
            None
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
    fn sitting_in_the_chair_opens_the_deck() {
        let (mut st, b, level, _) = crate::clinic::testutil::inside();
        let doc = crate::clinic::doc_at_work(&st, b).unwrap();
        let mut p = Popup::about(
            None,
            None,
            vec![Choice::new('i', "Sit", Pick::Chair { building: b, level })],
            st.tick,
        );
        let cmd = p.update(&mut st, &[key(crossterm::event::KeyCode::Char('i'))]);
        assert!(matches!(cmd, ScreenCmd::Replace { .. }));
        assert_eq!(
            st.chair,
            Some(crate::clinic::Chair {
                building: b,
                level,
                doc
            })
        );
        let said = last(&st).unwrap();
        assert!(
            said.starts_with(&crate::clinic::doc_name(&st, doc)) && said.contains('“'),
            "{said}"
        );
    }

    #[test]
    fn watch_switches_the_set_on_and_opens_the_broadcast() {
        use crate::missions::news;
        let mut st = crate::jobs::testutil::state_home();
        crate::missions::begin(&mut st);
        news::put_out(&mut st, "marlo", None);
        news::switch_off(&mut st);
        let mut p = Popup::about(
            None,
            None,
            vec![Choice::new('i', "Interact", Pick::Watch)],
            st.tick,
        );
        let cmd = p.update(&mut st, &[key(crossterm::event::KeyCode::Char('i'))]);
        assert!(matches!(
            cmd,
            ScreenCmd::Replace {
                fx: TransitionKind::Glitch,
                ..
            }
        ));
        assert!(news::current(&st).unwrap().on);
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
                Pick::Talk {
                    lines: vec![Line::from("a spool")],
                    said: "a spool".into(),
                    who: None,
                },
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
            vec![Choice::new(
                'l',
                "look",
                Pick::Talk {
                    lines: vec![Line::from(line.clone())],
                    said: line,
                    who: None,
                },
            )],
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
                Choice::on(
                    'i',
                    "Interact",
                    "Keypad",
                    crate::ui::hud::AMBER,
                    Pick::Close,
                )
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
                Pick::Talk {
                    lines: vec![Line::from("a spool")],
                    said: "a spool".into(),
                    who: None,
                },
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
    fn space_fires_the_selected_row_not_an_unlettered_one() {
        let mut st = state();
        let at = |x: i32| citygen::Pos::new(x, 0);
        let rows = || {
            vec![
                Choice::new('x', "Look", Pick::Look(at(0))),
                Choice::on('a', "Ride", "12F", TEXT, Pick::Look(at(12))),
                Choice::on(' ', "Ride", "11F", TEXT, Pick::Look(at(11))),
                Choice::on(' ', "Ride", "10F", TEXT, Pick::Look(at(10))),
            ]
        };
        // Space on the default row fires it.
        let mut p = Popup::verbs("Lift", rows(), 0);
        p.update(&mut st, &[key(KeyCode::Char(' '))]);
        assert_eq!(
            st.look_at,
            Some(at(0)),
            "space fires Look, the selected row"
        );
        // Space on an unlettered row fires that row, not the first unlettered one.
        let mut p = Popup::verbs("Lift", rows(), 0);
        p.update(
            &mut st,
            &[key(KeyCode::Down), key(KeyCode::Down), key(KeyCode::Down)],
        );
        assert_eq!(p.selected, 3);
        p.update(&mut st, &[key(KeyCode::Char(' '))]);
        assert_eq!(
            st.look_at,
            Some(at(10)),
            "space fires 10F, the selected row"
        );
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

    /// Any screen drawn alone at `w`×`h`, a while after it opened.
    fn drawn(screen: &dyn Screen, st: &mut AppState, w: u16, h: u16) -> String {
        st.tick += 40;
        let mut t = Terminal::new(TestBackend::new(w, h)).unwrap();
        t.draw(|f| screen.draw(st, f)).unwrap();
        buffer_text(t.backend().buffer())
    }

    #[test]
    fn sitting_in_her_chair_holds_her_waiting_tray_out_first() {
        use crate::missions::{Effect, Need, TraySet, effects};
        let (mut st, b, level, _) = crate::clinic::testutil::inside();
        st.book = crate::missions::tests::doc_book(crate::missions::tests::IDLE, "");
        let doc = crate::missions::tests::cast_doc(&mut st, b);
        let tray = Effect::Tray {
            from: "doc".into(),
            tab: "doc".into(),
            title: "her tray".into(),
            line: "One piece.".into(),
            sets: vec![TraySet {
                need: Need::All(vec![]),
                items: vec!["neural_port".into(), "subdermal_mesh".into()],
            }],
        };
        effects::apply(&mut st, 0, 0, &tray);
        // Put up over the map once and put down with esc.
        st.missions.tray.as_mut().unwrap().shown = true;
        let sit = || {
            Popup::about(
                None,
                None,
                vec![Choice::new('i', "Sit", Pick::Chair { building: b, level })],
                0,
            )
        };
        // Sat down with her tray waiting: the tray first, the chair not yet.
        let ScreenCmd::Replace { to: mut up, .. } =
            sit().update(&mut st, &[key(KeyCode::Char('i'))])
        else {
            panic!("the chair holds the tray out");
        };
        assert_eq!(st.chair, None);
        assert!(drawn(up.as_ref(), &mut st, 120, 40).contains("Doc Vance · her tray"));
        // Esc: back to the map, standing, the tray still waiting.
        assert!(matches!(
            up.update(&mut st, &[key(KeyCode::Esc)]),
            ScreenCmd::Pop { .. }
        ));
        assert_eq!(st.chair, None);
        assert!(st.missions.tray.is_some());
        // Sat down again and a piece taken: the chair opens.
        let ScreenCmd::Replace { to: mut up, .. } =
            sit().update(&mut st, &[key(KeyCode::Char('i'))])
        else {
            panic!("the chair holds the tray out again");
        };
        assert!(matches!(
            up.update(&mut st, &[key(KeyCode::Char('1'))]),
            ScreenCmd::Replace { .. }
        ));
        assert_eq!(st.missions.tray, None);
        assert_eq!(st.character.count(&ItemId::new("neural_port")), 1);
        let chair = crate::clinic::Chair {
            building: b,
            level,
            doc,
        };
        assert_eq!(st.chair, Some(chair));
        // Another's tray waiting: the chair is only the chair.
        st.chair = None;
        effects::apply(&mut st, 0, 0, &tray);
        st.missions
            .cast
            .insert("doc".into(), sim::PersonId(doc.0 + 1));
        assert!(matches!(
            sit().update(&mut st, &[key(KeyCode::Char('i'))]),
            ScreenCmd::Replace { .. }
        ));
        assert_eq!(st.chair, Some(chair));
        assert!(st.missions.tray.is_some());
    }

    #[test]
    fn esc_leaves_the_tray_and_talking_reopens_it() {
        use crate::missions::{Effect, Need, TraySet, effects};
        let (mut st, b, _, _) = crate::clinic::testutil::inside();
        st.book = crate::missions::tests::doc_book(crate::missions::tests::IDLE, "");
        let doc = crate::missions::tests::cast_doc(&mut st, b);
        let tray = Effect::Tray {
            from: "doc".into(),
            tab: "doc".into(),
            title: "her tray".into(),
            line: "One piece, on his tab. The rest go back in the drawer.".into(),
            sets: vec![TraySet {
                need: Need::All(vec![]),
                items: vec!["neural_port".into(), "subdermal_mesh".into()],
            }],
        };
        effects::apply(&mut st, 0, 0, &tray);
        // Put up over the map once.
        let mut map = crate::screens::explore::MapScreen::walking(&mut st);
        let ScreenCmd::Push { to: mut up, .. } = map.update(&mut st, &[]) else {
            panic!("the tray opens over the map");
        };
        let text = drawn(up.as_ref(), &mut st, 120, 40);
        assert!(text.contains("Doc Vance · her tray"), "{text}");
        assert!(
            text.contains("One piece, on his tab. The rest go back in the drawer."),
            "{text}"
        );
        assert!(
            text.contains("1  Take: Neural port · neural · grants Ping, Static"),
            "{text}"
        );
        assert!(text.contains("2  Take: Subdermal mesh · skin"), "{text}");
        // Esc puts it down, pending, and says how to get it back; the map doesn't put it
        // up again.
        assert!(matches!(
            up.update(&mut st, &[key(KeyCode::Esc)]),
            ScreenCmd::Pop { .. }
        ));
        assert!(st.missions.tray.as_ref().is_some_and(|t| t.shown));
        assert_eq!(
            last(&st),
            Some("Doc Vance · the tray waits · talk to take a piece")
        );
        assert!(!matches!(map.update(&mut st, &[]), ScreenCmd::Push { .. }));
        // Talking to her holds it out again; anyone else talks as ever.
        let talk = |who| {
            Popup::verbs(
                "DOC",
                vec![Choice::new(
                    't',
                    "Talk",
                    Pick::Talk {
                        lines: vec![Line::from("a line")],
                        said: "Doc Vance: hi".into(),
                        who: Some(who),
                    },
                )],
                0,
            )
        };
        assert!(matches!(
            talk(sim::PersonId(doc.0 + 1)).update(&mut st, &[key(KeyCode::Char('t'))]),
            ScreenCmd::Stay
        ));
        // Held out by talking before the map put it up, it counts as shown.
        st.missions.tray.as_mut().unwrap().shown = false;
        let ScreenCmd::Replace { to: mut again, .. } =
            talk(doc).update(&mut st, &[key(KeyCode::Char('t'))])
        else {
            panic!("talking to her opens the tray");
        };
        assert!(st.missions.tray.as_ref().is_some_and(|t| t.shown));
        assert_eq!(last(&st), Some("Doc Vance: hi"));
        assert!(drawn(again.as_ref(), &mut st, 120, 40).contains("Doc Vance · her tray"));
        // Taking one puts it away for good.
        assert!(matches!(
            again.update(&mut st, &[key(KeyCode::Char('2'))]),
            ScreenCmd::Pop { .. }
        ));
        assert_eq!(st.missions.tray, None);
        assert_eq!(
            st.missions.tab.as_ref().map(|t| t.item.clone()),
            Some(ItemId::new("subdermal_mesh"))
        );
        assert_eq!(st.character.count(&ItemId::new("subdermal_mesh")), 1);
        assert!(matches!(
            talk(doc).update(&mut st, &[key(KeyCode::Char('t'))]),
            ScreenCmd::Stay
        ));
    }
}
