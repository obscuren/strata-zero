//! MASTIFF: the black ICE coiled round a black credchip in your rig's reader. Woken from
//! the chip's popup it puts the room on the Net's turns (you, each free core seat, then
//! MASTIFF) and fights on its leash round the slot: eight plates of coat round a core
//! that takes two hits, two tiles a turn at you, a bite marked one turn and bitten the
//! next. It lives on the room (`RigRoom.mastiff`), which isn't saved: a run reopened finds
//! it asleep and whole. Inside your own firewall nothing traces you: no heat, no trace,
//! no tracer.

use super::layout::front;
use super::{RigRoom, room};
use crate::app::AppState;
use crate::net::{Actor, Step};
use citygen::{Construct, Dir, IceGrade, Place, Pos, Rect};
use game::{ItemId, Target};

/// MASTIFF out of its slot: awake, or grit once its core broke.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Mastiff {
    /// The reader slot whose black chip it guards.
    pub slot: usize,
    /// The top-left of its 3x3.
    pub pos: Pos,
    /// Its coat, true for a whole plate, clockwise from the top-left as drawn:
    /// `▛ ▀ ▜ ▐ ▟ ▄ ▙ ▌`.
    pub plates: [bool; 8],
    /// Hits its core takes before it goes to grit; 0 is grit.
    pub core: u8,
    /// The 3x3 it bites on its next turn, once marked.
    pub bite: Option<Rect>,
    /// The ground it keeps to: the strip in front of its slot (`leash_of`).
    pub leash: Rect,
}

impl Mastiff {
    /// The 3x3 it stands on.
    pub fn body(&self) -> Rect {
        Rect::new(self.pos.x, self.pos.y, 3, 3)
    }

    /// Its whole plates.
    pub fn coat(&self) -> usize {
        self.plates.iter().filter(|w| **w).count()
    }
}

/// The security behind MASTIFF's coat: what your Attack rolls against.
pub const MASTIFF_SECURITY: u8 = 180;
/// What its bite takes from your body before the deck and your feedback resistance.
pub const BITE: u32 = 7;
/// Hits its core takes once the coat is gone.
const CORE: u8 = 2;
/// Tiles it walks a turn, toward you.
pub const MOVES: u8 = 2;
/// Cells its leash reaches past its slot's box: west and east (short of the next slots'
/// boxes, 2 columns off), above it (short of the READER title) and below it.
const LEASH_WEST: i32 = 2;
const LEASH_EAST: i32 = 1;
const LEASH_NORTH: i32 = 2;
const LEASH_SOUTH: i32 = 10;
/// The best a seat's beam does, in percent.
const SEAT_CAP: u32 = 95;
/// Each plate's tile in its 3x3, in the coat's order.
pub const PLATE_AT: [(i32, i32); 8] = [
    (0, 0),
    (1, 0),
    (2, 0),
    (2, 1),
    (2, 2),
    (1, 2),
    (0, 2),
    (0, 1),
];
/// Why your Attack and the seats don't reach it.
const OFF_THE_LEASH: &str = "off the leash · out of reach";
/// What killed you, with the deck's failsafe off.
const DEATH: &str = "MASTIFF · in your own rig";

/// MASTIFF out of its slot, its core unbroken.
fn up(st: &AppState) -> Option<&Mastiff> {
    room(st)?.mastiff.as_ref().filter(|m| m.core > 0)
}

fn up_mut(st: &mut AppState) -> Option<&mut Mastiff> {
    (st.constructs.rig.as_mut()?.mastiff.as_mut()).filter(|m| m.core > 0)
}

/// MASTIFF is awake: out of its slot, its core unbroken, the room on its turns.
pub fn awake(st: &AppState) -> bool {
    up(st).is_some() && (st.net.engaged.as_ref()).is_some_and(|e| e.place == Place::Rig)
}

/// MASTIFF is awake off the chip in reader slot `slot`.
pub fn awake_on(st: &AppState, slot: usize) -> bool {
    awake(st) && up(st).is_some_and(|m| m.slot == slot)
}

/// The free core seats in the fight, in seat order: each runs your Attack at MASTIFF on
/// its turn.
pub fn fighting_seats(st: &AppState) -> Vec<u8> {
    let Some(e) = st.net.engaged.as_ref().filter(|_| awake(st)) else {
        return Vec::new();
    };
    (e.order.iter())
        .filter_map(|a| match a {
            Actor::Seat(n) => Some(*n),
            _ => None,
        })
        .collect()
}

/// The seat whose beam is out this frame: on its turn, when it has a shot at MASTIFF.
pub fn beam(st: &AppState) -> Option<u8> {
    match st.net.engaged.as_ref()?.actor() {
        Actor::Seat(n) if awake(st) && seat_chance(st, n).is_ok() => Some(n),
        _ => None,
    }
}

/// Where you stand in the room.
fn you(st: &AppState) -> Option<Pos> {
    (st.player.as_ref())
        .filter(|p| p.place == Place::Rig)
        .map(|p| p.pos)
}

/// You stand on MASTIFF's leash, where it and you reach each other.
fn on_the_leash(st: &AppState) -> bool {
    up(st)
        .zip(you(st))
        .is_some_and(|(m, at)| m.leash.contains(at))
}

/// A floor tile of the room.
fn floor(k: &Construct, p: Pos) -> bool {
    k.tiles.get(p).is_some_and(|t| t.is_walkable())
}

/// Why MASTIFF in slot `slot` won't wake now, as the chip's row says it.
pub fn refusal(st: &AppState, slot: usize) -> Option<&'static str> {
    if you(st).is_none() {
        return Some("only in your rig's Net");
    }
    let black = (st.character.reader.get(slot))
        .and_then(|s| s.as_ref())
        .is_some_and(|s| s.item.0 == "credchip_black");
    if !black || room(st).is_none_or(|r| r.slots.get(slot).is_none()) {
        return Some("no black chip there");
    }
    if awake(st) {
        return Some("MASTIFF is awake");
    }
    st.net.engaged.as_ref().map(|_| "not in a fight")
}

/// Wakes the MASTIFF coiled round the black chip in `slot`: it leaves the slot for the
/// tile in front of it (you step aside if you stood there) and the room goes on turns,
/// you first, then each free core seat, then MASTIFF (`order`). Refused, nothing changes.
pub fn wake(st: &mut AppState, slot: usize) -> Result<(), String> {
    if let Some(why) = refusal(st, slot) {
        return Err(why.to_string());
    }
    let room = st.constructs.rig.as_mut().ok_or("no rig here")?;
    let leash = leash_of(room, slot).ok_or("no black chip there")?;
    let at = front(room.slots[slot]);
    room.mastiff = Some(Mastiff {
        slot,
        pos: Pos::new(at.x - 1, at.y),
        plates: [true; 8],
        core: CORE,
        bite: None,
        leash,
    });
    step_aside(st);
    crate::net::engage(st, Place::Rig, false);
    Ok(())
}

