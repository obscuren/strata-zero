//! Dev console commands: spawn and drop items, give credits, heal, xp, focus, count the
//! people, jump the clock, clear a pile, show who sees you and how lit the map is, and
//! free chrome from the ripperdoc's chair.

use crate::anim::effects::pulse;
use crate::app::AppState;
use crate::messages::{Entry, Kind};
use crate::ui::chrome::{CYAN, DIM, MAGENTA, MAGENTA_RGB, TEXT};
use crate::ui::console::Console;
use citygen::Pos;
use game::{At, ItemId};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};

const COMMANDS: [&str; 43] = [
    "spawn", "drop", "give", "heal", "xp", "focus", "sp", "pts", "talent", "tpts", "program",
    "heat", "alarm", "wake", "engage", "calm", "ice", "failsafe", "hp", "who", "clock", "tp",
    "clear", "help", "trace", "seize", "backdoor", "eyes", "light", "time", "job", "standing",
    "jobs", "target", "foe", "foes", "dummy", "throw", "live", "ripper", "story", "track", "wait",
];

/// The screen a line that ran well opens over the map: the Wait window for `wait`.
pub fn opens(st: &AppState, line: &str) -> Option<Box<dyn crate::app::Screen>> {
    (line.split_whitespace().next() == Some("wait")).then(|| {
        Box::new(crate::screens::rest::RestScreen::open(
            st,
            crate::rest::Kind::Wait,
        )) as Box<dyn crate::app::Screen>
    })
}

