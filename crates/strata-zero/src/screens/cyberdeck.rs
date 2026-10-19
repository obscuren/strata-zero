//! A deck on its blueprint: the pointer on the deck's parts, what the glass reads, a
//! cart carried from the inventory to a slot, and the failsafe under its cover. The
//! jacked deck works; a deck in the pack can only be read.

use crate::actions::{perform, report};
use crate::app::{AppState, ScreenCmd, TransitionKind};
use crate::screens::deck::{Deck, FAILSAFE_TEXT, FAILSAFE_WARN, REVEAL_TICKS};
use crate::ui::chrome::{DIM, MAGENTA};
use crate::ui::cyberdeck::{
    Cart, DeckView, Dir, GLASS_ROWS, GLASS_TEXT_W, GlassTone, Held, Part, Switch, parts, step,
};
use crate::ui::sidebar::wrap_text;
use citygen::Place;
use crossterm::event::KeyCode;
use game::{AbilityId, Action, DeckSpec, Item, ItemId, ItemKind, SlotPlan, Source};
use ratatui::style::Modifier;
use ratatui::text::Line;

/// Glass lines and how each is lit.
type Glass = Vec<(String, GlassTone)>;

/// What the blueprint keeps of a deck between frames: which deck it is, the part the
/// brackets are on, the cart carried, the failsafe's cover, and the glass's scroll and
/// typing.
#[derive(Clone, Debug, PartialEq)]
pub struct DeckTab {
    pub deck: ItemId,
    /// The deck is the jacked one. A deck in the pack is pointed at and read, never
    /// changed.
    pub equipped: bool,
    pub point: Part,
    /// A program lifted from the inventory, waiting for a slot.
    pub held: Option<ItemId>,
    /// The failsafe's cover is up. The blueprint's own state; the game never sees it.
    pub cover_up: bool,
    /// The first glass row shown.
    pub scroll: usize,
    /// Tick the glass started typing.
    pub changed: u64,
}

impl DeckTab {
    /// The blueprint of `deck`: the jacked deck on the part it was left on
    /// (`st.deck_part`), a carried one on slot A, or the nearest part it has.
    pub fn for_deck(tick: u64, deck: ItemId, equipped: bool, st: &AppState) -> Self {
        let mut tab = DeckTab {
            deck,
            equipped,
            point: Part::Slot(0),
            held: None,
            cover_up: false,
            scroll: 0,
            changed: tick,
        };
        let want = if equipped {
            Part::unpack(st.deck_part)
        } else {
            Part::Slot(0)
        };
        tab.point = tab.clamp(st, want);
        tab
    }

    /// The part, for `st.deck_part`.
    pub fn pack(&self) -> usize {
        self.point.pack()
    }