/// The ground MASTIFF of slot `slot` keeps to, asleep or awake, as the mock draws it: a
/// strip in front of its slot, from 2 columns west of the box to 1 east and from 2 rows
/// above it to 10 below, clipped to the hall. The slots stand 2 columns apart, so it
/// never takes in another slot's box.
pub fn leash_of(room: &RigRoom, slot: usize) -> Option<Rect> {
    let r = *room.slots.get(slot)?;
    let strip = Rect::new(
        r.x - LEASH_WEST,
        r.y - LEASH_NORTH,
        r.w + LEASH_WEST + LEASH_EAST,
        r.h + LEASH_NORTH + LEASH_SOUTH,
    );
    Some(strip.intersection(room.hall).unwrap_or(r))
}

/// You out from under MASTIFF where it lands: onto the nearest floor beside it.
fn step_aside(st: &mut AppState) {
    let (Some(me), Some(room)) = (you(st), room(st)) else {
        return;
    };
    let Some(body) = room.mastiff.as_ref().map(Mastiff::body) else {
        return;
    };
    if !body.contains(me) {
        return;
    }
    let to = (body.grow(3).iter())
        .filter(|p| !body.contains(*p) && floor(&room.construct, *p))
        .min_by_key(|p| (p.chebyshev(me), p.y, p.x));
    if let (Some(to), Some(p)) = (to, st.player.as_mut()) {
        p.pos = to;
    }
}

/// The order of a fight in the room: you; with MASTIFF out, each free core seat in seat
/// order while there's an Attack to run, then MASTIFF.
pub fn order(st: &AppState) -> Vec<Actor> {
    let mut out = vec![Actor::You];
    if up(st).is_none() {
        return out;
    }
    if odds(st).is_ok() {
        let seats = room(st).map_or(0, |r| r.seats.len());
        let busy = st.backdoors.jobs.running.len();
        out.extend(
            (busy..seats)
                .filter_map(|s| u8::try_from(s).ok())
                .map(Actor::Seat),
        );
    }
    out.push(Actor::Mastiff);
    out
}

/// One action of MASTIFF's turn, paced as a piece of ICE's (`net::pace`): a tile toward
/// you while it has moves and a step closer keeps it on its leash; else, with its action
/// left, a bite marked (the 3x3 beside it on your side, or round you where it can't get
/// beside you: `mark`) or, with one marked, bitten: on
/// its tiles you take `BITE` as a black ICE hit (`net::black_hit`: Shield soaks it, the
/// failsafe pulls you out first, with it off it can kill you), and the marks clear.
pub fn step(st: &mut AppState, can_move: bool, can_act: bool) -> Step {
    let Some(me) = you(st) else {
        return Step::Nothing;
    };
    let Some(room) = st.constructs.rig.as_mut() else {
        return Step::Nothing;
    };
    let k = &room.construct;
    let Some(m) = room.mastiff.as_mut().filter(|m| m.core > 0) else {
        return Step::Nothing;
    };
    if can_move && let Some(to) = step_toward(k, m, me) {
        m.pos = to;
        return Step::Moved(to);
    }
    if !can_act {
        return Step::Nothing;
    }
    let from = m.body().center();
    match m.bite.take() {
        None => {
            m.bite = mark(k, m, me);
            if m.bite.is_some() {
                st.warn("MASTIFF bites the × tiles next turn");
            }
        }
        Some(bite) if bite.contains(me) => {
            crate::net::black_hit(st, BITE, from, "MASTIFF bites", DEATH);
        }
        Some(_) => st.say("MASTIFF bites · nothing there"),
    }
    Step::Hit
}

/// MASTIFF's whole turn at once: its steps, then its mark or bite.
#[cfg(test)]
pub(crate) fn turn(st: &mut AppState) {
    let (mut moves, mut act) = (MOVES, true);
    loop {
        match step(st, moves > 0, act) {
            Step::Moved(_) => moves -= 1,
            Step::Hit => act = false,
            Step::Nothing => return,
        }
        if moves == 0 && !act {
            return;
        }
    }
}

/// Where MASTIFF's next step takes it toward you: its 3x3 kept on the leash and the
/// floor, never on you; none once it stands beside you or no step brings it closer.
fn step_toward(k: &Construct, m: &Mastiff, me: Pos) -> Option<Pos> {
    let key = |p: Pos| {
        let r = Rect::new(p.x, p.y, 3, 3);
        let c = r.center();
        (gap(r, me), (c.x - me.x).pow(2) + (c.y - me.y).pow(2))
    };
    let now = key(m.pos);
    if now.0 <= 1 {
        return None;
    }
    (Dir::ALL.iter())
        .map(|d| {
            let (dx, dy) = d.delta();
            Pos::new(m.pos.x + dx, m.pos.y + dy)
        })
        .filter(|p| {
            let r = Rect::new(p.x, p.y, 3, 3);
            !r.contains(me) && r.iter().all(|q| m.leash.contains(q) && floor(k, q))
        })
        .map(|p| (key(p), p))
        .filter(|(by, _)| *by < now)
        .min_by_key(|(by, _)| *by)
        .map(|(_, p)| p)
}

/// The steps from a box to a tile: 0 inside it, 1 beside it.
fn gap(r: Rect, p: Pos) -> i32 {
    let dx = (r.x - p.x).max(p.x - (r.right() - 1)).max(0);
    let dy = (r.y - p.y).max(p.y - (r.bottom() - 1)).max(0);
    dx.max(dy)
}

/// What MASTIFF marks to bite with you at `me`: the 3x3 beside it on your side, on its
/// leash. With you on its leash where no ground it can stand on puts you in that (beside
/// or above its slot's box, where its 3x3 doesn't fit), it lunges: the 3x3 round you, on
/// its leash.
fn mark(k: &Construct, m: &Mastiff, me: Pos) -> Option<Rect> {
    if m.leash.contains(me) && !in_reach(k, m.leash, me) {
        return Rect::new(me.x - 1, me.y - 1, 3, 3).intersection(m.leash);
    }
    beside(m.pos, m.leash, me)
}

