//! Foes on foot: who stands against you on a map, what they see, how they read
//! and how they are listed. Their turns are in `net` (the engine) and here (the step).

use crate::app::AppState;
use crate::render::Visibility;
use citygen::pathing::{astar, bresenham};
use citygen::{City, Place, Pos, Terrain, Tile};
use game::ItemId;

pub const GOON_HP: u8 = 30;
pub const GOON_REFLEX: u8 = 5;
pub const GOON_SIGHT: i32 = 8;
pub const GOON_MOVE: u8 = 3;
pub const GOON_AP: u8 = 1;

/// What a foe is doing.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum FoeMode {
    Idle,
    Hunting { seen: Pos, lost: u8 },
}

/// Someone with a gun on your map this session.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Foe {
    pub name: String,
    pub place: Place,
    pub pos: Pos,
    pub hp: u8,
    pub hp_max: u8,
    pub reflex: u8,
    /// What it shoots with; unarmed without one.
    pub weapon: Option<ItemId>,
    pub sight: i32,
    pub mode: FoeMode,
    /// Turns left standing stunned: it neither looks, moves nor fires.
    pub stunned: u8,
    pub down: bool,
    /// A target and nothing more: it never hunts, moves or fires, and nothing
    /// gets through it.
    pub inert: bool,
    /// How it is drawn.
    pub glyph: char,
    /// Wired meat: what a chrome-only blast (an EMP) hurts.
    pub chrome: bool,
    /// Read by Deadeye: your chance on it is better for the fight.
    pub marked: bool,
    /// ICE on the open Net: its grade, role and owner (`grid`).
    pub ice: Option<crate::grid::GridIce>,
}

impl Foe {
    /// A street goon with a pistol.
    pub fn goon(place: Place, pos: Pos) -> Foe {
        Foe {
            name: "goon".to_string(),
            place,
            pos,
            hp: GOON_HP,
            hp_max: GOON_HP,
            reflex: GOON_REFLEX,
            weapon: Some(ItemId::new("scrap_nine")),
            sight: GOON_SIGHT,
            mode: FoeMode::Idle,
            stunned: 0,
            down: false,
            inert: false,
            glyph: '@',
            chrome: true,
            marked: false,
            ice: None,
        }
    }

    /// A training dummy: stands there and takes it.
    pub fn dummy(place: Place, pos: Pos) -> Foe {
        Foe {
            name: "dummy".to_string(),
            place,
            pos,
            hp: GOON_HP,
            hp_max: GOON_HP,
            reflex: 0,
            weapon: None,
            sight: 0,
            mode: FoeMode::Idle,
            stunned: 0,
            down: false,
            inert: true,
            glyph: '&',
            chrome: true,
            marked: false,
            ice: None,
        }
    }

    pub fn standing(&self) -> bool {
        !self.down
    }

    pub fn hunting(&self) -> bool {
        matches!(self.mode, FoeMode::Hunting { .. })
    }
}

/// A tile nothing is seen or shot through.
pub fn blocks_sight(t: &Tile) -> bool {
    matches!(
        t.terrain,
        Terrain::Wall | Terrain::Door | Terrain::Window | Terrain::Mass | Terrain::NetWall
    )
}

/// Nothing blocks the line between two tiles of a stratum, the ends excluded.
/// False off a stratum (floors are not shot across yet) or off the map.
pub fn line_clear(city: &City, place: Place, from: Pos, to: Pos) -> bool {
    let Some(k) = place.stratum() else {
        return false;
    };
    bresenham(from, to)
        .into_iter()
        .filter(|p| *p != from && *p != to)
        .all(|p| city.tile(k, p).is_some_and(|t| !blocks_sight(t)))
}

/// A standing foe sees a tile within its sight with a clear line to it.
pub fn sees(city: &City, foe: &Foe, to: Pos) -> bool {
    foe.standing() && foe.pos.chebyshev(to) <= foe.sight && line_clear(city, foe.place, foe.pos, to)
}

/// The standing foe on a tile.
pub fn foe_at(st: &AppState, place: Place, pos: Pos) -> Option<usize> {
    st.foes
        .iter()
        .position(|f| f.standing() && f.place == place && f.pos == pos)
}