/// Runs one line against the app. `focus` is the map's focus tile on its stratum.
pub fn run(st: &mut AppState, focus: At, line: &str) -> Result<String, String> {
    let mut words = line.split_whitespace();
    let cmd = words.next().unwrap_or("");
    let args: Vec<&str> = words.collect();
    match cmd {
        "spawn" | "drop" => {
            let usage = format!("{cmd} <item> [qty]");
            let id = args.first().ok_or_else(|| usage.clone())?;
            let item = st
                .content
                .item(&ItemId::new(id))
                .ok_or_else(|| format!("unknown item {id}"))?
                .clone();
            let qty: u32 = match args.get(1) {
                Some(q) => q.parse().map_err(|_| usage.clone())?,
                None => 1,
            };
            if qty == 0 {
                return Err(usage);
            }
            if cmd == "spawn" {
                st.character.give_item(&st.content, item.id.clone(), qty);
                Ok(format!("spawned {qty}× `{}`", item.name))
            } else {
                let lies = st.city.as_ref().is_some_and(|c| {
                    crate::render::map::MapRef::for_place(c, &st.interiors, &st.constructs, focus.place, st.minute_of_day())
                        .is_some_and(|m| crate::render::map::TileMap::walkable(&m, focus.pos))
                });
                if !lies {
                    return Err("nothing can lie there".to_string());
                }
                if item.per_copy() {
                    // Hardware lies as copies, so picking it up never makes it up.
                    for _ in 0..qty {
                        let copy = st.character.new_copy(&st.content, &item.id);
                        st.ground.put(
                            focus.place,
                            focus.pos,
                            game::Stack {
                                item: item.id.clone(),
                                qty: 1,
                                copy: Some(copy),
                            },
                        );
                    }
                } else {
                    st.ground
                        .add(focus.place, focus.pos, item.id.clone(), qty);
                }
                Ok(format!("dropped {qty}× `{}`", item.name))
            }
        }
        "give" => {
            let n: u32 = args
                .first()
                .and_then(|a| a.parse().ok())
                .ok_or_else(|| "give <credits>".to_string())?;
            st.character.credits = st.character.credits.saturating_add(n);
            Ok(format!("credits {}", st.character.credits))
        }
        "heal" => {
            let max = st.character.hp_max(&st.content);
            st.character.hp = max;
            Ok(format!("hp {max}/{max}"))
        }
        "hp" => {
            let n: u32 = args
                .first()
                .and_then(|a| a.parse().ok())
                .ok_or_else(|| "hp <points>".to_string())?;
            let max = st.character.hp_max(&st.content);
            st.character.hp = n.min(max);
            Ok(format!("hp {}/{max}", st.character.hp))
        }
        "xp" => {
            let n: u32 = args
                .first()
                .and_then(|a| a.parse().ok())
                .ok_or_else(|| "xp <points>".to_string())?;
            let gained = st.character.gain_xp(n);
            let level = st.character.level;
            for i in (0..gained).rev() {
                let at = level - i;
                // A line a level, so at most one point a line.
                let attribute = if game::Character::attribute_points_for(at - 1, at) > 0 {
                    " · an attribute point"
                } else {
                    ""
                };
                st.story(format!(
                    "level {at} · a talent point · {} SP{attribute}",
                    game::SP_PER_LEVEL
                ));
            }
            Ok(format!("+{n} xp"))
        }
        "focus" => {
            let n: u32 = args
                .first()
                .and_then(|a| a.parse().ok())
                .ok_or_else(|| "focus <points>".to_string())?;
            st.character.focus = n.min(st.character.focus_max(&st.content));
            Ok(format!(
                "focus {}/{}",
                st.character.focus,
                st.character.focus_max(&st.content)
            ))
        }
        "sp" => {
            let n: u32 = args
                .first()
                .and_then(|a| a.parse().ok())
                .ok_or_else(|| "sp <points>".to_string())?;
            st.character.sp = st.character.sp.saturating_add(n);
            Ok(format!("sp {}", st.character.sp))
        }
        "pts" => {
            let n: u32 = args
                .first()
                .and_then(|a| a.parse().ok())
                .ok_or_else(|| "pts <points>".to_string())?;
            st.character.attribute_points = st.character.attribute_points.saturating_add(n);
            Ok(format!("points {}", st.character.attribute_points))
        }
        "talent" => {
            let id = args
                .first()
                .map(|a| game::TalentId::new(a))
                .ok_or_else(|| "talent <id>".to_string())?;
            let t = st
                .content
                .talent(&id)
                .ok_or_else(|| format!("unknown talent {id}"))?;
            let rank = st.character.learn(&st.content, &id)?;
            Ok(format!("`{}` {}/{}", t.name, rank, t.ranks))
        }
        "tpts" => {
            let n: u32 = args
                .first()
                .and_then(|a| a.parse().ok())
                .ok_or_else(|| "tpts <points>".to_string())?;
            st.character.talent_points = st.character.talent_points.saturating_add(n);
            Ok(format!("{} talent points", st.character.talent_points))
        }
        "program" => {
            let id = args
                .first()
                .map(|a| ItemId::new(a))
                .ok_or_else(|| "program <id>".to_string())?;
            let item = st
                .content
                .item(&id)
                .ok_or_else(|| format!("unknown item {id}"))?;
            if item.kind != game::ItemKind::Program {
                return Err(format!("{} is not a program", item.name));
            }
            st.character.give(id.clone(), 1);
            st.character.load(&st.content, &id)
        }
        "heat" => {
            let n: u8 = args
                .first()
                .and_then(|a| a.parse().ok())
                .ok_or_else(|| "heat <0-100>".to_string())?;
            let b = crate::net::building_here(st).ok_or_else(|| "not in a construct".to_string())?;
            let now = st.net.now(st);
            let was_alert = st.net.alert(b, now);
            let t = st.net.trace(b);
            t.heat = 0;
            match t.add(n, now) {
                game::TraceEvent::Dump => {
                    if !was_alert {
                        let line = crate::net::raise_alarm(st, b);
                        st.warn(line);
                    }
                    crate::net::dump(st, "the trace found you");
                }
                game::TraceEvent::Alarm => {
                    let line = crate::net::raise_alarm(st, b);
                    st.warn(line);
                }
                game::TraceEvent::None => {}
            }
            Ok(format!("heat {n}"))
        }
        "alarm" => {
            let b = crate::net::building_here(st).ok_or_else(|| "not in a building".to_string())?;
            let now = st.net.now(st);
            let t = st.net.trace(b);
            if t.heat < game::ALARM_HEAT {
                t.heat = game::ALARM_HEAT;
            }
            t.alert_until = Some(now + game::ALERT_MINUTES);
            let line = crate::net::raise_alarm(st, b);
            st.warn(line.clone());
            Ok(line)
        }
        "wake" => {
            let b = crate::net::building_here(st).ok_or_else(|| "not in a construct".to_string())?;
            let pos = st.player.as_ref().map(|p| p.pos).ok_or("no body")?;
            let k = st.constructs.get(b).ok_or("not in a construct")?;
            st.net.ice_of(k);
            st.net.wake_all(b, pos);
            let n = st.net.ice.get(&b).map_or(0, |v| v.len());
            Ok(format!("{n} piece{} awake", if n == 1 { "" } else { "s" }))
        }
        "engage" => {
            let (place, jacked) = st
                .player
                .as_ref()
                .map(|p| (p.place, p.jacked_in()))
                .ok_or("no body")?;
            if st.net.engaged.is_some() {
                return Ok("already engaged".to_string());
            }
            match place {
                citygen::Place::Construct(_) => {}
                _ if jacked => return Err("not in a construct".to_string()),
                _ if crate::foes::in_memory(st).is_empty() => {
                    return Err("nothing to engage".to_string());
                }
                _ => {}
            }
            crate::net::engage(st, place, true);
            Ok("engaged by hand".to_string())
        }
        "calm" => {
            if st.net.engaged.is_none() {
                return Ok("already calm".to_string());
            }
            if crate::net::hunting(st) {
                return Err("still hunted".to_string());
            }
            crate::net::disengage(st);
            Ok("calm".to_string())
        }
        "ice" => {
            let b = crate::net::building_here(st).ok_or_else(|| "not in a construct".to_string())?;
            let k = st.constructs.get(b).ok_or("not in a construct")?;
            st.net.ice_of(k);
            let k = st.constructs.get(b).ok_or("not in a construct")?;
            let lines: Vec<String> = k
                .guards
                .iter()
                .zip(st.net.ice.get(&b).map(|v| v.as_slice()).unwrap_or(&[]))
                .enumerate()
                .map(|(i, (g, ice))| {
                    let mode = match ice.mode {
                        crate::net::IceMode::Asleep => "asleep".to_string(),
                        crate::net::IceMode::Awake { seen, lost } => {
                            format!("awake→{},{} lost {lost}", seen.x, seen.y)
                        }
                    };
                    let down = if k.guard_at(g.pos).is_none() { " down" } else { "" };
                    format!(
                        "{i} {} {mode} {}/{} at {},{}{down}",
                        g.label(),
                        ice.integrity,
                        g.grade.integrity(),
                        g.pos.x,
                        g.pos.y
                    )
                })
                .collect();
            if lines.is_empty() {
                return Ok("no ICE here".to_string());
            }
            Ok(lines.join(" · "))
        }
        "foe" | "dummy" => {
            let n: i32 = args.first().and_then(|a| a.parse().ok()).unwrap_or(6);
            let (place, pos) = st
                .player
                .as_ref()
                .map(|p| (p.place, p.pos))
                .ok_or("no body")?;
            let at = pos.offset(n, 0);
            let open = place.stratum().is_some_and(|k| {
                st.city.as_ref().is_some_and(|c| c.walkable(k, at))
            }) && crate::foes::foe_at(st, place, at).is_none();
            if !open {
                return Err("no room there".to_string());
            }
            let foe = if cmd == "dummy" {
                crate::foes::Foe::dummy(place, at)
            } else {
                crate::foes::Foe::goon(place, at)
            };
            let line = format!("a {} at {},{}", foe.name, at.x, at.y);
            st.foes.push(foe);
            Ok(line)
        }
        "throw" => {
            let id = game::ItemId::new(args.first().copied().ok_or("throw what")?);
            let n: i32 = args.get(1).and_then(|a| a.parse().ok()).unwrap_or(4);
            let at = st.player.as_ref().ok_or("no body")?.pos.offset(n, 0);
            crate::blast::throw_free(st, &id, at)?;
            Ok(format!("thrown at {},{}", at.x, at.y))
        }
        "live" => {
            let lines: Vec<String> = st
                .live
                .iter()
                .map(|l| {
                    let s = if l.turns == 1 { "" } else { "s" };
                    format!(
                        "{} at {},{} · {} turn{s}",
                        l.name.to_lowercase(),
                        l.pos.x,
                        l.pos.y,
                        l.turns
                    )
                })
                .collect();
            if lines.is_empty() {
                return Ok("nothing live".to_string());
            }
            Ok(lines.join(" · "))
        }
        "foes" => {
            let lines: Vec<String> = st
                .foes
                .iter()
                .enumerate()
                .map(|(i, f)| {
                    let mode = match f.mode {
                        crate::foes::FoeMode::Idle => "idle".to_string(),
                        crate::foes::FoeMode::Hunting { seen, lost } => {
                            format!("hunting→{},{} lost {lost}", seen.x, seen.y)
                        }
                    };
                    let down = if f.down { " down" } else { "" };
                    format!(
                        "{i} {} {mode} {}/{} at {},{}{down}",
                        f.name, f.hp, f.hp_max, f.pos.x, f.pos.y
                    )
                })
                .collect();
            if lines.is_empty() {
                return Ok("no foes".to_string());
            }
            Ok(lines.join(" · "))
        }
        "failsafe" => {
            let on = match args.first().copied() {
                Some("on") => true,
                Some("off") => false,
                _ => return Err("failsafe on|off".to_string()),
            };
            st.net.failsafe = on;
            Ok(format!("failsafe {}", if on { "on" } else { "off" }))
        }
        "who" => {
            let w = st
                .world
                .as_ref()
                .ok_or_else(|| "no city, no people".to_string())?;
            let (mut out, mut inside, mut driving) = (0, 0, 0);
            for p in w.people() {
                match &p.whereabouts {
                    sim::Whereabouts::Inside(_) => inside += 1,
                    sim::Whereabouts::At { .. } => out += 1,
                    sim::Whereabouts::Travelling(t) => {
                        if t.legs.iter().any(|l| l.car.is_some()) {
                            driving += 1;
                        } else {
                            out += 1;
                        }
                    }
                }
            }
            Ok(format!("{out} out · {inside} inside · {driving} driving"))
        }
        "clock" | "time" => {
            // Jumps the clock forward to the next hh:mm and reseeds the city's day there.
            let usage = format!("{cmd} <hh:mm>");
            let (h, m) = args
                .first()
                .and_then(|a| a.split_once(':'))
                .and_then(|(h, m)| Some((h.parse::<u64>().ok()?, m.parse::<u64>().ok()?)))
                .filter(|(h, m)| *h < 24 && *m < 60)
                .ok_or(usage)?;
            let world_tick = st
                .world
                .as_ref()
                .map(|w| w.tick())
                .ok_or_else(|| "no city, no clock".to_string())?;
            let start = crate::ui::hud::START_MINUTE_OF_DAY as u64;
            let per_minute = sim::TICKS_PER_MINUTE;
            let from_start = (h * 60 + m + 1440 - start) % 1440;
            let now = world_tick / per_minute;
            let mut minute = now - now % 1440 + from_start;
            if minute * per_minute < world_tick {
                minute += 1440;
            }
            let city = st
                .city
                .as_ref()
                .ok_or_else(|| "no city, no clock".to_string())?;
            st.world = Some(sim::World::seed(
                city,
                &st.people,
                &sim::SimParams::default(),
                crate::ui::hud::START_MINUTE_OF_DAY,
                minute * per_minute,
            ));
            // Sight follows the sun: see the new hour at once.
            let bonus = st.sight_bonus();
            let now = st.minute_of_day();
            if let (Some(city), Some(player)) = (st.city.as_ref(), st.player.as_mut())
                && let Some(map) = crate::render::map::MapRef::for_place(
                    city,
                    &st.interiors,
                    &st.constructs,
                    player.place,
                    now,
                )
            {
                player.update_fov(&map, bonus);
            }
            st.repaint = true;
            Ok(format!("{h:02}:{m:02}"))
        }
        "trace" => {
            let now = st.net.now(st);
            let made = st.backdoors.route.is_none();
            if made {
                st.backdoors.start_route(None);
            }
            st.backdoors.start_trace(now);
            if made {
                st.backdoors.end_route();
            }
            Ok("a trace runs home".to_string())
        }
        "seize" => {
            let now = st.net.now(st);
            st.backdoors.rig_seized_until = Some(now + crate::backdoors::SEIZED_MINUTES);
            Ok("the rig is seized for a day".to_string())
        }
        "backdoor" => {
            let what = args.first().ok_or("backdoor <host>")?;
            let hosts: Vec<(citygen::BuildingId, String)> = st
                .city
                .as_ref()
                .map(|c| c.buildings.iter().map(|x| x.id).collect::<Vec<_>>())
                .unwrap_or_default()
                .into_iter()
                .map(|id| {
                    (
                        id,
                        crate::helix::hosts::hostname(st, crate::helix::shell::HostId::Building(id)),
                    )
                })
                .collect();
            let b = hosts
                .iter()
                .find(|(_, n)| n.starts_with(what))
                .map(|(id, _)| *id)
                .ok_or_else(|| format!("no host {what}"))?;
            let now = st.net.now(st);
            let seed = st.city.as_ref().map_or(0, |c| c.seed);
            st.backdoors.can_plant(b, now).map_err(|r| r.words())?;
            st.backdoors.plant_net(b, 1, seed, now);
            Ok(format!("backdoor on `{}`", crate::net::building_name(st, b)))
        }
        "tp" => {
            // Teleports the player on the map they are on.
            let usage = "tp <x> <y>".to_string();
            let (x, y) = match (args.first(), args.get(1)) {
                (Some(x), Some(y)) => (
                    x.parse::<i32>().map_err(|_| usage.clone())?,
                    y.parse::<i32>().map_err(|_| usage.clone())?,
                ),
                _ => return Err(usage),
            };
            let place = st
                .player
                .as_ref()
                .map(|p| p.place)
                .ok_or_else(|| "no body in the city".to_string())?;
            let city = st.city.as_ref().ok_or_else(|| "no city".to_string())?;
            let target = Pos::new(x, y);
            let map = crate::render::map::MapRef::for_place(city, &st.interiors, &st.constructs, place, st.minute_of_day())
                .ok_or_else(|| "no map here".to_string())?;
            if !crate::render::map::TileMap::walkable(&map, target) {
                return Err("nothing to stand on there".to_string());
            }
            let label = map.label();
            let bonus = st.sight_bonus();
            let player = st.player.as_mut().expect("checked above");
            player.pos = target;
            player.update_fov(&map, bonus);
            st.repaint = true;
            Ok(format!("{x},{y} · {label}"))
        }
        "eyes" => {
            st.show_eyes = !st.show_eyes;
            Ok(format!("eyes {}", if st.show_eyes { "on" } else { "off" }))
        }
        "light" => {
            st.show_light = !st.show_light;
            Ok(format!("light {}", if st.show_light { "on" } else { "off" }))
        }
        "ripper" => {
            st.free_chrome = !st.free_chrome;
            Ok(if st.free_chrome {
                "ripper on · chrome swaps anywhere, free".to_string()
            } else {
                "ripper off".to_string()
            })
        }
        "wait" => {
            if st.city.is_none() {
                return Err("no city, no clock".to_string());
            }
            if st.rest.is_some() {
                return Err("already resting".to_string());
            }
            Ok("wait · pick the time".to_string())
        }
        "clear" => {
            let n = st.ground.at(focus.place, focus.pos).len();
            st.ground.clear(focus.place, focus.pos);
            Ok(format!(
                "cleared {n} stack{}",
                if n == 1 { "" } else { "s" }
            ))
        }
        "job" => {
            let kind = match args.first().copied() {
                Some("fetch") => crate::jobs::KindTag::Fetch,
                Some("copy") => crate::jobs::KindTag::Copy,
                Some("plant") => crate::jobs::KindTag::Plant,
                Some("deliver") => crate::jobs::KindTag::Deliver,
                _ => return Err("job fetch|copy|plant|deliver".to_string()),
            };
            let from = st.player.as_ref().map(|p| p.pos).unwrap_or(Pos::new(0, 0));
            let fixer = crate::jobs::nearest_fixer(st, from).ok_or("no fixer in this city")?;
            let counter = st.jobs.fixer_mut(fixer).rolled;
            st.jobs.fixer_mut(fixer).rolled += 1;
            let job = crate::jobs::generate(st, fixer, kind, counter)
                .ok_or("nothing to roll for that kind")?;
            let title = job.title.clone();
            let id = st.jobs.push(job);
            crate::jobs::take_job(st, id)?;
            Ok(format!("{} · `{title}` taken", kind.label()))
        }
        "standing" => {
            let n: i32 = args
                .first()
                .and_then(|a| a.parse().ok())
                .ok_or_else(|| "standing <n>".to_string())?;
            let fixers: Vec<sim::PersonId> = st
                .world
                .as_ref()
                .map(|w| {
                    w.people()
                        .iter()
                        .filter(|p| p.profession == "fixer")
                        .map(|p| p.id)
                        .collect()
                })
                .unwrap_or_default();
            for f in &fixers {
                st.jobs.fixer_mut(*f).standing = n;
            }
            Ok(format!(
                "standing {n} with {} fixer{}",
                fixers.len(),
                if fixers.len() == 1 { "" } else { "s" }
            ))
        }
        "story" => story(st, &args),
        "track" => track(st, &args),
        "jobs" => {
            if st.jobs.list.is_empty() {
                return Ok("no jobs".to_string());
            }
            Ok(st
                .jobs
                .list
                .iter()
                .map(|j| format!("{} `{}` {:?} ¤{}", j.id.0, j.title, j.state, j.pay))
                .collect::<Vec<_>>()
                .join(" · "))
        }
        "target" => {
            let want = args.join(" ").to_lowercase();
            if want.is_empty() {
                return Err("target <name>".to_string());
            }
            let (pos, line) = {
                let (Some(city), Some(world)) = (st.city.as_ref(), st.world.as_ref()) else {
                    return Err("no city".to_string());
                };
                let people = world.people();
                let exact: Vec<&sim::Person> =
                    people.iter().filter(|p| p.name.to_lowercase() == want).collect();
                let found: Vec<&sim::Person> = if exact.is_empty() {
                    people
                        .iter()
                        .filter(|p| p.name.to_lowercase().starts_with(&want))
                        .collect()
                } else {
                    exact
                };
                let p = match found.as_slice() {
                    [] => return Err(format!("no one called {want}")),
                    [p] => *p,
                    many => {
                        return Err(format!(
                            "several: {}",
                            many.iter()
                                .take(3)
                                .map(|p| p.name.as_str())
                                .collect::<Vec<_>>()
                                .join(", ")
                        ));
                    }
                };
                let label = world.card(p.id, city).profession;
                match world.pos_of(p.id) {
                    Some(pos) => (pos, format!("`{}` · {label} · {},{}", p.name, pos.x, pos.y)),
                    None => {
                        let sim::Whereabouts::Inside(b) = p.whereabouts else {
                            return Err(format!("`{}` is nowhere to be seen", p.name));
                        };
                        let door = city
                            .portals(b)
                            .first()
                            .map(|(_, d)| *d)
                            .ok_or_else(|| "no door to stand at".to_string())?;
                        (
                            door,
                            format!(
                                "`{}` · {label} · inside {} · door {},{}",
                                p.name,
                                crate::net::building_name(st, b),
                                door.x,
                                door.y
                            ),
                        )
                    }
                }
            };
            st.jump_to = Some(pos);
            Ok(line)
        }
        "help" => Ok(
            "spawn <item> [qty] · drop <item> [qty] · give <credits> · heal · xp <points> · focus <points> · sp <points> · pts <points> · talent <id> · tpts <points> · program <id> · heat <0-100> · alarm · wake · engage · calm · ice · failsafe on|off · hp <points> · who · clock <hh:mm> (or time) · tp <x> <y> · trace · seize · backdoor <host> · job fetch|copy|plant|deliver · standing <n> · jobs · target <name> · eyes · light · ripper · foe [n] · dummy [n] · foes · throw <item> [n] · live · story [fire <mission> | scene <id> | skip | feed | call] · track [off | <x> <y> | <name>] · wait · clear · help"
                .to_string(),
        ),
        "" => Err("type a command; help lists them".to_string()),
        other => Err(format!("unknown command {other}")),
    }
}