/// Some ground on `leash` MASTIFF can stand on (its 3x3 on the leash and the floor, off
/// you) puts you at `me` in the bite it marks beside it.
fn in_reach(k: &Construct, leash: Rect, me: Pos) -> bool {
    let xs = leash.x..=leash.right() - 3;
    let ys = leash.y..=leash.bottom() - 3;
    ys.flat_map(|y| xs.clone().map(move |x| Pos::new(x, y)))
        .filter(|p| {
            let r = Rect::new(p.x, p.y, 3, 3);
            !r.contains(me) && r.iter().all(|q| floor(k, q))
        })
        .any(|p| beside(p, leash, me).is_some_and(|b| b.contains(me)))
}

/// The 3x3 beside MASTIFF's 3x3 at `pos` on your side, on its leash.
fn beside(pos: Pos, leash: Rect, me: Pos) -> Option<Rect> {
    let side = |lo: i32, at: i32| {
        if at < lo {
            -1
        } else if at > lo + 2 {
            1
        } else {
            0
        }
    };
    let (dx, dy) = (side(pos.x, me.x), side(pos.y, me.y));
    if (dx, dy) == (0, 0) {
        return None;
    }
    Rect::new(pos.x + 3 * dx, pos.y + 3 * dy, 3, 3).intersection(leash)
}

/// A hit on MASTIFF from `from`: the whole plate nearest it breaks, or with the coat gone
/// its core takes one. The line it makes, for the one who hit it to say. The core's
/// second says its own (`MASTIFF · to grit · the chip opens · ¤4,800`, as a cracked
/// chip's last lock does), leaves grit where it stood, opens the chip it guarded into a
/// `credchip` with its sum and ends the turns: none then.
pub fn hit(st: &mut AppState, from: Pos) -> Option<String> {
    let Some(m) = up_mut(st) else {
        return Some("nothing there".to_string());
    };
    let pos = m.pos;
    let plate = (0..PLATE_AT.len())
        .filter(|i| m.plates[*i])
        .min_by_key(|i| {
            let (dx, dy) = PLATE_AT[*i];
            (pos.x + dx - from.x).pow(2) + (pos.y + dy - from.y).pow(2)
        });
    if let Some(i) = plate {
        m.plates[i] = false;
        return Some(format!("MASTIFF · a plate breaks · coat {}/8", m.coat()));
    }
    m.core -= 1;
    if m.core > 0 {
        return Some(format!("MASTIFF · its core cracks · {} hit left", m.core));
    }
    m.bite = None;
    let slot = m.slot;
    let sum = match st.character.reader.get_mut(slot).and_then(|s| s.as_mut()) {
        Some(chip) if chip.item.0 == "credchip_black" => {
            chip.item = ItemId::new("credchip");
            chip.copy.as_ref().map_or(0, |k| k.credits)
        }
        _ => 0,
    };
    st.say(format!(
        "MASTIFF · to grit · the chip opens · ¤{}",
        super::crack::commas(sum)
    ));
    crate::net::disengage(st);
    None
}

/// A free core seat's turn: your loaded Attack at MASTIFF from the seat's box, at your
/// chance scaled by the core's speed. It sits out while you're off the leash, while a
/// job holds the seat, or with no Attack loaded.
pub fn seat_turn(st: &mut AppState, seat: u8) {
    let Ok(chance) = seat_chance(st, seat) else {
        return;
    };
    let Some(from) = (room(st).and_then(|r| r.seats.get(usize::from(seat))))
        .map(|b| Pos::new(b.x, b.y + b.h / 2))
    else {
        return;
    };
    let line = if game::round_hits(crate::combat::roll(st), chance) {
        hit(st, from)
    } else {
        Some(miss(st).to_string())
    };
    if let Some(line) = line {
        st.say(format!("seat {} · {line}", seat + 1));
    }
}

/// What a miss at MASTIFF says: its coat holds, or with the coat gone its core.
pub fn miss(st: &AppState) -> &'static str {
    match up(st) {
        Some(m) if m.coat() == 0 => "the core holds",
        _ => "the coat holds",
    }
}

/// A seat's chance at MASTIFF: `seat_odds`, while you stand on the leash and no job
/// holds the seat; or why it sits out.
fn seat_chance(st: &AppState, seat: u8) -> Result<u8, String> {
    if !on_the_leash(st) {
        return Err(OFF_THE_LEASH.to_string());
    }
    if st.backdoors.jobs.running.len() > usize::from(seat) {
        return Err("the seat is busy".to_string());
    }
    seat_odds(st)
}

/// A free seat's odds at MASTIFF: yours, times the core's speed over 100, at most 95.
pub fn seat_odds(st: &AppState) -> Result<u8, String> {
    let chance = u32::from(odds(st)?) * u32::from(crate::rig::of(st).speed) / 100;
    Ok(chance.min(SEAT_CAP) as u8)
}

/// Your loaded Attack's chance at MASTIFF behind `MASTIFF_SECURITY`, wherever you stand:
/// refused with no deck jacked or no Attack loaded.
fn odds(st: &AppState) -> Result<u8, String> {
    if st.character.deck(&st.content).is_none() {
        return Err("no deck jacked".to_string());
    }
    let spec = crate::combat::loaded_attack(st)
        .and_then(|(id, _)| st.content.item(&id)?.program)
        .ok_or("no attack loaded")?;
    Ok(game::chance(
        game::power(
            &st.character,
            &st.content,
            &spec,
            Target::Guard(IceGrade::Black),
        ),
        game::difficulty(MASTIFF_SECURITY),
    ))
}

/// Your chance a turn of your loaded Attack at MASTIFF, in percent: refused with nothing
/// to run it, off its leash, or past the Net's attack reach.
pub fn attack_chance(st: &AppState) -> Result<u8, String> {
    let chance = odds(st)?;
    let (Some(m), Some(me)) = (up(st), you(st)) else {
        return Err("MASTIFF sleeps".to_string());
    };
    if !m.leash.contains(me) {
        return Err(OFF_THE_LEASH.to_string());
    }
    let at = nearest(m, me);
    let d = me.chebyshev(at);
    let clear = room(st).is_some_and(|r| crate::net::line_clear_in(&r.construct, me, at));
    if d > 1 && !(d <= crate::net::ATTACK_RANGE && clear) {
        return Err("out of reach".to_string());
    }
    Ok(chance)
}

/// MASTIFF's tile nearest `p`: where an attack from there lands.
pub fn nearest(m: &Mastiff, p: Pos) -> Pos {
    Pos::new(
        p.x.clamp(m.pos.x, m.pos.x + 2),
        p.y.clamp(m.pos.y, m.pos.y + 2),
    )
}

