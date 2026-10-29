//! Shooting: a fire action resolved round by round, the pictures it pushes, and
//! what the rounds do to whoever they hit.

use crate::app::{AppState, FxInstance};
use crate::foes;
use crate::render::fx::tracer_ticks;
use citygen::pathing::bresenham;
use citygen::rng::hash64;
use citygen::{City, Place, Pos};
use game::{AbilityId, AbilityKind, Action, Active, Attribute, Fx, ItemId, Slot, Weapon};

/// Ticks between the rounds of one burst.
pub const ROUND_GAP: u64 = 4;

/// One roll of 1..=100.
pub fn roll(st: &mut AppState) -> u8 {
    // `force_hit` forces every roll, the armour's own too: with ARM > 0 every forced
    // hit on you also turns.
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
pub(crate) fn reach(st: &AppState, foe: usize, range: i32) -> Result<i32, String> {
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
    let bonus = if st.foes[foe].marked {
        mark_bonus(st)
    } else {
        0
    };
    Ok(game::hit_chance_with(
        st.character.attribute(&st.content, Attribute::Reflex),
        st.character.has_skill(&w.skill),
        dist,
        w.range,
        bonus,
    ))
}

/// What a mark adds to your chance: the bonus of the first mark ability you have
/// (one mark at a time, so the first is the one).
fn mark_bonus(st: &AppState) -> u8 {
    st.character
        .abilities(&st.content)
        .iter()
        .find_map(|(id, _)| match st.content.ability(id).map(|a| &a.kind) {
            Some(AbilityKind::Active(Active::Mark { bonus, .. })) => Some(*bonus),
            _ => None,
        })
        .unwrap_or(0)
}

/// Everything one round needs to be drawn.
struct Round {
    from: Pos,
    impact: Pos,
    hit: bool,
    damage: u8,
    k: usize,
    /// A colour the whole shot takes (the net's attacks); `None` for a gun's own.
    tint: Option<(u8, u8, u8)>,
}

/// Ticks from the muzzle to the impact: the tracer's flight, a streak's one, none
/// for a shape that needs no flight.
pub(crate) fn flight_ticks(fx: &[Fx], from: Pos, impact: Pos) -> u64 {
    fx.iter()
        .find_map(|f| match f {
            Fx::Tracer { speed, .. } => Some(tracer_ticks(from, impact, *speed)),
            Fx::Streak { .. } => Some(1),
            _ => None,
        })
        .unwrap_or(0)
}

/// Pushes the shapes of one round, offset `k` gaps into the burst: the flash and
/// the flight at once, the impact when the flight lands.
fn play_round(st: &mut AppState, place: Place, fx: &[Fx], r: Round) {
    let started = st.tick + r.k as u64 * ROUND_GAP;
    let flight = flight_ticks(fx, r.from, r.impact);
    for shape in fx {
        let (at, target) = match shape {
            Fx::Flash { .. } => (started, r.impact),
            f if f.is_flight() => (started, r.impact),
            // Drawn along a push or round the firer, not at the impact.
            Fx::Trail | Fx::Pulse { .. } => continue,
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
            tint: r.tint,
            arc: None,
        });
    }
}

/// The shapes of one shot from `from` to `impact`, pushed from `now`: the flash and
/// the flight at once, the impact shapes when the flight lands. For the net's
/// attack program, which rolls its own way but looks like a shot.
#[allow(clippy::too_many_arguments)]
pub fn play_shot(
    st: &mut AppState,
    place: Place,
    fx: &[Fx],
    from: Pos,
    impact: Pos,
    hit: bool,
    damage: u8,
    tint: Option<(u8, u8, u8)>,
) {
    play_round(
        st,
        place,
        fx,
        Round {
            from,
            impact,
            hit,
            damage,
            k: 0,
            tint,
        },
    );
}

/// The attack program loaded in the jacked deck, and its slot.
fn loaded_attack(st: &AppState) -> Option<(ItemId, usize)> {
    st.character
        .loaded
        .iter()
        .enumerate()
        .find(|(_, id)| {
            st.content
                .item(id)
                .and_then(|i| i.program)
                .is_some_and(|p| p.op == game::Op::Attack)
        })
        .map(|(slot, id)| (id.clone(), slot))
}

