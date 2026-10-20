//! The abilities that are not a gun's round at a foe: Redline's refill, Deadeye's
//! mark, and a strike on a piece of ICE. Each refuses first and books its cooldown
//! only once it will go.

// Called from the map's cursor and the ACT keys in the tasks that follow.
#![allow(dead_code)]

use crate::app::{AppState, FxInstance};
use crate::combat;
use crate::net::{self, Actor, Phase};
use citygen::{BuildingId, Place, Pos};
use game::{AbilityId, AbilityKind, Action, Active, Fx};

/// Whose turn and which half it is, and whether `ap` points are left; nothing
/// stands in the way in calm.
pub(crate) fn can_act(st: &AppState, ap: u8) -> Result<(), String> {
    match st.net.engaged.as_ref() {
        Some(e) if e.actor() != Actor::You => Err("not your turn".into()),
        Some(e) if e.phase == Phase::Move => Err("move first · space to stay".into()),
        Some(e) if e.ap < ap => Err("no AP left".into()),
        _ => Ok(()),
    }
}

/// Books the ability's cooldown.
pub(crate) fn book(st: &mut AppState, id: &AbilityId) -> Result<(), String> {
    let at = game::At {
        place: st
            .player
            .as_ref()
            .map(|p| p.place)
            .unwrap_or(Place::Stratum(citygen::StratumKind::Street)),
        pos: st.player.as_ref().map(|p| p.pos).unwrap_or_default(),
    };
    game::apply(
        &mut st.character,
        &mut st.ground,
        &st.content,
        at,
        st.tick,
        Action::Fire(id.clone()),
    )
    .map(|_| ())
}

fn in_construct(st: &AppState) -> bool {
    matches!(
        st.player.as_ref().map(|p| p.place),
        Some(Place::Construct(_))
    )
}

/// One shape on a tile of your place, from now.
fn shape(st: &mut AppState, fx: Fx, at: Pos, label: &str, tint: Option<(u8, u8, u8)>) {
    let Some((place, me)) = st.player.as_ref().map(|p| (p.place, p.pos)) else {
        return;
    };
    st.fx.push(FxInstance {
        fx,
        place,
        origin: me,
        radius: 0,
        started: st.tick,
        revealed: Vec::new(),
        targets: vec![at],
        label: label.to_string(),
        hit: true,
        tint,
        arc: None,
    });
}

/// You boost: moves and action points full again, back to the move half, no AP
/// spent; once a fight when the ability says so.
pub fn boost(st: &mut AppState, id: &AbilityId) -> Result<(), String> {
    let ability = st
        .character
        .ability(&st.content, id)
        .ok_or_else(|| format!("unknown ability {id}"))?;
    let AbilityKind::Active(Active::Boost { once }) = ability.kind else {
        return Err(format!("{} is not a boost", ability.name));
    };
    let Some(e) = st.net.engaged.as_ref() else {
        return Err("not in a fight".to_string());
    };
    if e.actor() != Actor::You {
        return Err("not your turn".to_string());
    }
    if once && e.boosted {
        return Err("once a fight".to_string());
    }
    book(st, id)?;
    if let Some(e) = st.net.engaged.as_mut() {
        e.moves = net::BASE_MOVE;
        e.ap = net::BASE_AP;
        e.phase = Phase::Move;
        e.spent_at = None;
        e.boosted = true;
    }
    let me = st.player.as_ref().map(|p| p.pos).unwrap_or_default();
    for f in &ability.fx {
        match f {
            Fx::Kick(ticks) => net::kick(st, *ticks),
            Fx::Pulse { .. } => shape(st, f.clone(), me, "", None),
            Fx::Number { .. } => shape(st, f.clone(), me, "+ turn", Some(net::BOLT_ROLL)),
            _ => {}
        }
    }
    st.say(format!("{} · your turn again", ability.name.to_lowercase()));
    Ok(())
}

