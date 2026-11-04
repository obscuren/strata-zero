//! Danger on the open Net: ICE out on the grid. A grid piece is a street foe with a
//! grade and a role. Sentinels walk beats near their district's hub and hunt what
//! they see along a lane; tracers leave an owner's gate or hub when its alarm goes up
//! from outside and follow your trail until they latch. Sweeps are the fallback to
//! sentinels: while a district's alert is up its hub rings out and every ring that
//! crosses you heats the trace. Spec
//! `docs/superpowers/specs/2026-11-04-net-nodes-design.md`, "Danger outside".

use crate::app::AppState;
use crate::foes::{Foe, FoeMode};
use crate::owners::Owner;
use citygen::{City, Dir, DistrictId, IceGrade, NodeKind, Place, Pos, StratumKind, Terrain};

/// On the open Net only pieces this near you count for a fight.
pub const FIGHT_RADIUS: i32 = 20;
/// Ticks a tracer takes for a tile.
pub const TRACER_TICKS: u64 = 4;
/// The most tiles of trail kept.
pub const TRAIL_MAX: usize = 600;
/// A trail point this near a hub is lost in its traffic.
pub const HUB_REACH: i32 = 2;
/// Ticks between a sentinel's steps on its beat.
pub const SENTINEL_TICKS: u64 = crate::net::BEAT_TICKS / 3;
/// Sweeps: a ring every this many beats, a tile every this many ticks, this far.
pub const SWEEP_BEATS: u64 = 2;
pub const RING_TICKS: u64 = 2;
pub const RING_MAX: i32 = 30;
pub const RING_HEAT: u8 = 12;

/// What a grid piece is for.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Role {
    /// Walks a beat, there and back, asleep until it sees you.
    Sentinel {
        beat: Vec<Pos>,
        step: usize,
        forward: bool,
    },
    /// Follows your trail: the index of the trail point it walks to, or none when the
    /// trail is gone and it waits at `wait`.
    Tracer { at: usize, wait: Option<Pos> },
}

/// A piece of ICE on the open Net.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GridIce {
    pub grade: IceGrade,
    pub role: Role,
    pub owner: Owner,
}

impl GridIce {
    pub fn tracer(&self) -> bool {
        matches!(self.role, Role::Tracer { .. })
    }
}

/// Which danger the open Net keeps besides tracers (a dev switch in Settings).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Patrols {
    #[default]
    Sentinels,
    Sweeps,
}

impl Patrols {
    pub fn label(self) -> &'static str {
        match self {
            Patrols::Sentinels => "sentinels",
            Patrols::Sweeps => "sweeps",
        }
    }

    pub fn next(self) -> Patrols {
        match self {
            Patrols::Sentinels => Patrols::Sweeps,
            Patrols::Sweeps => Patrols::Sentinels,
        }
    }
}

/// A ring out of a hub.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Ring {
    pub district: DistrictId,
    pub hub: Pos,
    /// The clock tick it left the hub.
    pub born: u64,
}

impl Ring {
    pub fn radius(&self, now: u64) -> i32 {
        (now.saturating_sub(self.born) / RING_TICKS) as i32
    }
}

/// The open Net's own state this session.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct GridState {
    /// The tiles you stood on on the open Net, in order, each marked when it lies by a
    /// hub (a tracer that gets there is lost).
    pub trail: Vec<(Pos, bool)>,
    /// The sentinels went out (once a session).
    pub posted: bool,
    /// The last clock tick a tracer stepped, a sentinel stepped, a sweep rang.
    pub tracer_tick: u64,
    pub sentinel_tick: u64,
    pub ring_tick: u64,
    pub rings: Vec<Ring>,
    /// Owners whose alarm went up from a run outside, while their alert lasts.
    pub alarmed: Vec<Owner>,
    /// Where you stood on the last frame.
    pub last: Option<(Place, Pos)>,
}

/// A grid piece's glyph.
pub const GLYPH: char = '◈';

/// A grade's piece name: `grey ICE`, `black tracer`.
pub fn name(grade: IceGrade, tracer: bool) -> String {
    format!(
        "{} {}",
        grade.label(),
        if tracer { "tracer" } else { "ICE" }
    )
}

/// A new grid piece on the Net.
pub fn piece(grade: IceGrade, role: Role, owner: Owner, pos: Pos) -> Foe {
    let tracer = matches!(role, Role::Tracer { .. });
    let integrity = grade.integrity();
    Foe {
        name: name(grade, tracer),
        place: Place::Stratum(StratumKind::Net),
        pos,
        hp: integrity,
        hp_max: integrity,
        reflex: score(grade),
        weapon: None,
        sight: grade.sight(),
        mode: FoeMode::Idle,
        stunned: 0,
        down: false,
        inert: false,
        glyph: GLYPH,
        chrome: false,
        marked: false,
        ice: Some(GridIce { grade, role, owner }),
    }
}

/// Initiative by grade, as construct ICE.
pub fn score(grade: IceGrade) -> u8 {
    match grade {
        IceGrade::White => 1,
        IceGrade::Grey => 2,
        IceGrade::Black => 3,
    }
}

/// A grid piece's turn: tiles and hits. Out on the grid even white walks.
pub fn budgets(grade: IceGrade) -> (u8, u8) {
    match grade {
        IceGrade::White => (2, 1),
        IceGrade::Grey => (2, 1),
        IceGrade::Black => (3, 2),
    }
}

/// The grade a tracer of an owner's security comes in.
pub fn tracer_grade(security: u8) -> IceGrade {
    match security {
        s if s < 200 => IceGrade::White,
        s if s < 230 => IceGrade::Grey,
        _ => IceGrade::Black,
    }
}

