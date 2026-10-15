//! Dev console commands: spawn and drop items, give credits, heal, xp, focus, count the
//! people, jump the clock, clear a pile.

use crate::anim::effects::pulse;
use crate::app::AppState;
use crate::messages::{Entry, Kind};
use crate::ui::chrome::{CYAN, DIM, MAGENTA, MAGENTA_RGB, TEXT};
use crate::ui::console::Console;
use citygen::Pos;
use game::{At, ItemId};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};

const COMMANDS: [&str; 15] = [
    "spawn", "drop", "give", "heal", "xp", "focus", "sp", "pts", "talent", "tpts", "who", "clock",
    "tp", "clear", "help",
];

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
                st.character.give(item.id.clone(), qty);
                Ok(format!("spawned {qty}× {}", item.name))
            } else {
                let lies = st.city.as_ref().is_some_and(|c| {
                    crate::render::map::MapRef::for_place(c, &st.interiors, focus.place)
                        .is_some_and(|m| crate::render::map::TileMap::walkable(&m, focus.pos))
                });
                if !lies {
                    return Err("nothing can lie there".to_string());
                }
                st.ground
                    .add(focus.place, focus.pos, item.id.clone(), qty);
                Ok(format!("dropped {qty}× {}", item.name))
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
        "xp" => {
            let n: u32 = args
                .first()
                .and_then(|a| a.parse().ok())
                .ok_or_else(|| "xp <points>".to_string())?;
            let gained = st.character.gain_xp(n);
            let level = st.character.level;
            for i in (0..gained).rev() {
                st.story(format!("level {} · a talent point", level - i));
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
            st.character.stat_points = st.character.stat_points.saturating_add(n);
            Ok(format!("points {}", st.character.stat_points))
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
            Ok(format!("{} {}/{}", t.name, rank, t.ranks))
        }
        "tpts" => {
            let n: u32 = args
                .first()
                .and_then(|a| a.parse().ok())
                .ok_or_else(|| "tpts <points>".to_string())?;
            st.character.talent_points = st.character.talent_points.saturating_add(n);
            Ok(format!("{} talent points", st.character.talent_points))
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
        "clock" => {
            // Jumps the clock forward to the next hh:mm and reseeds the city's day there.
            let usage = "clock <hh:mm>".to_string();
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
            Ok(format!("{h:02}:{m:02}"))
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
            let map = crate::render::map::MapRef::for_place(city, &st.interiors, place)
                .ok_or_else(|| "no map here".to_string())?;
            if !crate::render::map::TileMap::walkable(&map, target) {
                return Err("nothing to stand on there".to_string());
            }
            let label = map.label();
            let bonus = st.character.sight_bonus(&st.content);
            let player = st.player.as_mut().expect("checked above");
            player.pos = target;
            player.update_fov(&map, bonus);
            st.repaint = true;
            Ok(format!("{x},{y} · {label}"))
        }
        "clear" => {
            let n = st.ground.at(focus.place, focus.pos).len();
            st.ground.clear(focus.place, focus.pos);
            Ok(format!(
                "cleared {n} stack{}",
                if n == 1 { "" } else { "s" }
            ))
        }
        "help" => Ok(
            "spawn <item> [qty] · drop <item> [qty] · give <credits> · heal · xp <points> · focus <points> · sp <points> · pts <points> · talent <id> · tpts <points> · who · clock <hh:mm> · tp <x> <y> · clear · help"
                .to_string(),
        ),
        "" => Err("type a command; help lists them".to_string()),
        other => Err(format!("unknown command {other}")),
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
            "spawned 2× Copper coil"
        );
        assert_eq!(st.character.count(&coil), 5);
        assert_eq!(
            run(&mut st, at, "spawn copper_coil").unwrap(),
            "spawned 1× Copper coil"
        );
        assert_eq!(
            run(&mut st, at, "spawn cop").unwrap_err(),
            "unknown item cop"
        );
        assert_eq!(
            run(&mut st, at, "drop dead_phone").unwrap(),
            "dropped 1× Dead phone"
        );
        assert_eq!(st.ground.at(at.place, at.pos)[0].qty, 1);
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
            complete(&st, "spawn", "co"),
            vec!["copper_coil".to_string()]
        );
        assert!(complete(&st, "give", "co").is_empty());
        assert_eq!(run(&mut st, at, "xp 250").unwrap(), "+250 xp");
        assert_eq!((st.character.level, st.character.xp), (2, 50));
        assert_eq!(
            st.log.latest().map(|e| e.marked()),
            Some("▸ level 2 · a talent point".to_string())
        );
        assert_eq!(run(&mut st, at, "xp x").unwrap_err(), "xp <points>");
        st.character.level = u8::MAX;
        let before = st.log.len();
        assert_eq!(run(&mut st, at, "xp 1").unwrap(), "+1 xp");
        assert_eq!((st.character.level, st.log.len()), (u8::MAX, before));
        assert_eq!(run(&mut st, at, "focus 5").unwrap(), "focus 5/30");
        assert_eq!(run(&mut st, at, "focus 99").unwrap(), "focus 30/30");
        assert!(run(&mut st, at, "help").unwrap().contains("xp"));
        assert_eq!(complete(&st, "", "fo"), vec!["focus".to_string()]);
        assert_eq!(run(&mut st, at, "sp 2").unwrap(), "sp 2");
        assert_eq!(run(&mut st, at, "pts 3").unwrap(), "points 3");
        assert_eq!((st.character.sp, st.character.stat_points), (2, 3));
        assert_eq!(run(&mut st, at, "sp x").unwrap_err(), "sp <points>");
        assert!(run(&mut st, at, "help").unwrap().contains("pts"));
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
        assert_eq!(run(&mut st, at, "talent ice_pick").unwrap(), "Ice pick 1/2");
        assert_eq!(
            run(&mut st, at, "talent ice_pick").unwrap_err(),
            "no points left"
        );
        assert_eq!(run(&mut st, at, "tpts 3").unwrap(), "3 talent points");
        assert_eq!(st.character.talent_points, 3);
        assert_eq!(run(&mut st, at, "talent ice_pick").unwrap(), "Ice pick 2/2");
        assert_eq!(
            run(&mut st, at, "talent ice_pick").unwrap_err(),
            "already at its best"
        );
        // A level brings a point and the log says so.
        let before = st.character.talent_points;
        run(&mut st, at, "xp 200").unwrap();
        assert_eq!(st.character.talent_points, before + 1);
        assert_eq!(
            st.log.latest().map(|e| e.text.as_str()),
            Some("level 2 · a talent point")
        );
    }
}
