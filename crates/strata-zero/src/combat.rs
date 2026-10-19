//! Shooting: a fire action resolved round by round, the pictures it pushes, and
//! what the rounds do to whoever they hit.

use crate::app::{AppState, FxInstance};
use crate::foes;
use crate::render::fx::tracer_ticks;
use citygen::pathing::bresenham;
use citygen::rng::hash64;
use citygen::{City, Place, Pos};
use game::{AbilityId, AbilityKind, Action, Active, Fx, ItemId, Slot, Stat, Weapon};

/// Ticks between the rounds of one burst.
pub const ROUND_GAP: u64 = 4;

/// One roll of 1..=100.
pub fn roll(st: &mut AppState) -> u8 {
    #[cfg(test)]
    if let Some(hit) = st.force_hit {
        return if hit { 1 } else { 100 };
    }
    st.shots += 1;
    (hash64(st.tick, st.shots) % 100 + 1) as u8
}

/// Where a missed round stops: along the ray past the target, the first tile that
/// blocks sight (the wall itself), else the last walkable tile within `range` of
/// the shooter; the target's own tile when nothing lies past it.
pub fn miss_impact(city: &City, place: Place, from: Pos, target: Pos, range: i32) -> Pos {
    let Some(k) = place.stratum() else {
        return target;
    };
    let (dx, dy) = (target.x - from.x, target.y - from.y);
    let span = dx.abs().max(dy.abs()).max(1);
    let reach = range / span + 2;
    let far = Pos::new(from.x + dx * reach, from.y + dy * reach);
    let mut last = target;
    for p in bresenham(from, far)
        .into_iter()
        .skip_while(|p| *p != target)
        .skip(1)
    {
        // The round always clears the target by a tile, range or not: a miss is
        // never drawn on whoever it missed.
        if from.chebyshev(p) > range && last != target {
            break;
        }
        match city.tile(k, p) {
            Some(t) if foes::blocks_sight(t) => return p,
            Some(t) if t.is_walkable() => last = p,
            _ => break,
        }
    }
    last
}

/// The worn gun: the Primary slot's item with a weapon on it.
pub fn your_weapon(st: &AppState) -> Result<(ItemId, Weapon), String> {
    st.character
        .equipped(Slot::Primary)
        .and_then(|id| {
            st.content
                .item(id)
                .and_then(|i| i.weapon.clone())
                .map(|w| (id.clone(), w))
        })
        .ok_or_else(|| "nothing to shoot with".to_string())
}

/// A shot's way to a foe from where you stand: refused out of range or without a
/// clear line in sight; else the distance.
fn reach(st: &AppState, foe: usize, range: i32) -> Result<i32, String> {
    let (Some(p), Some(city)) = (st.player.as_ref(), st.city.as_ref()) else {
        return Err("no body in the city".to_string());
    };
    let Some(f) = st
        .foes
        .get(foe)
        .filter(|f| f.standing() && f.place == p.place)
    else {
        return Err("pick a target".to_string());
    };
    let dist = p.pos.chebyshev(f.pos);
    if dist > range {
        return Err("out of range".to_string());
    }
    let seen = p
        .vis()
        .get(f.pos)
        .is_some_and(|v| *v == crate::render::Visibility::Visible);
    if !seen || !foes::line_clear(city, p.place, p.pos, f.pos) {
        return Err("no line".to_string());
    }
    Ok(dist)
}

/// Your chance per round at a foe with the worn gun, in percent.
pub fn your_chance(st: &AppState, foe: usize) -> Result<u8, String> {
    let (_, w) = your_weapon(st)?;
    let dist = reach(st, foe, w.range)?;
    Ok(game::hit_chance(
        st.character.stat(&st.content, Stat::Reflex),
        st.character.has_skill(&w.skill),
        dist,
        w.range,
    ))
}

/// Everything one round needs to be drawn.
struct Round {
    from: Pos,
    impact: Pos,
    hit: bool,
    damage: u8,
    k: usize,
}