/// The sentinels a district keeps, by its security.
pub fn sentinels_for(security: u8) -> &'static [IceGrade] {
    use IceGrade::{Black, Grey, White};
    match security {
        s if s < 100 => &[],
        s if s < 160 => &[White],
        s if s < 200 => &[White, Grey],
        s if s < 230 => &[Grey, Grey],
        _ => &[Grey, Grey, Black],
    }
}

fn net_tile(city: &City, p: Pos) -> Option<&citygen::Tile> {
    city.tile(StratumKind::Net, p)
}

fn on_lane(city: &City, p: Pos) -> bool {
    net_tile(city, p).is_some_and(|t| matches!(t.terrain, Terrain::NetLane | Terrain::NetGate))
}

/// Whether a grid piece sees you at `to`: on a lane or gate in its row or column,
/// within its grade's sight (half while you are masked), no wall between.
pub fn sees(city: &City, foe: &Foe, to: Pos, masked: bool) -> bool {
    let Some(ice) = foe.ice.as_ref() else {
        return false;
    };
    if foe.down || foe.stunned > 0 || !on_lane(city, to) {
        return false;
    }
    if foe.pos.x != to.x && foe.pos.y != to.y {
        return false;
    }
    let sight = if masked {
        ice.grade.sight() / 2
    } else {
        ice.grade.sight()
    };
    foe.pos.chebyshev(to) <= sight && crate::foes::line_clear(city, foe.place, foe.pos, to)
}

/// The district hubs on the Net: each hub's district and tile.
pub fn hubs(city: &City) -> Vec<(DistrictId, Pos)> {
    city.net
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Backbone)
        .filter_map(|n| match crate::owners::owner_of(city, n.id)? {
            Owner::District(d) => Some((d, n.cyber)),
            Owner::Building(_) => None,
        })
        .collect()
}

/// Whether a tile lies in a hub's ring.
pub fn by_a_hub(city: &City, p: Pos) -> bool {
    hubs(city).iter().any(|(_, h)| h.chebyshev(p) <= HUB_REACH)
}

/// Straight lane runs near a hub, longest first, for sentinels' beats: from lane
/// tiles within 10 of it, along each of the four directions while the lane goes on,
/// 5 to 12 tiles.
pub fn beats_near(city: &City, hub: Pos) -> Vec<Vec<Pos>> {
    let mut runs: Vec<Vec<Pos>> = Vec::new();
    for dy in -10..=10 {
        for dx in -10..=10 {
            let start = Pos::new(hub.x + dx, hub.y + dy);
            if !on_lane(city, start) || start.chebyshev(hub) <= HUB_REACH {
                continue;
            }
            for d in [Dir::E, Dir::S] {
                // Only from a run's first tile.
                if on_lane(city, start.step(d.opposite())) {
                    continue;
                }
                let mut run = vec![start];
                let mut p = start;
                while run.len() < 12 {
                    p = p.step(d);
                    if !on_lane(city, p) {
                        break;
                    }
                    run.push(p);
                }
                if run.len() >= 5 {
                    runs.push(run);
                }
            }
        }
    }
    runs.sort_by_key(|r| {
        (
            std::cmp::Reverse(r.len()),
            r[0].chebyshev(hub),
            r[0].y,
            r[0].x,
        )
    });
    // Runs that share no tile, so two sentinels don't walk one lane.
    let mut out: Vec<Vec<Pos>> = Vec::new();
    for r in runs {
        if out.iter().all(|o| o.iter().all(|p| !r.contains(p))) {
            out.push(r);
        }
    }
    out
}

/// The sentinels go out: each district's by its security, each on a beat near its
/// hub. Once a session.
pub fn post_sentinels(st: &mut AppState) {
    if st.net.grid.posted {
        return;
    }
    st.net.grid.posted = true;
    let Some(city) = st.city.as_ref() else {
        return;
    };
    let mut out = Vec::new();
    for (d, hub) in hubs(city) {
        let grades = sentinels_for(crate::owners::security(st, Owner::District(d)));
        let beats = beats_near(city, hub);
        for (grade, beat) in grades.iter().zip(beats) {
            out.push(piece(
                *grade,
                Role::Sentinel {
                    beat: beat.clone(),
                    step: 0,
                    forward: true,
                },
                Owner::District(d),
                beat[0],
            ));
        }
    }
    st.foes.extend(out);
}

/// The district an owner sits in.
pub fn district_of(st: &AppState, o: Owner) -> Option<DistrictId> {
    match o {
        Owner::District(d) => Some(d),
        Owner::Building(b) => st
            .city
            .as_ref()?
            .buildings
            .get(b.index())
            .map(|x| x.district),
    }
}

/// An owner's alarm went up from a run outside: its district's sentinels wake toward
/// where you were, and a tracer leaves the owner's source.
pub fn alarm(st: &mut AppState, o: Owner) {
    let Some(me) = st.player.as_ref().map(|p| p.pos) else {
        return;
    };
    if !st.net.grid.alarmed.contains(&o) {
        st.net.grid.alarmed.push(o);
    }
    let d = district_of(st, o);
    for f in &mut st.foes {
        if let Some(ice) = f.ice.as_ref()
            && !f.down
            && matches!(ice.role, Role::Sentinel { .. })
            && district_of_owner(ice.owner) == d
        {
            f.mode = FoeMode::Hunting { seen: me, lost: 0 };
        }
    }
    send_tracer(st, o);
}

fn district_of_owner(o: Owner) -> Option<DistrictId> {
    match o {
        Owner::District(d) => Some(d),
        Owner::Building(_) => None,
    }
}

