//! Shooting: a fire action resolved round by round, the pictures it pushes, and
//! what the rounds do to whoever they hit.

use crate::app::{AppState, FxInstance};
use crate::foes;
use crate::render::fx::tracer_ticks;
use crate::render::map::{MapRef, TileMap};
use citygen::pathing::bresenham;
use citygen::rng::hash64;
use citygen::{Place, Pos};
use game::{AbilityId, AbilityKind, Action, Active, Attribute, BodyPart, Fx, ItemId, Weapon};

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
/// the shooter; the target's own tile when nothing lies past it. Over any map: on a
/// floor a shut door takes it as a wall does.
pub fn miss_impact(map: &MapRef, from: Pos, target: Pos, range: i32) -> Pos {
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
        match map.tile(p) {
            Some(t) if foes::blocks_sight(t) => return p,
            Some(t) if t.is_walkable() => last = p,
            _ => break,
        }
    }
    last
}

/// The chance, in percent, that each other hand's gun joins in on a shot.
pub const OFF_HAND_CHANCE: u8 = 25;

/// The gun in a hand, if it holds one.
fn gun_in(st: &AppState, hand: BodyPart) -> Option<(ItemId, Weapon)> {
    let id = st.character.equipped(hand)?;
    let w = st.content.item(id)?.weapon.clone()?;
    Some((id.clone(), w))
}

/// The gun that fires: the main hand's, else the first other hand's that holds one.
pub fn your_weapon(st: &AppState) -> Result<(BodyPart, ItemId, Weapon), String> {
    st.character
        .hands(&st.content)
        .into_iter()
        .find_map(|hand| gun_in(st, hand).map(|(id, w)| (hand, id, w)))
        .ok_or_else(|| "nothing to shoot with".to_string())
}