/// Your chance per turn of the loaded attack program against a standing piece,
/// in percent; refused without one or out of reach.
pub fn attack_chance(st: &AppState, b: citygen::BuildingId, i: usize) -> Result<u8, String> {
    let (id, _) = loaded_attack(st).ok_or_else(|| "no attack loaded".to_string())?;
    if !crate::net::piece_in_reach(st, b, i) {
        return Err("out of reach".to_string());
    }
    let spec = st
        .content
        .item(&id)
        .and_then(|i| i.program)
        .ok_or_else(|| "no attack loaded".to_string())?;
    let target = game::HackTarget::Guard {
        building: b,
        index: i,
    };
    let kind =
        crate::net::target_kind(st, b, target).ok_or_else(|| "nothing stands there".to_string())?;
    let security = st
        .constructs
        .get(b)
        .map(|k| k.security)
        .ok_or_else(|| "not in a construct".to_string())?;
    Ok(game::chance(
        game::power(&st.character, &st.content, &spec, kind),
        game::difficulty(security),
    ))
}

/// You run the loaded attack program at a piece of ICE under the cursor: one
/// action point, one turn of the run. Returns the program's slot, for the flash.
pub fn attack_ice(st: &mut AppState, b: citygen::BuildingId, i: usize) -> Result<usize, String> {
    let (id, slot) = loaded_attack(st).ok_or_else(|| "no attack loaded".to_string())?;
    if !crate::net::piece_in_reach(st, b, i) {
        return Err("out of reach".to_string());
    }
    crate::net::spend_ap(st)?;
    let target = game::HackTarget::Guard {
        building: b,
        index: i,
    };
    match crate::net::start_run(st, id, target) {
        Ok(line) => {
            st.say(line);
            crate::net::step_run(st);
            Ok(slot)
        }
        Err(why) => {
            // Nothing ran: the point comes back.
            if let Some(e) = st.net.engaged.as_mut() {
                e.ap = (e.ap + 1).min(crate::net::BASE_AP);
            }
            Err(why)
        }
    }
}

/// A foe takes damage; at zero it is down.
pub(crate) fn wound_foe(st: &mut AppState, foe: usize, damage: u8) {
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
        f.marked = false;
        let name = f.name.clone();
        st.say(format!("{name} down"));
    }
}