/// Your turn ends: off the leash, MASTIFF grows a plate back, the first broken in the
/// coat's order, up to eight.
pub fn end_of_your_turn(st: &mut AppState) {
    if !awake(st) || on_the_leash(st) {
        return;
    }
    let Some(m) = up_mut(st) else {
        return;
    };
    let Some(i) = m.plates.iter().position(|w| !*w) else {
        return;
    };
    m.plates[i] = true;
    let line = format!("MASTIFF · a plate grows back · coat {}/8", m.coat());
    st.say(line);
}

/// MASTIFF asleep and whole again, coiled round its chip (`room.mastiff` none): on a
/// dump, a jack-out, a run reopened. A fight in the room ends with it.
pub fn reset(st: &mut AppState) {
    if let Some(room) = st.constructs.rig.as_mut() {
        room.mastiff = None;
    }
    if (st.net.engaged.as_ref()).is_some_and(|e| e.place == Place::Rig) {
        crate::net::disengage(st);
    }
}

/// Why a step onto `to` in the room is refused while it's on turns: MASTIFF stands
/// there, or it's a door or the trunk (the fight holds you in the room).
pub fn blocks(st: &AppState, to: Pos) -> Option<&'static str> {
    if up(st).is_some_and(|m| m.body().contains(to)) {
        return Some("MASTIFF is in the way");
    }
    let room = room(st)?;
    let out = room.doors.contains(&to) || room.trunk == to;
    (out && st.net.engaged.is_some()).then_some("not in a fight")
}

/// MASTIFF's line in the footer's ICE list while it's awake, as a foe's reads:
/// `▣ MASTIFF · black ICE · coat 5/8 · 3 tiles`, its core's hits once the coat is gone.
pub fn ice_line(st: &AppState) -> Option<String> {
    let me = you(st)?;
    let m = up(st).filter(|_| awake(st))?;
    let left = match m.coat() {
        0 => format!(
            "core {} {}",
            m.core,
            if m.core == 1 { "hit" } else { "hits" }
        ),
        n => format!("coat {n}/8"),
    };
    let n = me.chebyshev(nearest(m, me));
    let tiles = if n == 1 { "tile" } else { "tiles" };
    Some(format!("▣ MASTIFF · black ICE · {left} · {n} {tiles}"))
}