/// Every standing foe of a place.
pub fn standing(st: &AppState, place: Place) -> Vec<usize> {
    st.foes
        .iter()
        .enumerate()
        .filter(|(_, f)| f.standing() && f.place == place)
        .map(|(i, _)| i)
        .collect()
}

/// Standing foes on the player's map whose tile is seen at least as well as
/// `least`, nearest first.
fn known(st: &AppState, least: Visibility) -> Vec<usize> {
    let Some(p) = st.player.as_ref() else {
        return Vec::new();
    };
    let vis = p.vis();
    // On the open Net everything is seen: only pieces near you count.
    let net = p.place == Place::Stratum(citygen::StratumKind::Net);
    let mut out: Vec<usize> = standing(st, p.place)
        .into_iter()
        .filter(|i| !net || st.foes[*i].pos.chebyshev(p.pos) <= crate::grid::FIGHT_RADIUS)
        .filter(|i| {
            if net {
                return true;
            }
            vis.get(st.foes[*i].pos).is_some_and(|v| match least {
                Visibility::Visible => *v == Visibility::Visible,
                _ => *v != Visibility::Unseen,
            })
        })
        .collect();
    out.sort_by_key(|i| (st.foes[*i].pos.chebyshev(p.pos), *i));
    out
}

/// Standing foes in the player's sight, nearest first.
pub fn in_sight(st: &AppState) -> Vec<usize> {
    known(st, Visibility::Visible)
}

/// Standing foes in the player's sight memory, nearest first.
pub fn in_memory(st: &AppState) -> Vec<usize> {
    known(st, Visibility::Remembered)
}

/// Any standing foe of the place hunts.
pub fn any_hunting(st: &AppState, place: Place) -> bool {
    // On the open Net only what hunts near you holds a fight.
    let near = |f: &Foe| {
        place != Place::Stratum(citygen::StratumKind::Net)
            || st
                .player
                .as_ref()
                .is_some_and(|p| f.pos.chebyshev(p.pos) <= crate::grid::FIGHT_RADIUS)
    };
    st.foes
        .iter()
        .any(|f| f.standing() && f.place == place && f.hunting() && near(f))
}

/// A foe looks: idle and seeing you, it hunts; hunting and seeing you, it marks
/// where. Returns whether it hunts after looking. A stunned or down foe sees nothing.
pub fn look(st: &mut AppState, i: usize) -> bool {
    let Some((city, me)) = st
        .city
        .as_ref()
        .zip(st.player.as_ref().map(|p| (p.place, p.pos)))
    else {
        return false;
    };
    let Some(foe) = st.foes.get(i) else {
        return false;
    };
    if foe.down || foe.inert || foe.stunned > 0 || foe.place != me.0 {
        return foe.hunting();
    }
    // A tracer always knows where you are: it follows the trail.
    if foe.ice.as_ref().is_some_and(crate::grid::GridIce::tracer) && foe.hunting() {
        st.foes[i].mode = FoeMode::Hunting {
            seen: me.1,
            lost: 0,
        };
        return true;
    }
    let seen = match &foe.ice {
        // On the open Net a piece only takes you for a threat while you're traced.
        Some(ice) => {
            (foe.hunting() || crate::grid::threat(st, ice))
                && crate::grid::sees(city, foe, me.1, st.net.mask > 0)
        }
        None => sees(city, foe, me.1),
    };
    if seen {
        st.foes[i].mode = FoeMode::Hunting {
            seen: me.1,
            lost: 0,
        };
        return true;
    }
    foe.hunting()
}

/// Every standing foe on the player's map looks; true when any hunts.
pub fn look_all(st: &mut AppState) -> bool {
    let Some((place, me)) = st.player.as_ref().map(|p| (p.place, p.pos)) else {
        return false;
    };
    // On the open Net only the pieces near you count: one hunting across the city
    // doesn't put you on turns.
    let net = place == Place::Stratum(citygen::StratumKind::Net);
    let mut any = false;
    for i in standing(st, place) {
        if net && st.foes[i].pos.chebyshev(me) > crate::grid::FIGHT_RADIUS {
            continue;
        }
        any |= look(st, i);
    }
    any
}