/// You take damage; at zero you are dead, and `cause` is the epitaph.
pub(crate) fn wound_you(st: &mut AppState, cause: &str, damage: u8) {
    st.character.hp = st.character.hp.saturating_sub(damage as u32);
    if st.character.hp == 0 && st.net.death.is_none() {
        st.net.death = Some(cause.to_string());
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
                tint: None,
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
    let turn = game::turn_chance(st.character.armor(&st.content));
    let damage = resisted_u8(
        w.damage,
        st.character.resist(&st.content, game::Harm::Kinetic),
    );
    let mut words = Vec::new();
    let mut landed = false;
    for k in 0..w.rounds as usize {
        let hit = game::round_hits(roll(st), chance);
        // The armour's own roll: a turned round sparks off you as a miss would.
        let turned = hit && turn > 0 && roll(st) as u32 <= turn;
        let wounds = hit && !turned;
        let impact = if hit {
            me
        } else {
            match st.city.as_ref() {
                Some(city) => miss_impact(city, place, from, me, w.range),
                None => me,
            }
        };
        if wounds {
            wound_you(st, &format!("a {name} on the street"), damage);
        }
        play_round(
            st,
            place,
            &w.fx,
            Round {
                from,
                impact,
                hit: wounds,
                damage,
                k,
                tint: None,
            },
        );
        landed |= wounds;
        words.push(match (hit, turned) {
            (true, false) => damage.to_string(),
            (true, true) => "turned".to_string(),
            (false, _) => "miss".to_string(),
        });
    }
    let words = words.join(" · ");
    let line = if landed {
        format!("{name} fires · {words} · hp {}", st.character.hp)
    } else {
        format!("{name} fires · {words}")
    };
    st.warn(line);
}

/// `game::resisted` on a harm held as a byte: a resistance below zero can double
/// it, and it stops at the top.
pub(crate) fn resisted_u8(amount: u8, resist: i32) -> u8 {
    u8::try_from(game::resisted(amount as u32, resist)).unwrap_or(u8::MAX)
}

/// Your chance with a strike ability at a foe, in percent: its attribute and skill, the
/// distance, its range.
pub fn strike_chance(st: &AppState, id: &AbilityId, foe: usize) -> Result<u8, String> {
    let ability = st
        .character
        .ability(&st.content, id)
        .ok_or_else(|| format!("unknown ability {id}"))?;
    let AbilityKind::Active(Active::Strike {
        range,
        skill,
        attribute,
        ..
    }) = ability.kind
    else {
        return Err(format!("{} is not a strike", ability.name));
    };
    let dist = reach(st, foe, range)?;
    Ok(game::hit_chance(
        st.character.attribute(&st.content, attribute),
        st.character.has_skill(&skill),
        dist,
        range,
    ))
}

/// Ticks a chained hop's shapes start after the first impact, per hop.
const HOP_TICKS: u64 = 2;

/// What a chain hop does: half the damage, at least 1.
pub(crate) fn hop_damage(damage: u8) -> u8 {
    (damage / 2).max(1)
}

/// The tick hop `hop` (1-based) of a chain starts its shapes, from the first impact.
pub(crate) fn hop_start(impact: u64, hop: u64) -> u64 {
    impact + HOP_TICKS * hop
}

/// The nearest candidate within two tiles of `from`, by distance then index, that
/// is not already in `hit`.
pub(crate) fn nearest_other(
    candidates: impl Iterator<Item = (usize, Pos)>,
    from: Pos,
    hit: &[usize],
) -> Option<usize> {
    candidates
        .filter(|(i, p)| !hit.contains(i) && p.chebyshev(from) <= 2)
        .min_by_key(|(i, p)| (p.chebyshev(from), *i))
        .map(|(i, _)| i)
}

/// Starts the shapes pushed since `from_len` at tick `start` instead of now.
pub(crate) fn start_shapes_at(st: &mut AppState, from_len: usize, start: u64) {
    let late = start.saturating_sub(st.tick);
    for i in &mut st.fx[from_len..] {
        i.started += late;
    }
}

/// On a hit the foe goes up to `steps` tiles straight away from `from`, stopping
/// before a tile that is not walkable or holds you or another standing foe. A
/// `Trail` plays on each tile it left, from `at`. Returns the tiles it moved.
fn push_foe(st: &mut AppState, foe: usize, from: Pos, steps: u8, at: u64, trail: bool) -> u8 {
    let Some((place, mut pos)) = st.foes.get(foe).map(|f| (f.place, f.pos)) else {
        return 0;
    };
    let (dx, dy) = ((pos.x - from.x).signum(), (pos.y - from.y).signum());
    let mut moved = 0;
    for _ in 0..steps {
        let next = pos.offset(dx, dy);
        let open = place
            .stratum()
            .zip(st.city.as_ref())
            .and_then(|(k, city)| city.tile(k, next))
            .is_some_and(|t| t.is_walkable());
        let held = next == from
            || st
                .foes
                .iter()
                .enumerate()
                .any(|(i, f)| i != foe && f.standing() && f.place == place && f.pos == next);
        if !open || held {
            break;
        }
        if trail {
            st.fx.push(FxInstance {
                fx: Fx::Trail,
                place,
                origin: pos,
                radius: 0,
                started: at + moved as u64,
                revealed: Vec::new(),
                targets: vec![next],
                label: String::new(),
                hit: true,
                tint: None,
                arc: None,
            });
        }
        pos = next;
        moved += 1;
    }
    if let Some(f) = st.foes.get_mut(foe) {
        f.pos = pos;
    }
    moved
}

/// What a chain carries from one victim to the next.
struct Chain<'a> {
    you: Pos,
    /// The tick the first victim's shapes land: the hops count from it.
    impact: u64,
    hops: u8,
    damage: u8,
    stun: u8,
    fx: &'a [Fx],
    source: &'a str,
}

