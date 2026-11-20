//! Your rig's own Net: the room you land in jacking in at home (`Place::Rig`), laid out
//! from the rig as it stands each time you go in and walked as a construct is.

pub mod crack;
pub mod layout;
pub mod mastiff;
pub mod panel;
pub mod reader;
pub mod summon;
#[cfg(test)]
pub(crate) mod testutil;
pub mod ways;
pub use layout::{Counts, RigRoom, lay_out};

use crate::app::AppState;
use crate::render::netlook::rig::{DoorView, RigView, SeatView, SlotView};
use citygen::{BuildingId, Pos};
use game::SocketKind;

/// What the HUD's place line and the footer's first row read in the room.
pub const PLACE_LINE: &str = "zenbo · your rig";

/// Whether you stand within a step of the room's terminal.
pub fn at_terminal(st: &AppState) -> bool {
    let (Some(room), Some(p)) = (room(st), st.player.as_ref()) else {
        return false;
    };
    p.place == citygen::Place::Rig && p.pos.chebyshev(room.terminal) <= 1
}

/// The rig's shell, from the room's terminal: the same one the desk opens, on the
/// jack window. What it runs and kills is read by the room from the same state.
pub fn open_shell(st: &mut AppState) -> Box<dyn crate::app::Screen> {
    let at = room(st).map_or(citygen::Pos::new(0, 0), |r| r.terminal);
    Box::new(crate::screens::terminal::Terminal::open(
        st,
        crate::helix::shell::HostId::Rig,
        (citygen::Place::Rig, at),
    ))
}

/// Columns of the Net's void the camera keeps either side of the room, where the held
/// backdoors' lanes run out.
pub const LANES: i32 = 4;

/// What the room is laid out for: a door for each of the relay's lines, or for each
/// standing backdoor where you hold more (lowering the relay burns nothing); the core's
/// seats; the rack's reader slots; the firewall's tier.
pub fn counts(st: &AppState) -> Counts {
    let rig = crate::rig::of(st);
    Counts {
        lines: rig.lines.max(st.backdoors.standing().count()),
        seats: rig.seats,
        slots: st.character.reader_slots(&st.content),
        firewall: rig.firewall,
    }
}

/// Lays the room out afresh from the rig into `st.constructs.rig`: on the way in, and
/// for a run reopened inside it. None without a home. MASTIFF is asleep in the new room
/// (its grit, a fight's, gone with the old one).
pub fn build(st: &mut AppState) {
    st.constructs.rig = st.home.map(|h| {
        let mut room = lay_out(counts(st), h.home.building);
        let standing: Vec<BuildingId> = st.backdoors.standing().map(|d| d.building).collect();
        fill(&mut room.door_for, &standing);
        room
    });
}

/// Gives each free slot, in order, a standing backdoor no slot holds yet.
fn fill(slots: &mut [Option<BuildingId>], standing: &[BuildingId]) {
    let held: Vec<BuildingId> = slots.iter().flatten().copied().collect();
    let mut spare = standing.iter().filter(|b| !held.contains(b));
    for slot in slots.iter_mut().filter(|s| s.is_none()) {
        *slot = spare.next().copied();
    }
}

/// What each door leads out to, top to bottom, read from the game as it stands: a
/// door keeps its backdoor while it stands, a burned one goes dark, and a free line
/// takes a backdoor planted since (`refresh` records it).
pub fn door_lines(st: &AppState) -> Vec<Option<BuildingId>> {
    let Some(room) = room(st) else {
        return Vec::new();
    };
    let mut slots: Vec<Option<BuildingId>> = (room.door_for.iter())
        .map(|b| b.filter(|b| st.backdoors.live(*b).is_some()))
        .collect();
    let standing: Vec<BuildingId> = st.backdoors.standing().map(|d| d.building).collect();
    fill(&mut slots, &standing);
    slots
}