/// A tracer leaves the owner's gate (a building) or hub (a district), of the grade
/// its security calls for, and walks to the trail.
pub fn send_tracer(st: &mut AppState, o: Owner) {
    let Some(city) = st.city.as_ref() else {
        return;
    };
    let source = match o {
        Owner::Building(b) => crate::constructs::city_gate(city, b),
        Owner::District(d) => hubs(city)
            .into_iter()
            .find(|(x, _)| *x == d)
            .map(|(_, p)| p),
    };
    let Some(source) = source else {
        return;
    };
    let grade = tracer_grade(crate::owners::security(st, o));
    // It makes for the trail point nearest it.
    let at = st
        .net
        .grid
        .trail
        .iter()
        .enumerate()
        .min_by_key(|(_, (p, _))| p.chebyshev(source))
        .map_or(0, |(i, _)| i);
    st.foes
        .push(piece(grade, Role::Tracer { at, wait: None }, o, source));
    let line = format!(
        "TRACER · {} ICE off `{}` is on your trail",
        grade.label(),
        crate::owners::name(st, o)
    );
    st.warn(line);
}

/// Records where you stand on the open Net: a new tile joins the trail (not while
/// masked); leaving the Net into a construct sends tracers to its gate, jacking out
/// sends them home.
fn record(st: &mut AppState) {
    let here = st.player.as_ref().map(|p| (p.place, p.pos));
    let last = st.net.grid.last;
    st.net.grid.last = here;
    let jacked = st.player.as_ref().is_some_and(|p| p.jacked_in());
    if !jacked {
        if !st.net.grid.trail.is_empty() || any_tracer(st) {
            st.net.grid.trail.clear();
            for f in &mut st.foes {
                if f.ice.as_ref().is_some_and(GridIce::tracer) {
                    f.down = true;
                }
            }
        }
        return;
    }
    let net = Place::Stratum(StratumKind::Net);
    match (last, here) {
        // Through a gate: the trail goes, a tracer waits at that gate.
        (Some((from, at)), Some((Place::Construct(_), _))) if from == net => {
            st.net.grid.trail.clear();
            for f in &mut st.foes {
                if let Some(GridIce {
                    role: Role::Tracer { wait, .. },
                    ..
                }) = f.ice.as_mut()
                {
                    *wait = Some(at);
                }
            }
        }
        (_, Some((place, pos))) if place == net => {
            if st.net.mask > 0 {
                return;
            }
            if st.net.grid.trail.last().map(|(p, _)| *p) != Some(pos) {
                let hub = st.city.as_ref().is_some_and(|c| by_a_hub(c, pos));
                st.net.grid.trail.push((pos, hub));
                if st.net.grid.trail.len() > TRAIL_MAX {
                    st.net.grid.trail.remove(0);
                    for f in &mut st.foes {
                        if let Some(GridIce {
                            role: Role::Tracer { at, .. },
                            ..
                        }) = f.ice.as_mut()
                        {
                            *at = at.saturating_sub(1);
                        }
                    }
                }
            }
        }
        _ => {}
    }
}

fn any_tracer(st: &AppState) -> bool {
    st.foes
        .iter()
        .any(|f| !f.down && f.ice.as_ref().is_some_and(GridIce::tracer))
}

/// The open Net's clock, each frame: your trail, sentinels out, tracers walking,
/// sentinels on their beats, sweeps ringing, and a piece that sees you engaging.
pub fn tick(st: &mut AppState) {
    record(st);
    let Some(p) = st.player.as_ref() else {
        return;
    };
    let on_net = p.place == Place::Stratum(StratumKind::Net);
    if on_net && st.grid_patrols == Patrols::Sentinels {
        post_sentinels(st);
    }
    let now = st.net.now(st);
    // An alarm that cleared lets its waiting tracers go.
    let alarmed: Vec<Owner> = st.net.grid.alarmed.clone();
    let cleared: Vec<Owner> = alarmed
        .into_iter()
        .filter(|o| !crate::owners::alert(st, *o, now))
        .collect();
    if !cleared.is_empty() {
        st.net.grid.alarmed.retain(|o| !cleared.contains(o));
        for f in &mut st.foes {
            if let Some(ice) = f.ice.as_ref()
                && ice.tracer()
                && cleared.contains(&ice.owner)
                && matches!(ice.role, Role::Tracer { wait: Some(_), .. })
            {
                f.down = true;
            }
        }
    }
    if st.net.engaged.is_some() {
        return;
    }
    let tick = st.clock_tick();
    if tick >= st.net.grid.tracer_tick + TRACER_TICKS {
        st.net.grid.tracer_tick = tick;
        step_tracers(st);
    }
    if on_net && tick >= st.net.grid.sentinel_tick + SENTINEL_TICKS {
        st.net.grid.sentinel_tick = tick;
        step_sentinels(st);
    }
    if st.grid_patrols == Patrols::Sweeps {
        sweep(st, tick);
    } else {
        st.net.grid.rings.clear();
    }
    // A piece that sees you on the open Net picks the fight.
    if on_net && st.net.engaged.is_none() && look_all(st) {
        crate::net::engage(st, Place::Stratum(StratumKind::Net), false);
    }
}

/// Every standing grid piece near you looks; true when one hunts you.
fn look_all(st: &mut AppState) -> bool {
    let Some(me) = st.player.as_ref().map(|p| p.pos) else {
        return false;
    };
    let mut any = false;
    for i in 0..st.foes.len() {
        let f = &st.foes[i];
        if f.ice.is_none() || f.down || f.pos.chebyshev(me) > FIGHT_RADIUS {
            continue;
        }
        any |= crate::foes::look(st, i);
    }
    any
}