/// `track`: shows the track; `off` stops it; `<x> <y>` tracks a spot in your place;
/// `<name>` tracks the listing on the map's stratum whose name starts with it.
fn track(st: &mut AppState, args: &[&str]) -> Result<String, String> {
    use crate::track::{self, Source, Target, route::Through};
    match args {
        [] => {
            let Some(t) = st.track.as_ref() else {
                return Ok("nothing tracked".to_string());
            };
            let label = t.label.clone();
            let target = match track::target_now(st) {
                Some(Target::Building(b)) => format!("Building({})", b.0),
                Some(Target::Person(p)) => format!("Person({})", p.0),
                Some(Target::Spot { pos, .. }) => format!("Spot({},{})", pos.x, pos.y),
                None => "?".to_string(),
            };
            let leg = match track::leg_now(st) {
                Some(Ok(l)) => {
                    let through = match l.through {
                        Through::Arrive => "Arrive",
                        Through::Door(_) => "Door",
                        Through::Lift(_) => "Lift",
                        Through::Stairs { .. } => "Stairs",
                        Through::Exit => "Exit",
                        Through::Connector(_) => "Connector",
                    };
                    format!("{through} · {} m", l.path.len())
                }
                _ => "no leg".to_string(),
            };
            Ok(format!("⌖ {label} · {target} · {leg}"))
        }
        ["off"] => {
            if st.track.is_none() {
                return Ok("nothing tracked".to_string());
            }
            track::stop(st);
            Ok("stopped".to_string())
        }
        [x, y] if x.parse::<i32>().is_ok() && y.parse::<i32>().is_ok() => {
            let place = st
                .player
                .as_ref()
                .map(|p| p.place)
                .ok_or_else(|| "no body in the city".to_string())?;
            let pos = Pos::new(x.parse().unwrap_or(0), y.parse().unwrap_or(0));
            track::set(
                st,
                Source::Spot {
                    place,
                    pos,
                    radius: 1,
                },
            )
        }
        words => {
            let want = words.join(" ").to_lowercase();
            let all = track::listings::listings(st, crate::screens::atlas::map_stratum(st));
            let hits: Vec<_> = all
                .iter()
                .filter(|l| l.name.to_lowercase().starts_with(&want))
                .collect();
            match hits.as_slice() {
                [] => Err(format!("no listing called {want}")),
                [one] => {
                    let source = track::listings::source_of(st, one);
                    track::set(st, source)
                }
                many => Err(format!(
                    "several: {}",
                    many.iter()
                        .take(6)
                        .map(|l| l.name.as_str())
                        .collect::<Vec<_>>()
                        .join(" · ")
                )),
            }
        }
    }
}