/// One step along the shortest walkable path from `from` to `goal`, never onto a
/// tile in `avoid` (the player's, other foes') unless it is the goal itself.
pub fn step_toward(city: &City, place: Place, from: Pos, goal: Pos, avoid: &[Pos]) -> Option<Pos> {
    if from == goal {
        return None;
    }
    let k = place.stratum()?;
    let path = astar(city.size, from, goal, |_, to| {
        if avoid.contains(&to) && to != goal {
            return None;
        }
        city.walkable(k, to).then_some(1)
    })?;
    path.into_iter().find(|p| *p != from)
}

/// A foe's turn begins: a stun counts down and the turn passes; else it looks, and
/// acts only when it hunts. A down foe never does.
pub fn turn_start(st: &mut AppState, i: usize) -> bool {
    let Some(foe) = st.foes.get_mut(i) else {
        return false;
    };
    if foe.down {
        return false;
    }
    if foe.stunned > 0 {
        foe.stunned -= 1;
        let name = foe.name.clone();
        st.say(format!("{name} · stunned"));
        return false;
    }
    look(st, i)
}

/// The weapon a foe carries: its reach, for the turn; nothing unarmed.
fn weapon_range(st: &AppState, foe: &Foe) -> i32 {
    foe.weapon
        .as_ref()
        .and_then(|id| st.content.item(id))
        .and_then(|i| i.weapon.as_ref())
        .map_or(0, |w| w.range)
}

/// Where a foe standing in a live footprint goes: of the walkable neighbours that
/// are free, one outside every footprint, nearest `seen`, else the greatest ring.
fn step_out(st: &AppState, i: usize, seen: Pos, live: &[(Pos, i32)]) -> Option<Pos> {
    let foe = &st.foes[i];
    let city = st.city.as_ref()?;
    let k = foe.place.stratum()?;
    let me = st.player.as_ref().map(|p| p.pos);
    let ring = |p: Pos| live.iter().filter(|(q, _)| *q == p).map(|(_, r)| *r).max();
    let free: Vec<Pos> = (-1..=1)
        .flat_map(|dy| (-1..=1).map(move |dx| (dx, dy)))
        .filter(|d| *d != (0, 0))
        .map(|(dx, dy)| Pos::new(foe.pos.x + dx, foe.pos.y + dy))
        .filter(|p| {
            city.walkable(k, *p)
                && Some(*p) != me
                && !st
                    .foes
                    .iter()
                    .enumerate()
                    .any(|(j, f)| j != i && f.standing() && f.place == foe.place && f.pos == *p)
        })
        .collect();
    free.iter()
        .copied()
        .filter(|p| ring(*p).is_none())
        .min_by_key(|p| p.chebyshev(seen))
        .or_else(|| free.iter().copied().max_by_key(|p| ring(*p)))
}

