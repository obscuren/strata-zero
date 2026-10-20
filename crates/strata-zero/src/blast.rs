//! Blasts: grenades thrown at a tile, their footprint by sight, the hurt by ring,
//! and the live ones that count your turns.

use crate::app::{AppState, FxInstance};
use crate::foes;
use citygen::{City, Place, Pos};
use game::{Action, Blast, Fx, ItemId, Slot};

/// Tiles a thrown thing crosses in a tick.
pub const THROW_SPEED: i32 = 2;
/// Ticks a thrown thing bounces and settles before it goes off.
pub const THROW_SETTLE: u64 = 2;
/// Ticks a blast's scorch stays on the ground.
pub const SCORCH_TICKS: u64 = 3600;

/// Ticks from the hand to the landing: two tiles a tick, then the settle.
pub fn throw_ticks(from: Pos, to: Pos) -> u64 {
    let dist = from.chebyshev(to).max(1) as u64;
    dist.div_ceil(THROW_SPEED as u64) + THROW_SETTLE
}

/// A blast lying live with a fuse, counting your turns.
#[derive(Clone, Debug, PartialEq)]
pub struct Live {
    pub place: Place,
    pub pos: Pos,
    pub blast: Blast,
    pub fx: Vec<Fx>,
    pub name: String,
    pub turns: u8,
}

/// Every tile within `radius` of `centre` that the centre can see, with its ring,
/// by ring then y then x. The centre always counts, whatever it is.
pub fn footprint(city: &City, place: Place, centre: Pos, radius: i32) -> Vec<(Pos, i32)> {
    let mut out = vec![(centre, 0)];
    let Some(k) = place.stratum() else {
        return out;
    };
    for ring in 1..=radius {
        for y in -ring..=ring {
            for x in -ring..=ring {
                if x.abs().max(y.abs()) != ring {
                    continue;
                }
                let p = centre.offset(x, y);
                let open = city.tile(k, p).is_some_and(|t| !foes::blocks_sight(t));
                if open && foes::line_clear(city, place, centre, p) {
                    out.push((p, ring));
                }
            }
        }
    }
    out
}

/// A blast's source as the log and the epitaph name it: `Satchel charge` reads
/// `satchel charge`.
fn log_name(source: &str) -> String {
    source.to_lowercase()
}

/// Whether you count as chrome: any cyberware worn.
fn you_are_chrome(st: &AppState) -> bool {
    Slot::ALL.iter().any(|s| {
        st.character
            .equipped(*s)
            .and_then(|id| st.content.item(id))
            .is_some_and(|i| i.kind == game::ItemKind::Cyberware)
    })
}

/// An idle, live foe turns to hunt toward `seen`.
fn wake(st: &mut AppState, foe: usize, seen: Pos) {
    if let Some(f) = st.foes.get_mut(foe)
        && f.standing()
        && !f.inert
        && f.mode == foes::FoeMode::Idle
    {
        f.mode = foes::FoeMode::Hunting { seen, lost: 0 };
    }
}

/// The impact shapes of a blast's `fx` on one victim: at the ring's tick, the
/// number offset by the victim's index.
#[allow(clippy::too_many_arguments)]
fn play_victim(
    st: &mut AppState,
    place: Place,
    centre: Pos,
    fx: &[Fx],
    pos: Pos,
    when: u64,
    index: usize,
    label: &str,
    tint: Option<(u8, u8, u8)>,
) {
    for shape in fx {
        let target = match shape {
            Fx::Number { .. } => pos.offset(index as i32, 0),
            Fx::Chips | Fx::Tear | Fx::Spark => pos,
            _ => continue,
        };
        st.fx.push(FxInstance {
            fx: shape.clone(),
            place,
            origin: centre,
            radius: 0,
            started: when,
            revealed: Vec::new(),
            targets: vec![target],
            label: if matches!(shape, Fx::Number { .. }) {
                label.to_string()
            } else {
                String::new()
            },
            hit: true,
            tint,
            arc: None,
        });
    }
}