    pub fn hint(&self) -> &'static str {
        if self.held.is_some() {
            "↑↓←→ carry · space drop · esc put back"
        } else {
            "↑↓←→ point · space act · esc close"
        }
    }

    /// One key: `Pop` for esc with nothing to undo, `Stay` for every other. The window
    /// has no tabs, so the keys the deck does not use do nothing.
    pub fn key(&mut self, st: &mut AppState, code: KeyCode) -> ScreenCmd {
        let dir = match code {
            KeyCode::Up | KeyCode::Char('k') => Dir::Up,
            KeyCode::Down | KeyCode::Char('j') => Dir::Down,
            KeyCode::Left | KeyCode::Char('h') => Dir::Left,
            KeyCode::Right | KeyCode::Char('l') => Dir::Right,
            KeyCode::Char(' ') | KeyCode::Enter => {
                if !self.equipped {
                    st.warn("equip the deck to work on it");
                } else if locked(st) {
                    st.warn("the deck is locked while you're jacked in");
                } else {
                    self.act(st);
                }
                return ScreenCmd::Stay;
            }
            KeyCode::Esc | KeyCode::Char('q') => return self.back(st),
            KeyCode::PageUp | KeyCode::PageDown => {
                let last = rows(st, self).0.len().saturating_sub(GLASS_ROWS);
                self.scroll = if code == KeyCode::PageUp {
                    self.scroll.min(last).saturating_sub(1)
                } else {
                    (self.scroll + 1).min(last)
                };
                return ScreenCmd::Stay;
            }
            _ => return ScreenCmd::Stay,
        };
        self.walk(st, dir);
        ScreenCmd::Stay
    }

    /// The deck as it stands, for the drawing.
    pub fn view(&self, st: &AppState) -> DeckView {
        let point = self.clamp(st, self.point);
        let mut view = self.layout(st, point);
        let tab = DeckTab {
            point,
            ..self.clone()
        };
        let (rows, pinned) = rows(st, &tab);
        let top = self.scroll.min(rows.len().saturating_sub(GLASS_ROWS));
        view.glass = rows.iter().skip(top).take(GLASS_ROWS).cloned().collect();
        view.more_above = top > 0;
        view.more_below = top + GLASS_ROWS < rows.len();
        // The whole readout types on in the detail pane's reveal time.
        let total: usize = view
            .glass
            .iter()
            .chain([&pinned])
            .map(|(t, _)| t.chars().count())
            .sum();
        let ticks = st.tick.saturating_sub(self.changed).min(REVEAL_TICKS) as usize;
        view.typed = total * ticks / REVEAL_TICKS as usize;
        view.pinned = pinned;
        view
    }

    /// Everything the drawing needs but the glass, with the brackets on `point`. A deck
    /// in the pack is cold and has no link: it is not the one jacked.
    fn layout(&self, st: &AppState, point: Part) -> DeckView {
        let deck = drawn(st, self).map(|(_, d)| d);
        let inventory = inventory(st);
        let from = self
            .held
            .as_ref()
            .and_then(|h| inventory.iter().position(|i| i == h));
        let held = self.held.as_ref().zip(from).and_then(|(id, from)| {
            let start = match point {
                Part::Slot(k) => landing(st, self, id, k).1,
                _ => 0,
            };
            Some(Held {
                from,
                cart: cart(st.content.item(id)?),
                start,
                letter: letter(start),
            })
        });
        let mut view = DeckView {
            look: deck.map(|d| d.look).unwrap_or_default(),
            slots: deck.map_or(0, |d| d.slots),
            rating: deck.map_or(0, |d| d.rating),
            bay: bay(st, self).into_iter().map(cart).collect(),
            roms: roms(st, self).into_iter().map(|(_, name)| name).collect(),
            inventory: inventory
                .iter()
                .filter_map(|id| st.content.item(id))
                .map(cart)
                .collect(),
            inv_offset: 0,
            point,
            held,
            switch: if !st.net.failsafe {
                Switch::Off
            } else if self.cover_up {
                Switch::Lifted
            } else {
                Switch::On
            },
            trace: trace(st).filter(|_| self.equipped),
            link: self.equipped && locked(st),
            plugged: self.equipped,
            handle: st.character.handle.clone(),
            glass: Vec::new(),
            pinned: (String::new(), GlassTone::Dim),
            more_above: false,
            more_below: false,
            typed: usize::MAX,
            tick: st.tick,
        };
        // The row scrolls just far enough to show the cart lifted, or the one pointed at.
        let keep = from.or(match point {
            Part::Inv(k) => Some(k),
            _ => None,
        });
        if let Some(k) = keep.filter(|k| *k < view.inventory.len()) {
            while view.inv_offset < k && !parts(&view).iter().any(|(p, _)| *p == Part::Inv(k)) {
                view.inv_offset += 1;
            }
        }
        view
    }

    /// `point` if the deck still has it, else slot A, else the first cart, else the plate.
    fn clamp(&self, st: &AppState, point: Part) -> Part {
        let parts = parts(&self.layout(st, point));
        [point, Part::Slot(0), Part::Inv(0)]
            .into_iter()
            .find(|p| parts.iter().any(|(q, _)| q == p))
            .unwrap_or(Part::Plate)
    }

    /// Puts the brackets on `to`: the glass starts over, and a lifted cover shuts behind.
    fn go(&mut self, st: &AppState, to: Part) {
        if to == self.point {
            return;
        }
        if self.point == Part::Failsafe {
            self.cover_up = false;
        }
        self.point = to;
        self.touch(st.tick);
    }

    /// The glass reads something new: from the top, typed again.
    fn touch(&mut self, tick: u64) {
        self.changed = tick;
        self.scroll = 0;
    }

    /// The arrows: the nearest part that way, only the bay's stops while carrying.
    fn walk(&mut self, st: &AppState, dir: Dir) {
        let holding = self.held.is_some();
        let view = self.layout(st, self.point);
        let to = match (self.point, dir) {
            // The inventory row scrolls: its next cart is next, shown or not yet.
            (Part::Inv(k), Dir::Right) if k + 1 < view.inventory.len() => Part::Inv(k + 1),
            (Part::Inv(k), Dir::Left) if k > 0 => Part::Inv(k - 1),
            _ => step(&parts(&view), self.point, dir, |p| {
                !holding || matches!(p, Part::Slot(_))
            }),
        };
        self.go(st, to);
    }

    /// Esc: a carried cart goes back, else a lifted cover shuts, else the window closes.
    fn back(&mut self, st: &AppState) -> ScreenCmd {
        if self.held.take().is_some() {
            self.touch(st.tick);
            return ScreenCmd::Stay;
        }
        if self.cover_up && st.net.failsafe {
            self.cover_up = false;
            self.touch(st.tick);
            return ScreenCmd::Stay;
        }
        ScreenCmd::Pop {
            fx: TransitionKind::None,
        }
    }

    /// Space on the pointed part.
    fn act(&mut self, st: &mut AppState) {
        match self.point {
            Part::Slot(k) => match self.held.clone() {
                Some(id) => self.slot_in(st, id, k),
                None => {
                    let Some(id) = bay(st, self).get(k).map(|i| i.id.clone()) else {
                        return;
                    };
                    let result = perform(st, Action::Unload(id));
                    if result.is_ok() {
                        self.touch(st.tick);
                    }
                    report(st, result);
                }
            },
            Part::Inv(k) if self.held.is_none() => self.pick_up(st, k),
            Part::Failsafe => self.throw(st),
            Part::Inv(_) | Part::Plate | Part::Rom(_) => {}
        }
    }

    /// Lifts cart `k` from the inventory, unless no slot could ever take it. The
    /// brackets jump to the first empty slot, else the first cart.
    fn pick_up(&mut self, st: &mut AppState, k: usize) {
        let Some(id) = inventory(st).get(k).cloned() else {
            return;
        };
        let end = st.character.loaded.len();
        if let Err(why) = st.character.plan_slot(&st.content, &id, end) {
            st.warn(why);
            return;
        }
        self.held = Some(id);
        let to = self.clamp(st, Part::Slot(bay(st, self).len()));
        self.go(st, to);
    }

    /// Drops the carried cart at bay stop `k`. A refusal keeps it carried.
    fn slot_in(&mut self, st: &mut AppState, id: ItemId, k: usize) {
        let at = position(st, self, k);
        let result = perform(st, Action::Slot { id: id.clone(), at });
        if result.is_ok() {
            self.held = None;
            if let Some(k) = bay(st, self).iter().position(|i| i.id == id) {
                self.point = Part::Slot(k);
            }
            self.touch(st.tick);
        }
        report(st, result);
    }

    /// The failsafe: space lifts the cover, then throws it off; off, one press throws
    /// it back on and shuts the cover.
    fn throw(&mut self, st: &mut AppState) {
        if !st.net.failsafe {
            st.net.failsafe = true;
            self.cover_up = false;
            st.say("failsafe on");
        } else if self.cover_up {
            st.net.failsafe = false;
            st.warn("failsafe off · black ICE can kill you");
        } else {
            self.cover_up = true;
        }
        self.touch(st.tick);
    }
}