/// `story`: the cast, missions and scenes; `fire <mission>` runs its waiting step;
/// `scene <id>` plays a scene, `skip` skips the one playing; `feed` and `call` queue a
/// test feed or ring a test call from the first role with a face.
fn story(st: &mut AppState, args: &[&str]) -> Result<String, String> {
    let usage = "story [fire <mission> | scene <id> | skip | feed | call]";
    let test_role = || {
        st.book
            .cast
            .iter()
            .find(|(_, r)| r.face.is_some())
            .map(|(id, _)| id.clone())
            .ok_or_else(|| "no role has a face".to_string())
    };
    match args {
        [] => Ok(crate::missions::ui::console_lines(st).join(" · ")),
        ["fire", id] => {
            let m = st
                .book
                .missions
                .iter()
                .position(|m| m.id == *id)
                .ok_or_else(|| format!("unknown mission {id}"))?;
            if st.missions.runs.get(m).is_none() {
                return Err("the story has not begun in this city".to_string());
            }
            if !crate::missions::force(st, m) {
                return Err(format!("nothing waits in {id}"));
            }
            Ok(format!("fired {id}"))
        }
        ["scene", id] => {
            if !st.book.scenes.contains_key(*id) {
                return Err(format!("unknown scene {id}"));
            }
            crate::missions::scene::play(st, id);
            Ok(format!("playing {id}"))
        }
        ["skip"] => {
            let id = crate::missions::scene::running(st)
                .map(|r| r.id().to_string())
                .ok_or_else(|| "no scene is playing".to_string())?;
            crate::missions::scene::skip(st);
            Ok(format!("skipped {id}"))
        }
        ["feed"] => {
            let from = test_role()?;
            st.missions.feeds.push_back(crate::missions::FeedView {
                mission: usize::MAX,
                step: usize::MAX,
                from,
                text: vec![
                    "Test feed. Words go here, then more words, wrapped to the box.".to_string(),
                    "A second paragraph, to see the gap between them.".to_string(),
                ],
                close: "close".to_string(),
            });
            Ok("feed queued".to_string())
        }
        ["call"] => {
            let from = test_role()?;
            crate::missions::call::ring(
                st,
                crate::missions::Call {
                    mission: usize::MAX,
                    step: usize::MAX,
                    from,
                    lines: vec![
                        "Test call. Can you hear me?".to_string(),
                        "A second line, to step to.".to_string(),
                    ],
                    rang: 0,
                    answered: None,
                    line: 0,
                    shown: 0,
                    full: false,
                },
            );
            Ok("ringing".to_string())
        }
        _ => Err(usage.to_string()),
    }
}