/// A shot's way to a foe from where you stand: refused out of range or without a
/// clear line in sight; else the distance.
pub(crate) fn reach(st: &AppState, foe: usize, range: i32) -> Result<i32, String> {
    let Some(p) = st.player.as_ref().filter(|_| st.city.is_some()) else {
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
    let line = foes::map_of(st, p.place).is_some_and(|m| foes::line_clear(&m, p.pos, f.pos));
    if !seen || !line {
        return Err("no line".to_string());
    }
    Ok(dist)
}

/// Your chance per round at a foe with the worn gun, in percent.
pub fn your_chance(st: &AppState, foe: usize) -> Result<u8, String> {
    let (_, _, w) = your_weapon(st)?;
    chance_with(st, foe, &w)
}

/// Your chance per round at a foe with the gun `w`, in percent.
pub fn chance_with(st: &AppState, foe: usize, w: &Weapon) -> Result<u8, String> {
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
pub(crate) fn loaded_attack(st: &AppState) -> Option<(ItemId, usize)> {
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

/// Your loaded attack at MASTIFF in your rig's room: one action point, one roll at
/// `rigroom::mastiff::attack_chance`, the plate nearest you broken on a hit (or its core,
/// the coat gone). Refused off its leash, out of reach or with nothing to run it, nothing
/// spent. The program's slot, for the strip's flash.
pub fn attack_mastiff(st: &mut AppState) -> Result<usize, String> {
    let chance = crate::rigroom::mastiff::attack_chance(st)?;
    let (_, slot) = loaded_attack(st).ok_or_else(|| "no attack loaded".to_string())?;
    let me = st.player.as_ref().map(|p| p.pos).ok_or("no body")?;
    let at = (crate::rigroom::room(st))
        .and_then(|r| r.mastiff.as_ref())
        .map(|m| crate::rigroom::mastiff::nearest(m, me))
        .ok_or("nothing there")?;
    crate::net::spend_ap(st)?;
    let hit = game::round_hits(roll(st), chance);
    play_shot(
        st,
        Place::Rig,
        &crate::net::ATTACK_FX,
        me,
        at,
        hit,
        1,
        Some(crate::net::ATTACK_TINT),
    );
    // The winning hit says its own line.
    let line = match hit {
        true => crate::rigroom::mastiff::hit(st, me),
        false => Some(format!("MASTIFF · {}", crate::rigroom::mastiff::miss(st))),
    };
    if let Some(line) = line {
        st.say(line);
    }
    Ok(slot)
}

/// The deck's loaded attack at a piece of ICE on the open Net: an action point, the
/// run started and rolled at once. The program's slot, for the strip's flash.
pub fn attack_grid(st: &mut AppState, foe: usize) -> Result<usize, String> {
    let (id, slot) = loaded_attack(st).ok_or_else(|| "no attack loaded".to_string())?;
    let me = st.player.as_ref().map(|p| p.pos).ok_or("no body")?;
    if !crate::grid::in_reach(st, me).iter().any(|(i, _)| *i == foe) {
        return Err("out of reach".to_string());
    }
    crate::net::spend_ap(st)?;
    match crate::net::start_run(st, id, game::HackTarget::Foe(foe)) {
        Ok(line) => {
            st.say(line);
            crate::net::step_run(st);
            Ok(slot)
        }
        Err(why) => {
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
        if f.turret.is_some() {
            // A wreck: nothing to drop.
            st.say(format!("{name} wrecked"));
            return;
        }
        st.say(format!("{name} down"));
        crate::guards::drop_card(st, foe);
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

/// What one round did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Shot {
    /// It landed, for this much.
    Hit(u8),
    /// It would have landed and the plate turned it.
    Turned,
    Miss,
}

/// A turret's plate takes a round that would have hit: with ARM above 0 it rolls the
/// plate's turn chance, and a turned round costs the plate a point.
pub(crate) fn plate_turns(st: &mut AppState, foe: usize) -> bool {
    let arm = st
        .foes
        .get(foe)
        .and_then(|f| f.turret.as_ref())
        .map_or(0, |t| t.arm);
    if arm == 0 || (roll(st) as u32) > game::turn_chance(arm as i32) {
        return false;
    }
    if let Some(t) = st.foes[foe].turret.as_mut() {
        t.arm -= 1;
    }
    true
}

/// A turret's plate, None for any other foe.
fn plate_of(st: &AppState, foe: usize) -> Option<u8> {
    st.foes.get(foe)?.turret.as_ref().map(|t| t.arm)
}

/// ` · plate {before} → {after}` when the plate changed over a volley.
fn plate_words(before: Option<u8>, after: Option<u8>) -> String {
    match (before, after) {
        (Some(b), Some(a)) if a != b => format!(" · plate {b} → {a}"),
        _ => String::new(),
    }
}

/// Rolls `rounds` at a foe from `from`, each landing or stopping past them; the
/// damage lands at once, the pictures follow, the first `first` gaps in (after the
/// rounds already drawn this shot). A gun's rounds (`plate`) meet a turret's plate
/// first. Returns each round's outcome.
#[allow(clippy::too_many_arguments)]
fn volley(
    st: &mut AppState,
    from: Pos,
    foe: usize,
    w: &Weapon,
    chance: u8,
    fx: &[Fx],
    first: usize,
    plate: bool,
) -> Vec<Shot> {
    let Some((place, target)) = st.foes.get(foe).map(|f| (f.place, f.pos)) else {
        return Vec::new();
    };
    // Shot at, it knows where from: a miss is loud enough to turn it on you.
    if st.foes.get(foe).is_some_and(|f| f.turret.is_some()) {
        crate::turrets::shot_at(st, foe, from);
    } else if let Some(f) = st.foes.get_mut(foe)
        && f.standing()
        && !f.inert
    {
        f.mode = foes::FoeMode::Hunting {
            seen: from,
            lost: 0,
        };
    }
    // From the cloak: every round lands, for double, and the cloak is gone.
    let ambush = crate::cloak::ambush(st);
    let damage = if ambush {
        w.damage.saturating_mul(2)
    } else {
        w.damage
    };
    let mut out = Vec::new();
    for k in first..first + w.rounds as usize {
        let hit = ambush || game::round_hits(roll(st), chance);
        let turned = hit && plate && plate_turns(st, foe);
        let impact = if hit {
            target
        } else {
            foes::map_of(st, place).map_or(target, |m| miss_impact(&m, from, target, w.range))
        };
        if hit && !turned {
            wound_foe(st, foe, damage);
        }
        play_round(
            st,
            place,
            fx,
            Round {
                from,
                impact,
                hit: hit && !turned,
                damage,
                k,
                tint: None,
            },
        );
        out.push(if turned {
            Shot::Turned
        } else if hit {
            Shot::Hit(damage)
        } else {
            Shot::Miss
        });
    }
    out
}

fn outcomes(rounds: &[Shot]) -> String {
    rounds
        .iter()
        .map(|r| match r {
            Shot::Hit(d) => d.to_string(),
            Shot::Turned => "turned".to_string(),
            Shot::Miss => "miss".to_string(),
        })
        .collect::<Vec<_>>()
        .join(" · ")
}

/// You fire the main hand's gun at a foe: one action point, every round rolled. Then
/// each other hand's gun joins in on a 1 in 4 roll, free, if the foe still stands in
/// its reach. The cloak's ambush went with the first volley, so it is that gun's alone.
pub fn fire(st: &mut AppState, foe: usize) -> Result<(), String> {
    let (fired, _, w) = your_weapon(st)?;
    let chance = your_chance(st, foe)?;
    crate::net::spend_ap(st)?;
    let from = st.player.as_ref().map(|p| p.pos).unwrap_or_default();
    let name = st.foes[foe].name.clone();
    // A gunshot on a floor wakes the calm guards in earshot, toward you; a strike
    // (which shares `volley`) is no gunshot.
    if let Some(place) = st.player.as_ref().map(|p| p.place) {
        crate::guards::hear(st, place, from, from);
    }
    let plate = plate_of(st, foe);
    let rounds = volley(st, from, foe, &w, chance, &w.fx.clone(), 0, true);
    st.say(format!(
        "you fire · {name} {}{}",
        outcomes(&rounds),
        plate_words(plate, plate_of(st, foe))
    ));
    // A follow-up's rounds are drawn after the rounds before it.
    let mut drawn = rounds.len();
    for hand in st.character.hands(&st.content) {
        if hand == fired {
            continue;
        }
        let Some((id, w)) = gun_in(st, hand) else {
            continue;
        };
        if !st.foes.get(foe).is_some_and(|f| f.standing()) {
            break;
        }
        if roll(st) > OFF_HAND_CHANCE {
            continue;
        }
        let Ok(chance) = chance_with(st, foe, &w) else {
            continue;
        };
        let plate = plate_of(st, foe);
        let rounds = volley(st, from, foe, &w, chance, &w.fx.clone(), drawn, true);
        drawn += rounds.len();
        let gun = st
            .content
            .item(&id)
            .map_or_else(String::new, |i| i.name.clone());
        st.say(format!(
            "off hand · {gun} {}{}",
            outcomes(&rounds),
            plate_words(plate, plate_of(st, foe))
        ));
    }
    Ok(())
}

/// What your death reads when a foe called `name` on `place` kills you: on a floor
/// the building it stands in, on any stratum the street.
pub(crate) fn epitaph(st: &AppState, name: &str, place: Place) -> String {
    match place {
        Place::Floor { building, .. } => {
            format!("a {name} in {}", crate::net::building_name(st, building))
        }
        _ => format!("a {name} on the street"),
    }
}

/// A foe fires its weapon at you.
pub fn fire_at_you(st: &mut AppState, foe: usize) {
    fire_at_you_with(st, foe, None);
}

/// A foe fires its weapon at you, its line led by what it did first this action
/// (`turret turns SE › fires · …`). A turret fires what its magazine still holds of the
/// gun's rounds, each round one off it.
pub(crate) fn fire_at_you_with(st: &mut AppState, foe: usize, lead: Option<&str>) {
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
    // Its shot wakes the calm guards in earshot, toward it.
    crate::guards::hear(st, place, from, from);
    let dist = from.chebyshev(me);
    let chance = game::hit_chance(reflex, true, dist, w.range);
    let turn = game::turn_chance(st.character.armor(&st.content));
    let damage = resisted_u8(
        w.damage,
        st.character.resist(&st.content, game::Harm::Kinetic),
    );
    let rounds = (st.foes[foe].turret.as_ref()).map_or(w.rounds, |t| w.rounds.min(t.rounds));
    let mut words = Vec::new();
    let mut landed = false;
    for k in 0..rounds as usize {
        if let Some(t) = st.foes[foe].turret.as_mut() {
            t.rounds = t.rounds.saturating_sub(1);
        }
        let hit = game::round_hits(roll(st), chance);
        // The armour's own roll: a turned round sparks off you as a miss would.
        let turned = hit && turn > 0 && roll(st) as u32 <= turn;
        let wounds = hit && !turned;
        let impact = if hit {
            me
        } else {
            foes::map_of(st, place).map_or(me, |m| miss_impact(&m, from, me, w.range))
        };
        if wounds {
            let cause = epitaph(st, &name, place);
            wound_you(st, &cause, damage);
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
    let fires = match lead {
        Some(lead) => format!("{name} {lead} › fires"),
        None => format!("{name} fires"),
    };
    let line = if landed {
        format!("{fires} · {words} · hp {}", st.character.hp)
    } else {
        format!("{fires} · {words}")
    };
    st.warn(line);
}

/// A foe fires its weapon at another foe: a turret on the guards it fights, or a guard
/// on a turret that's turned on it. The shooter's gun, its reflex, the distance; each
/// round that hits meets a turret target's plate first, else wounds. A turret shooter
/// fires what its magazine holds, a round at a time. The target, if it holds a post and
/// stands, turns to hunt the shooter. The line is led by what the shooter did first.
#[allow(dead_code)] // a turret's turn calls it in the next task
pub(crate) fn fire_on_foe(st: &mut AppState, shooter: usize, target: usize, lead: Option<&str>) {
    let (Some(s), Some(t)) = (st.foes.get(shooter), st.foes.get(target)) else {
        return;
    };
    let (name, from, reflex, place) = (s.name.clone(), s.pos, s.reflex, s.place);
    let (target_name, at) = (t.name.clone(), t.pos);
    let Some(w) = s
        .weapon
        .as_ref()
        .and_then(|id| st.content.item(id))
        .and_then(|i| i.weapon.clone())
    else {
        return;
    };
    // Its shot wakes the calm guards in earshot, toward it.
    crate::guards::hear(st, place, from, from);
    let chance = game::hit_chance(reflex, true, from.chebyshev(at), w.range);
    let rounds = (st.foes[shooter].turret.as_ref()).map_or(w.rounds, |t| w.rounds.min(t.rounds));
    let plate = plate_of(st, target);
    let mut shots = Vec::new();
    for k in 0..rounds as usize {
        if let Some(t) = st.foes[shooter].turret.as_mut() {
            t.rounds = t.rounds.saturating_sub(1);
        }
        let hit = game::round_hits(roll(st), chance);
        let turned = hit && plate_turns(st, target);
        let impact = if hit {
            at
        } else {
            foes::map_of(st, place).map_or(at, |m| miss_impact(&m, from, at, w.range))
        };
        if hit && !turned {
            wound_foe(st, target, w.damage);
        }
        play_round(
            st,
            place,
            &w.fx,
            Round {
                from,
                impact,
                hit: hit && !turned,
                damage: w.damage,
                k,
                tint: None,
            },
        );
        shots.push(if turned {
            Shot::Turned
        } else if hit {
            Shot::Hit(w.damage)
        } else {
            Shot::Miss
        });
    }
    // A guard that stands turns on whoever fired; a turret shot at wakes as it does for you.
    if st.foes[target].turret.is_some() {
        crate::turrets::shot_at(st, target, from);
    } else if let Some(f) = st.foes.get_mut(target)
        && f.post.is_some()
        && f.standing()
    {
        f.mode = foes::FoeMode::Hunting {
            seen: from,
            lost: 0,
        };
    }
    let fires = match lead {
        Some(lead) => format!("{name} {lead} › fires on {target_name}"),
        None => format!("{name} fires on {target_name}"),
    };
    st.say(format!(
        "{fires} · {}{}",
        outcomes(&shots),
        plate_words(plate, plate_of(st, target))
    ));
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
/// before a tile no foe may step onto (`foes::can_step` with no card: on a floor no
/// exit and no shut door) or that holds you or another standing foe. A `Trail` plays
/// on each tile it left, from `at`. Returns the tiles it moved.
fn push_foe(st: &mut AppState, foe: usize, from: Pos, steps: u8, at: u64, trail: bool) -> u8 {
    let Some((place, mut pos)) = st.foes.get(foe).map(|f| (f.place, f.pos)) else {
        return 0;
    };
    // Bolted down: a turret never leaves its tile.
    if st.foes[foe].turret.is_some() {
        return 0;
    }
    let (dx, dy) = ((pos.x - from.x).signum(), (pos.y - from.y).signum());
    let mut moved = 0;
    for _ in 0..steps {
        let next = pos.offset(dx, dy);
        let open = foes::map_of(st, place).is_some_and(|m| foes::can_step(&m, None, pos, next));
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
        let Some(map) = foes::map_of(st, place) else {
            return;
        };
        let candidates = st
            .foes
            .iter()
            .enumerate()
            .filter(|(_, f)| f.standing() && f.place == place && foes::line_clear(&map, at, f.pos))
            .map(|(i, f)| (i, f.pos));
        let next = nearest_other(candidates, at, &hit);
        let Some(next) = next else {
            return;
        };
        let target = st.foes[next].pos;
        wound_foe(st, next, half);
        if let Some(f) = st.foes.get_mut(next) {
            f.stunned = f.stunned.max(arc.stun);
            if f.standing() && !f.inert && f.turret.is_none() {
                f.mode = foes::FoeMode::Hunting {
                    seen: arc.you,
                    lost: 0,
                };
            }
        }
        if st.foes[next].turret.is_some() {
            crate::turrets::shot_at(st, next, arc.you);
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
    // Inside the Net (your rig's room too) the deck fights, not your fists.
    if (st.player.as_ref()).is_some_and(|p| p.place.in_construct() || p.place == Place::Rig) {
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
    let rounds = volley(st, from, foe, &w, chance, &ability.fx, 0, false);
    let hit = rounds.iter().any(|r| matches!(r, Shot::Hit(_)));
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
        let map = foes::map_of(&st, STREET).unwrap();
        assert_eq!(
            miss_impact(&map, Pos::new(6, 8), Pos::new(38, 8), 40),
            Pos::new(40, 8)
        );
        assert_eq!(
            miss_impact(&map, Pos::new(6, 8), Pos::new(6, 2), 10),
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
        let map = foes::map_of(&st, STREET).unwrap();
        // Against the wall the wall takes it, beyond the range too.
        assert_eq!(
            miss_impact(&map, Pos::new(29, 8), Pos::new(39, 8), 10),
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
    fn a_shot_across_a_floor_has_its_line() {
        use crate::guards::testutil::*;
        let mut st = ward();
        let near = goon(&mut st, 0, Pos::new(12, 3));
        let far = goon(&mut st, 0, Pos::new(18, 3));
        assert_eq!(reach(&st, near, 12), Ok(6));
        assert_eq!(reach(&st, far, 12), Err("no line".to_string()));
        let map = foes::map_of(&st, on(0)).unwrap();
        assert!(
            !foes::line_clear(&map, Pos::new(6, 3), Pos::new(18, 3)),
            "the line is what refuses"
        );
    }

    #[test]
    fn a_miss_on_a_floor_stops_at_the_door() {
        use crate::guards::testutil::*;
        let st = ward();
        let map = foes::map_of(&st, on(0)).unwrap();
        assert_eq!(
            miss_impact(&map, Pos::new(6, 3), Pos::new(12, 3), 12),
            Pos::new(15, 3)
        );
    }

    #[test]
    fn a_push_on_a_floor_stops_before_a_shut_door() {
        use crate::guards::testutil::*;
        let mut st = ward();
        let i = goon(&mut st, 0, Pos::new(13, 3));
        assert_eq!(push_foe(&mut st, i, Pos::new(10, 3), 4, 0, false), 1);
        assert_eq!(st.foes[i].pos, Pos::new(14, 3));
    }

    #[test]
    fn a_push_on_a_floor_never_throws_a_body_out_an_exit() {
        use crate::guards::testutil::*;
        let mut st = ward();
        let i = goon(&mut st, 0, Pos::new(1, 3));
        assert_eq!(push_foe(&mut st, i, Pos::new(4, 3), 3, 0, false), 0);
        assert_eq!(st.foes[i].pos, Pos::new(1, 3));
    }

    #[test]
    fn a_guard_that_kills_you_indoors_names_the_building() {
        use crate::guards::testutil::*;
        let mut st = ward();
        let i = guard(&mut st, 0, 1, &[Pos::new(10, 3)]);
        st.character.hp = 1;
        st.force_hit = Some(true);
        fire_at_you(&mut st, i);
        let want = format!("a guard in {}", crate::net::building_name(&st, WARD));
        assert_eq!(st.net.death.as_deref(), Some(want.as_str()));
    }

    #[test]
    fn fire_refuses_out_of_range_and_without_a_gun() {
        let mut st = engaged(Pos::new(20, 8));
        assert_eq!(fire(&mut st, 0).unwrap_err(), "out of range");
        st.character
            .wear(BodyPart::RightHand, ItemId::new("shiv_blade"), None);
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

    #[test]
    fn the_first_shot_from_the_cloak_is_an_ambush() {
        use crate::guards::testutil::*;
        let mut st = ward();
        let i = guard(&mut st, 0, 0, &[Pos::new(10, 3)]);
        st.foes[i].hp = 200;
        st.foes[i].hp_max = 200;
        crate::cloak::testutil::armed(&mut st);
        crate::cloak::testutil::stealthy(&mut st);
        st.character.focus = 10;
        crate::cloak::testutil::fire(&mut st, "cloak").unwrap();
        crate::cloak::engage_by_hand(&mut st, on(0));
        assert!(crate::cloak::hidden(&st), "turns by hand keep it");
        settle(&mut st);
        crate::net::stay(&mut st);
        st.force_hit = Some(false); // every roll would miss
        let (_, _, w) = your_weapon(&st).unwrap();
        fire(&mut st, i).unwrap();
        let dealt = 2 * u32::from(w.damage) * u32::from(w.rounds);
        assert_eq!(
            u32::from(st.foes[i].hp),
            200u32.saturating_sub(dealt),
            "every round lands, ×2"
        );
        assert!(logged(&st, "AMBUSH"));
        assert!(!crate::cloak::hidden(&st));
        assert!(st.foes[i].hunting(), "the fight is on");
        assert_eq!(
            st.veil.fade.unwrap().phase,
            crate::cloak::Phase::SwitchingOff
        );
    }

    /// The ward with a guard of 200 HP at (10,3), you cloaked on FOCUS and on turns by
    /// hand from it, on your act half: the guard's index.
    fn cloaked_on_turns(st: &mut AppState) -> usize {
        use crate::guards::testutil::*;
        let i = guard(st, 0, 0, &[Pos::new(10, 3)]);
        st.foes[i].hp = 200;
        st.foes[i].hp_max = 200;
        crate::cloak::testutil::stealthy(st);
        st.character.focus = 10;
        crate::cloak::testutil::fire(st, "cloak").unwrap();
        crate::cloak::engage_by_hand(st, on(0));
        settle(st);
        crate::net::stay(st);
        i
    }

    #[test]
    fn a_strike_from_the_cloak_is_an_ambush() {
        let mut st = crate::guards::testutil::ward();
        st.character.give(ItemId::new("neural_port"), 1);
        st.character
            .chrome
            .insert(game::Site::Neural, ItemId::new("neural_port"));
        let i = cloaked_on_turns(&mut st);
        st.force_hit = Some(false); // every roll would miss
        strike(&mut st, &AbilityId::new("static"), i).unwrap();
        assert_eq!(st.foes[i].hp, 200 - 2 * 6, "it lands, ×2");
        assert!(logged(&st, "AMBUSH"));
        assert!(logged(&st, "static · guard 12"));
        assert!(!crate::cloak::hidden(&st));
    }

    #[test]
    fn the_second_shot_is_a_plain_one() {
        let mut st = crate::guards::testutil::ward();
        crate::cloak::testutil::armed(&mut st);
        let i = cloaked_on_turns(&mut st);
        let (_, _, w) = your_weapon(&st).unwrap();
        st.force_hit = Some(false);
        fire(&mut st, i).unwrap();
        let after_ambush = st.foes[i].hp;
        assert_eq!(
            u32::from(after_ambush),
            200 - 2 * u32::from(w.damage) * u32::from(w.rounds)
        );
        // The rolls rule again: every one missing, the second shot lands nothing.
        st.force_hit = Some(false);
        fire(&mut st, i).unwrap();
        assert_eq!(
            st.foes[i].hp, after_ambush,
            "a missed volley does no damage"
        );
        // Every one landing, a third (an AP more this turn) does plain damage, not ×2.
        st.net.engaged.as_mut().unwrap().ap = 1;
        st.force_hit = Some(true);
        fire(&mut st, i).unwrap();
        assert_eq!(
            u32::from(after_ambush - st.foes[i].hp),
            u32::from(w.damage) * u32::from(w.rounds)
        );
        let ambushes = (st.log.iter()).filter(|e| e.text == "AMBUSH").count();
        assert_eq!(ambushes, 1);
    }

    /// Sets the tick so that `ok` holds of the rolls the next volleys take: `ok(n)`
    /// reads the nth roll from now (1-based), in percent.
    fn seed(st: &mut AppState, ok: impl Fn(&dyn Fn(u64) -> u32) -> bool) {
        let shots = st.shots;
        st.tick = (500..)
            .find(|t| ok(&|n| (hash64(*t, shots + n) % 100 + 1) as u32))
            .unwrap();
    }

    /// Puts an item from the content in a hand.
    fn put_in(st: &mut AppState, hand: BodyPart, item: &str) {
        let id = ItemId::new(item);
        let copy = st.character.new_copy(&st.content, &id);
        st.character.wear(hand, id, Some(copy));
    }

    fn off_hand_logged(st: &AppState) -> bool {
        logged(st, "off hand")
    }

    #[test]
    fn with_no_gun_in_the_main_hand_the_other_hands_gun_fires() {
        let mut st = engaged(Pos::new(14, 8));
        st.character.take_off(BodyPart::RightHand);
        put_in(&mut st, BodyPart::LeftHand, "scrap_nine");
        let (hand, id, _) = your_weapon(&st).unwrap();
        assert_eq!((hand, id), (BodyPart::LeftHand, ItemId::new("scrap_nine")));
        // Every roll would call the off hand in, yet nothing else is held.
        st.force_hit = Some(true);
        fire(&mut st, 0).unwrap();
        assert!(logged(&st, "you fire · goon 5"), "{:?}", st.log.latest());
        assert!(!off_hand_logged(&st), "the gun that fired never follows up");
        assert_eq!(st.foes[0].hp, foes::GOON_HP - 5);
    }

    #[test]
    fn the_off_hand_joins_in_about_one_shot_in_four() {
        let mut st = engaged(Pos::new(14, 8));
        put_in(&mut st, BodyPart::LeftHand, "scrap_nine");
        // Three rounds of the ratchet, then the follow-up's roll under 25.
        seed(&mut st, |r| r(4) == u32::from(OFF_HAND_CHANCE));
        fire(&mut st, 0).unwrap();
        assert!(
            logged(&st, "off hand · Scrap nine"),
            "{:?}",
            st.log.latest()
        );
        assert_eq!(st.net.engaged.as_ref().unwrap().ap, 1, "free: one AP spent");

        let mut st = engaged(Pos::new(14, 8));
        put_in(&mut st, BodyPart::LeftHand, "scrap_nine");
        seed(&mut st, |r| r(4) == u32::from(OFF_HAND_CHANCE) + 1);
        fire(&mut st, 0).unwrap();
        assert!(!off_hand_logged(&st));
        assert_eq!(st.net.engaged.as_ref().unwrap().ap, 1);
    }

    #[test]
    fn the_follow_up_is_drawn_after_the_main_burst() {
        let mut st = engaged(Pos::new(14, 8));
        put_in(&mut st, BodyPart::LeftHand, "scrap_nine");
        let (_, _, main) = your_weapon(&st).unwrap();
        st.force_hit = Some(true); // every roll wins, the follow-up's too
        fire(&mut st, 0).unwrap();
        assert!(off_hand_logged(&st));
        // One flash a round, in the order fired.
        let flashes: Vec<u64> = (st.fx.iter())
            .filter(|i| matches!(i.fx, Fx::Flash { .. }))
            .map(|i| i.started)
            .collect();
        let main = usize::from(main.rounds);
        assert_eq!(flashes.len(), main + 1, "{flashes:?}");
        assert!(
            flashes[main] > flashes[main - 1],
            "the follow-up starts after the main burst's last round: {flashes:?}"
        );
        // Its number stands past the main burst's numbers, not on the first.
        let numbers: Vec<Pos> = (st.fx.iter())
            .filter(|i| matches!(i.fx, Fx::Number { .. }))
            .map(|i| i.targets[0])
            .collect();
        let last = numbers.last().unwrap();
        assert!(!numbers[..main].contains(last), "{numbers:?}");
    }

    #[test]
    fn the_off_hand_stays_quiet_out_of_its_range_or_when_the_target_is_down() {
        // The scrap nine reaches 8, the foe stands 10 off, within the ratchet's 12.
        let mut st = engaged(Pos::new(16, 8));
        st.player
            .as_mut()
            .unwrap()
            .set_vis(Pos::new(16, 8), crate::render::Visibility::Visible);
        put_in(&mut st, BodyPart::LeftHand, "scrap_nine");
        st.force_hit = Some(true); // every roll wins, the follow-up's too
        fire(&mut st, 0).unwrap();
        assert!(logged(&st, "you fire"));
        assert!(!off_hand_logged(&st), "out of its range");

        // The main volley downs the foe: nothing left to shoot at.
        let mut st = engaged(Pos::new(14, 8));
        put_in(&mut st, BodyPart::LeftHand, "scrap_nine");
        st.foes[0].hp = 1;
        st.force_hit = Some(true);
        fire(&mut st, 0).unwrap();
        assert!(!st.foes[0].standing());
        assert!(!off_hand_logged(&st), "the target is down");
    }

    #[test]
    fn the_ambush_is_the_main_hands_alone() {
        let mut st = crate::guards::testutil::ward();
        crate::cloak::testutil::armed(&mut st);
        put_in(&mut st, BodyPart::LeftHand, "scrap_nine");
        let i = cloaked_on_turns(&mut st);
        let (_, _, main) = your_weapon(&st).unwrap();
        // The ambush rolls nothing: the follow-up's roll is the first, its one round
        // the second, at the scrap nine's own chance.
        let chance = chance_with(&st, i, &gun_in(&st, BodyPart::LeftHand).unwrap().1).unwrap();
        seed(&mut st, |r| {
            r(1) == u32::from(OFF_HAND_CHANCE) && r(2) <= u32::from(chance)
        });
        fire(&mut st, i).unwrap();
        assert!(logged(&st, "AMBUSH"));
        assert!(off_hand_logged(&st));
        let main = 2 * u32::from(main.damage) * u32::from(main.rounds);
        assert_eq!(
            u32::from(st.foes[i].hp),
            200 - main - 5,
            "the main rounds land x2, the follow-up's round plain"
        );
    }

    #[test]
    fn the_shiv_in_the_off_hand_never_fires() {
        let mut st = engaged(Pos::new(14, 8));
        let bare = st.character.attribute(&st.content, Attribute::Reflex);
        put_in(&mut st, BodyPart::LeftHand, "shiv_blade");
        st.force_hit = Some(true);
        fire(&mut st, 0).unwrap();
        assert!(!off_hand_logged(&st), "no weapon spec, no follow-up");
        assert_eq!(
            st.character.attribute(&st.content, Attribute::Reflex),
            bare + 1,
            "its perk still counts"
        );
    }

    #[test]
    fn a_turret_fires_on_a_guard_and_the_guard_turns_on_it() {
        use crate::guards::testutil::*;
        let mut st = ward();
        stand(&mut st, 0, Pos::new(2, 5));
        let t =
            crate::turrets::testutil::turret_in_ward(&mut st, 0, Pos::new(10, 3), citygen::Dir::W);
        let g = guard(&mut st, 0, 0, &[Pos::new(6, 3)]);
        st.force_hit = Some(true);
        fire_on_foe(&mut st, t, g, None);
        assert!(logged(&st, "turret fires on guard · 5 · 5 · 5"));
        assert_eq!(st.foes[g].hp, 15);
        assert_eq!(st.foes[t].turret.as_ref().unwrap().rounds, 27);
        assert_eq!(
            st.foes[g].mode,
            crate::foes::FoeMode::Hunting {
                seen: Pos::new(10, 3),
                lost: 0
            }
        );
    }

    #[test]
    fn a_guard_fires_on_a_turret_and_its_plate_turns_rounds() {
        use crate::guards::testutil::*;
        let mut st = ward();
        let t =
            crate::turrets::testutil::turret_in_ward(&mut st, 0, Pos::new(10, 3), citygen::Dir::W);
        let g = guard(&mut st, 0, 0, &[Pos::new(6, 3)]);
        st.force_hit = Some(true);
        fire_on_foe(&mut st, g, t, None);
        // The guard's scrap nine fires one round: turned off ARM 4.
        assert!(logged(&st, "guard fires on turret · turned · plate 4 → 3"));
        assert_eq!(st.foes[t].hp, 40);
    }

    fn at_a_turret(arm: u8) -> AppState {
        let mut st = crate::turrets::testutil::lot_turret(Pos::new(14, 8), citygen::Dir::W);
        st.foes[0].turret.as_mut().unwrap().arm = arm;
        crate::net::engage(&mut st, STREET, true);
        assert!(crate::net::stay(&mut st));
        st
    }

    #[test]
    fn rounds_that_would_hit_a_turret_spark_off_its_plate_first() {
        let mut st = at_a_turret(4);
        st.force_hit = Some(true); // forced rolls also turn while ARM > 0
        fire(&mut st, 0).unwrap();
        assert_eq!(st.foes[0].hp, 40);
        assert_eq!(st.foes[0].turret.as_ref().unwrap().arm, 1);
        assert!(logged(
            &st,
            "you fire · turret turned · turned · turned · plate 4 → 1"
        ));
    }

    #[test]
    fn with_its_plate_gone_rounds_take_its_hp() {
        let mut st = at_a_turret(0);
        st.force_hit = Some(true);
        fire(&mut st, 0).unwrap();
        assert_eq!(st.foes[0].hp, 28);
        assert!(logged(&st, "you fire · turret 4 · 4 · 4") && !logged(&st, "plate"));
    }

    #[test]
    fn at_zero_it_is_wrecked_and_never_acts_again() {
        let mut st = at_a_turret(0);
        st.foes[0].hp = 4;
        st.force_hit = Some(true);
        fire(&mut st, 0).unwrap();
        assert!(st.foes[0].down && logged(&st, "turret wrecked") && !logged(&st, "turret down"));
        assert_eq!(
            crate::turrets::state(&st.foes[0]),
            Some(crate::turrets::TurretState::Wrecked)
        );
        assert!(!crate::foes::turn_start(&mut st, 0));
    }

    #[test]
    fn a_strike_and_a_blast_skip_the_plate() {
        let mut st = at_a_turret(6);
        st.character.give(ItemId::new("neural_port"), 1);
        st.character
            .chrome
            .insert(game::Site::Neural, ItemId::new("neural_port"));
        st.force_hit = Some(true);
        strike(&mut st, &AbilityId::new("static"), 0).unwrap();
        assert!(st.foes[0].hp < 40, "the strike takes HP");
        assert_eq!(st.foes[0].turret.as_ref().unwrap().arm, 6);
        assert!(st.foes[0].stunned > 0);
        let hp = st.foes[0].hp;
        wound_foe(&mut st, 0, 5);
        assert_eq!(st.foes[0].hp, hp - 5);
        assert_eq!(st.foes[0].turret.as_ref().unwrap().arm, 6);
        // A stun passes its next turn.
        assert!(!crate::foes::turn_start(&mut st, 0));
        assert!(logged(&st, "turret · stunned"));
    }

    #[test]
    fn shot_at_asleep_it_wakes_hunting_you() {
        let mut st = at_a_turret(6);
        st.force_hit = Some(false);
        fire(&mut st, 0).unwrap();
        assert_eq!(
            crate::turrets::state(&st.foes[0]),
            Some(crate::turrets::TurretState::Hunting)
        );
        assert!(st.foes[0].turret.as_ref().unwrap().awake);
        assert!(logged(&st, "turret wakes"));
    }

    #[test]
    fn a_blind_turret_shot_at_stays_blind_and_idle() {
        let mut st = at_a_turret(6);
        st.foes[0].turret.as_mut().unwrap().hacked = crate::turrets::Hacked::Blind;
        st.force_hit = Some(false);
        fire(&mut st, 0).unwrap();
        assert_eq!(st.foes[0].mode, foes::FoeMode::Idle);
        assert!(!st.foes[0].turret.as_ref().unwrap().awake);
    }

    #[test]
    fn jolts_arc_leaves_a_blind_turret_blind_and_idle() {
        let mut st = at_a_turret(6);
        st.foes[0].turret.as_mut().unwrap().hacked = crate::turrets::Hacked::Blind;
        st.foes.push(foes::Foe::goon(STREET, Pos::new(15, 8)));
        let arc = Chain {
            you: Pos::new(6, 8),
            impact: 500,
            hops: 1,
            damage: 8,
            stun: 1,
            fx: &[],
            source: "jolt",
        };
        chain_foes(&mut st, 1, arc);
        assert_eq!(st.foes[0].hp, 40 - hop_damage(8), "it still takes the arc");
        assert_eq!(st.foes[0].mode, foes::FoeMode::Idle);
        assert!(!st.foes[0].turret.as_ref().unwrap().awake);
    }

    #[test]
    fn knockback_leaves_a_turret_on_its_tile() {
        let mut st = at_a_turret(6);
        assert_eq!(push_foe(&mut st, 0, Pos::new(10, 8), 3, 0, false), 0);
        assert_eq!(st.foes[0].pos, Pos::new(14, 8));
    }
}