/// Brings the room in step with the backdoors you hold, as it is about to be walked or
/// drawn: a backdoor planted since you came in takes a free door and keeps it; with
/// more backdoors than doors the room is laid out again with one for each, you left
/// where you stand if that is still floor, else on the landing. Not while the room is on
/// turns: a fight keeps the room it started in, and the new doors wait for it to end.
pub fn refresh(st: &mut AppState) {
    let Some(doors) = room(st).map(|r| r.doors.len()) else {
        return;
    };
    let lines = door_lines(st);
    let fighting = (st.net.engaged.as_ref()).is_some_and(|e| e.place == citygen::Place::Rig);
    if st.backdoors.standing().count() > doors && !fighting {
        let old = lines;
        build(st);
        if let Some(room) = st.constructs.rig.as_mut() {
            // The doors you knew keep their backdoors; the new ones take the rest.
            let standing: Vec<BuildingId> = st.backdoors.standing().map(|d| d.building).collect();
            let mut slots = vec![None; room.doors.len()];
            let kept = old.len().min(slots.len());
            slots[..kept].copy_from_slice(&old[..kept]);
            fill(&mut slots, &standing);
            room.door_for = slots;
        }
        keep_player(st);
    } else if let Some(room) = st.constructs.rig.as_mut() {
        room.door_for = lines;
    }
}

/// After the room is laid out again: you stay put on a floor tile of it, else you are on
/// the landing.
pub fn keep_player(st: &mut AppState) {
    let Some(land) = room(st).map(|r| r.land) else {
        return;
    };
    let pos = st.player.as_ref().map(|p| p.pos);
    let stands = room(st)
        .zip(pos)
        .is_some_and(|(r, pos)| r.construct.tiles.get(pos).is_some_and(|t| t.is_walkable()));
    if !stands {
        crate::constructs::land(st, citygen::Place::Rig, land);
    }
}