/// Completion candidates: commands for the first word, item ids after spawn or drop.
pub fn complete(st: &AppState, head: &str, word: &str) -> Vec<String> {
    if head.is_empty() {
        COMMANDS
            .iter()
            .filter(|c| c.starts_with(word))
            .map(|c| c.to_string())
            .collect()
    } else if matches!(head.trim(), "spawn" | "drop") {
        st.content
            .items
            .iter()
            .map(|i| i.id.0.clone())
            .filter(|id| id.starts_with(word))
            .collect()
    } else if head.trim() == "story" {
        ["fire", "scene", "skip", "feed", "call"]
            .iter()
            .filter(|c| c.starts_with(word))
            .map(|c| c.to_string())
            .collect()
    } else if head.trim() == "track" {
        let want = word.to_lowercase();
        let mut v = vec!["off".to_string()];
        v.extend(
            crate::track::listings::listings(st, crate::screens::atlas::map_stratum(st))
                .into_iter()
                .map(|l| l.name),
        );
        v.retain(|c| c.to_lowercase().starts_with(&want));
        v
    } else if head.trim() == "story fire" {
        st.book
            .missions
            .iter()
            .map(|m| m.id.clone())
            .filter(|id| id.starts_with(word))
            .collect()
    } else if head.trim() == "story scene" {
        st.book
            .scenes
            .keys()
            .filter(|id| id.starts_with(word))
            .cloned()
            .collect()
    } else if head.trim() == "target" {
        let want = word.to_lowercase();
        st.world
            .as_ref()
            .map(|w| {
                w.people()
                    .iter()
                    .map(|p| p.name.clone())
                    .filter(|n| n.to_lowercase().starts_with(&want))
                    .collect()
            })
            .unwrap_or_default()
    } else {
        Vec::new()
    }
}

/// The console as the bottom row: prompt, input, a pulsing cursor, then the completion
/// hint or, without one, the newest log entry while it is recent.
pub fn line(con: &Console, tick: u64, recent: Option<&Entry>) -> Line<'static> {
    let mut spans = vec![
        Span::styled(": ", Style::default().fg(CYAN).add_modifier(Modifier::BOLD)),
        Span::styled(con.input.clone(), Style::default().fg(TEXT)),
        Span::styled(
            "▮",
            Style::default().fg(MAGENTA_RGB.scale(0.5 + 0.5 * pulse(tick, 20)).to_color()),
        ),
    ];
    if let Some(hint) = &con.hint {
        spans.push(Span::styled(format!("  {hint}"), Style::default().fg(DIM)));
    } else if let Some(e) = recent {
        let style = if e.kind == Kind::Warn {
            Style::default().fg(MAGENTA).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(CYAN)
        };
        spans.push(Span::styled(format!("  {}", e.marked()), style));
    }
    Line::from(spans)
}

#[cfg(test)]
mod tests {
    use super::*;
    use citygen::{GenParams, Generator, Pos, SizePreset, StratumKind};
    use game::ItemId;

    fn setup() -> (AppState, At) {
        let mut st = AppState::new();
        let city = Generator::run(GenParams {
            seed: 6,
            size: SizePreset::Custom { w: 120, h: 60 },
            districts: 5,
            ..Default::default()
        })
        .unwrap();
        let p = crate::walker::Player::spawn(&city, 1);
        let at = At {
            place: p.place,
            pos: p.pos,
        };
        st.city = Some(city);
        (st, at)
    }