/// Tracers walk a tile each along the trail; beside you, one latches.
fn step_tracers(st: &mut AppState) {
    let Some((place, me)) = st.player.as_ref().map(|p| (p.place, p.pos)) else {
        return;
    };
    let on_net = place == Place::Stratum(StratumKind::Net);
    for i in 0..st.foes.len() {
        let Some(GridIce {
            role: Role::Tracer { at, wait },
            ..
        }) = st.foes[i].ice.clone()
        else {
            continue;
        };
        if st.foes[i].down {
            continue;
        }
        let pos = st.foes[i].pos;
        if on_net && pos.chebyshev(me) <= 1 {
            latch(st, i);
            return;
        }
        let goal = match wait {
            Some(gate) => gate,
            None => {
                let Some(&(goal, hub)) = st.net.grid.trail.get(at) else {
                    // The trail ran out: a mask, or it never had one.
                    st.foes[i].down = true;
                    st.say("the tracer loses your trail");
                    continue;
                };
                if pos == goal {
                    if hub {
                        st.foes[i].down = true;
                        st.say("the tracer loses you in the hub's traffic");
                        continue;
                    }
                    // On to the next point; at the head of the trail it waits for you.
                    if at + 1 < st.net.grid.trail.len()
                        && let Some(GridIce {
                            role: Role::Tracer { at: a, .. },
                            ..
                        }) = st.foes[i].ice.as_mut()
                    {
                        *a = at + 1;
                    }
                    continue;
                }
                goal
            }
        };
        if pos == goal {
            continue;
        }
        let Some(city) = st.city.as_ref() else {
            return;
        };
        let to = if pos.chebyshev(goal) <= 1 {
            Some(goal)
        } else {
            crate::foes::step_toward(city, st.foes[i].place, pos, goal, &[me])
        };
        if let Some(to) = to.filter(|t| !on_net || *t != me) {
            st.foes[i].pos = to;
        }
    }
}

/// A tracer beside you latches: dumped.
pub fn latch(st: &mut AppState, i: usize) {
    let name = st.foes.get(i).map_or_else(String::new, |f| f.name.clone());
    if let Some(f) = st.foes.get_mut(i) {
        f.down = true;
    }
    crate::net::dump(st, &format!("the {name} latched on"));
}

/// Asleep sentinels take a step along their beat, there and back; woken ones (an
/// alarm) walk toward where they last had you, and lose you there.
fn step_sentinels(st: &mut AppState) {
    let woken: Vec<usize> = st
        .foes
        .iter()
        .enumerate()
        .filter(|(_, f)| {
            !f.down
                && f.hunting()
                && f.ice
                    .as_ref()
                    .is_some_and(|i| matches!(i.role, Role::Sentinel { .. }))
        })
        .map(|(i, _)| i)
        .collect();
    for i in woken {
        crate::foes::step(st, i, true, false);
    }
    let me = st.player.as_ref().map(|p| p.pos);
    for f in &mut st.foes {
        if f.down || f.stunned > 0 || f.hunting() {
            continue;
        }
        let Some(GridIce {
            role:
                Role::Sentinel {
                    beat,
                    step,
                    forward,
                },
            ..
        }) = f.ice.as_mut()
        else {
            continue;
        };
        if beat.len() < 2 {
            continue;
        }
        let home = beat[(*step).min(beat.len() - 1)];
        if f.pos != home {
            // Back to the beat after a hunt: straight toward its spot on it.
            let (dx, dy) = ((home.x - f.pos.x).signum(), (home.y - f.pos.y).signum());
            let to = Pos::new(f.pos.x + dx, f.pos.y + dy);
            if Some(to) != me {
                f.pos = to;
            }
            continue;
        }
        let last = beat.len() - 1;
        if *forward && *step >= last {
            *forward = false;
        } else if !*forward && *step == 0 {
            *forward = true;
        }
        let next = if *forward { *step + 1 } else { *step - 1 };
        if Some(beat[next]) != me {
            *step = next;
            f.pos = beat[next];
        }
    }
}

/// Sweeps: every alarmed district's hub rings out every `SWEEP_BEATS` beats; a ring
/// crossing your tile on the open Net heats the district's trace.
fn sweep(st: &mut AppState, tick: u64) {
    let Some(city) = st.city.as_ref() else {
        return;
    };
    let now = st.net.now(st);
    let districts: Vec<DistrictId> =
        hubs(city)
            .into_iter()
            .map(|(d, _)| d)
            .filter(|d| {
                crate::owners::alert(st, Owner::District(*d), now)
                    || st.net.grid.alarmed.iter().any(|o| {
                        district_of(st, *o) == Some(*d) && crate::owners::alert(st, *o, now)
                    })
            })
            .collect();
    if !districts.is_empty() && tick >= st.net.grid.ring_tick + SWEEP_BEATS * crate::net::BEAT_TICKS
    {
        st.net.grid.ring_tick = tick;
        for (d, hub) in hubs(city) {
            if districts.contains(&d) {
                st.net.grid.rings.push(Ring {
                    district: d,
                    hub,
                    born: tick,
                });
            }
        }
    }
    let me = st
        .player
        .as_ref()
        .filter(|p| p.place == Place::Stratum(StratumKind::Net))
        .map(|p| p.pos);
    let mut heat: Vec<DistrictId> = Vec::new();
    for r in &st.net.grid.rings {
        let (was, is) = (r.radius(tick.saturating_sub(1)), r.radius(tick));
        if let Some(me) = me {
            let d = ring_distance(r.hub, me);
            if was < d && d <= is {
                heat.push(r.district);
            }
        }
    }
    st.net
        .grid
        .rings
        .retain(|r| r.radius(tick) <= RING_MAX && districts.contains(&r.district));
    for d in heat {
        st.warn("a sweep passes over you");
        crate::owners::heat_on(st, Owner::District(d), RING_HEAT);
    }
}