/// One action of a foe's turn: out of a live blast's footprint first, then a shot
/// when it hunts you in range with a clear line, else a step toward where it saw
/// you; at that tile without sight it loses you. Outside every footprint it never
/// steps into one: it holds at the edge, and that loses it nothing.
pub fn step(st: &mut AppState, i: usize, can_move: bool, can_hit: bool) -> crate::net::Step {
    use crate::net::Step;
    let Some((me_place, me)) = st.player.as_ref().map(|p| (p.place, p.pos)) else {
        return Step::Nothing;
    };
    let Some(foe) = st.foes.get(i).cloned() else {
        return Step::Nothing;
    };
    if foe.down || foe.inert || foe.place != me_place {
        return Step::Nothing;
    }
    let FoeMode::Hunting { seen, lost } = foe.mode else {
        return Step::Nothing;
    };
    let live = crate::blast::live_footprints(st, foe.place);
    let in_live = |p: Pos| live.iter().any(|(q, _)| *q == p);
    if can_move
        && in_live(foe.pos)
        && let Some(to) = step_out(st, i, seen, &live)
    {
        st.foes[i].pos = to;
        return Step::Moved(to);
    }
    let Some(city) = st.city.as_ref() else {
        return Step::Nothing;
    };
    // ICE on the grid hits from beside you, by its grade.
    if foe.ice.is_some() && foe.pos.chebyshev(me) <= 1 {
        if can_hit {
            crate::grid::hit(st, i);
            return Step::Hit;
        }
        return Step::Nothing;
    }
    let in_reach = foe.ice.is_none()
        && foe.pos.chebyshev(me) <= weapon_range(st, &foe)
        && line_clear(city, foe.place, foe.pos, me)
        && sees(city, &foe, me);
    // In reach it fires, and with its shots gone it holds where it stands: no
    // point closing in on what it can already hit.
    if in_reach {
        if can_hit {
            crate::combat::fire_at_you(st, i);
            return Step::Hit;
        }
        return Step::Nothing;
    }
    if !can_move {
        return Step::Nothing;
    }
    let avoid: Vec<Pos> = std::iter::once(me)
        .chain(
            st.foes
                .iter()
                .enumerate()
                .filter(|(j, f)| *j != i && f.standing() && f.place == foe.place)
                .map(|(_, f)| f.pos),
        )
        .collect();
    let next = if foe.pos == seen {
        None
    } else {
        step_toward(city, foe.place, foe.pos, seen, &avoid).filter(|p| *p != me)
    };
    match next {
        // The way on runs into a live blast: it waits outside it.
        Some(to) if in_live(to) && !in_live(foe.pos) => Step::Nothing,
        Some(to) => {
            st.foes[i].pos = to;
            Step::Moved(to)
        }
        None => {
            let lost = lost + 1;
            st.foes[i].mode = if lost >= crate::net::LOST_TURNS {
                FoeMode::Idle
            } else {
                FoeMode::Hunting { seen, lost }
            };
            Step::Nothing
        }
    }
}

/// The weapon's name a foe carries, for the readouts.
pub fn weapon_name(st: &AppState, foe: &Foe) -> String {
    match &foe.weapon {
        Some(id) => st
            .content
            .item(id)
            .map(|i| i.name.clone())
            .unwrap_or_else(|| id.0.clone()),
        None => "unarmed".to_string(),
    }
}

/// The footer's and HERE's lines for the standing foes on the player's map that
/// `seen` admits, nearest first: `▣ goon · hunting · 14/20 · 9 tiles`.
pub fn lines(st: &AppState, seen: impl Fn(Pos) -> bool) -> Vec<String> {
    let Some(p) = st.player.as_ref() else {
        return Vec::new();
    };
    // On the open Net only the pieces near you.
    let net = p.place == Place::Stratum(citygen::StratumKind::Net);
    let mut foes: Vec<&Foe> = st
        .foes
        .iter()
        .filter(|f| f.standing() && f.place == p.place && seen(f.pos))
        .filter(|f| !net || f.pos.chebyshev(p.pos) <= crate::grid::FIGHT_RADIUS)
        .collect();
    foes.sort_by_key(|f| f.pos.chebyshev(p.pos));
    foes.into_iter()
        .map(|f| {
            let mode = if f.hunting() { "hunting" } else { "idle" };
            let stunned = if f.stunned > 0 {
                format!(" · stunned {}", f.stunned)
            } else {
                String::new()
            };
            format!(
                "▣ {} · {mode} · {}/{} · {} tiles{stunned}",
                f.name,
                f.hp,
                f.hp_max,
                f.pos.chebyshev(p.pos)
            )
        })
        .collect()
}

#[cfg(test)]
pub(crate) mod testutil {
    use super::*;
    use crate::render::map::MapRef;
    use citygen::StratumKind;