    #[test]
    fn commands_change_the_world() {
        let (mut st, at) = setup();
        let coil = ItemId::new("copper_coil");
        assert_eq!(
            run(&mut st, at, "spawn copper_coil 2").unwrap(),
            "spawned 2× `Copper coil`"
        );
        assert_eq!(st.character.count(&coil), 5);
        assert_eq!(
            run(&mut st, at, "spawn copper_coil").unwrap(),
            "spawned 1× `Copper coil`"
        );
        assert_eq!(
            run(&mut st, at, "spawn cop").unwrap_err(),
            "unknown item cop"
        );
        assert_eq!(
            run(&mut st, at, "drop dead_phone").unwrap(),
            "dropped 1× `Dead phone`"
        );
        assert_eq!(st.ground.at(at.place, at.pos)[0].qty, 1);
        run(&mut st, at, "drop coldfin_1 2").unwrap();
        let lying = st.ground.at(at.place, at.pos);
        assert_eq!(
            lying
                .iter()
                .filter(|s| s.item == ItemId::new("coldfin_1"))
                .map(|s| (s.qty, s.copy.is_some()))
                .collect::<Vec<_>>(),
            vec![(1, true), (1, true)],
            "hardware lies as copies"
        );
        // Back to the phone alone for what follows.
        let phone = st.ground.take_all(at.place, at.pos).remove(0);
        st.ground.put(at.place, at.pos, phone);
        let wall = {
            let city = st.city.as_ref().unwrap();
            let pos = (0..60)
                .flat_map(|y| (0..120).map(move |x| Pos::new(x, y)))
                .find(|q| !city.walkable(StratumKind::Street, *q))
                .unwrap();
            At {
                place: citygen::Place::Stratum(StratumKind::Street),
                pos,
            }
        };
        assert_eq!(
            run(&mut st, wall, "drop dead_phone").unwrap_err(),
            "nothing can lie there"
        );
        assert_eq!(run(&mut st, at, "give 100").unwrap(), "credits 420");
        run(&mut st, at, "give 4294967295").unwrap();
        assert_eq!(
            run(&mut st, at, "give 4294967295").unwrap(),
            "credits 4294967295",
            "saturates"
        );
        st.character.hp = 1;
        assert_eq!(run(&mut st, at, "heal").unwrap(), "hp 45/45");
        assert_eq!(run(&mut st, at, "clear").unwrap(), "cleared 1 stack");
        assert!(st.ground.at(at.place, at.pos).is_empty());
        assert!(run(&mut st, at, "help").unwrap().contains("spawn"));
        assert_eq!(run(&mut st, at, "fly").unwrap_err(), "unknown command fly");
        assert_eq!(run(&mut st, at, "give x").unwrap_err(), "give <credits>");
        assert_eq!(
            complete(&st, "", "sp"),
            vec!["spawn".to_string(), "sp".to_string()]
        );
        assert_eq!(
            complete(&st, "spawn", "copp"),
            vec!["copper_coil".to_string()]
        );
        assert!(complete(&st, "give", "co").is_empty());
        assert_eq!(run(&mut st, at, "xp 250").unwrap(), "+250 xp");
        assert_eq!((st.character.level, st.character.xp), (2, 50));
        assert_eq!(
            st.log.latest().map(|e| e.marked()),
            Some("▸ level 2 · a talent point · 100 SP".to_string())
        );
        assert_eq!(run(&mut st, at, "xp x").unwrap_err(), "xp <points>");
        st.character.level = u8::MAX;
        let before = st.log.len();
        assert_eq!(run(&mut st, at, "xp 1").unwrap(), "+1 xp");
        assert_eq!((st.character.level, st.log.len()), (u8::MAX, before));
        assert_eq!(run(&mut st, at, "focus 5").unwrap(), "focus 5/30");
        assert_eq!(run(&mut st, at, "focus 99").unwrap(), "focus 30/30");
        assert!(run(&mut st, at, "help").unwrap().contains("xp"));
        assert_eq!(
            complete(&st, "", "fo"),
            vec!["focus".to_string(), "foe".to_string(), "foes".to_string()]
        );
        assert_eq!(complete(&st, "", "thr"), vec!["throw".to_string()]);
        assert_eq!(complete(&st, "", "liv"), vec!["live".to_string()]);
        assert!(
            run(&mut st, at, "help")
                .unwrap()
                .contains("throw <item> [n]")
        );
        // On top of the 100 the level above brought.
        assert_eq!(run(&mut st, at, "sp 200").unwrap(), "sp 300");
        assert_eq!(run(&mut st, at, "pts 3").unwrap(), "points 3");
        assert_eq!((st.character.sp, st.character.attribute_points), (300, 3));
        assert_eq!(run(&mut st, at, "sp x").unwrap_err(), "sp <points>");
        assert!(run(&mut st, at, "help").unwrap().contains("pts"));
    }

    #[test]
    fn backdoor_trace_and_seize_drive_the_backdoor_state() {
        let (mut st, at) = setup();
        let b = st.city.as_ref().unwrap().buildings[0].id;
        let host = crate::helix::hosts::building_hostname(&st, b);
        assert!(run(&mut st, at, "backdoor").is_err());
        assert!(run(&mut st, at, "backdoor zzzz-no-such").is_err());
        run(&mut st, at, &format!("backdoor {host}")).unwrap();
        assert!(st.backdoors.held(b).is_some());
        assert_eq!(run(&mut st, at, "trace").unwrap(), "a trace runs home");
        assert!(st.backdoors.trace.is_some());
        assert!(st.backdoors.route.is_none());
        run(&mut st, at, "seize").unwrap();
        assert!(st.backdoors.seized(st.net.now(&st)));
    }

    #[test]
    fn who_counts_the_people() {
        let (mut st, at) = setup();
        let city = st.city.as_ref().unwrap();
        st.world = Some(sim::World::seed(
            city,
            &st.people,
            &sim::SimParams::default(),
            22 * 60,
            0,
        ));
        let msg = run(&mut st, at, "who").unwrap();
        assert!(
            msg.contains(" out · ") && msg.contains(" inside · ") && msg.contains(" driving"),
            "{msg}"
        );
        st.world = None;
        assert_eq!(run(&mut st, at, "who").unwrap_err(), "no city, no people");
    }

    #[test]
    fn eyes_and_light_toggle() {
        let (mut st, at) = setup();
        assert!(!st.show_eyes && !st.show_light);
        assert_eq!(run(&mut st, at, "eyes").unwrap(), "eyes on");
        assert_eq!(run(&mut st, at, "light").unwrap(), "light on");
        assert!(st.show_eyes && st.show_light);
        assert_eq!(run(&mut st, at, "eyes").unwrap(), "eyes off");
        assert!(!st.show_eyes);
    }

    /// Seed 8 with its home, the story begun, and a console aimed at the street.
    fn story_state() -> (AppState, At) {
        let mut st = crate::jobs::testutil::state_home();
        crate::missions::begin(&mut st);
        let at = At {
            place: citygen::Place::Stratum(citygen::StratumKind::Street),
            pos: Pos::new(0, 0),
        };
        (st, at)
    }

    #[test]
    fn track_shows_and_stops() {
        let (mut st, at) = story_state();
        let _ = crate::screens::explore::MapScreen::walking(&mut st);
        assert_eq!(run(&mut st, at, "track").unwrap(), "nothing tracked");
        let here = st.player.as_ref().unwrap().pos;
        let line = format!("track {} {}", here.x + 6, here.y);
        let out = run(&mut st, at, &line).unwrap();
        assert!(out.contains("tracking"), "{out}");
        assert!(run(&mut st, at, "track").unwrap().starts_with('⌖'));
        assert_eq!(run(&mut st, at, "track off").unwrap(), "stopped");
        assert!(st.track.is_none());
        assert_eq!(run(&mut st, at, "track").unwrap(), "nothing tracked");
    }