/// Jacked in: the deck is meatspace only, so pointing and reading work and acting does not.
fn locked(st: &AppState) -> bool {
    crate::actions::jacked_in(st)
}

/// The deck the blueprint draws and its spec; `None` once the content no longer knows
/// it as a deck.
fn drawn<'a>(st: &'a AppState, tab: &DeckTab) -> Option<(&'a Item, DeckSpec)> {
    let item = st.content.item(&tab.deck)?;
    Some((item, item.deck?))
}

/// Programs in the pack and not loaded, in pack order, one per id: the carts in
/// WAREZ.
pub(crate) fn inventory(st: &AppState) -> Vec<ItemId> {
    let mut out: Vec<ItemId> = Vec::new();
    for s in &st.character.inventory {
        let program = st
            .content
            .item(&s.item)
            .is_some_and(|i| i.kind == ItemKind::Program);
        let slotted = st.character.loaded.contains(&s.item);
        if program && !slotted && !out.contains(&s.item) {
            out.push(s.item.clone());
        }
    }
    out
}

/// The trace the HUD would show: inside a construct its heat, on the grid the
/// nearest gate's within reach; `None` when not jacked in.
pub(crate) fn trace(st: &AppState) -> Option<u8> {
    let p = st.player.as_ref().filter(|p| p.jacked_in())?;
    match p.place {
        Place::Construct(b) => Some(st.net.traces.get(&b).map_or(0, |t| t.heat)),
        _ => st
            .net
            .nearest_trace(st.city.as_ref()?, p.pos)
            .map(|(_, h)| h),
    }
}

/// What the glass reads for the pointed part: the lines, unwrapped, the head first, and
/// the pinned row naming the key that acts.
pub(crate) fn glass(st: &AppState, tab: &DeckTab) -> (Glass, (String, GlassTone)) {
    let line = |text: &str, tone| (text.to_string(), tone);
    let Some((deck, spec)) = drawn(st, tab) else {
        return (Vec::new(), line("", GlassTone::Dim));
    };
    if let (Some(id), Part::Slot(k)) = (&tab.held, tab.point) {
        return carrying(st, tab, id, k);
    }
    let bay = bay(st, tab);
    let used: u8 = bay.iter().filter_map(|i| i.program).map(|p| p.size).sum();
    match tab.point {
        Part::Slot(k) => {
            let start = bay.iter().take(k).map(|i| width(i)).sum();
            match bay.get(k) {
                Some(item) => {
                    let head = format!("SLOT {} ▸ {}", letter(start), item.name.to_uppercase());
                    (
                        program(st, &item.id, head),
                        line("[space] eject", GlassTone::Body),
                    )
                }
                None => (
                    vec![
                        (format!("SLOT {} ▸ EMPTY", letter(start)), GlassTone::Head),
                        (
                            format!(
                                "{} of {} slots free",
                                spec.slots.saturating_sub(used),
                                spec.slots
                            ),
                            GlassTone::Dim,
                        ),
                    ],
                    line("empty · bring a program from inventory", GlassTone::Dim),
                ),
            }
        }
        Part::Plate => (
            lines(&Deck::item_card(st, &deck.id)),
            (
                format!("{used}/{} slots · rating {}", spec.slots, spec.rating),
                GlassTone::Dim,
            ),
        ),
        Part::Rom(k) => {
            let Some((id, name)) = roms(st, tab).into_iter().nth(k) else {
                return (Vec::new(), line("", GlassTone::Dim));
            };
            // Burned into this deck, whatever else grants it.
            let source = Source::Item(deck.id.clone());
            let mut body = lines(&Deck::ability_card(st, &id, &source));
            let head = (format!("ROM ▸ {name}"), GlassTone::Head);
            match body.first_mut() {
                Some(first) => *first = head,
                None => body.push(head),
            }
            (body, line("burned in · can't be pulled", GlassTone::Dim))
        }
        Part::Failsafe => failsafe(st.net.failsafe, tab.cover_up),
        Part::Inv(k) => {
            let Some((id, item)) = inventory(st)
                .get(k)
                .and_then(|id| Some((id.clone(), st.content.item(id)?)))
            else {
                return (Vec::new(), line("", GlassTone::Dim));
            };
            let mut body = program(st, &id, upper(st, &id));
            let size = item.program.map_or(0, |p| p.size);
            let unit = if size == 1 { "slot" } else { "slots" };
            let (needs, pinned) = if size > spec.slots {
                (
                    format!("needs {size} {unit} · this deck has {}", spec.slots),
                    line("too big for this deck", GlassTone::Warn),
                )
            } else {
                (
                    format!(
                        "needs {size} {unit} · {} free",
                        spec.slots.saturating_sub(used)
                    ),
                    line("[space] pick up", GlassTone::Body),
                )
            };
            body.insert(body.len().min(2), (needs, GlassTone::Dim));
            (body, pinned)
        }
    }
}