/// Pushes the shapes of one round, offset `k` gaps into the burst: the flash and
/// the flight at once, the impact when the flight lands.
fn play_round(st: &mut AppState, place: Place, fx: &[Fx], r: Round) {
    let started = st.tick + r.k as u64 * ROUND_GAP;
    let flight = fx
        .iter()
        .find_map(|f| match f {
            Fx::Tracer { speed, .. } => Some(tracer_ticks(r.from, r.impact, *speed)),
            Fx::Streak { .. } => Some(1),
            _ => None,
        })
        .unwrap_or(0);
    for shape in fx {
        let (at, target) = match shape {
            Fx::Flash { .. } => (started, r.impact),
            f if f.is_flight() => (started, r.impact),
            Fx::Spark => {
                if r.hit {
                    continue;
                }
                (started + flight, r.impact)
            }
            Fx::Number { .. } => {
                if !r.hit {
                    continue;
                }
                (started + flight, r.impact.offset(r.k as i32, 0))
            }
            f if f.is_impact() => {
                if !r.hit {
                    continue;
                }
                (started + flight, r.impact)
            }
            _ => (started, r.impact),
        };
        st.fx.push(FxInstance {
            fx: shape.clone(),
            place,
            origin: r.from,
            radius: 0,
            started: at,
            revealed: Vec::new(),
            targets: vec![target],
            label: if matches!(shape, Fx::Number { .. }) {
                format!("-{}", r.damage)
            } else {
                String::new()
            },
            hit: r.hit,
            tint: None,
        });
    }
}

/// A foe takes damage; at zero it is down.
fn wound_foe(st: &mut AppState, foe: usize, damage: u8) {
    let Some(f) = st.foes.get_mut(foe) else {
        return;
    };
    if f.inert {
        // A dummy takes the round and keeps standing: nothing gets through.
        return;
    }
    f.hp = f.hp.saturating_sub(damage);
    if f.hp == 0 && !f.down {
        f.down = true;
        let name = f.name.clone();
        st.say(format!("{name} down"));
    }
}

/// You take damage; at zero you are dead on the street.
fn wound_you(st: &mut AppState, foe: usize, damage: u8) {
    st.character.hp = st.character.hp.saturating_sub(damage as u32);
    if st.character.hp == 0 && st.net.death.is_none() {
        let name = st
            .foes
            .get(foe)
            .map(|f| f.name.clone())
            .unwrap_or_else(|| "foe".to_string());
        st.net.death = Some(format!("a {name} on the street"));
        crate::net::disengage(st);
    }
}

/// Rolls `rounds` at a foe from `from`, each landing or stopping past them; the
/// damage lands at once, the pictures follow. Returns each round's outcome.
fn volley(
    st: &mut AppState,
    from: Pos,
    foe: usize,
    w: &Weapon,
    chance: u8,
    fx: &[Fx],
) -> Vec<Option<u8>> {
    let Some((place, target)) = st.foes.get(foe).map(|f| (f.place, f.pos)) else {
        return Vec::new();
    };
    // Shot at, it knows where from: a miss is loud enough to turn it on you.
    if let Some(f) = st.foes.get_mut(foe)
        && f.standing()
        && !f.inert
    {
        f.mode = foes::FoeMode::Hunting {
            seen: from,
            lost: 0,
        };
    }
    let mut out = Vec::new();
    for k in 0..w.rounds as usize {
        let hit = game::round_hits(roll(st), chance);
        let impact = if hit {
            target
        } else {
            match st.city.as_ref() {
                Some(city) => miss_impact(city, place, from, target, w.range),
                None => target,
            }
        };
        if hit {
            wound_foe(st, foe, w.damage);
        }
        play_round(
            st,
            place,
            fx,
            Round {
                from,
                impact,
                hit,
                damage: w.damage,
                k,
            },
        );
        out.push(hit.then_some(w.damage));
    }
    out
}

fn outcomes(rounds: &[Option<u8>]) -> String {
    rounds
        .iter()
        .map(|r| match r {
            Some(d) => d.to_string(),
            None => "miss".to_string(),
        })
        .collect::<Vec<_>>()
        .join(" · ")
}

/// You fire the worn gun at a foe: one action point, every round rolled.
pub fn fire(st: &mut AppState, foe: usize) -> Result<(), String> {
    let (_, w) = your_weapon(st)?;
    let chance = your_chance(st, foe)?;
    crate::net::spend_ap(st)?;
    let from = st.player.as_ref().map(|p| p.pos).unwrap_or_default();
    let name = st.foes[foe].name.clone();
    let rounds = volley(st, from, foe, &w, chance, &w.fx.clone());
    st.say(format!("you fire · {name} {}", outcomes(&rounds)));
    Ok(())
}