    #[test]
    fn track_by_name_finds_a_listing() {
        let (mut st, at) = story_state();
        let _ = crate::screens::explore::MapScreen::walking(&mut st);
        let out = run(&mut st, at, "track miracorp").unwrap();
        assert!(out.contains("Miracorp"), "{out}");
        assert!(run(&mut st, at, "track off").is_ok());
        assert_eq!(
            run(&mut st, at, "track zzz").unwrap_err(),
            "no listing called zzz"
        );
        assert!(
            run(&mut st, at, "track mira")
                .unwrap_err()
                .starts_with("several: ")
        );
        assert!(
            complete(&st, "track", "mi")
                .iter()
                .any(|c| c.starts_with("Miracorp"))
        );
    }

    #[test]
    fn story_lists_the_cast_and_missions() {
        let (mut st, at) = story_state();
        let out = run(&mut st, at, "story").unwrap();
        assert!(out.contains("mezz"), "{out}");
        assert!(out.contains("the_drop"), "{out}");
        assert!(out.contains("waits on"), "{out}");
        assert_eq!(complete(&st, "", "sto"), vec!["story".to_string()]);
        assert!(run(&mut st, at, "help").unwrap().contains("story"));
        assert!(
            run(&mut st, at, "story nope")
                .unwrap_err()
                .contains("story")
        );
    }

    #[test]
    fn story_fire_runs_the_waiting_step() {
        let (mut st, at) = story_state();
        let package = ItemId::new("taped_package");
        assert_eq!(st.character.count(&package), 0);
        let before = st.missions.runs[0].at;
        run(&mut st, at, "story fire the_drop").unwrap();
        assert_eq!(st.character.count(&package), 1);
        assert!(st.missions.runs[0].at > before);
        assert!(
            run(&mut st, at, "story fire nope")
                .unwrap_err()
                .contains("unknown mission")
        );
        assert!(complete(&st, "story", "fi").contains(&"fire".to_string()));
    }

    #[test]
    fn story_scene_plays_and_skip_skips() {
        use crate::missions::{Beat, Look, Pace, Scene, scene};
        let (mut st, at) = story_state();
        let aside = Scene {
            title: "Aside".into(),
            look: Look::Bubbles,
            pace: Pace {
                chars: 45.0,
                walk: 0.3,
                hold: None,
            },
            beats: vec![Beat::Stage("A stage line.".into()), Beat::Wait(1.0)],
            next: None,
        };
        let mut film = aside.clone();
        film.look = Look::Film;
        film.next = Some("aside".into());
        st.book.scenes.insert("aside".into(), aside);
        st.book.scenes.insert("film".into(), film);
        let out = run(&mut st, at, "story").unwrap();
        // The aside names no next; the film names the aside.
        assert!(
            out.contains(
                "scene aside · bubbles · 2 beats · scene film · film · 2 beats · then aside"
            ),
            "{out}"
        );
        assert!(
            run(&mut st, at, "story scene nope")
                .unwrap_err()
                .contains("unknown scene")
        );
        assert_eq!(
            run(&mut st, at, "story skip").unwrap_err(),
            "no scene is playing"
        );
        assert_eq!(
            run(&mut st, at, "story scene aside").unwrap(),
            "playing aside"
        );
        crate::missions::tick(&mut st);
        assert_eq!(scene::running(&st).map(|r| r.id()), Some("aside"));
        assert_eq!(run(&mut st, at, "story skip").unwrap(), "skipped aside");
        assert!(scene::running(&st).is_none());
        let words = complete(&st, "story", "s");
        assert!(words.contains(&"scene".to_string()) && words.contains(&"skip".to_string()));
        assert_eq!(
            complete(&st, "story scene", "as"),
            vec!["aside".to_string()]
        );
        assert!(run(&mut st, at, "help").unwrap().contains("scene <id>"));
    }

    #[test]
    fn story_feed_and_call_queue_tests() {
        let (mut st, at) = story_state();
        st.missions.feeds.clear();
        run(&mut st, at, "story feed").unwrap();
        assert_eq!(st.missions.feeds.len(), 1);
        assert!(st.missions.call.is_none());
        run(&mut st, at, "story call").unwrap();
        assert!(st.missions.call.is_some() || !st.missions.calls.is_empty());
    }

    #[test]
    fn ripper_toggles_free_chrome() {
        let (mut st, at) = setup();
        assert!(!st.free_chrome);
        assert_eq!(
            run(&mut st, at, "ripper").unwrap(),
            "ripper on · chrome swaps anywhere, free"
        );
        assert!(st.free_chrome);
        assert_eq!(run(&mut st, at, "ripper").unwrap(), "ripper off");
        assert!(!st.free_chrome);
        assert_eq!(complete(&st, "", "rip"), vec!["ripper".to_string()]);
        assert!(run(&mut st, at, "help").unwrap().contains("ripper"));
    }

    #[test]
    fn clock_sets_the_hour_and_reseeds_the_day() {
        let (mut st, at) = setup();
        let city = st.city.as_ref().unwrap();
        st.world = Some(sim::World::seed(
            city,
            &st.people,
            &sim::SimParams::default(),
            22 * 60,
            0,
        ));
        assert_eq!(run(&mut st, at, "clock 23:30").unwrap(), "23:30");
        assert_eq!(st.clock(), (23, 30));
        assert_eq!(st.tick, 0, "the frame tick is not the game's clock");
        assert_eq!(st.world.as_ref().unwrap().minute_of_day(), 23 * 60 + 30);
        assert_eq!(run(&mut st, at, "clock 12:00").unwrap(), "12:00");
        assert_eq!(st.clock(), (12, 0));
        assert_eq!(run(&mut st, at, "clock noon").unwrap_err(), "clock <hh:mm>");
        assert_eq!(
            run(&mut st, at, "clock 25:00").unwrap_err(),
            "clock <hh:mm>"
        );
        // The clock refreshes sight: noon sees the whole street at once.
        {
            let city = st.city.as_ref().unwrap();
            let mut p = crate::walker::Player::spawn(city, 1);
            p.update_fov(
                &crate::render::map::MapRef::stratum_of(city, StratumKind::Street),
                0,
            );
            st.player = Some(p);
        }
        let night = st
            .player
            .as_ref()
            .unwrap()
            .vis()
            .count(|v| *v == crate::walker::Visibility::Visible);
        assert_eq!(run(&mut st, at, "time 12:00").unwrap(), "12:00");
        let noon = st
            .player
            .as_ref()
            .unwrap()
            .vis()
            .count(|v| *v == crate::walker::Visibility::Visible);
        assert!(noon > 2 * night, "{noon} at noon vs {night} at night");
        // `time` is the same command by another name.
        assert_eq!(run(&mut st, at, "time 08:00").unwrap(), "08:00");
        assert_eq!(st.clock(), (8, 0));
        assert_eq!(run(&mut st, at, "time 8").unwrap_err(), "time <hh:mm>");
    }