/// The room as it stands this frame, for drawing: each door's backdoor and its clock,
/// what each reader slot holds, the job in each seat, and the sleds' names. Nothing in
/// it is kept: a job run or killed, a door burned, a slot filled shows next frame.
pub fn view(st: &AppState) -> Option<RigView<'_>> {
    use crate::rig::jobs::{HEAT_PATCH, JobKind, RENT_PER_HOUR, sift_left};
    let room = room(st)?;
    let now = st.net.now(st);
    let rig = crate::rig::of(st);
    let running = &st.backdoors.jobs.running;
    let lines = door_lines(st);
    let doors = (lines.iter())
        .map(|b| match b.and_then(|b| st.backdoors.live(b)) {
            Some(d) => DoorView {
                name: clip(&crate::net::building_name(st, d.building), DOOR_NAME),
                left: format!("{} left", span(d.left(now))),
                held_by: (running.iter())
                    .any(|j| j.kind == JobKind::Keepalive(d.building))
                    .then_some("keepalive"),
                lit: true,
            },
            None => DoorView {
                name: "free line".to_string(),
                left: String::new(),
                held_by: None,
                lit: false,
            },
        })
        .collect();
    let slots = (0..room.slots.len())
        .map(|i| {
            let stack = st.character.reader.get(i)?.as_ref()?;
            let holds = reader::holds(st, &stack.item)?;
            let (glyph, colour) = reader::glyph(holds);
            let name = (st.content.item(&stack.item))
                .and_then(|it| it.name.split_whitespace().next().map(str::to_string))
                .unwrap_or_else(|| stack.item.0.clone());
            let tail = match holds {
                reader::Holds::Chip => {
                    format!("¤{}", stack.copy.as_ref().map_or(0, |k| k.credits))
                }
                reader::Holds::Program(_) => (reader::summon_word(st, stack))
                    .split_whitespace()
                    .next()
                    .unwrap_or_default()
                    .to_string(),
                _ => reader::summon_word(st, stack),
            };
            Some(SlotView {
                glyph,
                colour,
                name: clip(&name, SLOT_WORD),
                tail: clip(&tail, SLOT_WORD),
            })
        })
        .collect();
    let seats = (0..room.seats.len())
        .map(|i| {
            let job = running.get(i)?;
            let (on, state) = match &job.kind {
                JobKind::Sift { shard } => {
                    let (left, all) = sift_left(job, rig.speed, now).unwrap_or((0, 1));
                    let done = ((all - left) * 5 + all / 2) / all.max(1);
                    let bar: String = (0..5).map(|n| if n < done { '▰' } else { '▱' }).collect();
                    let name = st.content.item(shard).map_or(shard.0.as_str(), |i| &i.name);
                    (name.to_string(), format!("{bar} {}", span(left)))
                }
                JobKind::Sniff(b) | JobKind::Keepalive(b) => {
                    let heat = st.backdoors.jobs.heat.get(b).copied().unwrap_or(0);
                    let noise = format!("noise {} of {}", heat / 4, HEAT_PATCH / 4);
                    (crate::net::building_name(st, *b), noise)
                }
                JobKind::Rent => ("spare cycles".to_string(), format!("¤{RENT_PER_HOUR}/h")),
            };
            Some(SeatView {
                job: job.kind.name().to_string(),
                on: clip(&on, SEAT_TEXT),
                state: clip(&state, SEAT_TEXT),
                door: (job.kind.line()).and_then(|b| lines.iter().position(|d| *d == Some(b))),
            })
        })
        .collect();
    let sleds = |kind: SocketKind| -> Vec<String> {
        (st.character.rig_fitted(&st.content).into_iter())
            .filter(|(k, _)| *k == kind)
            .filter_map(|(_, id)| st.content.item(&id?).map(|i| sled_name(&i.name)))
            .collect()
    };
    let one = |kind: SocketKind, none: &str| sleds(kind).into_iter().next().unwrap_or(none.into());
    let cores = sleds(SocketKind::Core);
    let core = match cores.as_slice() {
        [] => "no core".to_string(),
        [one] => format!("{one} · {}", rig.seats),
        many => format!("{} cores · {}", many.len(), rig.seats),
    };
    Some(RigView {
        room,
        doors,
        slots,
        seats,
        you: (st.player.as_ref())
            .filter(|p| p.place == citygen::Place::Rig)
            .map_or(room.land, |p| p.pos),
        origin: Pos::new(0, 0),
        tick: st.tick,
        firewall: (one(SocketKind::Firewall, "").to_lowercase(), rig.firewall),
        relay: format!("{} · {}", one(SocketKind::Relay, "no relay"), rig.lines),
        core,
        line: format!("{} · {}d", one(SocketKind::Line, "no line"), rig.hold_days),
    })
}

/// A door's name beside it, inside PORTS: the rest goes to `…`.
const DOOR_NAME: usize = 15;
/// A slot's word under its box, so the next slot's has room.
const SLOT_WORD: usize = 9;
/// A line of text inside a seat's box.
const SEAT_TEXT: usize = 13;

/// `s` in at most `n` characters, its end cut to `…`.
fn clip(s: &str, n: usize) -> String {
    if s.chars().count() <= n {
        return s.to_string();
    }
    let mut out: String = s.chars().take(n - 1).collect();
    out.push('…');
    out
}

/// A sled's name without the kind it ends in: `Tinman 2X core` → `Tinman 2X`,
/// `CU-1 copper line` → `CU-1`, `Sandbag firewall` → `Sandbag`.
fn sled_name(name: &str) -> String {
    for kind in [" core", " relay", " copper line", " firewall"] {
        if let Some(short) = name.strip_suffix(kind) {
            return short.to_string();
        }
    }
    name.to_string()
}

/// A stretch of game minutes in its biggest whole unit: `2d`, `6h`, `40m`.
pub fn span(minutes: u64) -> String {
    match minutes {
        m if m >= 24 * 60 => format!("{}d", m / (24 * 60)),
        m if m >= 60 => format!("{}h", m / 60),
        m => format!("{m}m"),
    }
}

/// The room as last laid out.
pub fn room(st: &AppState) -> Option<&RigRoom> {
    st.constructs.rig.as_ref()
}