/// How far a tile is from a hub for a ring: a cell is twice as tall as wide.
pub fn ring_distance(hub: Pos, p: Pos) -> i32 {
    let (dx, dy) = ((p.x - hub.x) as f32, (2 * (p.y - hub.y)) as f32);
    (dx * dx + dy * dy).sqrt().round() as i32
}

/// A grid piece's hit, by its grade, as construct ICE hits; a tracer's hit is its
/// latch.
pub fn hit(st: &mut AppState, i: usize) {
    let Some(f) = st.foes.get(i) else {
        return;
    };
    let Some(ice) = f.ice.clone() else {
        return;
    };
    if ice.tracer() {
        latch(st, i);
        return;
    }
    let security = crate::owners::security(st, ice.owner);
    crate::net::hit_by(st, ice.grade, security, f.pos, ice.owner);
}

/// A program's hit on a grid piece: integrity falls, it turns on you, and at zero it
/// breaks. The log line.
pub fn hit_piece(st: &mut AppState, i: usize, by: &str, damage: u8) -> Option<String> {
    let me = st.player.as_ref()?.pos;
    let f = st.foes.get_mut(i)?;
    f.ice.as_ref()?;
    f.hp = f.hp.saturating_sub(damage);
    f.mode = FoeMode::Hunting { seen: me, lost: 0 };
    let (left, max, name) = (f.hp, f.hp_max, f.name.clone());
    if left == 0 {
        f.down = true;
        if st
            .net
            .run
            .as_ref()
            .is_some_and(|r| r.target == game::HackTarget::Foe(i))
        {
            st.net.run = None;
        }
        let line = format!("{name} broken");
        st.say(line.clone());
        return Some(line);
    }
    let line = format!("{by} · {name} {left}/{max}");
    st.say(line.clone());
    Some(line)
}

/// A crack lands on a grid piece: it lets you go.
pub fn lift(st: &mut AppState, i: usize) -> String {
    match st.foes.get_mut(i) {
        Some(f) => {
            f.down = true;
            format!("{} cracked · it lets you go", f.name)
        }
        None => "nothing there".to_string(),
    }
}

/// The grid pieces a program reaches from `pos`: beside you, or in sight at range.
pub fn in_reach(st: &AppState, pos: Pos) -> Vec<(usize, String)> {
    let Some(city) = st.city.as_ref() else {
        return Vec::new();
    };
    let net = Place::Stratum(StratumKind::Net);
    st.foes
        .iter()
        .enumerate()
        .filter(|(_, f)| f.ice.is_some() && !f.down && f.place == net)
        .filter(|(_, f)| {
            let d = f.pos.chebyshev(pos);
            d <= 1
                || (d <= crate::net::ATTACK_RANGE && crate::foes::line_clear(city, net, pos, f.pos))
        })
        .map(|(i, f)| (i, f.name.clone()))
        .collect()
}

/// Whether a grid piece near you hunts you.
pub fn hunted(st: &AppState) -> bool {
    let Some(p) = st
        .player
        .as_ref()
        .filter(|p| p.place == Place::Stratum(StratumKind::Net))
    else {
        return false;
    };
    st.foes.iter().any(|f| {
        f.ice.is_some()
            && !f.down
            && f.place == p.place
            && f.hunting()
            && f.pos.chebyshev(p.pos) <= FIGHT_RADIUS
    })
}

/// The rings to draw on the Net map.
pub struct RingOverlay<'a> {
    pub rings: &'a [Ring],
    pub tick: u64,
}

const RING: ratatui::style::Color = ratatui::style::Color::Rgb(255, 0, 200);
const RING_FAINT: ratatui::style::Color = ratatui::style::Color::Rgb(70, 0, 55);

impl crate::render::viewport::Overlay for RingOverlay<'_> {
    fn draw(
        &self,
        _map: &crate::render::map::MapRef,
        origin: Pos,
        area: ratatui::layout::Rect,
        _tick: u64,
        buf: &mut ratatui::buffer::Buffer,
        _style: &crate::render::style::StylePack,
    ) {
        for r in self.rings {
            let radius = r.radius(self.tick);
            for sy in area.y..area.bottom() {
                for sx in area.x..area.right() {
                    let p = Pos::new(
                        sx as i32 - area.x as i32 + origin.x,
                        sy as i32 - area.y as i32 + origin.y,
                    );
                    let d = ring_distance(r.hub, p);
                    if d == radius {
                        buf[(sx, sy)].set_bg(RING);
                    } else if d < radius && d >= radius - 3 {
                        buf[(sx, sy)].set_bg(RING_FAINT);
                    }
                }
            }
        }
    }
}