/// Jolt's arc: from each victim, the nearest other standing foe within two tiles
/// with a clear line takes half the damage and the stun, no roll; `hops` of them.
fn chain_foes(st: &mut AppState, first: usize, arc: Chain) {
    let half = hop_damage(arc.damage);
    let fx: Vec<Fx> = arc
        .fx
        .iter()
        .filter(|f| !matches!(f, Fx::Flash { .. }))
        .cloned()
        .collect();
    let mut hit = vec![first];
    for hop in 1..=arc.hops as u64 {
        let last = *hit.last().expect("the first victim is in");
        let Some((place, at)) = st.foes.get(last).map(|f| (f.place, f.pos)) else {
            return;
        };
        let Some(city) = st.city.as_ref() else {
            return;
        };
        let candidates = st
            .foes
            .iter()
            .enumerate()
            .filter(|(_, f)| {
                f.standing() && f.place == place && foes::line_clear(city, place, at, f.pos)
            })
            .map(|(i, f)| (i, f.pos));
        let next = nearest_other(candidates, at, &hit);
        let Some(next) = next else {
            return;
        };
        let target = st.foes[next].pos;
        wound_foe(st, next, half);
        if let Some(f) = st.foes.get_mut(next) {
            f.stunned = f.stunned.max(arc.stun);
            if f.standing() && !f.inert {
                f.mode = foes::FoeMode::Hunting {
                    seen: arc.you,
                    lost: 0,
                };
            }
        }
        let from_len = st.fx.len();
        play_round(
            st,
            place,
            &fx,
            Round {
                from: at,
                impact: target,
                hit: true,
                damage: half,
                k: 0,
                tint: None,
            },
        );
        start_shapes_at(st, from_len, hop_start(arc.impact, hop));
        let name = st.foes[next].name.clone();
        st.say(format!(
            "{} arcs · {name} {half}",
            arc.source.to_lowercase()
        ));
        hit.push(next);
    }
}