    #[test]
    fn tp_moves_the_player_on_the_current_map() {
        let (mut st, at) = setup();
        let city = st.city.as_ref().unwrap().clone();
        st.player = Some(crate::walker::Player::spawn(&city, 1));
        let free = (0..city.size.h as i32)
            .flat_map(|y| (0..city.size.w as i32).map(move |x| Pos::new(x, y)))
            .find(|p| city.walkable(StratumKind::Street, *p))
            .unwrap();
        assert_eq!(
            run(&mut st, at, &format!("tp {} {}", free.x, free.y)).unwrap(),
            format!("{},{} · Street", free.x, free.y)
        );
        assert_eq!(st.player.as_ref().unwrap().pos, free);
        let wall = (0..city.size.h as i32)
            .flat_map(|y| (0..city.size.w as i32).map(move |x| Pos::new(x, y)))
            .find(|p| !city.walkable(StratumKind::Street, *p))
            .unwrap();
        assert_eq!(
            run(&mut st, at, &format!("tp {} {}", wall.x, wall.y)).unwrap_err(),
            "nothing to stand on there"
        );
        assert_eq!(run(&mut st, at, "tp 3").unwrap_err(), "tp <x> <y>");
        st.player = None;
        assert_eq!(
            run(&mut st, at, "tp 1 1").unwrap_err(),
            "no body in the city"
        );
    }

    #[test]
    fn talent_commands_learn_and_add_points() {
        let (mut st, at) = setup();
        assert_eq!(run(&mut st, at, "talent").unwrap_err(), "talent <id>");
        assert_eq!(
            run(&mut st, at, "talent ice_pick").unwrap(),
            "`Ice pick` 1/2"
        );
        assert_eq!(
            run(&mut st, at, "talent ice_pick").unwrap_err(),
            "no points left"
        );
        assert_eq!(run(&mut st, at, "tpts 3").unwrap(), "3 talent points");
        assert_eq!(st.character.talent_points, 3);
        assert_eq!(
            run(&mut st, at, "talent ice_pick").unwrap(),
            "`Ice pick` 2/2"
        );
        assert_eq!(
            run(&mut st, at, "talent ice_pick").unwrap_err(),
            "already at its best"
        );
        // A level brings a point and SP, and the log says so.
        let before = st.character.talent_points;
        run(&mut st, at, "xp 200").unwrap();
        assert_eq!(st.character.talent_points, before + 1);
        assert_eq!(
            st.log.latest().map(|e| e.text.as_str()),
            Some("level 2 · a talent point · 100 SP")
        );
        // Every third level an attribute point as well; levels crossed at once each
        // have their line.
        let (level, points) = (st.character.level, st.character.attribute_points);
        assert_eq!(level, 2);
        let xp: u32 = (2..7).map(game::Character::xp_to_next).sum();
        let before = st.log.len();
        run(&mut st, at, &format!("xp {xp}")).unwrap();
        assert_eq!(st.character.level, 7);
        assert_eq!(st.character.attribute_points, points + 2);
        let lines: Vec<String> = st.log.iter().skip(before).map(|e| e.text.clone()).collect();
        assert_eq!(
            lines,
            [
                "level 3 · a talent point · 100 SP · an attribute point",
                "level 4 · a talent point · 100 SP",
                "level 5 · a talent point · 100 SP",
                "level 6 · a talent point · 100 SP · an attribute point",
                "level 7 · a talent point · 100 SP",
            ]
        );
    }

    fn city_state() -> AppState {
        let mut st = AppState::new();
        st.enter_city(
            Generator::run(GenParams {
                seed: 6,
                size: SizePreset::Custom { w: 120, h: 60 },
                districts: 5,
                ..Default::default()
            })
            .unwrap(),
        );
        st
    }

    #[test]
    fn target_finds_a_person_and_asks_the_map_to_jump() {
        let mut st = city_state();
        let at = At {
            place: citygen::Place::Stratum(StratumKind::Street),
            pos: Pos::new(0, 0),
        };
        // Someone on the street right now, found by a lower-case prefix of their name.
        let (name, pos) = {
            let w = st.world.as_ref().unwrap();
            w.people()
                .iter()
                .find_map(|p| w.pos_of(p.id).map(|pos| (p.name.clone(), pos)))
                .expect("someone outside")
        };
        let whole = run(&mut st, at, &format!("target {}", name.to_lowercase())).unwrap();
        assert!(whole.contains(&name), "{whole}");
        assert_eq!(st.jump_to, Some(pos));
        // Nobody by that name; a prefix shared by several is refused with names.
        assert!(run(&mut st, at, "target zzzzzz").is_err());
        let first: char = name.chars().next().unwrap();
        let shared = st
            .world
            .as_ref()
            .unwrap()
            .people()
            .iter()
            .filter(|p| {
                p.name
                    .to_lowercase()
                    .starts_with(first.to_ascii_lowercase())
            })
            .count();
        if shared > 1 {
            let err = run(&mut st, at, &format!("target {first}")).unwrap_err();
            assert!(err.contains("several"), "{err}");
        }
        assert!(run(&mut st, at, "target").is_err());
        // Someone indoors: the cursor goes to their building's street door.
        let (inside_name, b) = {
            let w = st.world.as_ref().unwrap();
            w.people()
                .iter()
                .find_map(|p| match p.whereabouts {
                    sim::Whereabouts::Inside(b) => Some((p.name.clone(), b)),
                    _ => None,
                })
                .expect("someone inside")
        };
        let out = run(&mut st, at, &format!("target {inside_name}")).unwrap();
        assert!(out.contains("inside"), "{out}");
        let door = st.city.as_ref().unwrap().portals(b)[0].1;
        assert_eq!(st.jump_to, Some(door));
        // Tab completion offers names after `target`, whatever the case typed.
        let lower = name.to_lowercase();
        let prefix: String = lower.chars().take(2).collect();
        let names = complete(&st, "target", &prefix);
        assert!(names.iter().any(|n| n == &name), "{names:?}");
    }

    #[test]
    fn wait_opens_the_wait_window() {
        let mut st = crate::jobs::testutil::state_home();
        crate::screens::explore::MapScreen::walking(&mut st);
        let p = st.player.as_ref().unwrap();
        let at = At {
            place: p.place,
            pos: p.pos,
        };
        assert_eq!(run(&mut st, at, "wait").unwrap(), "wait · pick the time");
        assert!(opens(&st, "wait").is_some());
        assert!(opens(&st, "heal").is_none());
        assert_eq!(complete(&st, "", "wai"), vec!["wait".to_string()]);
        assert!(run(&mut st, at, "help").unwrap().contains("wait"));
        let until = crate::rest::now(&st) + 600;
        crate::rest::start(&mut st, crate::rest::Kind::Sleep, until);
        assert!(run(&mut st, at, "wait").is_err(), "not while resting");
    }
}