/// What a blast does to one thing at `ring`: its damage and stun, none of either
/// for the chromeless under a chrome-only blast.
fn effect(blast: &Blast, ring: i32, chrome: bool) -> (u8, u8) {
    if blast.chrome && !chrome {
        return (0, 0);
    }
    (
        game::blast_damage(blast.damage, blast.radius, ring),
        blast.stun,
    )
}

fn effect_label(damage: u8, stun: u8) -> String {
    if damage == 0 && stun > 0 {
        "stun".to_string()
    } else {
        format!("-{damage}")
    }
}

fn effect_words(damage: u8, stun: u8) -> String {
    match (damage, stun) {
        (0, s) => format!("stun {s}"),
        (d, 0) => d.to_string(),
        (d, s) => format!("{d} stun {s}"),
    }
}

/// A blast goes off at `centre`: everyone in the footprint is hit by ring, the
/// shapes start at `at`. Nothing rolls.
#[allow(clippy::too_many_arguments)]
pub fn detonate(
    st: &mut AppState,
    place: Place,
    centre: Pos,
    blast: Blast,
    fx: &[Fx],
    source: &str,
    at: u64,
    tint: Option<(u8, u8, u8)>,
) {
    let Some(city) = st.city.as_ref() else {
        return;
    };
    let tiles = footprint(city, place, centre, blast.radius);
    let you = st
        .player
        .as_ref()
        .filter(|p| p.place == place)
        .map(|p| p.pos);
    let toward = you.unwrap_or(centre);
    if let Some(shape) = fx.iter().find(|f| f.is_blast()) {
        st.fx.push(FxInstance {
            fx: shape.clone(),
            place,
            origin: centre,
            radius: blast.radius,
            started: at,
            revealed: Vec::new(),
            targets: tiles.iter().map(|(p, _)| *p).collect(),
            label: String::new(),
            hit: blast.damage > 0,
            tint,
            arc: None,
        });
    }
    let mut words = Vec::new();
    let mut index = 0;
    let mut you_ring = None;
    for (pos, ring) in &tiles {
        if you == Some(*pos) {
            you_ring = Some(*ring);
        }
        let Some(i) = foes::foe_at(st, place, *pos) else {
            continue;
        };
        let (inert, chrome) = (st.foes[i].inert, st.foes[i].chrome);
        let (damage, stun) = effect(&blast, *ring, chrome);
        if damage == 0 && stun == 0 {
            // The blast passes through it, but the noise still turns heads.
            wake(st, i, toward);
            continue;
        }
        play_victim(
            st,
            place,
            centre,
            fx,
            *pos,
            at + *ring as u64,
            index,
            &effect_label(damage, stun),
            tint,
        );
        index += 1;
        words.push(format!(
            "{} {}",
            st.foes[i].name,
            effect_words(damage, stun)
        ));
        if !inert {
            crate::combat::wound_foe(st, i, damage);
            st.foes[i].stunned = st.foes[i].stunned.max(stun);
            wake(st, i, toward);
        }
    }
    let mut hurt_you = false;
    if let (Some(ring), Some(pos)) = (you_ring, you) {
        let (damage, stun) = effect(&blast, ring, you_are_chrome(st));
        if damage > 0 || stun > 0 {
            hurt_you = true;
            play_victim(
                st,
                place,
                centre,
                fx,
                pos,
                at + ring as u64,
                index,
                &effect_label(damage, stun),
                tint,
            );
            words.push(format!("you {}", effect_words(damage, stun)));
            st.net.stunned_you = st.net.stunned_you.max(stun);
            crate::combat::wound_you(st, &format!("your own {}", source.to_lowercase()), damage);
        }
    }
    let name = log_name(source);
    let line = if words.is_empty() {
        format!("{name} · nothing in it")
    } else {
        format!("{name} · {}", words.join(" · "))
    };
    if hurt_you {
        st.warn(line);
    } else {
        st.say(line);
    }
}

// Read by the foes' step-out and the overlay in the tasks that follow.
#[allow(dead_code)]
/// The engagement lets you act: your turn, the act half, an action point left.
fn throw_allowed(st: &AppState) -> Result<(), String> {
    match st.net.engaged.as_ref() {
        Some(e) if !matches!(e.place, Place::Construct(_)) => {
            if e.actor() != crate::net::Actor::You {
                Err("not your turn".into())
            } else if e.phase == crate::net::Phase::Move {
                Err("move first · space to stay".into())
            } else if e.ap == 0 {
                Err("no AP left".into())
            } else {
                Ok(())
            }
        }
        _ => Err("not in a fight".into()),
    }
}

