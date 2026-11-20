//! MASTIFF: the black ICE coiled round a black credchip in your rig's reader. Woken from
//! the chip's popup it puts the room on the Net's turns (you, each free core seat, then
//! MASTIFF) and fights on its leash round the slot: eight plates of coat round a core
//! that takes two hits, two tiles a turn at you, a bite marked one turn and bitten the
//! next. It lives on the room (`RigRoom.mastiff`), which isn't saved: a run reopened finds
//! it asleep and whole. Inside your own firewall nothing traces you: no heat, no trace,
//! no tracer.

use super::layout::front;
use super::room;
use crate::app::AppState;
use crate::net::Actor;
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
    /// The ground it keeps to: its slot's box grown by `LEASH`, clipped to the hall.
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
/// Cells its leash reaches past its slot's box each way.
const LEASH: i32 = 6;
/// The best a seat's beam does, in percent.
const SEAT_CAP: u32 = 95;
/// Each plate's tile in its 3x3, in the coat's order.
const PLATE_AT: [(i32, i32); 8] = [
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
    let r = room.slots[slot];
    let at = front(r);
    room.mastiff = Some(Mastiff {
        slot,
        pos: Pos::new(at.x - 1, at.y),
        plates: [true; 8],
        core: CORE,
        bite: None,
        leash: r.grow(LEASH).intersection(room.hall).unwrap_or(r),
    });
    step_aside(st);
    crate::net::engage(st, Place::Rig, false);
    Ok(())
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

/// MASTIFF's turn: two tiles toward you, keeping to its leash, then a bite marked (the
/// 3x3 beside it on your side) or, with one marked, bitten: on its tiles you take `BITE`
/// as a black ICE hit (`net::black_hit`: Shield soaks it, the failsafe pulls you out
/// first, with it off it can kill you), and the marks clear.
pub fn turn(st: &mut AppState) {
    let Some(me) = you(st) else {
        return;
    };
    let Some(room) = st.constructs.rig.as_mut() else {
        return;
    };
    let k = &room.construct;
    let Some(m) = room.mastiff.as_mut().filter(|m| m.core > 0) else {
        return;
    };
    for _ in 0..MOVES {
        match step_toward(k, m, me) {
            Some(to) => m.pos = to,
            None => break,
        }
    }
    let from = m.body().center();
    match m.bite.take() {
        None => {
            m.bite = beside(m, me);
            if m.bite.is_some() {
                st.warn("MASTIFF bites the × tiles next turn");
            }
        }
        Some(bite) if bite.contains(me) => {
            crate::net::black_hit(st, BITE, from, "MASTIFF bites", DEATH);
        }
        Some(_) => st.say("MASTIFF bites · nothing there"),
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

/// The 3x3 beside MASTIFF on your side, on its leash: what it marks to bite.
fn beside(m: &Mastiff, me: Pos) -> Option<Rect> {
    let side = |lo: i32, at: i32| {
        if at < lo {
            -1
        } else if at > lo + 2 {
            1
        } else {
            0
        }
    };
    let (dx, dy) = (side(m.pos.x, me.x), side(m.pos.y, me.y));
    if (dx, dy) == (0, 0) {
        return None;
    }
    Rect::new(m.pos.x + 3 * dx, m.pos.y + 3 * dy, 3, 3).intersection(m.leash)
}

/// A hit on MASTIFF from `from`: the whole plate nearest it breaks, or with the coat gone
/// its core takes one. The core's second leaves grit where it stood, ends the turns and
/// opens the chip it guarded into a `credchip` with its sum. The line it makes.
pub fn hit(st: &mut AppState, from: Pos) -> String {
    let Some(m) = up_mut(st) else {
        return "nothing there".to_string();
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
        return format!("MASTIFF · a plate breaks · coat {}/8", m.coat());
    }
    m.core -= 1;
    if m.core > 0 {
        return format!("MASTIFF · its core cracks · {} hit left", m.core);
    }
    m.bite = None;
    let slot = m.slot;
    crate::net::disengage(st);
    let sum = match st.character.reader.get_mut(slot).and_then(|s| s.as_mut()) {
        Some(chip) if chip.item.0 == "credchip_black" => {
            chip.item = ItemId::new("credchip");
            chip.copy.as_ref().map_or(0, |k| k.credits)
        }
        _ => 0,
    };
    format!(
        "MASTIFF goes to grit · the chip opens · ¤{}",
        super::crack::commas(sum)
    )
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
        "the coat holds".to_string()
    };
    st.say(format!("seat {} · {line}", seat + 1));
}

/// A seat's chance at MASTIFF: yours, times the core's speed over 100, at most 95; or
/// why it sits out.
fn seat_chance(st: &AppState, seat: u8) -> Result<u8, String> {
    if !on_the_leash(st) {
        return Err(OFF_THE_LEASH.to_string());
    }
    if st.backdoors.jobs.running.len() > usize::from(seat) {
        return Err("the seat is busy".to_string());
    }
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
    use crate::rigroom::testutil::at_the_rig;
    use citygen::Place;
    use game::{ItemId, Stack};

    /// The chip's sum.
    const SUM: u32 = 4800;
    /// Its reader slot.
    const SLOT: usize = 2;

    /// A starting runner (Vex) with a deck and Attack loaded, a Civitas cabinet (three
    /// reader slots) with a Tinman 2X core (two seats), a black chip of ¤4,800 in slot 2, jacked into the room
    /// and standing at the front of slot 2.
    fn armed() -> AppState {
        let mut st = at_the_rig();
        let c = st.content.clone();
        if st.character.deck(&c).is_none() {
            let deck = ItemId::new("pawnshop_deck");
            let copy = st.character.new_copy(&c, &deck);
            st.character.wear(game::BodyPart::Waist, deck, Some(copy));
        }
        let attack = ItemId::new("attack");
        if !st.character.loaded.contains(&attack) {
            st.character.give(attack.clone(), 1);
            st.character.loaded.push(attack);
        }
        crate::rig::fit_new_rack(&mut st.character, &c, "rack_civitas");
        let core = ItemId::new("sled_tinman");
        let copy = Some(st.character.new_copy(&c, &core));
        st.character.rig_copy_mut(&c).fitted[0] = Some(Stack {
            item: core,
            qty: 1,
            copy,
        });
        let id = ItemId::new("credchip_black");
        let mut chip = Stack::new(id.clone(), 1);
        let mut copy = st.character.new_copy(&c, &id);
        copy.credits = SUM;
        let copy_id = copy.id;
        chip.copy = Some(copy);
        st.character.inventory.push(chip);
        st.character
            .put_in_reader(&c, SLOT, copy_id)
            .expect("in the reader");
        crate::net::jack_in_here(&mut st).expect("jacked in");
        let at = front(room(&st).unwrap().slots[SLOT]);
        stand(&mut st, at);
        st
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
        assert!(hit(&mut st, west).contains("1 hit left"));
        assert!(st.net.engaged.is_some());
        let line = hit(&mut st, west);
        assert!(line.contains("¤4,800"), "{line}");
        assert!(st.net.engaged.is_none(), "the turns end");
        let chip = st.character.reader[SLOT].as_ref().unwrap();
        assert_eq!(chip.item.0, "credchip");
        assert_eq!(chip.copy.as_ref().unwrap().credits, SUM);
        assert_eq!(mastiff(&st).core, 0, "grit where it stood");
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