/// A foe fires its weapon at you.
pub fn fire_at_you(st: &mut AppState, foe: usize) {
    let Some((name, from, reflex, place)) = st
        .foes
        .get(foe)
        .map(|f| (f.name.clone(), f.pos, f.reflex, f.place))
    else {
        return;
    };
    let Some(w) = st
        .foes
        .get(foe)
        .and_then(|f| f.weapon.as_ref())
        .and_then(|id| st.content.item(id))
        .and_then(|i| i.weapon.clone())
    else {
        return;
    };
    let Some(me) = st.player.as_ref().map(|p| p.pos) else {
        return;
    };
    let dist = from.chebyshev(me);
    let chance = game::hit_chance(reflex, true, dist, w.range);
    let mut rounds = Vec::new();
    for k in 0..w.rounds as usize {
        let hit = game::round_hits(roll(st), chance);
        let impact = if hit {
            me
        } else {
            match st.city.as_ref() {
                Some(city) => miss_impact(city, place, from, me, w.range),
                None => me,
            }
        };
        if hit {
            wound_you(st, foe, w.damage);
        }
        play_round(
            st,
            place,
            &w.fx,
            Round {
                from,
                impact,
                hit,
                damage: w.damage,
                k,
            },
        );
        rounds.push(hit.then_some(w.damage));
    }
    let line = if rounds.iter().any(Option::is_some) {
        format!(
            "{name} fires · {} · hp {}",
            outcomes(&rounds),
            st.character.hp
        )
    } else {
        format!("{name} fires · {}", outcomes(&rounds))
    };
    st.warn(line);
}