// Read by the foes' step-out and the overlay in the tasks that follow.
#[allow(dead_code)]
/// You throw a consumable at a tile: it leaves the pack, the flight plays, the
/// blast goes off when it lands or lies live for its fuse. One action point.
pub fn throw(st: &mut AppState, id: &ItemId, at: Pos) -> Result<(), String> {
    throw_allowed(st)?;
    throw_from_hand(st, id, at, true)
}

/// `throw` without the engagement or the action point: the console's door, and
/// the calm tests'.
pub fn throw_free(st: &mut AppState, id: &ItemId, at: Pos) -> Result<(), String> {
    throw_from_hand(st, id, at, false)
}

fn throw_from_hand(st: &mut AppState, id: &ItemId, at: Pos, spend: bool) -> Result<(), String> {
    let item = st
        .content
        .item(id)
        .cloned()
        .ok_or_else(|| format!("unknown item {id}"))?;
    let (range, arc, blast) = item
        .throw()
        .ok_or_else(|| format!("{} is not thrown", item.name))?;
    if st.character.count(id) == 0 {
        return Err("none left".to_string());
    }
    let (Some(p), Some(city)) = (st.player.as_ref(), st.city.as_ref()) else {
        return Err("no body in the city".to_string());
    };
    let (place, from) = (p.place, p.pos);
    if from.chebyshev(at) > range {
        return Err("out of range".to_string());
    }
    let seen = p
        .vis()
        .get(at)
        .is_some_and(|v| *v == crate::render::Visibility::Visible);
    if !seen || !foes::line_clear(city, place, from, at) {
        return Err("no line".to_string());
    }
    let lands = place.stratum().is_some_and(|k| {
        city.tile(k, at)
            .is_some_and(|t| t.is_walkable() && !foes::blocks_sight(t))
    });
    if !lands {
        return Err("no landing".to_string());
    }
    if spend {
        crate::net::spend_ap(st)?;
    }
    game::apply(
        &mut st.character,
        &mut st.ground,
        &st.content,
        game::At { place, pos: from },
        st.tick,
        Action::Throw(id.clone()),
    )?;
    let now = st.tick;
    let land = now + throw_ticks(from, at);
    if item.fx.contains(&Fx::Throw) {
        st.fx.push(FxInstance {
            fx: Fx::Throw,
            place,
            origin: from,
            radius: 0,
            started: now,
            revealed: Vec::new(),
            targets: vec![at],
            label: String::new(),
            hit: false,
            tint: None,
            arc: Some(arc),
        });
    }
    if blast.fuse == 0 {
        detonate(st, place, at, blast, &item.fx, &item.name, land, None);
        return Ok(());
    }
    // It lies live: everyone it would reach has heard it land.
    if let Some(city) = st.city.as_ref() {
        let tiles = footprint(city, place, at, blast.radius);
        for (pos, _) in tiles {
            if let Some(i) = foes::foe_at(st, place, pos) {
                wake(st, i, from);
            }
        }
    }
    st.live.push(Live {
        place,
        pos: at,
        blast,
        fx: item.fx.clone(),
        name: item.name.clone(),
        turns: blast.fuse,
    });
    let s = if blast.fuse == 1 { "" } else { "s" };
    st.say(format!("{} · {} turn{s}", log_name(&item.name), blast.fuse));
    Ok(())
}

/// A turn off every live blast on your place; one at zero goes off at once.
pub fn tick_live(st: &mut AppState) {
    let Some(place) = st.player.as_ref().map(|p| p.place) else {
        return;
    };
    let mut due = Vec::new();
    for mut l in std::mem::take(&mut st.live) {
        if l.place == place {
            l.turns = l.turns.saturating_sub(1);
            if l.turns == 0 {
                due.push(l);
                continue;
            }
        }
        st.live.push(l);
    }
    let now = st.tick;
    for l in due {
        detonate(st, l.place, l.pos, l.blast, &l.fx, &l.name, now, None);
    }
}