    /// The lot with you at (6,8) and one goon at `goon`, sight refreshed.
    pub fn lot(goon: Pos) -> AppState {
        let mut st = AppState::new();
        st.enter_city(City::lot(1));
        let place = Place::Stratum(StratumKind::Street);
        let mut p = crate::walker::Player::at(place, Pos::new(6, 8));
        let minute = st.minute_of_day();
        let bonus = st.sight_bonus();
        p.update_fov(
            &MapRef::stratum_at(st.city.as_ref().unwrap(), StratumKind::Street, minute),
            bonus,
        );
        st.player = Some(p);
        st.foes.push(Foe::goon(place, goon));
        st
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::net::Step;
    use citygen::StratumKind;

    const STREET: Place = Place::Stratum(StratumKind::Street);

    #[test]
    fn a_goon_sees_you_at_eight_not_nine() {
        let st = testutil::lot(Pos::new(14, 8));
        let city = st.city.as_ref().unwrap();
        assert!(sees(city, &st.foes[0], Pos::new(6, 8)));
        let mut far = st.foes[0].clone();
        far.pos = Pos::new(15, 8);
        assert!(!sees(city, &far, Pos::new(6, 8)));
        far.pos = Pos::new(14, 16);
        assert!(
            sees(city, &far, Pos::new(6, 8)),
            "chebyshev: 8 on both axes"
        );
        let mut down = st.foes[0].clone();
        down.down = true;
        assert!(!sees(city, &down, Pos::new(6, 8)));
    }

    #[test]
    fn a_wall_blocks_sight() {
        let st = testutil::lot(Pos::new(14, 8));
        let city = st.city.as_ref().unwrap();
        assert!(line_clear(city, STREET, Pos::new(2, 2), Pos::new(38, 2)));
        // Across the wall ring at x = 0.
        assert!(!line_clear(city, STREET, Pos::new(-1, 8), Pos::new(2, 8)));
        assert!(!line_clear(
            city,
            Place::Construct(citygen::BuildingId(0)),
            Pos::new(2, 2),
            Pos::new(3, 2)
        ));
    }

    #[test]
    fn look_marks_hunting_and_refreshes_seen() {
        let mut st = testutil::lot(Pos::new(14, 8));
        assert!(look(&mut st, 0));
        assert_eq!(
            st.foes[0].mode,
            FoeMode::Hunting {
                seen: Pos::new(6, 8),
                lost: 0
            }
        );
        st.player.as_mut().unwrap().pos = Pos::new(7, 8);
        assert!(look_all(&mut st));
        assert!(matches!(st.foes[0].mode, FoeMode::Hunting { seen, .. } if seen == Pos::new(7, 8)));
        // Out of sight it keeps hunting where it last saw you.
        st.player.as_mut().unwrap().pos = Pos::new(30, 16);
        st.foes[0].pos = Pos::new(2, 2);
        assert!(look(&mut st, 0));
        assert!(matches!(st.foes[0].mode, FoeMode::Hunting { seen, .. } if seen == Pos::new(7, 8)));
        // An idle foe far away stays idle; a stunned one sees nothing.
        let mut st = testutil::lot(Pos::new(18, 8));
        assert!(!look(&mut st, 0));
        assert_eq!(st.foes[0].mode, FoeMode::Idle);
        st.foes[0].pos = Pos::new(10, 8);
        st.foes[0].stunned = 1;
        assert!(!look(&mut st, 0));
    }

    #[test]
    fn step_toward_walks_the_shortest_way_and_never_onto_you() {
        let st = testutil::lot(Pos::new(14, 8));
        let city = st.city.as_ref().unwrap();
        let me = Pos::new(6, 8);
        assert_eq!(
            step_toward(city, STREET, Pos::new(14, 8), me, &[me]),
            Some(Pos::new(13, 8))
        );
        // Beside you the only step left is onto your tile, which the goal allows;
        // the turn engine never asks for it (it fires instead).
        assert_eq!(
            step_toward(city, STREET, Pos::new(7, 8), me, &[me]),
            Some(me)
        );
        assert_eq!(step_toward(city, STREET, me, me, &[]), None);
        // Another foe's tile is walked around.
        let block = Pos::new(13, 8);
        let next = step_toward(city, STREET, Pos::new(14, 8), me, &[me, block]).unwrap();
        assert_ne!(next, block);
        assert_eq!(next.chebyshev(Pos::new(14, 8)), 1);
        assert!(
            step_toward(
                city,
                Place::Construct(citygen::BuildingId(0)),
                me,
                Pos::new(7, 8),
                &[]
            )
            .is_none()
        );
    }

    #[test]
    fn in_sight_is_nearest_first() {
        let mut st = testutil::lot(Pos::new(14, 8));
        st.foes.push(Foe::goon(STREET, Pos::new(9, 8)));
        assert_eq!(in_sight(&st), vec![1, 0]);
        assert_eq!(in_memory(&st), vec![1, 0]);
        assert_eq!(foe_at(&st, STREET, Pos::new(9, 8)), Some(1));
        st.foes[1].down = true;
        assert_eq!(in_sight(&st), vec![0]);
        assert_eq!(foe_at(&st, STREET, Pos::new(9, 8)), None);
        // Remembered but not visible: in memory, not in sight.
        st.player
            .as_mut()
            .unwrap()
            .set_vis(Pos::new(14, 8), Visibility::Remembered);
        assert!(in_sight(&st).is_empty());
        assert_eq!(in_memory(&st), vec![0]);
        st.player
            .as_mut()
            .unwrap()
            .set_vis(Pos::new(14, 8), Visibility::Unseen);
        assert!(in_memory(&st).is_empty());
    }

    #[test]
    fn lines_read_the_spec_format() {
        let mut st = testutil::lot(Pos::new(15, 8));
        assert_eq!(
            lines(&st, |_| true),
            vec!["▣ goon · idle · 30/30 · 9 tiles"]
        );
        st.foes[0].mode = FoeMode::Hunting {
            seen: Pos::new(6, 8),
            lost: 0,
        };
        st.foes[0].hp = 14;
        st.foes[0].stunned = 1;
        assert_eq!(
            lines(&st, |_| true),
            vec!["▣ goon · hunting · 14/30 · 9 tiles · stunned 1"]
        );
        assert!(lines(&st, |_| false).is_empty());
        assert!(any_hunting(&st, STREET));
    }

    /// The clock moves `n` ticks: the app's and the living city's, which the clock
    /// reads while there is a city.
    fn advance(st: &mut AppState, n: u64) {
        st.tick += n;
        if let (Some(city), Some(world)) = (st.city.as_ref(), st.world.as_mut()) {
            world.advance(city, world.tick() + n);
        }
    }

    /// One pace of the clock, and whoever acts acts.
    fn pace(st: &mut AppState) {
        advance(st, crate::net::PACE_TICKS);
        crate::net::pace(st);
    }

    /// Lets the foe act on the pace until the turn is yours or the fight ends.
    fn settle(st: &mut AppState) {
        for _ in 0..80 {
            if crate::net::your_turn(st) {
                return;
            }
            pace(st);
        }
        panic!("the foe never hands the turn back");
    }

    fn hunting_from(st: &mut AppState, seen: Pos) {
        st.foes[0].mode = FoeMode::Hunting { seen, lost: 0 };
    }

    #[test]
    fn a_foes_turn_on_the_pace_steps_toward_you_then_fires() {
        let mut st = testutil::lot(Pos::new(18, 8));
        hunting_from(&mut st, Pos::new(6, 8));
        crate::net::engage(&mut st, STREET, true);
        assert_eq!(
            st.net.engaged.as_ref().unwrap().actor(),
            crate::net::Actor::You
        );
        crate::net::end_turn(&mut st);
        assert_eq!(
            st.net.engaged.as_ref().unwrap().actor(),
            crate::net::Actor::Foe(0)
        );
        // Three steps, one a pace; at nine tiles the pistol is out of range.
        pace(&mut st);
        assert_eq!(st.foes[0].pos, Pos::new(17, 8));
        assert_eq!(st.net.engaged.as_ref().unwrap().moves, 2);
        settle(&mut st);
        assert_eq!(st.foes[0].pos, Pos::new(15, 8));
        assert!(!logged(&st, "fires"));
        // Next turn: one step brings you into its reach; it fires and holds there.
        crate::net::end_turn(&mut st);
        settle(&mut st);
        assert_eq!(st.foes[0].pos, Pos::new(14, 8));
        assert!(logged(&st, "goon fires"), "{:?}", st.log.latest());
        assert!(crate::net::your_turn(&st));
    }

    #[test]
    fn a_foe_in_reach_fires_and_does_not_close_in() {
        let mut st = testutil::lot(Pos::new(12, 8));
        hunting_from(&mut st, Pos::new(6, 8));
        crate::net::engage(&mut st, STREET, true);
        crate::net::end_turn(&mut st);
        settle(&mut st);
        assert!(logged(&st, "goon fires"), "{:?}", st.log.latest());
        assert_eq!(st.foes[0].pos, Pos::new(12, 8), "six tiles is close enough");
    }

    #[test]
    fn a_stunned_foe_passes_and_the_stun_counts_down() {
        let mut st = testutil::lot(Pos::new(12, 8));
        hunting_from(&mut st, Pos::new(6, 8));
        st.foes[0].stunned = 1;
        crate::net::engage(&mut st, STREET, true);
        crate::net::end_turn(&mut st);
        assert!(crate::net::your_turn(&st), "the stunned foe passed at once");
        assert_eq!(st.foes[0].stunned, 0);
        assert!(logged(&st, "goon · stunned"));
        assert_eq!(st.foes[0].pos, Pos::new(12, 8));
    }

    #[test]
    fn a_foe_with_no_path_ends_its_turn() {
        let mut st = testutil::lot(Pos::new(1, 1));
        st.player.as_mut().unwrap().pos = Pos::new(30, 8);
        hunting_from(&mut st, Pos::new(30, 8));
        for p in [Pos::new(2, 1), Pos::new(1, 2), Pos::new(2, 2)] {
            st.foes.push(Foe::goon(STREET, p));
        }
        crate::net::engage(&mut st, STREET, true);
        crate::net::end_turn(&mut st);
        pace(&mut st);
        assert_eq!(st.foes[0].pos, Pos::new(1, 1));
        assert_ne!(
            st.net.engaged.as_ref().unwrap().actor(),
            crate::net::Actor::Foe(0)
        );
    }

    #[test]
    fn a_foe_loses_you_after_three_turns() {
        let mut st = testutil::lot(Pos::new(14, 8));
        st.player.as_mut().unwrap().pos = Pos::new(36, 16);
        st.player
            .as_mut()
            .unwrap()
            .set_vis(Pos::new(14, 8), Visibility::Remembered);
        hunting_from(&mut st, Pos::new(14, 8));
        crate::net::engage(&mut st, STREET, true);
        for lost in 1..=2 {
            crate::net::end_turn(&mut st);
            settle(&mut st);
            assert_eq!(
                st.foes[0].mode,
                FoeMode::Hunting {
                    seen: Pos::new(14, 8),
                    lost
                }
            );
        }
        crate::net::end_turn(&mut st);
        settle(&mut st);
        assert_eq!(st.foes[0].mode, FoeMode::Idle);
    }

    fn logged(st: &AppState, needle: &str) -> bool {
        st.log.iter().any(|e| e.text.contains(needle))
    }

    #[test]
    fn a_down_foe_is_not_standing_and_not_listed() {
        let mut st = testutil::lot(Pos::new(14, 8));
        st.foes[0].down = true;
        assert!(standing(&st, STREET).is_empty());
        assert!(lines(&st, |_| true).is_empty());
        assert!(!any_hunting(&st, STREET));
        assert_eq!(weapon_name(&st, &st.foes[0]), "Scrap nine");
    }

    /// A live blast of `radius` lying at `at` on the lot.
    fn live_at(st: &mut AppState, at: Pos, radius: i32) {
        st.live.push(crate::blast::Live {
            place: STREET,
            pos: at,
            blast: game::Blast {
                radius,
                damage: 1,
                stun: 0,
                fuse: 2,
                chrome: false,
                harm: game::Harm::Kinetic,
            },
            fx: Vec::new(),
            name: "satchel".to_string(),
            turns: 2,
            lands: 0,
            wakes: None,
        });
    }

    fn hunting(st: &mut AppState) {
        st.foes[0].mode = FoeMode::Hunting {
            seen: Pos::new(6, 8),
            lost: 0,
        };
    }

    fn inside(st: &AppState, p: Pos) -> bool {
        crate::blast::live_footprints(st, STREET)
            .iter()
            .any(|(q, _)| *q == p)
    }

    #[test]
    fn a_foe_in_a_live_footprint_steps_out_before_anything_else() {
        let mut st = testutil::lot(Pos::new(13, 8));
        hunting(&mut st);
        live_at(&mut st, Pos::new(14, 8), 1);
        let hp = st.character.hp;
        let Step::Moved(to) = step(&mut st, 0, true, true) else {
            panic!("it should step out");
        };
        assert!(!inside(&st, to));
        assert_eq!(st.foes[0].pos, to);
        assert_eq!(st.character.hp, hp, "no shot fired");
    }

    #[test]
    fn it_picks_the_out_tile_nearest_its_seen() {
        let mut st = testutil::lot(Pos::new(14, 9));
        hunting(&mut st);
        live_at(&mut st, Pos::new(14, 8), 1);
        assert_eq!(step(&mut st, 0, true, true), Step::Moved(Pos::new(13, 10)));
    }

    #[test]
    fn with_no_way_out_it_takes_the_greatest_ring() {
        let mut st = testutil::lot(Pos::new(12, 8));
        hunting(&mut st);
        live_at(&mut st, Pos::new(14, 8), 3);
        let Step::Moved(to) = step(&mut st, 0, true, true) else {
            panic!("it should move");
        };
        let ring = crate::blast::live_footprints(&st, STREET)
            .iter()
            .find(|(q, _)| *q == to)
            .map(|(_, r)| *r);
        assert_eq!(ring, Some(3));
    }

    #[test]
    fn it_fires_once_out() {
        let mut st = testutil::lot(Pos::new(13, 8));
        hunting(&mut st);
        live_at(&mut st, Pos::new(14, 8), 1);
        assert!(matches!(step(&mut st, 0, true, true), Step::Moved(_)));
        assert_eq!(step(&mut st, 0, true, true), Step::Hit);
    }

    #[test]
    fn a_foe_with_no_move_in_a_footprint_fires_as_today() {
        let mut st = testutil::lot(Pos::new(13, 8));
        hunting(&mut st);
        live_at(&mut st, Pos::new(14, 8), 1);
        assert_eq!(step(&mut st, 0, false, true), Step::Hit);
        assert_eq!(st.foes[0].pos, Pos::new(13, 8));
    }

    #[test]
    fn a_foe_outside_a_live_footprint_holds_at_its_edge() {
        let mut st = testutil::lot(Pos::new(19, 8));
        hunting(&mut st);
        // Out to x 17: one step to the edge, then it waits there.
        live_at(&mut st, Pos::new(14, 8), 3);
        let Step::Moved(to) = step(&mut st, 0, true, true) else {
            panic!("it should close in to the edge");
        };
        assert!(!inside(&st, to) && to.x == 18, "{to:?}");
        assert_eq!(step(&mut st, 0, true, true), Step::Nothing);
        assert_eq!(st.foes[0].pos, to);
        assert_eq!(
            st.foes[0].mode,
            FoeMode::Hunting {
                seen: Pos::new(6, 8),
                lost: 0
            },
            "holding loses it nothing"
        );
        // In the air the blast is no edge: it walks on.
        st.live[0].lands = st.tick + 10;
        assert!(matches!(step(&mut st, 0, true, true), Step::Moved(_)));
    }

    #[test]
    fn a_foe_steps_out_fires_and_the_satchel_goes_off_on_your_turn() {
        let mut st = testutil::lot(Pos::new(12, 8));
        hunting(&mut st);
        let satchel = st.content.item(&ItemId::new("satchel")).unwrap().clone();
        let (_, _, blast) = satchel.throw().unwrap();
        st.live.push(crate::blast::Live {
            place: STREET,
            pos: Pos::new(10, 8),
            blast,
            fx: satchel.fx.clone(),
            name: satchel.name.clone(),
            turns: 1,
            lands: 0,
            wakes: None,
        });
        crate::net::engage(&mut st, STREET, true);
        crate::net::end_turn(&mut st);
        settle(&mut st);
        let at = st.foes[0].pos;
        assert!(at.chebyshev(Pos::new(10, 8)) > 3, "out of it: {at:?}");
        assert!(logged(&st, "goon fires"), "{:?}", st.log.latest());
        assert!(st.live.is_empty(), "gone off as your turn opened");
        assert!(
            logged(&st, "satchel charge · nothing in it"),
            "{:?}",
            st.log.latest()
        );
        assert_eq!(st.foes[0].hp, GOON_HP);
    }
}