/// You fire a strike ability at a foe: its action points, one round of its damage
/// with its own attribute, its stun on a hit; a push throws the foe back, a chain arcs
/// on to the next.
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
        push,
        chain,
        ..
    }) = ability.kind.clone()
    else {
        return Err(format!("{} is not a strike", ability.name));
    };
    if matches!(
        st.player.as_ref().map(|p| p.place),
        Some(Place::Construct(_))
    ) {
        return Err("not in here".to_string());
    }
    let chance = strike_chance(st, id, foe)?;
    crate::casts::can_act(st, ap)?;
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
    let w = Weapon {
        skill,
        damage,
        rounds: 1,
        range,
        fx: ability.fx.clone(),
    };
    let from = at.pos;
    let name = st.foes[foe].name.clone();
    let impact = st.tick + flight_ticks(&ability.fx, from, st.foes[foe].pos);
    let rounds = volley(st, from, foe, &w, chance, &ability.fx);
    let hit = rounds.iter().any(Option::is_some);
    let mut line = format!(
        "{} · {name} {}",
        ability.name.to_lowercase(),
        outcomes(&rounds)
    );
    if hit && stun > 0 {
        if let Some(f) = st.foes.get_mut(foe) {
            f.stunned = f.stunned.max(stun);
        }
        st.say(format!("{name} stunned"));
    }
    if hit && push > 0 && st.foes[foe].standing() {
        let trail = ability.fx.contains(&Fx::Trail);
        let moved = push_foe(st, foe, from, push, impact, trail);
        line.push_str(&format!(" · back {moved}"));
    }
    st.say(line);
    if hit && chain > 0 {
        let arc = Chain {
            you: from,
            impact,
            hops: chain,
            damage,
            stun,
            fx: &ability.fx,
            source: &ability.name,
        };
        chain_foes(st, foe, arc);
    }
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

    /// The ceramic plating made to do only `perks`, installed on your skin.
    fn plated(st: &mut AppState, perks: Vec<game::Perk>) {
        let plating = ItemId::new("ceramic_plating");
        let item = st.content.items.iter_mut().find(|i| i.id == plating);
        item.unwrap().perks = perks;
        st.character.chrome.insert(game::Site::Skin, plating);
    }

    #[test]
    fn armour_turns_rounds() {
        assert_eq!(game::turn_chance(0), 0);
        assert_eq!(game::turn_chance(3), 30);
        assert_eq!(game::turn_chance(6), 60);
        assert_eq!(game::turn_chance(9), 60, "never past 60");
        assert_eq!(game::turn_chance(-2), 0);
        let mut st = engaged(Pos::new(14, 8));
        plated(&mut st, vec![game::Perk::Armor(6)]);
        assert_eq!(st.character.armor(&st.content), 6);
        // A tick whose first roll lands the scrap nine's round at eight tiles and
        // whose second, the armour's own, turns it at 60%.
        let chance = game::hit_chance(st.foes[0].reflex, true, 8, 8);
        let shots = st.shots;
        let roll_at = |tick: u64, n: u64| (hash64(tick, shots + n) % 100 + 1) as u32;
        let tick = (500..)
            .find(|t| roll_at(*t, 1) <= chance as u32 && roll_at(*t, 2) <= 60)
            .unwrap();
        st.tick = tick;
        let hp = st.character.hp;
        fire_at_you(&mut st, 0);
        assert_eq!(st.character.hp, hp, "turned: nothing gets through");
        assert_eq!(
            st.log.latest().map(|m| m.text.as_str()),
            Some("goon fires · turned")
        );
        let me = Pos::new(6, 8);
        let spark = st.fx.iter().find(|i| matches!(i.fx, Fx::Spark));
        assert_eq!(spark.map(|i| i.targets[0]), Some(me), "it sparks off you");
        assert!(
            !st.fx
                .iter()
                .any(|i| matches!(i.fx, Fx::Chips | Fx::Number { .. })),
            "no wound drawn"
        );
        // Bare, the same tick wounds.
        st.character.chrome.remove(&game::Site::Skin);
        assert_eq!(st.character.armor(&st.content), 0);
        st.shots = shots;
        st.fx.clear();
        fire_at_you(&mut st, 0);
        assert_eq!(st.character.hp, hp - 5);
        assert!(logged(&st, &format!("goon fires · 5 · hp {}", hp - 5)));
    }

    #[test]
    fn kinetic_resist_cuts_rounds() {
        let mut st = engaged(Pos::new(14, 8));
        plated(&mut st, vec![game::Perk::Resist(game::Harm::Kinetic, 50)]);
        assert_eq!(st.character.resist(&st.content, game::Harm::Kinetic), 50);
        // The goon with a Ratchet: three rounds of 4, each halved.
        st.foes[0].weapon = Some(ItemId::new("ratchet_smg"));
        st.force_hit = Some(true);
        let hp = st.character.hp;
        fire_at_you(&mut st, 0);
        assert_eq!(st.character.hp, hp - 6);
        assert!(
            logged(&st, &format!("goon fires · 2 · 2 · 2 · hp {}", hp - 6)),
            "{:?}",
            st.log.latest()
        );
        let labels: Vec<&str> = st
            .fx
            .iter()
            .filter(|i| matches!(i.fx, Fx::Number { .. }))
            .map(|i| i.label.as_str())
            .collect();
        assert_eq!(labels, vec!["-2", "-2", "-2"]);
    }

    #[test]
    fn static_costs_two_ap_stuns_and_tears() {
        let mut st = engaged(Pos::new(14, 8));
        st.character.give(ItemId::new("neural_port"), 1);
        st.character
            .chrome
            .insert(game::Site::Neural, ItemId::new("neural_port"));
        st.force_hit = Some(true);
        let id = AbilityId::new("static");
        strike(&mut st, &id, 0).unwrap();
        assert_eq!(st.net.engaged.as_ref().unwrap().ap, 0);
        assert_eq!(st.foes[0].hp, foes::GOON_HP - 6);
        assert_eq!(st.foes[0].stunned, 1);
        assert!(logged(&st, "goon stunned"));
        assert!(logged(&st, "static · goon 6"));
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
            .chrome
            .insert(game::Site::Neural, ItemId::new("neural_port"));
        st.character.fired.insert(id.clone(), st.tick);
        assert!(strike(&mut st, &id, 0).unwrap_err().contains("cooling"));
        assert_eq!(st.net.engaged.as_ref().unwrap().ap, 2);
        // Off a foe, or on the move half.
        st.foes[0].down = true;
        assert_eq!(strike(&mut st, &id, 0).unwrap_err(), "pick a target");
        let mut st = foes::testutil::lot(Pos::new(14, 8));
        st.character.give(ItemId::new("neural_port"), 1);
        st.character
            .chrome
            .insert(game::Site::Neural, ItemId::new("neural_port"));
        crate::net::engage(&mut st, STREET, true);
        assert_eq!(crate::net::phase(&st), Some(Phase::Move));
        assert_eq!(
            strike(&mut st, &id, 0).unwrap_err(),
            "move first · space to stay"
        );
    }
}