/// You fire a strike ability at a foe: its action points, one round of its damage,
/// its stun on a hit.
pub fn strike(st: &mut AppState, id: &AbilityId, foe: usize) -> Result<(), String> {
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
    }) = ability.kind.clone()
    else {
        return Err(format!("{} is not a strike", ability.name));
    };
    let dist = reach(st, foe, range)?;
    match st.net.engaged.as_ref() {
        Some(e) if e.actor() != crate::net::Actor::You => return Err("not your turn".into()),
        Some(e) if e.phase == crate::net::Phase::Move => {
            return Err("move first · space to stay".into());
        }
        Some(e) if e.ap < ap => return Err("no AP left".into()),
        _ => {}
    }
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
    )?;
    for _ in 0..ap {
        crate::net::spend_ap(st)?;
    }
    let chance = game::hit_chance(
        st.character.stat(&st.content, Stat::Net),
        st.character.has_skill(&skill),
        dist,
        range,
    );
    let w = Weapon {
        skill,
        damage,
        rounds: 1,
        range,
        fx: ability.fx.clone(),
    };
    let from = at.pos;
    let name = st.foes[foe].name.clone();
    let rounds = volley(st, from, foe, &w, chance, &ability.fx);
    if rounds.iter().any(Option::is_some) && stun > 0 {
        if let Some(f) = st.foes.get_mut(foe) {
            f.stunned = f.stunned.max(stun);
        }
        st.say(format!("{name} stunned"));
    }
    st.say(format!("{} · {name} {}", ability.name, outcomes(&rounds)));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::net::{Actor, Phase};
    use citygen::StratumKind;
    use game::SkillId;

    const STREET: Place = Place::Stratum(StratumKind::Street);

    /// The lot with the goon at `goon`, you skilled with the Ratchet, engaged by
    /// hand and on your act half.
    fn engaged(goon: Pos) -> AppState {
        let mut st = foes::testutil::lot(goon);
        st.character.skills.insert(SkillId::new("handguns"));
        crate::net::engage(&mut st, STREET, true);
        assert_eq!(st.net.engaged.as_ref().unwrap().actor(), Actor::You);
        assert!(crate::net::stay(&mut st));
        st.tick = 500;
        st
    }

    fn kinds(st: &AppState) -> Vec<String> {
        st.fx
            .iter()
            .map(|i| format!("{:?}@{}→{:?}", i.fx, i.started, i.targets[0]))
            .collect()
    }

    fn logged(st: &AppState, needle: &str) -> bool {
        st.log.iter().any(|e| e.text.contains(needle))
    }

    #[test]
    fn a_burst_rolls_three_rounds_and_pushes_the_fx_in_order() {
        let mut st = engaged(Pos::new(14, 8));
        st.force_hit = Some(true);
        assert_eq!(
            your_chance(&st, 0).unwrap(),
            74,
            "reflex 6, skilled, 8 of 12"
        );
        fire(&mut st, 0).unwrap();
        assert_eq!(st.foes[0].hp, 18);
        assert_eq!(st.net.engaged.as_ref().unwrap().ap, 1);
        assert!(
            logged(&st, "you fire · goon 4 · 4 · 4"),
            "{:?}",
            st.log.latest()
        );
        let got = kinds(&st);
        let want: Vec<String> = (0..3)
            .flat_map(|k| {
                let t = 500 + 4 * k;
                // Eight tiles at three a tick: the impact lands three ticks in.
                vec![
                    format!("Flash {{ big: true }}@{t}→Pos {{ x: 14, y: 8 }}"),
                    format!(
                        "Tracer {{ colour: (255, 170, 40), speed: 3 }}@{t}→Pos {{ x: 14, y: 8 }}"
                    ),
                    format!("Casing@{t}→Pos {{ x: 14, y: 8 }}"),
                    format!("Chips@{}→Pos {{ x: 14, y: 8 }}", t + 3),
                    format!(
                        "Number {{ tag: false }}@{}→Pos {{ x: {}, y: 8 }}",
                        t + 3,
                        14 + k
                    ),
                ]
            })
            .collect();
        assert_eq!(got, want);
        assert!(
            st.fx
                .iter()
                .all(|i| i.hit && i.place == STREET && i.origin == Pos::new(6, 8))
        );
        let labels: Vec<&str> = st
            .fx
            .iter()
            .filter(|i| !i.label.is_empty())
            .map(|i| i.label.as_str())
            .collect();
        assert_eq!(labels, vec!["-4", "-4", "-4"]);
    }

    #[test]
    fn a_miss_sparks_on_the_wall_behind() {
        let mut st = engaged(Pos::new(37, 8));
        st.player.as_mut().unwrap().pos = Pos::new(30, 8);
        st.player
            .as_mut()
            .unwrap()
            .set_vis(Pos::new(37, 8), crate::render::Visibility::Visible);
        st.force_hit = Some(false);
        fire(&mut st, 0).unwrap();
        assert_eq!(st.foes[0].hp, foes::GOON_HP);
        assert!(logged(&st, "you fire · goon miss · miss · miss"));
        let sparks: Vec<Pos> = st
            .fx
            .iter()
            .filter(|i| matches!(i.fx, Fx::Spark))
            .map(|i| i.targets[0])
            .collect();
        assert_eq!(sparks, vec![Pos::new(40, 8); 3], "the wall ring");
        assert!(
            st.fx
                .iter()
                .all(|i| !i.hit && i.targets[0] == Pos::new(40, 8))
        );
        assert!(
            !st.fx
                .iter()
                .any(|i| matches!(i.fx, Fx::Chips | Fx::Number { .. }))
        );
    }

    #[test]
    fn a_miss_stops_at_range_on_open_ground() {
        let mut st = engaged(Pos::new(10, 8));
        st.force_hit = Some(false);
        fire(&mut st, 0).unwrap();
        let spark = st.fx.iter().find(|i| matches!(i.fx, Fx::Spark)).unwrap();
        assert_eq!(
            spark.targets[0],
            Pos::new(18, 8),
            "twelve tiles from the muzzle"
        );
        let city = st.city.as_ref().unwrap();
        assert_eq!(
            miss_impact(city, STREET, Pos::new(6, 8), Pos::new(38, 8), 40),
            Pos::new(40, 8)
        );
        assert_eq!(
            miss_impact(city, STREET, Pos::new(6, 8), Pos::new(6, 2), 10),
            Pos::new(6, 0),
            "straight up into the wall"
        );
    }

    #[test]
    fn a_dummy_stands_shrugs_every_round_off_and_never_hunts() {
        let mut st = foes::testutil::lot(Pos::new(14, 8));
        st.foes[0] = foes::Foe::dummy(STREET, Pos::new(14, 8));
        st.character.skills.insert(SkillId::new("handguns"));
        crate::net::engage(&mut st, STREET, true);
        assert!(crate::net::stay(&mut st));
        st.force_hit = Some(true);
        fire(&mut st, 0).unwrap();
        assert_eq!(st.foes[0].hp, st.foes[0].hp_max, "nothing gets through");
        assert!(!st.foes[0].hunting() && st.foes[0].standing());
        assert_eq!(
            st.fx.iter().filter(|i| i.label == "-4").count(),
            3,
            "the numbers still lift off it"
        );
        assert!(logged(&st, "you fire · dummy 4 · 4 · 4"));
        // Its turn passes at once; with nothing hunting, `e` calms the lot.
        crate::net::end_turn(&mut st);
        assert!(crate::net::your_turn(&st));
        assert!(!crate::net::hunting(&st));
        assert_eq!(foes::weapon_name(&st, &st.foes[0]), "unarmed");
    }

    #[test]
    fn a_shot_foe_hunts_you_even_when_it_had_not_seen_you() {
        // Twelve tiles: outside the goon's sight, inside the Ratchet's reach.
        let mut st = engaged(Pos::new(18, 8));
        assert!(!st.foes[0].hunting());
        st.force_hit = Some(false);
        fire(&mut st, 0).unwrap();
        assert_eq!(
            st.foes[0].mode,
            foes::FoeMode::Hunting {
                seen: Pos::new(6, 8),
                lost: 0
            },
            "a miss is loud enough"
        );
        // Its turn then runs: it walks toward you on the pace.
        crate::net::end_turn(&mut st);
        assert_eq!(st.net.engaged.as_ref().unwrap().actor(), Actor::Foe(0));
        st.tick += crate::net::PACE_TICKS;
        if let (Some(city), Some(world)) = (st.city.as_ref(), st.world.as_mut()) {
            world.advance(city, world.tick() + crate::net::PACE_TICKS);
        }
        crate::net::pace(&mut st);
        assert_eq!(st.foes[0].pos, Pos::new(17, 8));
    }

    #[test]
    fn a_miss_at_exactly_range_never_sparks_on_the_target() {
        let mut st = engaged(Pos::new(18, 8));
        st.force_hit = Some(false);
        fire(&mut st, 0).unwrap();
        let spark = st.fx.iter().find(|i| matches!(i.fx, Fx::Spark)).unwrap();
        assert_eq!(
            spark.targets[0],
            Pos::new(19, 8),
            "one tile past, over the range"
        );
        let city = st.city.as_ref().unwrap();
        // Against the wall the wall takes it, beyond the range too.
        assert_eq!(
            miss_impact(city, STREET, Pos::new(29, 8), Pos::new(39, 8), 10),
            Pos::new(40, 8)
        );
    }

    #[test]
    fn fire_refuses_without_a_line() {
        let mut st = engaged(Pos::new(14, 8));
        st.player
            .as_mut()
            .unwrap()
            .set_vis(Pos::new(14, 8), crate::render::Visibility::Remembered);
        assert_eq!(fire(&mut st, 0).unwrap_err(), "no line");
        assert_eq!(st.net.engaged.as_ref().unwrap().ap, 2);
        assert!(st.fx.is_empty());
        // A foe round the wall ring's corner: the line crosses the wall.
        let mut st = engaged(Pos::new(14, 8));
        st.foes[0].pos = Pos::new(39, 1);
        st.player.as_mut().unwrap().pos = Pos::new(39, 10);
        st.foes.push(foes::Foe::goon(STREET, Pos::new(39, 1)));
        assert!(matches!(
            fire(&mut st, 0).unwrap_err().as_str(),
            "no line" | "out of range"
        ));
    }

    #[test]
    fn fire_refuses_out_of_range_and_without_a_gun() {
        let mut st = engaged(Pos::new(20, 8));
        assert_eq!(fire(&mut st, 0).unwrap_err(), "out of range");
        st.character
            .wear(Slot::Primary, ItemId::new("shiv_blade"), None);
        assert_eq!(fire(&mut st, 0).unwrap_err(), "nothing to shoot with");
        let mut st = engaged(Pos::new(14, 8));
        st.foes[0].down = true;
        assert_eq!(fire(&mut st, 0).unwrap_err(), "pick a target");
        assert_eq!(st.net.engaged.as_ref().unwrap().ap, 2);
    }

    #[test]
    fn the_goon_goes_down_at_zero() {
        let mut st = engaged(Pos::new(14, 8));
        st.foes[0].hp = 4;
        st.force_hit = Some(true);
        fire(&mut st, 0).unwrap();
        assert!(st.foes[0].down && st.foes[0].hp == 0);
        assert!(logged(&st, "goon down"));
        assert!(foes::standing(&st, STREET).is_empty());
    }

    #[test]
    fn the_foes_shot_takes_your_hp_and_can_kill() {
        let mut st = engaged(Pos::new(14, 8));
        let hp = st.character.hp;
        st.force_hit = Some(true);
        fire_at_you(&mut st, 0);
        assert_eq!(st.character.hp, hp - 5);
        assert!(
            logged(&st, &format!("goon fires · 5 · hp {}", hp - 5)),
            "{:?}",
            st.log.latest()
        );
        let shapes: Vec<&Fx> = st.fx.iter().map(|i| &i.fx).collect();
        assert!(
            matches!(
                shapes[..],
                [
                    Fx::Flash { big: false },
                    Fx::Tracer { .. },
                    Fx::Casing,
                    Fx::Chips,
                    Fx::Number { .. }
                ]
            ),
            "{shapes:?}"
        );
        assert!(
            st.fx
                .iter()
                .all(|i| i.origin == Pos::new(14, 8) && i.targets[0].x <= 6)
        );
        assert_eq!(st.fx[1].until(), st.tick + 3, "eight tiles at three a tick");
        st.force_hit = Some(false);
        fire_at_you(&mut st, 0);
        assert!(logged(&st, "goon fires · miss"));
        st.character.hp = 3;
        st.force_hit = Some(true);
        fire_at_you(&mut st, 0);
        assert_eq!(st.character.hp, 0);
        assert_eq!(st.net.death.as_deref(), Some("a goon on the street"));
        assert!(st.net.engaged.is_none());
    }

    #[test]
    fn static_costs_two_ap_stuns_and_tears() {
        let mut st = engaged(Pos::new(14, 8));
        st.character.give(ItemId::new("neural_port"), 1);
        st.character
            .wear(Slot::Neural, ItemId::new("neural_port"), None);
        st.force_hit = Some(true);
        let id = AbilityId::new("static");
        strike(&mut st, &id, 0).unwrap();
        assert_eq!(st.net.engaged.as_ref().unwrap().ap, 0);
        assert_eq!(st.foes[0].hp, foes::GOON_HP - 6);
        assert_eq!(st.foes[0].stunned, 1);
        assert!(logged(&st, "goon stunned"));
        assert!(logged(&st, "Static · goon 6"));
        let shapes: Vec<&Fx> = st.fx.iter().map(|i| &i.fx).collect();
        assert!(
            matches!(
                shapes[..],
                [
                    Fx::Flash { big: false },
                    Fx::Streak { .. },
                    Fx::Tear,
                    Fx::Number { tag: true }
                ]
            ),
            "{shapes:?}"
        );
        assert_eq!(st.fx[3].label, "-6");
        assert_eq!(strike(&mut st, &id, 0).unwrap_err(), "no AP left");
        // Cooling: refused before any AP goes.
        let mut st = engaged(Pos::new(14, 8));
        st.character.give(ItemId::new("neural_port"), 1);
        st.character
            .wear(Slot::Neural, ItemId::new("neural_port"), None);
        st.character.fired.insert(id.clone(), st.tick);
        assert!(strike(&mut st, &id, 0).unwrap_err().contains("cooling"));
        assert_eq!(st.net.engaged.as_ref().unwrap().ap, 2);
        // Off a foe, or on the move half.
        st.foes[0].down = true;
        assert_eq!(strike(&mut st, &id, 0).unwrap_err(), "pick a target");
        let mut st = foes::testutil::lot(Pos::new(14, 8));
        st.character.give(ItemId::new("neural_port"), 1);
        st.character
            .wear(Slot::Neural, ItemId::new("neural_port"), None);
        crate::net::engage(&mut st, STREET, true);
        assert_eq!(crate::net::phase(&st), Some(Phase::Move));
        assert_eq!(
            strike(&mut st, &id, 0).unwrap_err(),
            "move first · space to stay"
        );
    }
}