/// The glass while a cart is carried over bay stop `k`: where it lands and what it
/// sends back to the inventory.
fn carrying(st: &AppState, tab: &DeckTab, id: &ItemId, k: usize) -> (Glass, (String, GlassTone)) {
    let (plan, start) = landing(st, tab, id, k);
    let plan = match plan {
        Ok(plan) => plan,
        Err(why) => {
            return (
                vec![(upper(st, id), GlassTone::Head), (why, GlassTone::Warn)],
                ("esc put back".into(), GlassTone::Dim),
            );
        }
    };
    let goes = format!("{} goes in slot {}", name(st, id), letter(start));
    let back: Vec<String> = plan.displaced.iter().map(|d| name(st, d)).collect();
    let Some((last, before)) = back.split_last() else {
        return (
            vec![
                (
                    format!("{} ▸ {}", upper(st, id), letter(start)),
                    GlassTone::Head,
                ),
                (format!("{goes}."), GlassTone::Body),
            ],
            ("[space] slot · esc put back".into(), GlassTone::Body),
        );
    };
    let list = if before.is_empty() {
        last.clone()
    } else {
        format!("{} and {last}", before.join(", "))
    };
    let caps: Vec<String> = back.iter().map(|n| n.to_uppercase()).collect();
    (
        vec![
            (
                format!("{} ⇄ {}", upper(st, id), caps.join(", ")),
                GlassTone::Head,
            ),
            (
                format!("{goes}, {list} back to inventory."),
                GlassTone::Body,
            ),
        ],
        ("[space] swap · esc put back".into(), GlassTone::Body),
    )
}

/// The failsafe on the glass: on, lifted, or off.
fn failsafe(on: bool, cover_up: bool) -> (Glass, (String, GlassTone)) {
    let line = |text: &str, tone| (text.to_string(), tone);
    if !on {
        return (
            vec![
                line("FAILSAFE ▸ OFF", GlassTone::Warn),
                line(FAILSAFE_WARN, GlassTone::Warn),
                line("", GlassTone::Body),
                line(FAILSAFE_TEXT, GlassTone::Body),
            ],
            line("[space] throw it back", GlassTone::Body),
        );
    }
    if cover_up {
        return (
            vec![
                line("FAILSAFE ▸ ON · cover up", GlassTone::Head),
                line("Throw it and black ICE can kill you.", GlassTone::Warn),
            ],
            line("[space] throw · esc shut", GlassTone::Body),
        );
    }
    (
        vec![
            line("FAILSAFE ▸ ON", GlassTone::Head),
            line(FAILSAFE_TEXT, GlassTone::Body),
            line("", GlassTone::Body),
            line(FAILSAFE_WARN, GlassTone::Warn),
        ],
        line("[space] lift the cover", GlassTone::Body),
    )
}

/// A program in the glass's order under `head`: its numbers and what it is first, then
/// what it hits, the odds and whether it is loaded. The lines are `program_card`'s.
fn program(st: &AppState, id: &ItemId, head: String) -> Glass {
    // The card: head, op · targets, numbers, power, [loaded], blank, description.
    let card = lines(&Deck::program_card(st, id));
    let n = card.len();
    if n < 6 {
        return card;
    }
    let mut out = vec![
        (head, GlassTone::Head),
        card[2].clone(),
        card[n - 1].clone(),
        (String::new(), GlassTone::Body),
        card[1].clone(),
        card[3].clone(),
    ];
    out.extend(card[4..n - 2].iter().cloned());
    out
}

/// A card's lines as glass lines: its head bold, dim lines dim, warnings red.
fn lines(card: &[Line<'static>]) -> Glass {
    card.iter()
        .map(|l| {
            let text: String = l.spans.iter().map(|s| s.content.as_ref()).collect();
            let style = l.spans.first().map(|s| s.style).unwrap_or_default();
            let tone = if style.add_modifier.contains(Modifier::BOLD) {
                GlassTone::Head
            } else if style.fg == Some(MAGENTA) {
                GlassTone::Warn
            } else if style.fg == Some(DIM) {
                GlassTone::Dim
            } else {
                GlassTone::Body
            };
            (text, tone)
        })
        .collect()
}

/// The glass's lines wrapped to its width, and the pinned row: a deck that cannot be
/// changed says why in place of its key.
fn rows(st: &AppState, tab: &DeckTab) -> (Glass, (String, GlassTone)) {
    let (body, mut pinned) = glass(st, tab);
    if !tab.equipped {
        pinned = ("not jacked · equip it first".into(), GlassTone::Warn);
    } else if locked(st) {
        pinned = ("locked · jacked in".into(), GlassTone::Warn);
    }
    let rows = body
        .into_iter()
        .flat_map(|(text, tone)| {
            wrap_text(&text, GLASS_TEXT_W)
                .into_iter()
                .map(move |row| (row, tone))
        })
        .collect();
    (rows, pinned)
}

/// The carts in the bay: the loaded programs the content knows, in loaded order. A
/// deck in the pack holds none.
fn bay<'a>(st: &'a AppState, tab: &DeckTab) -> Vec<&'a Item> {
    if !tab.equipped {
        return Vec::new();
    }
    st.character.loaded_programs(&st.content).collect()
}