/// You mark a foe in sight: one mark at a time, until the fight ends or it falls.
pub fn mark(st: &mut AppState, id: &AbilityId, foe: usize) -> Result<(), String> {
    let ability = st
        .character
        .ability(&st.content, id)
        .ok_or_else(|| format!("unknown ability {id}"))?;
    let AbilityKind::Active(Active::Mark { range, ap, .. }) = ability.kind else {
        return Err(format!("{} is not a mark", ability.name));
    };
    if in_construct(st) {
        return Err("not in here".to_string());
    }
    combat::reach(st, foe, range)?;
    can_act(st, ap)?;
    book(st, id)?;
    for _ in 0..ap {
        net::spend_ap(st)?;
    }
    for (i, f) in st.foes.iter_mut().enumerate() {
        f.marked = i == foe;
    }
    let at = st.foes[foe].pos;
    for f in &ability.fx {
        if let Fx::Mark { colour, ticks } = f {
            net::mark(st, at, *colour, *ticks);
        }
    }
    let name = st.foes[foe].name.clone();
    st.say(format!("{} · {name} marked", ability.name.to_lowercase()));
    Ok(())
}

/// The nearest other standing piece within two tiles of `from`, by distance then
/// index, that is not in `hit`.
fn next_piece(st: &AppState, b: BuildingId, from: Pos, hit: &[usize]) -> Option<usize> {
    let k = st.constructs.get(b)?;
    k.guards
        .iter()
        .enumerate()
        .filter(|(i, g)| {
            !hit.contains(i) && k.guard_at(g.pos) == Some(*i) && g.pos.chebyshev(from) <= 2
        })
        .min_by_key(|(i, g)| (g.pos.chebyshev(from), *i))
        .map(|(i, _)| i)
}

/// The shot of a strike at a piece: the ability's shapes less the muzzle flash, in
/// the net's tint, `delay` ticks late.
#[allow(clippy::too_many_arguments)]
fn shoot_piece(
    st: &mut AppState,
    b: BuildingId,
    fx: &[Fx],
    from: Pos,
    at: Pos,
    hit: bool,
    damage: u8,
    delay: u64,
) {
    let fx: Vec<Fx> = fx
        .iter()
        .filter(|f| !matches!(f, Fx::Flash { .. }))
        .cloned()
        .collect();
    let from_len = st.fx.len();
    combat::play_shot(
        st,
        Place::Construct(b),
        &fx,
        from,
        at,
        hit,
        damage,
        Some(net::ATTACK_TINT),
    );
    for i in &mut st.fx[from_len..] {
        i.started += delay;
    }
}