/// MASTIFF in a word for the turn column: its coat (`5/8`), then its core.
pub fn coat_line(st: &AppState) -> String {
    match up(st) {
        Some(m) if m.coat() > 0 => format!("{}/8", m.coat()),
        Some(m) => format!("core {}", m.core),
        None => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::guards::testutil::{advance, settle};
    use crate::net::Actor;
    use crate::rig::jobs::{self, JobKind};
    use crate::rigroom::layout::front;
    use crate::rigroom::room;
    use crate::rigroom::testutil::{BLACK_SLOT as SLOT, BLACK_SUM as SUM, a_black_chip};
    use citygen::Place;
    use game::ItemId;

    /// A starting runner (Vex) with a deck and Attack loaded, a Civitas cabinet (three
    /// reader slots) with a Tinman 2X core (two seats), a black chip of ¤4,800 in slot 2,
    /// jacked into the room and standing at the front of slot 2.
    fn armed() -> AppState {
        a_black_chip()
    }

    fn stand(st: &mut AppState, at: Pos) {
        st.player.as_mut().unwrap().pos = at;
    }

    fn here(st: &AppState) -> Pos {
        st.player.as_ref().unwrap().pos
    }

    fn mastiff(st: &AppState) -> &Mastiff {
        room(st).unwrap().mastiff.as_ref().expect("MASTIFF out")
    }

    fn order_of(st: &AppState) -> Vec<Actor> {
        st.net.engaged.as_ref().expect("on turns").order.clone()
    }

    fn whole(st: &AppState) -> usize {
        mastiff(st).plates.iter().filter(|w| **w).count()
    }

    fn walkable(st: &AppState, p: Pos) -> bool {
        room(st)
            .unwrap()
            .construct
            .tiles
            .get(p)
            .is_some_and(|t| t.is_walkable())
    }

    /// A floor tile of the bite you can stand on.
    fn in_the_bite(st: &AppState) -> Pos {
        let m = mastiff(st);
        let bite = m.bite.expect("a bite marked");
        bite.iter()
            .find(|p| walkable(st, *p) && !m.body().contains(*p))
            .expect("a floor tile under the marks")
    }

    /// A floor tile on the leash, off the marks and off MASTIFF.
    fn off_the_marks(st: &AppState) -> Pos {
        let m = mastiff(st);
        let bite = m.bite.expect("a bite marked");
        m.leash
            .iter()
            .find(|p| walkable(st, *p) && !bite.contains(*p) && !m.body().grow(1).contains(*p))
            .expect("a floor tile on the leash off the marks")
    }

    /// A floor tile of the room off the leash.
    fn off_the_leash(st: &AppState) -> Pos {
        let r = room(st).unwrap();
        let leash = mastiff(st).leash;
        r.floor
            .iter()
            .find(|p| walkable(st, *p) && !leash.contains(*p) && r.hall.contains(*p))
            .expect("hall floor off the leash")
    }

    fn logged(st: &AppState, text: &str) -> bool {
        st.log.iter().any(|e| e.text.contains(text))
    }

    /// Your turn's act half, as `space` opens it.
    fn act(st: &mut AppState) {
        assert!(crate::net::stay(st), "your move half");
    }

    fn ap(st: &AppState) -> u8 {
        st.net.engaged.as_ref().unwrap().ap
    }

    #[test]
    fn waking_puts_the_room_on_turns() {
        let mut st = armed();
        assert_eq!(crate::rig::of(&st).seats, 2);
        jobs::start(&mut st, JobKind::Rent).expect("the first seat busy");
        let slot = room(&st).unwrap().slots[SLOT];
        wake(&mut st, SLOT).expect("it wakes");
        let e = st.net.engaged.as_ref().expect("on turns");
        assert_eq!(e.place, Place::Rig);
        assert_eq!(e.order, [Actor::You, Actor::Seat(1), Actor::Mastiff]);
        assert_eq!(e.actor(), Actor::You);
        let m = mastiff(&st);
        assert_eq!(m.pos, Pos::new(front(slot).x - 1, front(slot).y));
        assert!(m.body().contains(front(slot)), "on the tile in front of it");
        assert_eq!((m.plates, m.core, m.bite), ([true; 8], 2, None));
        assert!(!m.body().contains(here(&st)), "you step out from under it");
        assert!(walkable(&st, here(&st)));
        // One awake at a time.
        assert_eq!(wake(&mut st, SLOT), Err("MASTIFF is awake".to_string()));
    }

    #[test]
    fn it_marks_then_bites() {
        let mut st = armed();
        wake(&mut st, SLOT).unwrap();
        let hp = st.character.hp;
        turn(&mut st);
        assert!(mastiff(&st).bite.is_some(), "the first turn marks");
        assert_eq!(st.character.hp, hp, "a mark takes nothing");
        assert!(logged(&st, "MASTIFF bites the × tiles next turn"));
        let at = in_the_bite(&st);
        stand(&mut st, at);
        // A black ICE hit of 7: the deck's rating off it, then your feedback resistance.
        let rating = st
            .character
            .deck(&st.content)
            .map_or(0, |(_, d)| d.rating as u32);
        let feedback = st.character.resist(&st.content, game::Harm::Feedback);
        let damage = game::resisted(BITE - rating, feedback);
        assert_eq!(crate::net::black_damage(&st, BITE), damage);
        turn(&mut st);
        assert_eq!(hp - st.character.hp, damage, "bitten as black ICE bites");
        assert!(mastiff(&st).bite.is_none(), "the marks clear");
        // Off the marks: untouched.
        let hp = st.character.hp;
        turn(&mut st);
        let away = off_the_marks(&st);
        stand(&mut st, away);
        turn(&mut st);
        assert_eq!(st.character.hp, hp);
        assert!(mastiff(&st).bite.is_none());
    }

    #[test]
    fn every_tile_of_its_leash_can_be_bitten() {
        use crate::rigroom::layout::{Counts, lay_out};
        for slots in 1..=6 {
            let counts = Counts {
                lines: 5,
                seats: 2,
                slots,
                firewall: 1,
            };
            let room = lay_out(counts, citygen::BuildingId(7));
            let k = &room.construct;
            for (slot, r) in room.slots.iter().enumerate() {
                let leash = leash_of(&room, slot).unwrap();
                let at = front(*r);
                let woke = Mastiff {
                    slot,
                    pos: Pos::new(at.x - 1, at.y),
                    plates: [true; 8],
                    core: CORE,
                    bite: None,
                    leash,
                };
                // For a tile it woke on, it comes from the far end of its strip.
                let far = Pos::new(woke.pos.x, leash.bottom() - 3);
                for me in leash.iter().filter(|p| floor(k, *p)) {
                    let mut m = woke.clone();
                    if m.body().contains(me) {
                        m.pos = far;
                    }
                    assert!(!m.body().contains(me));
                    // Its best ground: as near you as its steps take it.
                    while let Some(to) = step_toward(k, &m, me) {
                        m.pos = to;
                    }
                    let bite = mark(k, &m, me);
                    assert!(
                        bite.is_some_and(|b| b.contains(me)),
                        "slot {slot} of {slots}: you at {me:?}, it at {:?}, marks {bite:?}",
                        m.pos
                    );
                }
            }
        }
    }

    #[test]
    fn beside_its_box_it_lunges() {
        let mut st = armed();
        wake(&mut st, SLOT).unwrap();
        let slot = room(&st).unwrap().slots[SLOT];
        // Two west of the box, a row down: no ground it stands on puts you beside it.
        let at = Pos::new(slot.x - 2, slot.y + 1);
        stand(&mut st, at);
        let leash = mastiff(&st).leash;
        assert!(walkable(&st, at) && leash.contains(at));
        let hp = st.character.hp;
        turn(&mut st);
        let bite = mastiff(&st).bite.expect("a bite marked");
        let round = Rect::new(at.x - 1, at.y - 1, 3, 3).intersection(leash);
        assert_eq!(Some(bite), round, "the 3x3 round you, on its leash");
        assert!(logged(&st, "MASTIFF bites the × tiles next turn"));
        turn(&mut st);
        assert!(st.character.hp < hp, "the bite lands");
        assert!(logged(&st, "MASTIFF bites · hp"));
        assert!(mastiff(&st).bite.is_none(), "the marks clear");
    }

    #[test]
    fn its_turn_plays_out_a_step_at_a_time() {
        use crate::guards::testutil::pace_once;
        let mut st = armed();
        st.force_hit = Some(false);
        wake(&mut st, SLOT).unwrap();
        let from = mastiff(&st).pos;
        // You on the leash's west edge: two steps for it to come.
        stand(&mut st, Pos::new(from.x - 4, from.y + 1));
        assert!(mastiff(&st).leash.contains(here(&st)));
        crate::net::end_turn(&mut st);
        for _ in 0..10 {
            if st.net.engaged.as_ref().unwrap().actor() == Actor::Mastiff {
                break;
            }
            pace_once(&mut st);
        }
        assert_eq!(st.net.engaged.as_ref().unwrap().actor(), Actor::Mastiff);
        pace_once(&mut st);
        let one = mastiff(&st).pos;
        assert_eq!(one.chebyshev(from), 1, "a step a beat");
        assert!(mastiff(&st).bite.is_none(), "the mark waits for its steps");
        pace_once(&mut st);
        let two = mastiff(&st).pos;
        assert_eq!(two.chebyshev(one), 1);
        assert!(mastiff(&st).bite.is_none());
        pace_once(&mut st);
        assert_eq!(mastiff(&st).pos, two);
        assert!(mastiff(&st).bite.is_some(), "then it marks");
        assert!(crate::net::your_turn(&st), "and the turn comes round");
    }

    #[test]
    fn with_the_coat_gone_a_miss_is_the_cores() {
        let mut st = armed();
        wake(&mut st, SLOT).unwrap();
        let west = Pos::new(mastiff(&st).pos.x - 3, mastiff(&st).pos.y + 1);
        hit(&mut st, west);
        st.force_hit = Some(false);
        act(&mut st);
        crate::combat::attack_mastiff(&mut st).expect("a roll");
        assert_eq!(st.log.latest().unwrap().text, "MASTIFF · the coat holds");
        for _ in 0..7 {
            hit(&mut st, west);
        }
        assert_eq!(whole(&st), 0);
        crate::combat::attack_mastiff(&mut st).expect("a roll");
        assert_eq!(st.log.latest().unwrap().text, "MASTIFF · the core holds");
        seat_turn(&mut st, 0);
        assert_eq!(st.log.latest().unwrap().text, "seat 1 · the core holds");
    }

    #[test]
    fn its_line_in_the_ice_list() {
        let mut st = armed();
        assert_eq!(ice_line(&st), None, "asleep it isn't hunting you");
        wake(&mut st, SLOT).unwrap();
        let m = mastiff(&st).clone();
        let west = Pos::new(m.pos.x - 3, m.pos.y + 1);
        for _ in 0..3 {
            hit(&mut st, west);
        }
        stand(&mut st, Pos::new(m.pos.x - 3, m.pos.y + 1));
        assert_eq!(
            ice_line(&st).as_deref(),
            Some("▣ MASTIFF · black ICE · coat 5/8 · 3 tiles")
        );
        for _ in 0..5 {
            hit(&mut st, west);
        }
        assert_eq!(
            ice_line(&st).as_deref(),
            Some("▣ MASTIFF · black ICE · core 2 hits · 3 tiles")
        );
    }

    #[test]
    fn shield_soaks_the_bite() {
        let mut st = armed();
        wake(&mut st, SLOT).unwrap();
        turn(&mut st);
        let at = in_the_bite(&st);
        stand(&mut st, at);
        st.net.shield = 1;
        let hp = st.character.hp;
        turn(&mut st);
        assert_eq!(st.character.hp, hp);
        assert_eq!(st.net.shield, 0);
        assert!(logged(&st, "shield absorbs the hit"));
    }

    #[test]
    fn plates_break_from_your_side_then_the_core() {
        let mut st = armed();
        wake(&mut st, SLOT).unwrap();
        let m = mastiff(&st).clone();
        let west = Pos::new(m.pos.x - 3, m.pos.y + 1);
        for _ in 0..3 {
            hit(&mut st, west);
        }
        // ▌ first, then ▛ and ▙: the west side's three.
        assert_eq!(
            mastiff(&st).plates,
            [false, true, true, true, true, true, false, false]
        );
        for _ in 0..5 {
            hit(&mut st, west);
        }
        assert_eq!(whole(&st), 0);
        assert_eq!(mastiff(&st).core, 2, "the coat first");
        assert_eq!(
            hit(&mut st, west).as_deref(),
            Some("MASTIFF · its core cracks · 1 hit left")
        );
        assert!(st.net.engaged.is_some());
        assert_eq!(hit(&mut st, west), None, "the last hit says its own line");
        assert!(logged(&st, "MASTIFF · to grit · the chip opens · ¤4,800"));
        assert!(st.net.engaged.is_none(), "the turns end");
        let chip = st.character.reader[SLOT].as_ref().unwrap();
        assert_eq!(chip.item.0, "credchip");
        assert_eq!(chip.copy.as_ref().unwrap().credits, SUM);
        assert_eq!(mastiff(&st).core, 0, "grit where it stood");
    }

    #[test]
    fn the_winning_hit_says_the_chip_opens() {
        // Your Attack's, then a seat's: the line once, before the calm.
        for by_seat in [false, true] {
            let mut st = armed();
            wake(&mut st, SLOT).unwrap();
            let west = Pos::new(mastiff(&st).pos.x - 3, mastiff(&st).pos.y + 1);
            for _ in 0..9 {
                hit(&mut st, west);
            }
            assert_eq!(mastiff(&st).core, 1);
            st.force_hit = Some(true);
            if by_seat {
                seat_turn(&mut st, 0);
            } else {
                act(&mut st);
                crate::combat::attack_mastiff(&mut st).expect("the last hit");
            }
            let last: Vec<&str> = (st.log.iter().rev().take(2))
                .map(|e| e.text.as_str())
                .collect();
            assert_eq!(
                last,
                ["calm", "MASTIFF · to grit · the chip opens · ¤4,800"],
                "by a seat: {by_seat}"
            );
            let said = (st.log.iter())
                .filter(|e| e.text.contains("to grit"))
                .count();
            assert_eq!(said, 1, "by a seat: {by_seat}");
        }
    }

    #[test]
    fn the_leash_is_the_strip_in_front_of_its_slot() {
        use crate::rigroom::layout::{Counts, lay_out};
        for slots in 1..=6 {
            let counts = Counts {
                lines: 5,
                seats: 2,
                slots,
                firewall: 1,
            };
            let room = lay_out(counts, citygen::BuildingId(7));
            for (i, r) in room.slots.iter().enumerate() {
                let leash = leash_of(&room, i).unwrap();
                // The mock's: 2 west of the box, 1 east, 2 above, 10 below.
                let want = Rect::new(r.x - 2, r.y - 2, r.w + 3, r.h + 12);
                assert_eq!(leash, want, "slot {i} of {slots}");
                assert_eq!(
                    leash.intersection(room.hall),
                    Some(leash),
                    "inside the hall"
                );
                assert!(leash.y > room.floor.y + 1, "short of the READER title");
                for (j, other) in room.slots.iter().enumerate() {
                    assert!(
                        i == j || leash.intersection(*other).is_none(),
                        "slot {i}'s leash over slot {j} of {slots}"
                    );
                }
            }
        }
    }

    #[test]
    fn off_the_leash_a_plate_grows_back() {
        let mut st = armed();
        // The seats' beams all miss.
        st.force_hit = Some(false);
        wake(&mut st, SLOT).unwrap();
        let west = Pos::new(mastiff(&st).pos.x - 3, mastiff(&st).pos.y + 1);
        hit(&mut st, west);
        hit(&mut st, west);
        assert_eq!(whole(&st), 6);
        // On the leash nothing grows.
        crate::net::end_turn(&mut st);
        assert_eq!(whole(&st), 6);
        settle(&mut st);
        let away = off_the_leash(&st);
        stand(&mut st, away);
        crate::net::end_turn(&mut st);
        assert_eq!(whole(&st), 7);
    }

    #[test]
    fn off_the_leash_nobody_hits_it() {
        let mut st = armed();
        st.force_hit = Some(true);
        wake(&mut st, SLOT).unwrap();
        assert!(order_of(&st).contains(&Actor::Seat(0)));
        act(&mut st);
        let away = off_the_leash(&st);
        stand(&mut st, away);
        let left = ap(&st);
        assert_eq!(
            crate::combat::attack_mastiff(&mut st),
            Err("off the leash · out of reach".to_string())
        );
        assert_eq!(ap(&st), left, "no AP spent");
        seat_turn(&mut st, 0);
        assert_eq!(whole(&st), 8, "the seat sits out");
    }

    #[test]
    fn a_free_seat_attacks_and_a_busy_one_sits_out() {
        let mut st = armed();
        let c = st.content.clone();
        st.character.give_item(&c, ItemId::new("paydata"), 1);
        let shard = ItemId::new("paydata");
        jobs::start(&mut st, JobKind::Sift { shard }).expect("sift in the first seat");
        st.force_hit = Some(true);
        wake(&mut st, SLOT).unwrap();
        let seats: Vec<Actor> = (order_of(&st).into_iter())
            .filter(|a| matches!(a, Actor::Seat(_)))
            .collect();
        assert_eq!(seats, [Actor::Seat(1)]);
        seat_turn(&mut st, 0);
        assert_eq!(whole(&st), 8, "the busy seat sits out");
        seat_turn(&mut st, 1);
        assert_eq!(whole(&st), 7, "the free seat's beam breaks a plate");
        assert!(logged(&st, "seat 2 · "));
    }

    #[test]
    fn with_no_attack_loaded_the_seats_sit_out() {
        let mut st = armed();
        st.character.loaded.retain(|id| id.0 != "attack");
        wake(&mut st, SLOT).unwrap();
        assert!(
            !order_of(&st).iter().any(|a| matches!(a, Actor::Seat(_))),
            "{:?}",
            order_of(&st)
        );
        act(&mut st);
        let left = ap(&st);
        assert_eq!(
            crate::combat::attack_mastiff(&mut st),
            Err("no attack loaded".to_string())
        );
        assert_eq!(ap(&st), left, "no AP spent");
    }

    #[test]
    fn losing_resets_it() {
        let mut st = armed();
        wake(&mut st, SLOT).unwrap();
        let west = Pos::new(mastiff(&st).pos.x - 3, mastiff(&st).pos.y + 1);
        for _ in 0..9 {
            hit(&mut st, west);
        }
        crate::net::dump(&mut st, "the failsafe pulled you out");
        assert!(st.net.engaged.is_none());
        assert_eq!(room(&st).unwrap().mastiff, None, "asleep");
        assert!(!st.player.as_ref().unwrap().jacked_in());
        st.net.cooling_until = None;
        crate::net::jack_in_here(&mut st).expect("jacked in again");
        let at = front(room(&st).unwrap().slots[SLOT]);
        stand(&mut st, at);
        wake(&mut st, SLOT).unwrap();
        assert_eq!((mastiff(&st).plates, mastiff(&st).core), ([true; 8], 2));
        assert_eq!(
            st.character.reader[SLOT].as_ref().unwrap().item.0,
            "credchip_black"
        );
    }

    #[test]
    fn with_the_failsafe_off_it_can_kill() {
        let mut st = armed();
        st.net.failsafe = false;
        wake(&mut st, SLOT).unwrap();
        turn(&mut st);
        let at = in_the_bite(&st);
        stand(&mut st, at);
        st.character.hp = 5;
        assert!(crate::net::black_damage(&st, BITE) >= 5);
        turn(&mut st);
        assert_eq!(st.character.hp, 0);
        assert_eq!(st.net.death.as_deref(), Some("MASTIFF · in your own rig"));
    }

    #[test]
    fn a_restored_fight_finds_mastiff_asleep() {
        let dir = crate::runs::testutil::Dir::new("mastiff-fight");
        let mut st = armed();
        wake(&mut st, SLOT).unwrap();
        let west = Pos::new(mastiff(&st).pos.x - 3, mastiff(&st).pos.y + 1);
        hit(&mut st, west);
        turn(&mut st);
        let st = crate::runs::testutil::running(st);
        let path = crate::runs::file::write(&dir.0, &st).expect("saved mid-fight");
        let mut back = AppState::new();
        crate::runs::open(&mut back, &path).expect("it reopens");
        assert_eq!(back.player.as_ref().unwrap().place, Place::Rig);
        assert!(back.net.engaged.is_none(), "not on turns");
        assert_eq!(room(&back).expect("the room").mastiff, None);
        let chip = back.character.reader[SLOT].as_ref().unwrap();
        assert_eq!(chip.item.0, "credchip_black", "the chip still black");
    }

    #[test]
    fn no_trace_inside_your_firewall() {
        let mut st = armed();
        st.force_hit = Some(true);
        st.net.shield = 2;
        let traces = st.net.traces.clone();
        let districts = st.net.district_traces.clone();
        let foes = st.foes.len();
        assert!(st.backdoors.trace.is_none());
        wake(&mut st, SLOT).unwrap();
        for _ in 0..10 {
            if st.net.engaged.is_none() {
                break;
            }
            act(&mut st);
            let _ = crate::combat::attack_mastiff(&mut st);
            let _ = crate::combat::attack_mastiff(&mut st);
            if st.net.engaged.is_some() {
                crate::net::end_turn(&mut st);
                settle(&mut st);
            }
            advance(&mut st, 1);
        }
        assert!(st.net.engaged.is_none(), "the fight is won");
        assert_eq!(
            st.character.reader[SLOT].as_ref().unwrap().item.0,
            "credchip"
        );
        assert_eq!(st.net.traces, traces);
        assert_eq!(st.net.district_traces, districts);
        assert!(st.backdoors.trace.is_none(), "no trace home");
        assert_eq!(st.foes.len(), foes, "no tracer");
        assert!(st.net.grid.alarmed.is_empty());
    }

    #[test]
    fn the_chips_popup_wakes_it() {
        use crate::app::Screen;
        use crate::screens::testutil::key;
        use crossterm::event::KeyCode;
        let mut st = armed();
        let mut popup = crate::rigroom::summon::interact(&mut st).expect("the chip's popup");
        let rows: Vec<String> = (popup.choices().iter())
            .map(|c| format!("{}{}", c.label, c.tail))
            .collect();
        assert_eq!(rows, ["Wake MASTIFF", "Leave it"]);
        popup.update(&mut st, &[key(KeyCode::Char('s'))]);
        assert!(awake(&st));
        assert!(logged(&st, "ENGAGED · MASTIFF wakes"));
        // Awake, its row is greyed with why (you stepped aside, still by the slot).
        let popup = crate::rigroom::summon::interact(&mut st).expect("the chip's popup");
        let wake = &popup.choices()[0];
        assert!(!wake.enabled);
        assert_eq!(
            format!("{}{}", wake.label, wake.tail),
            "Wake MASTIFF · MASTIFF is awake"
        );
    }

    #[test]
    fn the_map_steps_round_it_and_space_attacks_it() {
        use crate::app::Screen;
        use crate::screens::explore::MapScreen;
        use crate::screens::testutil::key;
        use crossterm::event::KeyCode;
        let mut st = armed();
        st.force_hit = Some(true);
        wake(&mut st, SLOT).unwrap();
        let mut map = MapScreen::walking(&mut st);
        let m = mastiff(&st).clone();
        let at = here(&st);
        assert_eq!(at, Pos::new(m.pos.x - 1, m.pos.y), "beside it, west");
        let moves = st.net.engaged.as_ref().unwrap().moves;
        // Into MASTIFF: refused, nothing spent.
        map.update(&mut st, &[key(KeyCode::Char('l'))]);
        assert_eq!(here(&st), at);
        assert!(logged(&st, "MASTIFF is in the way"));
        assert_eq!(st.net.engaged.as_ref().unwrap().moves, moves);
        // Away from it: a move.
        map.update(&mut st, &[key(KeyCode::Char('h'))]);
        assert_eq!(here(&st), Pos::new(at.x - 1, at.y));
        assert_eq!(st.net.engaged.as_ref().unwrap().moves, moves - 1);
        // Space stays and the fire cursor opens on MASTIFF; space again attacks it.
        map.update(&mut st, &[key(KeyCode::Char(' '))]);
        let left = ap(&st);
        map.update(&mut st, &[key(KeyCode::Char(' '))]);
        assert_eq!(ap(&st), left - 1);
        assert_eq!(whole(&st), 7, "a plate broke");
        // The Attack's own key does it too.
        let n = (st.character.loaded.iter())
            .position(|id| id.0 == "attack")
            .unwrap();
        let digit = char::from_digit(n as u32 + 1, 10).unwrap();
        map.update(&mut st, &[key(KeyCode::Char(digit))]);
        assert_eq!(whole(&st), 6);
    }

    /// `id` loaded in the deck; its number key.
    fn load_program(st: &mut AppState, id: &str) -> char {
        let id = ItemId::new(id);
        if !st.character.loaded.contains(&id) {
            st.character.give(id.clone(), 1);
            st.character.loaded.push(id.clone());
        }
        let n = st.character.loaded.iter().position(|l| *l == id).unwrap();
        assert!(n < crate::ui::hud::SLOTS, "a number key reaches it");
        char::from_digit(n as u32 + 1, 10).unwrap()
    }

    #[test]
    fn shield_runs_in_the_fight_and_soaks_the_bite() {
        use crate::app::Screen;
        use crate::screens::explore::MapScreen;
        use crate::screens::testutil::key;
        use crossterm::event::KeyCode;
        let mut st = armed();
        let shield = load_program(&mut st, "shield");
        let traces = st.net.traces.clone();
        wake(&mut st, SLOT).unwrap();
        let mut map = MapScreen::walking(&mut st);
        map.update(&mut st, &[key(KeyCode::Char(' '))]);
        let left = ap(&st);
        // The Shield's key picks you; space runs it, an action point, and it lands.
        map.update(&mut st, &[key(KeyCode::Char(shield))]);
        map.update(&mut st, &[key(KeyCode::Char(' '))]);
        assert_eq!(st.net.shield, 1, "{:?}", st.log.latest());
        assert!(logged(&st, "shield raised · 1 charge"));
        assert_eq!(ap(&st), left - 1);
        assert_eq!(st.net.traces, traces, "no heat behind your firewall");
        // MASTIFF marks, you stand in it, and the shield takes the bite.
        turn(&mut st);
        let at = in_the_bite(&st);
        stand(&mut st, at);
        let hp = st.character.hp;
        turn(&mut st);
        assert_eq!(st.character.hp, hp);
        assert_eq!(st.net.shield, 0);
        assert!(logged(&st, "shield absorbs the hit"));
    }

    #[test]
    fn what_means_nothing_in_the_room_says_why() {
        use crate::app::Screen;
        use crate::net::Scene;
        use crate::screens::explore::MapScreen;
        use crate::screens::testutil::key;
        use crossterm::event::KeyCode;
        let mut st = armed();
        let mask = load_program(&mut st, "mask");
        let crack = load_program(&mut st, "crack");
        wake(&mut st, SLOT).unwrap();
        let mut map = MapScreen::walking(&mut st);
        map.update(&mut st, &[key(KeyCode::Char(' '))]);
        let left = ap(&st);
        // A Mask hides you from eyes: MASTIFF knows where you are.
        map.update(&mut st, &[key(KeyCode::Char(mask))]);
        assert_eq!(
            st.log.latest().unwrap().text,
            "mask · nothing in your rig hunts by sight"
        );
        assert_eq!(
            crate::net::start_run(&mut st, ItemId::new("mask"), game::HackTarget::Deck),
            Err("mask · nothing in your rig hunts by sight".to_string())
        );
        // A Crack has no node here to work on.
        map.update(&mut st, &[key(KeyCode::Char(crack))]);
        assert_eq!(
            st.log.latest().unwrap().text,
            "crack · nothing to run it against"
        );
        assert_eq!((ap(&st), st.net.mask), (left, 0), "nothing spent, no mask");
        assert!(st.net.run.is_none());
        // Space's list of programs greys it with the same reason.
        let rows = crate::interact::run_rows_at(
            &st,
            Scene::Rig,
            here(&st),
            Some(&ItemId::new("mask")),
            &mut "abc".chars(),
        );
        assert_eq!(rows.len(), 1);
        assert!(!rows[0].enabled);
        assert!(rows[0].tail.contains("nothing in your rig hunts by sight"));
    }

    #[test]
    fn g_aims_a_walk_round_it() {
        use crate::app::Screen;
        use crate::screens::explore::MapScreen;
        use crate::screens::testutil::key;
        use crossterm::event::KeyCode;
        let mut st = armed();
        wake(&mut st, SLOT).unwrap();
        let mut map = MapScreen::walking(&mut st);
        let at = here(&st);
        let moves = st.net.engaged.as_ref().unwrap().moves;
        // Into MASTIFF the aim doesn't go.
        map.update(&mut st, &[key(KeyCode::Char('g')), key(KeyCode::Char('l'))]);
        assert_eq!(
            map.aim_path(&st).map(|(p, _)| p.len()),
            Some(0),
            "the aim stays on you"
        );
        // Two west, and enter walks it.
        map.update(
            &mut st,
            &[
                key(KeyCode::Char('h')),
                key(KeyCode::Char('h')),
                key(KeyCode::Enter),
            ],
        );
        for _ in 0..40 {
            if here(&st) == Pos::new(at.x - 2, at.y) {
                break;
            }
            advance(&mut st, 1);
            map.update(&mut st, &[]);
        }
        assert_eq!(here(&st), Pos::new(at.x - 2, at.y));
        assert_eq!(st.net.engaged.as_ref().unwrap().moves, moves - 2);
    }

    #[test]
    fn a_fight_keeps_its_room() {
        use crate::rigroom::testutil::hold_a_backdoor;
        let mut st = armed();
        wake(&mut st, SLOT).unwrap();
        let doors = room(&st).unwrap().doors.len();
        for n in 0..doors + 1 {
            hold_a_backdoor(&mut st, n);
        }
        crate::rigroom::refresh(&mut st);
        assert_eq!(
            room(&st).unwrap().doors.len(),
            doors,
            "no rebuild mid-fight"
        );
        assert!(room(&st).unwrap().mastiff.is_some());
        assert!(st.net.engaged.is_some());
        // Once it's over the room takes its new doors.
        reset(&mut st);
        crate::rigroom::refresh(&mut st);
        assert_eq!(room(&st).unwrap().doors.len(), doors + 1);
    }
}