/// The deck's ROM in strip order: each grant the content knows and its name in
/// capitals.
fn roms(st: &AppState, tab: &DeckTab) -> Vec<(AbilityId, String)> {
    let Some((deck, _)) = drawn(st, tab) else {
        return Vec::new();
    };
    deck.grants
        .iter()
        .filter_map(|id| Some((id.clone(), st.content.ability(id)?.name.to_uppercase())))
        .collect()
}

/// Bay stop `k` as a position in `Character::loaded`: its cart's, or the end.
fn position(st: &AppState, tab: &DeckTab, k: usize) -> usize {
    bay(st, tab)
        .get(k)
        .and_then(|item| st.character.loaded.iter().position(|l| *l == item.id))
        .unwrap_or(st.character.loaded.len())
}

/// The plan for dropping `id` at bay stop `k`, and the slot it starts in once the bay
/// packs (the stop's own slot when it cannot land).
fn landing(st: &AppState, tab: &DeckTab, id: &ItemId, k: usize) -> (Result<SlotPlan, String>, u8) {
    let plan = st
        .character
        .plan_slot(&st.content, id, position(st, tab, k));
    let start = match &plan {
        Ok(plan) => plan.loaded[..plan.at]
            .iter()
            .filter_map(|p| st.content.item(p))
            .map(width)
            .sum(),
        Err(_) => bay(st, tab).into_iter().take(k).map(width).sum(),
    };
    (plan, start)
}

fn cart(item: &Item) -> Cart {
    Cart {
        name: item.name.to_uppercase(),
        rating: item.program.map_or(0, |p| p.rating),
        size: item.program.map_or(0, |p| p.size),
    }
}

/// The slots a program's cart spans in the drawing.
fn width(item: &Item) -> u8 {
    item.program.map_or(0, |p| p.size).max(1)
}

/// The letter of physical slot `start`.
fn letter(start: u8) -> char {
    char::from(b'A'.saturating_add(start))
}

/// An item's name, its id when the content does not know it.
fn name(st: &AppState, id: &ItemId) -> String {
    st.content
        .item(id)
        .map_or_else(|| id.to_string(), |i| i.name.clone())
}