/// You strike a piece of ICE in reach: an attack. It makes the attack's heat, rolls
/// with the strike's stat and skill, wakes the piece, and a break runs the attack's
/// break path. Shove and its like are refused here.
pub fn strike_ice(
    st: &mut AppState,
    id: &AbilityId,
    b: BuildingId,
    i: usize,
) -> Result<(), String> {
    let ability = st
        .character
        .ability(&st.content, id)
        .ok_or_else(|| format!("unknown ability {id}"))?;
    let AbilityKind::Active(Active::Strike {
        range,
        damage,
        stun,
        skill,
        ap,
        stat,
        push,
        chain,
    }) = ability.kind.clone()
    else {
        return Err(format!("{} is not a strike", ability.name));
    };
    if push > 0 || st.player.as_ref().map(|p| p.place) != Some(Place::Construct(b)) {
        return Err("not in here".to_string());
    }
    if !net::piece_in_reach(st, b, i) {
        return Err("out of reach".to_string());
    }
    can_act(st, ap)?;
    book(st, id)?;
    for _ in 0..ap {
        net::spend_ap(st)?;
    }
    if net::noise_on(st, b, net::STRIKE_NOISE) {
        return Ok(());
    }
    let me = st.player.as_ref().map(|p| p.pos).unwrap_or_default();
    let (at, label) = st
        .constructs
        .get(b)
        .and_then(|k| k.guards.get(i))
        .map(|g| (g.pos, g.label()))
        .ok_or("nothing stands there")?;
    let chance = game::hit_chance(
        st.character.stat(&st.content, stat),
        st.character.has_skill(&skill),
        me.chebyshev(at),
        range,
    );
    let hit = game::round_hits(combat::roll(st), chance);
    shoot_piece(st, b, &ability.fx, me, at, hit, damage, 0);
    let now = st.net.now(st);
    let name = ability.name.to_lowercase();
    if !hit {
        net::mark(st, at, net::ATTACK_TINT, net::MARK_TICKS);
        st.say(format!("{name} · {label} ICE miss"));
        return Ok(());
    }
    net::hit_piece(st, b, i, damage, stun, now);
    if stun > 0
        && st
            .net
            .ice
            .get(&b)
            .and_then(|v| v.get(i))
            .is_some_and(|s| s.stunned > 0)
    {
        st.say(format!("{label} ICE stunned"));
    }
    // The arc hops on, no roll: half the damage and the stun, a hop late each.
    let half = (damage / 2).max(1);
    let (mut hits, mut last) = (vec![i], at);
    for hop in 1..=chain as u64 {
        let Some(next) = next_piece(st, b, last, &hits) else {
            break;
        };
        let Some(target) = st
            .constructs
            .get(b)
            .and_then(|k| k.guards.get(next))
            .map(|g| g.pos)
        else {
            break;
        };
        shoot_piece(st, b, &ability.fx, last, target, true, half, 2 * hop);
        net::hit_piece(st, b, next, half, stun, now);
        hits.push(next);
        last = target;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::combat::{strike, strike_chance, your_chance};
    use crate::foes;
    use citygen::StratumKind;
    use game::{Character, Stat};

    const STREET: Place = Place::Stratum(StratumKind::Street);

    /// The lot with the runner's kit on your act half, a goon at `goon`.
    fn fight(goon: Pos) -> AppState {
        let mut st = foes::testutil::lot(goon);
        st.character = Character::combat_runner(&st.content);
        net::engage(&mut st, STREET, true);
        assert!(net::stay(&mut st));
        st.tick = 500;
        st
    }

    /// You at `pos`, the foes' tiles in sight.
    fn stand(st: &mut AppState, pos: Pos) {
        let seen: Vec<Pos> = st.foes.iter().map(|f| f.pos).collect();
        let p = st.player.as_mut().unwrap();
        p.pos = pos;
        for t in seen {
            p.set_vis(t, crate::render::Visibility::Visible);
        }
    }

    fn logged(st: &AppState, needle: &str) -> bool {
        st.log.iter().any(|e| e.text.contains(needle))
    }

    fn id(s: &str) -> AbilityId {
        AbilityId::new(s)
    }

    fn ap(st: &AppState) -> u8 {
        st.net.engaged.as_ref().unwrap().ap
    }

    #[test]
    fn strike_rolls_with_its_own_stat() {
        let mut st = fight(Pos::new(7, 8));
        st.character.stats.body = 10;
        st.character.stats.net = 1;
        st.character.skills.insert(game::SkillId::new("athletics"));
        let want = game::hit_chance(st.character.stat(&st.content, Stat::Body), true, 1, 1);
        assert_eq!(strike_chance(&st, &id("shove"), 0).unwrap(), want);
        assert_eq!(want, 95);
        st.character.skills.clear();
        let unskilled = strike_chance(&st, &id("shove"), 0).unwrap();
        assert_eq!(
            unskilled,
            game::hit_chance(st.character.stat(&st.content, Stat::Body), false, 1, 1)
        );
        // Static keeps the Net stat.
        st.foes[0].pos = Pos::new(10, 8);
        let skilled = st.character.has_skill(&game::SkillId::new("hacking"));
        let want = game::hit_chance(st.character.stat(&st.content, Stat::Net), skilled, 4, 8);
        assert_eq!(strike_chance(&st, &id("static"), 0).unwrap(), want);
    }

    #[test]
    fn shove_pushes_two_tiles_with_a_trail() {
        let mut st = fight(Pos::new(7, 8));
        st.force_hit = Some(true);
        strike(&mut st, &id("shove"), 0).unwrap();
        assert_eq!(st.foes[0].pos, Pos::new(9, 8));
        assert_eq!(st.foes[0].hp, foes::GOON_HP - 3);
        let trails: Vec<(Pos, Pos, u64)> = st
            .fx
            .iter()
            .filter(|i| matches!(i.fx, Fx::Trail))
            .map(|i| (i.origin, i.targets[0], i.started))
            .collect();
        assert_eq!(
            trails,
            vec![
                (Pos::new(7, 8), Pos::new(8, 8), 500),
                (Pos::new(8, 8), Pos::new(9, 8), 501)
            ]
        );
        assert!(
            logged(&st, "shove · goon 3 · back 2"),
            "{:?}",
            st.log.latest()
        );
        assert_eq!(ap(&st), 1);
    }

    #[test]
    fn shove_stops_at_a_wall() {
        // One tile from the wall (the last floor is x = 39).
        let mut st = fight(Pos::new(38, 8));
        stand(&mut st, Pos::new(37, 8));
        st.force_hit = Some(true);
        strike(&mut st, &id("shove"), 0).unwrap();
        assert_eq!(st.foes[0].pos, Pos::new(39, 8));
        assert!(
            logged(&st, "shove · goon 3 · back 1"),
            "{:?}",
            st.log.latest()
        );
        // Against it.
        let mut st = fight(Pos::new(39, 8));
        stand(&mut st, Pos::new(38, 8));
        st.force_hit = Some(true);
        strike(&mut st, &id("shove"), 0).unwrap();
        assert_eq!(st.foes[0].pos, Pos::new(39, 8));
        assert_eq!(st.foes[0].hp, foes::GOON_HP - 3, "the damage still lands");
        assert!(logged(&st, "shove · goon 3 · back 0"));
        assert!(!st.fx.iter().any(|i| matches!(i.fx, Fx::Trail)));
    }

    #[test]
    fn shove_against_another_foe() {
        let mut st = fight(Pos::new(7, 8));
        st.foes.push(foes::Foe::goon(STREET, Pos::new(8, 8)));
        st.force_hit = Some(true);
        strike(&mut st, &id("shove"), 0).unwrap();
        assert_eq!(st.foes[0].pos, Pos::new(7, 8), "no push past the other");
        assert!(logged(&st, "back 0"));
        // A downed foe is no wall.
        let mut st = fight(Pos::new(7, 8));
        let mut down = foes::Foe::goon(STREET, Pos::new(8, 8));
        down.down = true;
        st.foes.push(down);
        st.force_hit = Some(true);
        strike(&mut st, &id("shove"), 0).unwrap();
        assert_eq!(st.foes[0].pos, Pos::new(9, 8));
    }

    #[test]
    fn jolt_chains_to_the_nearest_within_two() {
        let mut st = fight(Pos::new(9, 8));
        st.foes.push(foes::Foe::goon(STREET, Pos::new(11, 8)));
        st.foes.push(foes::Foe::goon(STREET, Pos::new(12, 8)));
        // The third stands three from A; the second two from A.
        st.foes.push(foes::Foe::goon(STREET, Pos::new(9, 11)));
        st.force_hit = Some(true);
        strike(&mut st, &id("jolt"), 0).unwrap();
        assert_eq!(st.foes[0].hp, foes::GOON_HP - 5);
        assert_eq!(st.foes[0].stunned, 1);
        assert_eq!(st.foes[1].hp, foes::GOON_HP - 2, "half, rounded down");
        assert_eq!(st.foes[1].stunned, 1);
        assert_eq!(st.foes[2].hp, foes::GOON_HP, "three away is out of the arc");
        assert_eq!(st.foes[3].hp, foes::GOON_HP);
        // B's shapes play from A to B, two ticks after A's.
        let streaks: Vec<(Pos, Pos, u64)> = st
            .fx
            .iter()
            .filter(|i| matches!(i.fx, Fx::Streak { .. }))
            .map(|i| (i.origin, i.targets[0], i.started))
            .collect();
        assert_eq!(
            streaks,
            vec![
                (Pos::new(6, 8), Pos::new(9, 8), 500),
                (Pos::new(9, 8), Pos::new(11, 8), 502)
            ]
        );
        // A miss chains nothing.
        let mut st = fight(Pos::new(9, 8));
        st.foes.push(foes::Foe::goon(STREET, Pos::new(11, 8)));
        st.force_hit = Some(false);
        strike(&mut st, &id("jolt"), 0).unwrap();
        assert_eq!(st.foes[1].hp, foes::GOON_HP);
        assert_eq!(st.foes[1].stunned, 0);
        assert_eq!(
            st.fx
                .iter()
                .filter(|i| matches!(i.fx, Fx::Streak { .. }))
                .count(),
            1
        );
    }

    #[test]
    fn redline_refills_once_a_fight() {
        let mut st = fight(Pos::new(14, 8));
        {
            let e = st.net.engaged.as_mut().unwrap();
            e.moves = 0;
            e.ap = 0;
        }
        boost(&mut st, &id("redline")).unwrap();
        let e = st.net.engaged.as_ref().unwrap();
        assert_eq!((e.moves, e.ap), (net::BASE_MOVE, net::BASE_AP));
        assert_eq!(e.phase, Phase::Move);
        assert!(e.boosted);
        assert!(logged(&st, "redline · your turn again"));
        let number = st
            .fx
            .iter()
            .find(|i| matches!(i.fx, Fx::Number { .. }))
            .unwrap();
        assert_eq!(number.label, "+ turn");
        assert_eq!(number.tint, Some(net::BOLT_ROLL));
        assert_eq!(number.targets[0], Pos::new(6, 8));
        assert!(st.fx.iter().any(|i| matches!(i.fx, Fx::Kick(_))));
        assert!(st.fx.iter().any(|i| matches!(i.fx, Fx::Pulse { .. })));
        // Once a fight, and the refusal books nothing.
        let fired = st.character.fired.clone();
        assert_eq!(boost(&mut st, &id("redline")).unwrap_err(), "once a fight");
        assert_eq!(st.character.fired, fired);
        // Calm.
        net::disengage(&mut st);
        assert_eq!(
            boost(&mut st, &id("redline")).unwrap_err(),
            "not in a fight"
        );
        // A new fight is a new boost.
        let mut st = fight(Pos::new(14, 8));
        boost(&mut st, &id("redline")).unwrap();
        net::disengage(&mut st);
        net::engage(&mut st, STREET, true);
        assert!(!st.net.engaged.as_ref().unwrap().boosted);
    }

    #[test]
    fn deadeye_marks_one_foe_and_lifts_the_chance() {
        let mut st = fight(Pos::new(14, 8));
        st.foes.push(foes::Foe::goon(STREET, Pos::new(13, 9)));
        let before = your_chance(&st, 0).unwrap();
        mark(&mut st, &id("deadeye"), 0).unwrap();
        assert!(st.foes[0].marked && !st.foes[1].marked);
        assert_eq!(ap(&st), 1);
        assert!(logged(&st, "deadeye · goon marked"));
        assert!(st.fx.iter().any(|i| matches!(i.fx, Fx::Mark { .. })));
        assert_eq!(your_chance(&st, 0).unwrap(), (before + 20).min(95));
        assert_eq!(
            your_chance(&st, 1).unwrap(),
            your_chance(&st, 1).unwrap(),
            "unmarked is untouched"
        );
        // Marking another clears the first.
        st.tick += 1000;
        mark(&mut st, &id("deadeye"), 1).unwrap_or_else(|e| panic!("{e}"));
        assert!(!st.foes[0].marked && st.foes[1].marked);
        // Down clears it.
        st.force_hit = Some(true);
        combat::wound_foe(&mut st, 1, 200);
        assert!(st.foes[1].down && !st.foes[1].marked);
        // Calm clears it.
        st.foes[0].marked = true;
        net::disengage(&mut st);
        assert!(!st.foes[0].marked);
    }

    #[test]
    fn deadeye_refuses_range_and_no_target() {
        let mut st = fight(Pos::new(19, 8));
        assert_eq!(
            mark(&mut st, &id("deadeye"), 0).unwrap_err(),
            "out of range"
        );
        assert_eq!(ap(&st), 2);
        let mut st = fight(Pos::new(14, 8));
        st.foes[0].down = true;
        assert_eq!(
            mark(&mut st, &id("deadeye"), 0).unwrap_err(),
            "pick a target"
        );
        assert_eq!(
            mark(&mut st, &id("deadeye"), 5).unwrap_err(),
            "pick a target"
        );
        assert!(!st.character.fired.contains_key(&id("deadeye")));
    }

    #[test]
    fn deck_use_refuses_boost_mark_and_throwables() {
        let mut st = fight(Pos::new(14, 8));
        let go = |st: &mut AppState, a: Action| crate::actions::perform(st, a);
        let err = go(&mut st, Action::Fire(id("redline"))).unwrap_err();
        assert_eq!(err, "use it from the map");
        let err = go(&mut st, Action::Fire(id("deadeye"))).unwrap_err();
        assert_eq!(err, "use it from the map");
        let err = go(&mut st, Action::Use(game::ItemId::new("frag"))).unwrap_err();
        assert_eq!(err, "throw it from the map");
        assert_eq!(ap(&st), 2, "nothing was spent");
    }

    /// The net lot, engaged on your act half beside the grey ICE.
    fn net_fight() -> (AppState, BuildingId) {
        let mut st = AppState::new();
        st.enter_city(citygen::City::lot(1));
        st.player = Some(crate::walker::Player::at(STREET, Pos::new(6, 8)));
        net::start_in_net_lot(&mut st).unwrap();
        st.character = Character::combat_runner(&st.content);
        let b = BuildingId(0);
        st.player.as_mut().unwrap().pos = Pos::new(37, 9);
        net::engage(&mut st, Place::Construct(b), true);
        assert!(net::stay(&mut st));
        st.tick = 500;
        st.net.ice.get_mut(&b).unwrap()[0].integrity = 12;
        (st, b)
    }

    #[test]
    fn static_on_a_piece_is_an_attack() {
        let (mut st, b) = net_fight();
        st.force_hit = Some(true);
        let heat = st.net.traces.get(&b).map_or(0, |t| t.heat);
        strike_ice(&mut st, &id("static"), b, 0).unwrap();
        let ice = &st.net.ice[&b][0];
        assert_eq!(ice.integrity, 6);
        assert_eq!(ice.stunned, 1);
        assert!(matches!(ice.mode, net::IceMode::Awake { .. }));
        assert_eq!(st.net.traces[&b].heat, heat + net::STRIKE_NOISE);
        assert_eq!(ap(&st), 0, "static costs two");
        assert!(st.fx.iter().all(|i| i.tint == Some(net::ATTACK_TINT)));
        assert!(!st.fx.iter().any(|i| matches!(i.fx, Fx::Flash { .. })));
        assert!(st.fx.iter().any(|i| matches!(i.fx, Fx::Number { .. })));
        assert!(logged(&st, "grey ICE stunned"));
        // The next one breaks it.
        net::end_turn(&mut st);
        {
            let e = st.net.engaged.as_mut().unwrap();
            e.turn = 0;
            e.ap = 2;
            e.phase = Phase::Act;
        }
        st.tick += 1000;
        strike_ice(&mut st, &id("static"), b, 0).unwrap();
        assert!(logged(&st, "grey ICE broken"));
    }

    #[test]
    fn static_on_the_dummy_leaves_it_standing() {
        let (mut st, b) = net_fight();
        st.player.as_mut().unwrap().pos = Pos::new(5, 3);
        st.force_hit = Some(true);
        strike_ice(&mut st, &id("static"), b, 1).unwrap();
        let dummy = &st.net.ice[&b][1];
        assert_eq!(dummy.integrity, dummy_integrity());
        assert_eq!(dummy.stunned, 0);
        assert!(
            st.fx
                .iter()
                .any(|i| matches!(i.fx, Fx::Number { tag: true })),
            "the number still lifts"
        );
    }

    fn dummy_integrity() -> u8 {
        citygen::IceGrade::Grey.integrity()
    }

    #[test]
    fn jolt_arcs_between_pieces_and_shove_is_not_for_the_net() {
        let (mut st, b) = net_fight();
        // The dummy two tiles from the grey piece would take the arc; stand them close.
        st.constructs.get_mut(b).unwrap().guards[1].pos = Pos::new(38, 11);
        st.force_hit = Some(true);
        strike_ice(&mut st, &id("jolt"), b, 0).unwrap();
        assert_eq!(st.net.ice[&b][0].integrity, 7);
        let streaks: Vec<u64> = st
            .fx
            .iter()
            .filter(|i| matches!(i.fx, Fx::Streak { .. }))
            .map(|i| i.started)
            .collect();
        assert_eq!(streaks, vec![500, 502]);
        assert_eq!(
            strike_ice(&mut st, &id("shove"), b, 0).unwrap_err(),
            "not in here"
        );
        assert_eq!(mark(&mut st, &id("deadeye"), 0).unwrap_err(), "not in here");
        assert_eq!(strike(&mut st, &id("jolt"), 0).unwrap_err(), "not in here");
    }
}