// Read by the foes' step-out and the overlay in the tasks that follow.
#[allow(dead_code)]
/// Every tile a live blast on `place` would reach, with its ring: for foes to
/// step out of and the overlay to shade.
pub fn live_footprints(st: &AppState, place: Place) -> Vec<(Pos, i32)> {
    let Some(city) = st.city.as_ref() else {
        return Vec::new();
    };
    st.live
        .iter()
        .filter(|l| l.place == place)
        .flat_map(|l| footprint(city, place, l.pos, l.blast.radius))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use citygen::StratumKind;
    use game::SkillId;

    const STREET: Place = Place::Stratum(StratumKind::Street);

    /// The lot with the goon at `goon`, engaged by hand and on your act half.
    fn engaged(goon: Pos) -> AppState {
        let mut st = foes::testutil::lot(goon);
        st.character.skills.insert(SkillId::new("handguns"));
        crate::net::engage(&mut st, STREET, true);
        assert!(crate::net::stay(&mut st));
        st.tick = 500;
        st
    }

    fn logged(st: &AppState, needle: &str) -> bool {
        st.log.iter().any(|e| e.text.contains(needle))
    }

    // ---- Blasts ----

    fn set_terrain(st: &mut AppState, p: Pos, terrain: citygen::Terrain) {
        st.city
            .as_mut()
            .unwrap()
            .tile_mut(StratumKind::Street, p)
            .unwrap()
            .terrain = terrain;
    }

    fn thrown(st: &AppState, id: &str) -> (Blast, Vec<Fx>, String) {
        let item = st.content.item(&ItemId::new(id)).unwrap();
        let (_, _, blast) = item.throw().unwrap();
        (blast, item.fx.clone(), item.name.clone())
    }

    fn rings(tiles: &[(Pos, i32)]) -> Vec<usize> {
        (0..=2)
            .map(|r| tiles.iter().filter(|(_, ring)| *ring == r).count())
            .collect()
    }

    #[test]
    fn footprint_on_open_ground_is_the_square_by_ring() {
        let st = foes::testutil::lot(Pos::new(14, 8));
        let city = st.city.as_ref().unwrap();
        let tiles = footprint(city, STREET, Pos::new(20, 8), 2);
        assert_eq!(tiles.len(), 25);
        assert_eq!(tiles[0], (Pos::new(20, 8), 0));
        assert_eq!(rings(&tiles), vec![1, 8, 16]);
        assert_eq!(tiles[1], (Pos::new(19, 7), 1), "ring then y then x");
        assert!(tiles.windows(2).all(|w| w[0].1 <= w[1].1));
    }

    #[test]
    fn footprint_stops_at_walls_and_behind_them() {
        let mut st = foes::testutil::lot(Pos::new(20, 8));
        set_terrain(&mut st, Pos::new(21, 8), citygen::Terrain::Wall);
        let city = st.city.as_ref().unwrap();
        let tiles: Vec<Pos> = footprint(city, STREET, Pos::new(20, 8), 2)
            .into_iter()
            .map(|(p, _)| p)
            .collect();
        assert!(!tiles.contains(&Pos::new(21, 8)), "the wall itself");
        assert!(!tiles.contains(&Pos::new(22, 8)), "behind it");
        assert!(tiles.contains(&Pos::new(20, 8)), "the centre, foe or not");
        assert!(tiles.contains(&Pos::new(19, 8)));
        // Against the lot's wall ring.
        let edge = footprint(city, STREET, Pos::new(1, 1), 2);
        assert!(edge.iter().all(|(p, _)| p.x >= 1 && p.y >= 1));
        assert!(edge.contains(&(Pos::new(3, 3), 2)));
    }

    #[test]
    fn a_footprint_walled_in_is_the_centre_alone() {
        let mut st = foes::testutil::lot(Pos::new(14, 8));
        for (dx, dy) in [
            (-1, -1),
            (0, -1),
            (1, -1),
            (-1, 0),
            (1, 0),
            (-1, 1),
            (0, 1),
            (1, 1),
        ] {
            set_terrain(&mut st, Pos::new(20 + dx, 8 + dy), citygen::Terrain::Wall);
        }
        let city = st.city.as_ref().unwrap();
        assert_eq!(
            footprint(city, STREET, Pos::new(20, 8), 3),
            vec![(Pos::new(20, 8), 0)]
        );
    }

    #[test]
    fn detonate_hits_every_standing_foe_by_ring_and_wakes_them() {
        let mut st = foes::testutil::lot(Pos::new(21, 8));
        st.foes.push(foes::Foe::goon(STREET, Pos::new(22, 8)));
        let mut down = foes::Foe::goon(STREET, Pos::new(19, 8));
        down.down = true;
        down.hp = 0;
        st.foes.push(down);
        let (blast, fx, name) = thrown(&st, "frag");
        detonate(
            &mut st,
            STREET,
            Pos::new(20, 8),
            blast,
            &fx,
            &name,
            500,
            None,
        );
        assert_eq!(st.foes[0].hp, 22);
        assert_eq!(st.foes[1].hp, 26);
        assert_eq!(st.foes[2].hp, 0, "the down one is left alone");
        let hunting = foes::FoeMode::Hunting {
            seen: Pos::new(6, 8),
            lost: 0,
        };
        assert_eq!(st.foes[0].mode, hunting);
        assert_eq!(st.foes[1].mode, hunting);
        let bloom = st
            .fx
            .iter()
            .find(|i| matches!(i.fx, Fx::Bloom))
            .expect("the bloom");
        assert_eq!(
            (
                bloom.targets.len(),
                bloom.radius,
                bloom.started,
                bloom.origin
            ),
            (25, 2, 500, Pos::new(20, 8))
        );
        assert!(bloom.hit && bloom.label.is_empty());
        let chips: Vec<(u64, Pos)> = st
            .fx
            .iter()
            .filter(|i| matches!(i.fx, Fx::Chips))
            .map(|i| (i.started, i.targets[0]))
            .collect();
        assert_eq!(
            chips,
            vec![(501, Pos::new(21, 8)), (502, Pos::new(22, 8))],
            "a tick per ring"
        );
        let numbers: Vec<(&str, Pos)> = st
            .fx
            .iter()
            .filter(|i| matches!(i.fx, Fx::Number { .. }))
            .map(|i| (i.label.as_str(), i.targets[0]))
            .collect();
        assert_eq!(
            numbers,
            vec![("-8", Pos::new(21, 8)), ("-4", Pos::new(23, 8))],
            "columns offset by index"
        );
        assert!(
            logged(&st, "frag · goon 8 · goon 4"),
            "{:?}",
            st.log.latest()
        );
    }

    #[test]
    fn detonate_spares_the_dummy_and_the_chromeless() {
        let mut st = foes::testutil::lot(Pos::new(22, 8));
        st.foes[0].chrome = false;
        st.foes.push(foes::Foe::dummy(STREET, Pos::new(21, 8)));
        let (frag, fx, name) = thrown(&st, "frag");
        detonate(
            &mut st,
            STREET,
            Pos::new(20, 8),
            frag,
            &fx,
            &name,
            500,
            None,
        );
        assert_eq!(st.foes[1].hp, st.foes[1].hp_max, "the dummy shrugs");
        assert!(!st.foes[1].hunting());
        assert!(
            st.fx.iter().any(|i| i.label == "-8"),
            "its number still lifts off"
        );
        assert!(logged(&st, "frag · dummy 8 · goon 4"));
        // An EMP: the chromeless goon takes nothing, the stun-only label is `stun`.
        let mut st = foes::testutil::lot(Pos::new(21, 8));
        st.foes[0].chrome = false;
        let (emp, fx, name) = thrown(&st, "emp_puck");
        detonate(&mut st, STREET, Pos::new(20, 8), emp, &fx, &name, 500, None);
        assert_eq!((st.foes[0].hp, st.foes[0].stunned), (foes::GOON_HP, 0));
        assert!(!st.fx.iter().any(|i| matches!(i.fx, Fx::Number { .. })));
        assert!(
            logged(&st, "emp puck · nothing in it"),
            "{:?}",
            st.log.latest()
        );
        let mut st = foes::testutil::lot(Pos::new(21, 8));
        let (flash, fx, name) = thrown(&st, "flashbang");
        detonate(
            &mut st,
            STREET,
            Pos::new(20, 8),
            flash,
            &fx,
            &name,
            500,
            None,
        );
        assert_eq!((st.foes[0].hp, st.foes[0].stunned), (foes::GOON_HP, 2));
        let number = st
            .fx
            .iter()
            .find(|i| matches!(i.fx, Fx::Number { .. }))
            .unwrap();
        assert_eq!(number.label, "stun");
        let whiteout = st.fx.iter().find(|i| matches!(i.fx, Fx::Whiteout)).unwrap();
        assert!(!whiteout.hit, "no damage, no scorch");
        assert!(logged(&st, "flashbang · goon stun 2"));
    }

    #[test]
    fn a_blast_hits_you_by_your_ring() {
        let mut st = foes::testutil::lot(Pos::new(30, 8));
        let hp = st.character.hp;
        let (frag, fx, name) = thrown(&st, "frag");
        detonate(&mut st, STREET, Pos::new(6, 8), frag, &fx, &name, 500, None);
        assert_eq!(st.character.hp, hp - 12);
        let on_you = st
            .fx
            .iter()
            .find(|i| matches!(i.fx, Fx::Number { .. }))
            .unwrap();
        assert_eq!(
            (on_you.label.as_str(), on_you.targets[0]),
            ("-12", Pos::new(6, 8))
        );
        assert!(logged(&st, "· you 12"), "{:?}", st.log.latest());
        assert_eq!(
            st.log.latest().unwrap().kind,
            crate::messages::Kind::Warn,
            "in the warn colour"
        );
        // Stun on you.
        let (flash, fx, name) = thrown(&st, "flashbang");
        detonate(
            &mut st,
            STREET,
            Pos::new(7, 8),
            flash,
            &fx,
            &name,
            500,
            None,
        );
        assert_eq!(st.net.stunned_you, 2);
        // Dead by your own hand.
        st.character.hp = 5;
        let frag_fx = fx_of(&st, "frag");
        detonate(
            &mut st,
            STREET,
            Pos::new(6, 8),
            frag,
            &frag_fx,
            "Frag",
            500,
            None,
        );
        assert_eq!(st.net.death.as_deref(), Some("your own frag"));
    }

    fn fx_of(st: &AppState, id: &str) -> Vec<Fx> {
        thrown(st, id).1
    }

    fn with_pack(goon: Pos, items: &[(&str, u32)]) -> AppState {
        let mut st = engaged(goon);
        for (id, n) in items {
            st.character.give(ItemId::new(id), *n);
        }
        st
    }

    #[test]
    fn throw_refusals_in_order() {
        let frag = ItemId::new("frag");
        let ap = |st: &AppState| st.net.engaged.as_ref().unwrap().ap;
        // Calm.
        let mut st = foes::testutil::lot(Pos::new(14, 8));
        st.character.give(frag.clone(), 1);
        assert_eq!(
            throw(&mut st, &frag, Pos::new(10, 8)).unwrap_err(),
            "not in a fight"
        );
        assert_eq!(st.character.count(&frag), 1);
        // The move half.
        let mut st = foes::testutil::lot(Pos::new(14, 8));
        st.character.give(frag.clone(), 1);
        crate::net::engage(&mut st, STREET, true);
        assert_eq!(
            throw(&mut st, &frag, Pos::new(10, 8)).unwrap_err(),
            "move first · space to stay"
        );
        // No AP.
        let mut st = with_pack(Pos::new(14, 8), &[("frag", 1)]);
        st.net.engaged.as_mut().unwrap().ap = 0;
        assert_eq!(
            throw(&mut st, &frag, Pos::new(10, 8)).unwrap_err(),
            "no AP left"
        );
        // Nothing in the pack.
        let mut st = with_pack(Pos::new(14, 8), &[]);
        assert_eq!(
            throw(&mut st, &frag, Pos::new(10, 8)).unwrap_err(),
            "none left"
        );
        // Range, line, landing.
        let mut st = with_pack(Pos::new(30, 8), &[("frag", 1)]);
        assert_eq!(
            throw(&mut st, &frag, Pos::new(15, 8)).unwrap_err(),
            "out of range"
        );
        set_terrain(&mut st, Pos::new(9, 8), citygen::Terrain::Wall);
        assert_eq!(
            throw(&mut st, &frag, Pos::new(12, 8)).unwrap_err(),
            "no line"
        );
        set_terrain(&mut st, Pos::new(12, 8), citygen::Terrain::Wall);
        set_terrain(&mut st, Pos::new(9, 8), citygen::Terrain::Road);
        assert_eq!(
            throw(&mut st, &frag, Pos::new(12, 8)).unwrap_err(),
            "no landing"
        );
        assert_eq!(st.character.count(&frag), 1, "nothing consumed");
        assert_eq!(ap(&st), 2, "no AP spent");
        assert!(st.fx.is_empty());
    }

    #[test]
    fn throw_lands_and_blasts_on_the_landing_tick() {
        let mut st = with_pack(Pos::new(12, 8), &[("frag", 2)]);
        throw(&mut st, &ItemId::new("frag"), Pos::new(12, 8)).unwrap();
        assert_eq!(st.character.count(&ItemId::new("frag")), 1);
        assert_eq!(st.net.engaged.as_ref().unwrap().ap, 1);
        let flight = st.fx.iter().find(|i| matches!(i.fx, Fx::Throw)).unwrap();
        assert_eq!(
            (flight.arc, flight.origin, flight.targets[0], flight.started),
            (Some(game::Arc::Lob), Pos::new(6, 8), Pos::new(12, 8), 500)
        );
        assert_eq!(flight.until(), 505, "ceil(6 / 2) + 2");
        let bloom = st.fx.iter().find(|i| matches!(i.fx, Fx::Bloom)).unwrap();
        assert_eq!(bloom.started, 505);
        assert_eq!(st.foes[0].hp, foes::GOON_HP - 12, "the damage lands now");
        assert_eq!(
            throw_ticks(Pos::new(6, 8), Pos::new(6, 8)),
            3,
            "at least one tile"
        );
    }

    #[test]
    fn a_fused_throw_goes_live_and_counts_your_turns() {
        let mut st = with_pack(Pos::new(12, 8), &[("satchel", 1)]);
        throw(&mut st, &ItemId::new("satchel"), Pos::new(12, 8)).unwrap();
        assert!(!st.fx.iter().any(|i| matches!(i.fx, Fx::Bloom)));
        assert_eq!(st.live.len(), 1);
        assert_eq!(st.live[0].turns, 1);
        assert!(
            logged(&st, "satchel charge · 1 turn"),
            "{:?}",
            st.log.latest()
        );
        assert!(st.foes[0].hunting(), "it heard that land");
        assert_eq!(st.foes[0].hp, foes::GOON_HP);
        assert_eq!(live_footprints(&st, STREET).len(), 49);
        tick_live(&mut st);
        assert!(st.live.is_empty());
        assert!(st.fx.iter().any(|i| matches!(i.fx, Fx::Bloom)));
        assert_eq!(st.foes[0].hp, foes::GOON_HP - 16);
    }

    #[test]
    fn a_live_blast_counts_calm_beats() {
        let mut st = foes::testutil::lot(Pos::new(12, 8));
        st.character.give(ItemId::new("satchel"), 1);
        throw_free(&mut st, &ItemId::new("satchel"), Pos::new(12, 8)).unwrap();
        assert_eq!(st.live.len(), 1);
        crate::net::beat(&mut st);
        crate::net::beat(&mut st);
        assert!(st.live.is_empty(), "gone off");
        assert_eq!(st.foes[0].hp, foes::GOON_HP - 16);
    }

    #[test]
    fn a_live_blast_waits_on_its_own_place() {
        let mut st = foes::testutil::lot(Pos::new(12, 8));
        st.character.give(ItemId::new("satchel"), 1);
        throw_free(&mut st, &ItemId::new("satchel"), Pos::new(12, 8)).unwrap();
        st.live[0].place = Place::Stratum(StratumKind::Undercity);
        tick_live(&mut st);
        assert_eq!(st.live.len(), 1, "another map's fuse does not burn");
    }
}