/// The name in capitals, as the deck labels carts.
fn upper(st: &AppState, id: &ItemId) -> String {
    name(st, id).to_uppercase()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::{AppState, ScreenCmd};
    use crate::messages::Kind;
    use crate::ui::cyberdeck::{GlassTone, Part, Switch, parts};
    use citygen::{GenParams, Generator, Place, Pos, SizePreset, StratumKind};
    use crossterm::event::KeyCode;
    use game::{DeckSpec, ItemId, ItemKind};

    fn state() -> AppState {
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
        st
    }

    fn last(st: &AppState) -> Option<&str> {
        st.log.latest().map(|e| e.text.as_str())
    }

    fn last_is_warning(st: &AppState) -> bool {
        st.log.latest().is_some_and(|e| e.kind == Kind::Warn)
    }

    fn id(s: &str) -> ItemId {
        ItemId::new(s)
    }

    fn texts(body: &[(String, GlassTone)]) -> Vec<&str> {
        body.iter().map(|(t, _)| t.as_str()).collect()
    }

    fn inv_of(st: &AppState, item: &str) -> Part {
        Part::Inv(inventory(st).iter().position(|i| *i == id(item)).unwrap())
    }

    /// The blueprint of the jacked pawnshop deck.
    fn jacked(st: &AppState) -> DeckTab {
        DeckTab::for_deck(0, id("pawnshop_deck"), true, st)
    }

    fn space(tab: &mut DeckTab, st: &mut AppState) -> ScreenCmd {
        tab.key(st, KeyCode::Char(' '))
    }

    /// The pawnshop deck cloned under a new id with `slots`, and equipped.
    fn equip_deck(st: &mut AppState, slots: u8) {
        let mut deck = st.content.item(&id("pawnshop_deck")).unwrap().clone();
        deck.id = id("test_deck");
        deck.deck = Some(DeckSpec {
            slots,
            ..deck.deck.unwrap()
        });
        st.content.items.push(deck);
        st.character
            .equipment
            .insert(game::Slot::Deck, id("test_deck"));
    }

    #[test]
    fn glass_reads_the_pointed_program() {
        let st = state();
        let tab = jacked(&st);
        let (body, pinned) = glass(&st, &tab);
        assert_eq!(body[0], ("SLOT A ▸ CRACK".into(), GlassTone::Head));
        assert_eq!(body[1].0, "rating 1 · size 1 · noise 4 · work 3");
        assert_eq!(
            body[2].0,
            "Brute force with manners. Opens anything that answers, loudly."
        );
        let rest = texts(&body[3..]);
        assert!(rest.contains(&"crack · any node or ICE"), "{rest:?}");
        assert!(
            rest.iter()
                .any(|t| t.starts_with("your power 19 vs 13 mid-city")),
            "{rest:?}"
        );
        assert!(rest.contains(&"loaded"), "{rest:?}");
        assert_eq!(pinned.0, "[space] eject");
    }

    #[test]
    fn glass_reads_the_failsafe_three_ways() {
        let mut st = state();
        let mut tab = jacked(&st);
        tab.point = Part::Failsafe;
        let (body, pinned) = glass(&st, &tab);
        assert_eq!(body[0], ("FAILSAFE ▸ ON".into(), GlassTone::Head));
        assert!(
            body.iter()
                .any(|(t, _)| t.contains("pulls you out at a quarter of your hp")),
            "{body:?}"
        );
        assert_eq!(pinned.0, "[space] lift the cover");

        tab.cover_up = true;
        let (body, pinned) = glass(&st, &tab);
        assert_eq!(body[0].0, "FAILSAFE ▸ ON · cover up");
        assert!(texts(&body).contains(&"Throw it and black ICE can kill you."));
        assert_eq!(pinned.0, "[space] throw · esc shut");

        st.net.failsafe = false;
        let (body, pinned) = glass(&st, &tab);
        assert_eq!(body[0], ("FAILSAFE ▸ OFF".into(), GlassTone::Warn));
        assert_eq!(pinned.0, "[space] throw it back");
    }

    #[test]
    fn glass_reads_rom_and_plate() {
        let mut st = state();
        let mut tab = jacked(&st);
        tab.point = Part::Rom(0);
        let (body, pinned) = glass(&st, &tab);
        assert_eq!(body[0], ("ROM ▸ NETSCAN".into(), GlassTone::Head));
        let all = texts(&body).join("\n");
        assert!(all.contains("within 12 tiles"), "{all}");
        assert!(all.contains("cooldown 10s"), "{all}");
        assert!(all.contains("from Pawnshop deck"), "{all}");
        assert_eq!(
            pinned,
            ("burned in · can't be pulled".into(), GlassTone::Dim)
        );

        // Talents sharpen what the ROM reads.
        for t in ["wide_band", "ice_pick"] {
            st.character.talents.insert(game::TalentId::new(t), 2);
        }
        let all = texts(&glass(&st, &tab).0).join("\n");
        assert!(all.contains("cooldown 8s"), "{all}");
        assert!(all.contains("18 tiles"), "{all}");

        tab.point = Part::Plate;
        let (body, pinned) = glass(&st, &tab);
        assert_eq!(body[0].0, "PAWNSHOP DECK");
        assert!(
            texts(&body).contains(
                &"Three previous owners, one of them dead. Boots in four seconds and runs hot."
            ),
            "{body:?}"
        );
        assert_eq!(pinned.0, "2/2 slots · rating 1");
        // The ROM strips carry the abilities' names.
        assert_eq!(tab.view(&st).roms, vec!["NETSCAN", "ARC TRACE"]);
    }

    #[test]
    fn glass_for_inventory_says_what_it_needs() {
        let mut st = state();
        st.character.give(id("backdoor"), 1);
        let mut tab = jacked(&st);
        tab.point = inv_of(&st, "backdoor");
        let (body, pinned) = glass(&st, &tab);
        assert_eq!(body[0].0, "BACKDOOR");
        assert!(texts(&body).contains(&"needs 2 slots · 0 free"), "{body:?}");
        assert_eq!(pinned.0, "[space] pick up");

        // A deck too small for it says so before it is picked up, and refuses it.
        equip_deck(&mut st, 1);
        let mut tab = DeckTab::for_deck(0, id("test_deck"), true, &st);
        tab.point = inv_of(&st, "backdoor");
        let (body, pinned) = glass(&st, &tab);
        assert!(
            texts(&body).contains(&"needs 2 slots · this deck has 1"),
            "{body:?}"
        );
        assert_eq!(pinned, ("too big for this deck".into(), GlassTone::Warn));
        assert!(matches!(space(&mut tab, &mut st), ScreenCmd::Stay));
        assert_eq!(last(&st), Some("too big for this deck: needs 2 of 1 slots"));
        assert!(last_is_warning(&st));
        assert_eq!(tab.held, None);
    }

    #[test]
    fn space_ejects_and_the_bay_packs_left() {
        let mut st = state();
        let mut tab = jacked(&st);
        assert!(matches!(space(&mut tab, &mut st), ScreenCmd::Stay));
        assert_eq!(st.character.loaded, vec![id("copy")]);
        assert_eq!(last(&st), Some("Crack unloaded"));
        assert!(inventory(&st).contains(&id("crack")));
        // Copy slid into A; B is the empty stop now.
        assert_eq!(glass(&st, &tab).0[0].0, "SLOT A ▸ COPY");
        tab.key(&mut st, KeyCode::Right);
        assert_eq!(tab.point, Part::Slot(1));
        let (_, pinned) = glass(&st, &tab);
        assert_eq!(pinned.0, "empty · bring a program from inventory");
        space(&mut tab, &mut st);
        assert_eq!(st.character.loaded, vec![id("copy")]);
        assert_eq!(tab.held, None);
        assert_eq!(last(&st), Some("Crack unloaded"));
    }

    #[test]
    fn pick_up_carry_drop_swaps() {
        let mut st = state();
        let mut tab = jacked(&st);
        let from = inv_of(&st, "attack");
        tab.point = from;
        space(&mut tab, &mut st);
        assert_eq!(tab.held, Some(id("attack")));
        assert_eq!(tab.point, Part::Slot(0), "the bay is full: the first cart");
        assert_eq!(tab.hint(), "↑↓←→ carry · space drop · esc put back");
        tab.key(&mut st, KeyCode::Right);
        assert_eq!(tab.point, Part::Slot(1));
        tab.key(&mut st, KeyCode::Down);
        assert_eq!(tab.point, Part::Slot(1), "only the bay while holding");
        let (body, pinned) = glass(&st, &tab);
        assert_eq!(body[0].0, "ATTACK ⇄ COPY");
        assert!(
            texts(&body).contains(&"Attack goes in slot B, Copy back to inventory."),
            "{body:?}"
        );
        assert_eq!(pinned.0, "[space] swap · esc put back");
        let held = tab.view(&st).held.unwrap();
        assert_eq!(
            (Part::Inv(held.from), held.start, held.letter),
            (from, 1, 'B')
        );

        space(&mut tab, &mut st);
        assert_eq!(st.character.loaded, vec![id("crack"), id("attack")]);
        assert_eq!(tab.held, None);
        assert_eq!(last(&st), Some("Attack slotted · Copy to inventory"));
        assert_eq!(tab.point, Part::Slot(1));
        assert_eq!(tab.hint(), "↑↓←→ point · space act · esc close");
    }

    #[test]
    fn the_glass_words_every_landing() {
        let mut st = state();
        st.character.give(id("backdoor"), 1);
        st.character.give(id("spoof"), 1);
        let mut tab = jacked(&st);
        // A size-2 cart over two size-1 carts sends both back.
        tab.point = inv_of(&st, "backdoor");
        space(&mut tab, &mut st);
        assert_eq!(tab.point, Part::Slot(0));
        let (body, _) = glass(&st, &tab);
        assert_eq!(body[0].0, "BACKDOOR ⇄ CRACK, COPY");
        assert!(
            texts(&body).contains(&"Backdoor goes in slot A, Crack and Copy back to inventory."),
            "{body:?}"
        );
        tab.key(&mut st, KeyCode::Esc);

        // Into the empty slot, nothing goes back.
        st.character.loaded.pop();
        tab.point = inv_of(&st, "spoof");
        space(&mut tab, &mut st);
        assert_eq!(tab.point, Part::Slot(1), "the first empty slot");
        let (body, pinned) = glass(&st, &tab);
        assert_eq!(body[0].0, "SPOOF ▸ B");
        assert!(texts(&body).contains(&"Spoof goes in slot B."), "{body:?}");
        assert_eq!(pinned.0, "[space] slot · esc put back");
        space(&mut tab, &mut st);
        assert_eq!(last(&st), Some("Spoof loaded · 2/2 slots"));
    }

    #[test]
    fn the_failsafe_takes_two_presses_to_turn_off() {
        let mut st = state();
        let mut tab = jacked(&st);
        tab.point = Part::Failsafe;
        space(&mut tab, &mut st);
        assert!(tab.cover_up && st.net.failsafe);
        assert_eq!(tab.view(&st).switch, Switch::Lifted);
        assert!(matches!(tab.key(&mut st, KeyCode::Esc), ScreenCmd::Stay));
        assert!(!tab.cover_up);
        assert_eq!(tab.view(&st).switch, Switch::On);

        space(&mut tab, &mut st);
        space(&mut tab, &mut st);
        assert!(!st.net.failsafe);
        assert_eq!(last(&st), Some("failsafe off · black ICE can kill you"));
        assert!(last_is_warning(&st));
        assert_eq!(tab.view(&st).switch, Switch::Off);
        space(&mut tab, &mut st);
        assert!(st.net.failsafe && !tab.cover_up);
        assert_eq!(last(&st), Some("failsafe on"));

        // Moving away shuts a lifted cover.
        space(&mut tab, &mut st);
        assert!(tab.cover_up);
        tab.key(&mut st, KeyCode::Left);
        assert_ne!(tab.point, Part::Failsafe);
        assert!(!tab.cover_up && st.net.failsafe);
    }

    #[test]
    fn view_reports_trace_link_and_switch() {
        let mut st = state();
        let tab = jacked(&st);
        let v = tab.view(&st);
        assert_eq!((v.trace, v.link, v.switch), (None, false, Switch::On));
        assert_eq!(trace(&st), None);

        jack_in(&mut st);
        let b = st.city.as_ref().unwrap().buildings[0].id;
        st.net.trace(b).heat = 60;
        let v = tab.view(&st);
        assert_eq!((v.trace, v.link), (Some(60), true));
        assert_eq!(v.handle, st.character.handle);

        // The whole glass types on in the detail pane's reveal time, however long.
        let total = |v: &DeckView| -> usize {
            v.glass
                .iter()
                .chain([&v.pinned])
                .map(|(t, _)| t.chars().count())
                .sum()
        };
        st.tick = 0;
        assert_eq!(tab.view(&st).typed, 0);
        st.tick = REVEAL_TICKS / 2;
        let v = tab.view(&st);
        assert!(total(&v) > 0);
        assert_eq!(v.typed, total(&v) / 2);
        for tick in [REVEAL_TICKS, 10_000] {
            st.tick = tick;
            let v = tab.view(&st);
            assert_eq!(v.typed, total(&v));
        }
    }

    /// Jacks the player into the construct of the first building, body on the street.
    fn jack_in(st: &mut AppState) {
        let b = st.city.as_ref().unwrap().buildings[0].id;
        let mut p = crate::walker::Player::at(Place::Construct(b), Pos::new(1, 1));
        p.body = Some((Place::Stratum(StratumKind::Street), Pos::new(1, 1)));
        st.player = Some(p);
    }

    #[test]
    fn jacked_in_the_deck_is_locked() {
        let mut st = state();
        let mut tab = jacked(&st);
        let out = tab.view(&st);
        jack_in(&mut st);
        let v = tab.view(&st);
        assert_eq!(v.pinned, ("locked · jacked in".into(), GlassTone::Warn));
        assert_eq!(v.glass, out.glass);

        for code in [KeyCode::Char(' '), KeyCode::Enter] {
            assert!(matches!(tab.key(&mut st, code), ScreenCmd::Stay));
            assert_eq!(st.character.loaded, vec![id("crack"), id("copy")]);
            assert_eq!(last(&st), Some("the deck is locked while you're jacked in"));
            assert!(last_is_warning(&st));
        }

        st.character.give(id("spoof"), 1);
        tab.point = inv_of(&st, "spoof");
        assert_eq!(tab.view(&st).pinned.0, "locked · jacked in");
        space(&mut tab, &mut st);
        assert_eq!(tab.held, None);

        tab.point = Part::Failsafe;
        assert_eq!(tab.view(&st).pinned.0, "locked · jacked in");
        space(&mut tab, &mut st);
        assert!(!tab.cover_up && st.net.failsafe);

        // Pointing, scrolling and closing still work.
        tab.point = Part::Slot(0);
        tab.key(&mut st, KeyCode::Right);
        assert_eq!(tab.point, Part::Slot(1));
        tab.point = Part::Rom(1);
        tab.key(&mut st, KeyCode::PageDown);
        assert_eq!(tab.scroll, 1);
        assert!(matches!(
            tab.key(&mut st, KeyCode::Esc),
            ScreenCmd::Pop { .. }
        ));
    }

    #[test]
    fn jacking_out_unlocks_the_deck() {
        let mut st = state();
        let mut tab = jacked(&st);
        jack_in(&mut st);
        space(&mut tab, &mut st);
        assert_eq!(st.character.loaded, vec![id("crack"), id("copy")]);
        st.player.as_mut().unwrap().body = None;
        assert_eq!(tab.view(&st).pinned.0, "[space] eject");
        space(&mut tab, &mut st);
        assert_eq!(st.character.loaded, vec![id("copy")]);
    }

    #[test]
    fn a_carried_deck_reads_but_is_not_jacked() {
        let mut st = state();
        st.character.give(id("rebuilt_deck"), 1);
        jack_in(&mut st);
        let b = st.city.as_ref().unwrap().buildings[0].id;
        st.net.trace(b).heat = 60;
        let mut tab = DeckTab::for_deck(0, id("rebuilt_deck"), false, &st);
        // Its own slots, rating and ROM; an empty bay, no trace, no link, though the
        // jacked deck has all three.
        let v = tab.view(&st);
        assert_eq!((v.slots, v.rating), (5, 3));
        assert!(v.bay.is_empty());
        assert_eq!(v.roms, vec!["NETSCAN", "ARC TRACE"]);
        assert_eq!((v.trace, v.link), (None, false));
        assert_eq!(v.point, Part::Slot(0));
        assert_eq!(
            v.pinned,
            ("not jacked · equip it first".into(), GlassTone::Warn)
        );
        let (body, _) = glass(&st, &tab);
        assert_eq!(texts(&body), vec!["SLOT A ▸ EMPTY", "5 of 5 slots free"]);
        tab.point = Part::Rom(0);
        let all = texts(&glass(&st, &tab).0).join("\n");
        assert!(all.contains("from Rebuilt pawn deck"), "{all}");
        tab.point = inv_of(&st, "attack");
        assert!(
            texts(&glass(&st, &tab).0).contains(&"needs 1 slot · 5 free"),
            "the bay is empty"
        );
    }

    #[test]
    fn pgdn_scrolls_the_glass() {
        let mut st = state();
        let mut tab = jacked(&st);
        tab.point = Part::Rom(1);
        let v = tab.view(&st);
        assert_eq!(v.glass[0].0, "ROM ▸ ARC TRACE");
        assert!(v.more_below && !v.more_above);
        tab.key(&mut st, KeyCode::PageDown);
        assert_eq!(tab.scroll, 1);
        let v = tab.view(&st);
        assert!(v.more_above);
        assert_eq!(v.glass.len(), 5);
        tab.key(&mut st, KeyCode::PageUp);
        assert_eq!(tab.scroll, 0);
        // It stops at the last row.
        for _ in 0..30 {
            tab.key(&mut st, KeyCode::PageDown);
        }
        assert!(!tab.view(&st).more_below);
        tab.key(&mut st, KeyCode::Left);
        assert_eq!(tab.point, Part::Rom(0));
        assert_eq!(tab.scroll, 0);
    }

    #[test]
    fn inventory_scrolls_with_the_point() {
        let mut st = state();
        let programs: Vec<game::Item> = st
            .content
            .items
            .iter()
            .filter(|i| i.kind == ItemKind::Program)
            .cloned()
            .collect();
        for n in 0..8 {
            let mut p = programs[n % programs.len()].clone();
            p.id = ItemId::new(&format!("extra_{n}"));
            st.character.give(p.id.clone(), 1);
            st.content.items.push(p);
        }
        let n = inventory(&st).len();
        assert_eq!(n, 9);
        let mut tab = jacked(&st);
        tab.point = Part::Inv(n - 1);
        let v = tab.view(&st);
        assert!(v.inv_offset > 0);
        assert!(parts(&v).iter().any(|(p, _)| *p == Part::Inv(n - 1)));

        // The arrows walk the row past its edge, and back.
        tab.point = Part::Inv(0);
        for _ in 1..n {
            tab.key(&mut st, KeyCode::Right);
        }
        assert_eq!(tab.point, Part::Inv(n - 1));
        for _ in 1..n {
            tab.key(&mut st, KeyCode::Char('h'));
        }
        assert_eq!(tab.point, Part::Inv(0));
        assert_eq!(tab.view(&st).inv_offset, 0);
    }

    #[test]
    fn a_stale_point_clamps() {
        let mut st = state();
        st.deck_part = Part::Inv(9).pack();
        let tab = jacked(&st);
        assert!(
            parts(&tab.view(&st)).iter().any(|(p, _)| *p == tab.point),
            "{:?}",
            tab.point
        );
        st.deck_part = Part::Failsafe.pack();
        assert_eq!(jacked(&st).pack(), 300);
    }
}