/// The straight-line path a tile is on, for tests.
#[cfg(test)]
fn lane_tiles(city: &City) -> Vec<Pos> {
    let net = city.stratum(StratumKind::Net).unwrap();
    net.tiles
        .iter()
        .filter(|(_, t)| t.terrain == Terrain::NetLane)
        .map(|(p, _)| p)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::net::tests::under_kiroflux;

    /// You on the Net at a lane tile with a grid piece `d` tiles east of you on the
    /// same lane, or none such.
    fn on_a_lane(st: &mut AppState, d: i32) -> Option<(Pos, Pos)> {
        let city = st.city.as_ref().unwrap();
        let lanes = lane_tiles(city);
        let (me, them) = lanes.iter().find_map(|p| {
            let q = Pos::new(p.x + d, p.y);
            ((0..=d).all(|k| on_lane(city, Pos::new(p.x + k, p.y)))).then_some((*p, q))
        })?;
        let pl = st.player.as_mut().unwrap();
        pl.place = Place::Stratum(StratumKind::Net);
        pl.pos = me;
        Some((me, them))
    }

    fn district(st: &AppState) -> DistrictId {
        st.city.as_ref().unwrap().districts[0].id
    }

    #[test]
    fn grid_ice_sees_along_lanes_not_through_walls() {
        let (mut st, _) = under_kiroflux();
        let (me, them) = on_a_lane(&mut st, 4).expect("a straight lane");
        let city = st.city.as_ref().unwrap();
        let o = Owner::District(district(&st));
        let f = piece(IceGrade::Grey, Role::Tracer { at: 0, wait: None }, o, them);
        assert!(sees(city, &f, me, false), "along the lane");
        assert!(!sees(city, &f, me, true), "masked, half the sight");
        let off = piece(
            IceGrade::Grey,
            Role::Tracer { at: 0, wait: None },
            o,
            Pos::new(them.x, them.y + 1),
        );
        assert!(!sees(city, &off, me, false), "not off its row or column");
        // A Net wall between: nothing.
        let wall = city
            .stratum(StratumKind::Net)
            .unwrap()
            .tiles
            .iter()
            .find(|(p, t)| {
                t.terrain == Terrain::NetWall
                    && on_lane(city, Pos::new(p.x - 1, p.y))
                    && on_lane(city, Pos::new(p.x + 1, p.y))
            })
            .map(|(p, _)| p);
        if let Some(w) = wall {
            let f = piece(
                IceGrade::Black,
                Role::Tracer { at: 0, wait: None },
                o,
                Pos::new(w.x + 1, w.y),
            );
            assert!(!sees(city, &f, Pos::new(w.x - 1, w.y), false));
        }
        assert!(crate::foes::blocks_sight(&citygen::Tile {
            terrain: Terrain::NetWall,
            ..citygen::Tile::VOID
        }));
    }

    #[test]
    fn far_grid_ice_stays_out_of_the_fight() {
        let (mut st, _) = under_kiroflux();
        let me = st.player.as_ref().unwrap().pos;
        let o = Owner::District(district(&st));
        let mut near = piece(
            IceGrade::Grey,
            Role::Tracer { at: 0, wait: None },
            o,
            Pos::new(me.x + 3, me.y),
        );
        near.mode = FoeMode::Hunting { seen: me, lost: 0 };
        let mut far = near.clone();
        far.pos = Pos::new(me.x + FIGHT_RADIUS + 5, me.y);
        st.foes.push(near);
        st.foes.push(far);
        let known = crate::foes::in_memory(&st);
        assert_eq!(known, vec![0], "only the near one counts");
        crate::net::engage(&mut st, Place::Stratum(StratumKind::Net), false);
        let e = st.net.engaged.as_ref().unwrap();
        assert!(e.order.contains(&crate::net::Actor::Foe(0)));
        assert!(!e.order.contains(&crate::net::Actor::Foe(1)));
        assert!(hunted(&st));
    }

    #[test]
    fn attack_takes_grid_ice_integrity() {
        let (mut st, _) = under_kiroflux();
        let me = st.player.as_ref().unwrap().pos;
        let o = Owner::District(district(&st));
        st.foes.push(piece(
            IceGrade::Grey,
            Role::Tracer { at: 0, wait: None },
            o,
            Pos::new(me.x + 1, me.y),
        ));
        let line = hit_piece(&mut st, 0, "attack", 2).unwrap();
        assert_eq!(line, "attack · grey tracer 4/6");
        assert!(st.foes[0].hunting());
        hit_piece(&mut st, 0, "attack", 9);
        assert!(st.foes[0].down, "broken");
        let t = st.foes.len();
        st.foes.push(piece(
            IceGrade::White,
            Role::Sentinel {
                beat: vec![],
                step: 0,
                forward: true,
            },
            o,
            Pos::new(me.x + 1, me.y),
        ));
        assert!(lift(&mut st, t).contains("cracked"));
        assert!(st.foes[t].down);
    }

    #[test]
    fn a_program_reaches_grid_ice_from_the_net() {
        let (mut st, _) = under_kiroflux();
        st.grid_patrols = Patrols::Sweeps;
        let (me, them) = on_a_lane(&mut st, 1).expect("a lane");
        let o = Owner::District(district(&st));
        st.foes.push(piece(
            IceGrade::Grey,
            Role::Sentinel {
                beat: vec![],
                step: 0,
                forward: true,
            },
            o,
            them,
        ));
        let targets = crate::net::targets_outside(&st, me);
        assert!(targets.contains(&(game::HackTarget::Foe(0), "grey ICE".to_string())));
        let attack = game::ItemId::new("attack");
        if !st.character.loaded.contains(&attack) {
            st.character.loaded.push(attack.clone());
        }
        st.net.force_roll = Some(10);
        crate::net::start_run(&mut st, attack, game::HackTarget::Foe(0)).unwrap();
        assert!(
            st.net.engaged.is_some(),
            "pointing a program at it picks the fight"
        );
        crate::net::step_run(&mut st);
        assert!(st.foes[0].hp < st.foes[0].hp_max, "the attack lands");
        assert!(crate::owners::heat(&st, o) > 0, "on its owner's trace");
    }

    #[test]
    fn grid_ice_hits_by_grade() {
        let (mut st, _) = under_kiroflux();
        let me = st.player.as_ref().unwrap().pos;
        let d = district(&st);
        let o = Owner::District(d);
        st.foes.push(piece(
            IceGrade::White,
            Role::Sentinel {
                beat: vec![],
                step: 0,
                forward: true,
            },
            o,
            Pos::new(me.x + 1, me.y),
        ));
        hit(&mut st, 0);
        assert!(crate::owners::heat(&st, o) > 0, "white: heat on its owner");
        let focus = st.character.focus;
        st.foes.push(piece(
            IceGrade::Grey,
            Role::Sentinel {
                beat: vec![],
                step: 0,
                forward: true,
            },
            o,
            Pos::new(me.x + 1, me.y),
        ));
        hit(&mut st, 1);
        assert!(st.character.focus < focus, "grey: focus");
        let hp = st.character.hp;
        st.foes.push(piece(
            IceGrade::Black,
            Role::Sentinel {
                beat: vec![],
                step: 0,
                forward: true,
            },
            o,
            Pos::new(me.x + 1, me.y),
        ));
        hit(&mut st, 2);
        assert!(st.character.hp < hp, "black: HP");
    }

    #[test]
    fn sentinels_by_district_security() {
        assert!(sentinels_for(80).is_empty());
        assert_eq!(sentinels_for(150), &[IceGrade::White]);
        assert_eq!(sentinels_for(190), &[IceGrade::White, IceGrade::Grey]);
        assert_eq!(sentinels_for(220), &[IceGrade::Grey, IceGrade::Grey]);
        assert_eq!(
            sentinels_for(240),
            &[IceGrade::Grey, IceGrade::Grey, IceGrade::Black]
        );
        let (mut st, _) = under_kiroflux();
        tick(&mut st);
        assert!(st.net.grid.posted);
        let n = st.foes.iter().filter(|f| f.ice.is_some()).count();
        let city = st.city.as_ref().unwrap();
        let want: usize = hubs(city)
            .iter()
            .map(|(d, h)| {
                sentinels_for(crate::owners::security(&st, Owner::District(*d)))
                    .len()
                    .min(beats_near(city, *h).len())
            })
            .sum();
        assert_eq!(n, want);
        assert!(n > 0, "seed 8 keeps some");
        // Once a session.
        tick(&mut st);
        assert_eq!(st.foes.iter().filter(|f| f.ice.is_some()).count(), n);
    }

    #[test]
    fn a_sentinel_walks_its_beat() {
        let (mut st, _) = under_kiroflux();
        let city = st.city.as_ref().unwrap();
        let (d, hub) = hubs(city)[0];
        let beat = beats_near(city, hub).remove(0);
        st.foes.push(piece(
            IceGrade::White,
            Role::Sentinel {
                beat: beat.clone(),
                step: 0,
                forward: true,
            },
            Owner::District(d),
            beat[0],
        ));
        // Far from you so it never sees you.
        st.player.as_mut().unwrap().pos = Pos::new(hub.x + 60, hub.y + 40);
        for _ in 0..beat.len() - 1 {
            step_sentinels(&mut st);
        }
        assert_eq!(st.foes[0].pos, *beat.last().unwrap(), "out to the end");
        step_sentinels(&mut st);
        assert_eq!(st.foes[0].pos, beat[beat.len() - 2], "and back");
    }

    #[test]
    fn an_alarm_outside_wakes_the_sentinels_and_sends_a_tracer() {
        let (mut st, b) = under_kiroflux();
        tick(&mut st);
        let d = st.city.as_ref().unwrap().buildings[b.index()].district;
        let ours: Vec<usize> = st
            .foes
            .iter()
            .enumerate()
            .filter(|(_, f)| {
                f.ice
                    .as_ref()
                    .is_some_and(|i| i.owner == Owner::District(d))
            })
            .map(|(i, _)| i)
            .collect();
        let before = st.foes.len();
        alarm(&mut st, Owner::Building(b));
        for i in &ours {
            assert!(st.foes[*i].hunting(), "its district's sentinels wake");
        }
        assert_eq!(st.foes.len(), before + 1, "a tracer");
        let t = st.foes.last().unwrap();
        assert!(t.ice.as_ref().unwrap().tracer());
        let gate = crate::constructs::city_gate(st.city.as_ref().unwrap(), b).unwrap();
        assert_eq!(t.pos, gate, "off the building's gate");
        assert_eq!(
            t.ice.as_ref().unwrap().grade,
            IceGrade::White,
            "Kiroflux is 114"
        );
    }

    /// A trail of `n` tiles east along a lane, with you at its head.
    fn walk_east(st: &mut AppState, n: i32) -> Vec<Pos> {
        let (me, _) = on_a_lane(st, n).expect("a straight lane");
        let mut out = Vec::new();
        for k in 0..=n {
            let p = Pos::new(me.x + k, me.y);
            st.player.as_mut().unwrap().pos = p;
            record(st);
            out.push(p);
        }
        out
    }

    #[test]
    fn the_trail_records_your_steps_on_the_grid() {
        let (mut st, _) = under_kiroflux();
        let walked = walk_east(&mut st, 6);
        let trail: Vec<Pos> = st.net.grid.trail.iter().map(|(p, _)| *p).collect();
        assert!(trail.ends_with(&walked), "{trail:?}");
        // Jacked out, it's gone.
        st.player.as_mut().unwrap().body = None;
        record(&mut st);
        assert!(st.net.grid.trail.is_empty());
    }

    #[test]
    fn a_mask_leaves_no_trail() {
        let (mut st, _) = under_kiroflux();
        st.net.mask = 3;
        walk_east(&mut st, 4);
        assert!(st.net.grid.trail.is_empty());
        // A tracer sent now has nothing to follow, and is gone.
        let d = district(&st);
        send_tracer(&mut st, Owner::District(d));
        step_tracers(&mut st);
        assert!(st.foes.last().unwrap().down);
    }

    #[test]
    fn a_tracer_follows_the_trail_and_latches() {
        let (mut st, _) = under_kiroflux();
        let walked = walk_east(&mut st, 8);
        let d = district(&st);
        st.foes.push(piece(
            IceGrade::White,
            Role::Tracer { at: 0, wait: None },
            Owner::District(d),
            walked[0],
        ));
        st.net.grid.trail.iter_mut().for_each(|t| t.1 = false);
        for _ in 0..40 {
            if st.net.cooling_until.is_some() {
                break;
            }
            step_tracers(&mut st);
        }
        assert!(st.net.cooling_until.is_some(), "it latched: dumped");
        assert!(st.log.iter().any(|e| e.text.contains("latched on")));
    }

    #[test]
    fn a_hub_loses_the_tracer() {
        let (mut st, _) = under_kiroflux();
        let walked = walk_east(&mut st, 8);
        let d = district(&st);
        st.foes.push(piece(
            IceGrade::White,
            Role::Tracer { at: 0, wait: None },
            Owner::District(d),
            walked[0],
        ));
        // Say you passed a hub halfway.
        st.net.grid.trail.iter_mut().for_each(|t| t.1 = false);
        let half = st.net.grid.trail.len() - 4;
        st.net.grid.trail[half].1 = true;
        for _ in 0..40 {
            step_tracers(&mut st);
        }
        assert!(st.foes.last().unwrap().down, "gone at the hub");
        assert!(st.net.cooling_until.is_none());
    }

    #[test]
    fn sweeps_ring_out_and_heat_you() {
        let (mut st, _) = under_kiroflux();
        st.grid_patrols = Patrols::Sweeps;
        let city = st.city.as_ref().unwrap();
        let (d, hub) = hubs(city)[0];
        st.player.as_mut().unwrap().pos = Pos::new(hub.x + 6, hub.y);
        // The district on alert.
        crate::owners::heat_on(&mut st, Owner::District(d), game::ALARM_HEAT);
        let heat = crate::owners::heat(&st, Owner::District(d));
        let start = st.clock_tick() + SWEEP_BEATS * crate::net::BEAT_TICKS;
        for t in start..start + 20 {
            sweep(&mut st, t);
        }
        assert!(
            crate::owners::heat(&st, Owner::District(d)) > heat,
            "a ring crossed you"
        );
        assert!(!st.net.grid.rings.is_empty());
        assert_eq!(ring_distance(Pos::new(0, 0), Pos::new(0, 3)), 6);
    }

    #[test]
    fn a_woken_sentinel_walks_toward_you_in_calm() {
        let (mut st, _) = under_kiroflux();
        let (me, them) = on_a_lane(&mut st, 12).expect("a straight lane");
        let o = Owner::District(district(&st));
        let mut f = piece(
            IceGrade::Grey,
            Role::Sentinel {
                beat: vec![them],
                step: 0,
                forward: true,
            },
            o,
            them,
        );
        f.mode = FoeMode::Hunting { seen: me, lost: 0 };
        st.foes.push(f);
        step_sentinels(&mut st);
        assert!(
            st.foes[0].pos.chebyshev(me) < them.chebyshev(me),
            "a step closer"
        );
    }

    #[test]
    fn a_district_alarm_locks_its_street_ports() {
        let (mut st, _) = under_kiroflux();
        // On foot at a street terminal outside any building.
        let city = st.city.as_ref().unwrap();
        let street = city.stratum(StratumKind::Street).unwrap();
        let (port, d) = street
            .tiles
            .iter()
            .find(|(_, t)| t.building.is_none() && t.feature == Some(citygen::Feature::Terminal))
            .map(|(p, t)| (p, t.district.unwrap()))
            .unwrap();
        let stand = port
            .neighbors8()
            .find(|q| city.walkable(StratumKind::Street, *q))
            .unwrap();
        let p = st.player.as_mut().unwrap();
        p.place = Place::Stratum(StratumKind::Street);
        p.pos = stand;
        p.body = None;
        assert_eq!(crate::net::jack_in_refusal(&st), None);
        crate::owners::heat_on(&mut st, Owner::District(d), game::ALARM_HEAT);
        assert_eq!(
            crate::net::jack_in_refusal(&st),
            Some("port locked down".to_string())
        );
    }

    #[test]
    fn the_switch_swaps_them() {
        let (mut st, _) = under_kiroflux();
        st.grid_patrols = Patrols::Sweeps;
        tick(&mut st);
        assert!(
            st.foes.iter().all(|f| f.ice.is_none()),
            "no sentinels on sweeps"
        );
        assert_eq!(Patrols::Sweeps.next(), Patrols::Sentinels);
        st.grid_patrols = Patrols::Sentinels;
        tick(&mut st);
        assert!(st.foes.iter().any(|f| f.ice.is_some()));
    }

    #[test]
    fn tracers_go_when_you_jack_out_and_wait_at_a_gate() {
        let (mut st, b) = under_kiroflux();
        walk_east(&mut st, 3);
        send_tracer(&mut st, Owner::Building(b));
        // Into a construct: it waits at the gate you went in by.
        let gate = st.player.as_ref().unwrap().pos;
        st.player.as_mut().unwrap().place = Place::Construct(b);
        record(&mut st);
        let t = st.foes.last().unwrap();
        assert!(matches!(
            t.ice.as_ref().unwrap().role,
            Role::Tracer { wait: Some(g), .. } if g == gate
        ));
        // Jacked out: gone.
        st.player.as_mut().unwrap().body = None;
        record(&mut st);
        assert!(st.foes.last().unwrap().down);
    }
}
