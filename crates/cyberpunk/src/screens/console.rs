//! Dev console commands: spawn and drop items, give credits, heal, clear a pile.

use crate::anim::effects::pulse;
use crate::app::AppState;
use crate::ui::chrome::{CYAN, DIM, MAGENTA, MAGENTA_RGB, TEXT};
use crate::ui::console::Console;
use game::{At, ItemId};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};

const COMMANDS: [&str; 6] = ["spawn", "drop", "give", "heal", "clear", "help"];

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
                let lies = st
                    .city
                    .as_ref()
                    .is_some_and(|c| c.walkable(focus.stratum, focus.pos));
                if !lies {
                    return Err("nothing can lie there".to_string());
                }
                st.ground
                    .add(focus.stratum, focus.pos, item.id.clone(), qty);
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
        "clear" => {
            let n = st.ground.at(focus.stratum, focus.pos).len();
            st.ground.clear(focus.stratum, focus.pos);
            Ok(format!(
                "cleared {n} stack{}",
                if n == 1 { "" } else { "s" }
            ))
        }
        "help" => Ok(
            "spawn <item> [qty] · drop <item> [qty] · give <credits> · heal · clear · help"
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

/// The console as the status row: prompt, input, a pulsing cursor, then the completion
/// hint or, without one, the last result (`status`, with its warning flag).
pub fn line(con: &Console, tick: u64, status: Option<(&str, bool)>) -> Line<'static> {
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
    } else if let Some((msg, warn)) = status {
        let style = if warn {
            Style::default().fg(MAGENTA).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(CYAN)
        };
        spans.push(Span::styled(format!("  {msg}"), style));
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
            stratum: p.stratum,
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
        assert_eq!(st.ground.at(at.stratum, at.pos)[0].qty, 1);
        let wall = {
            let city = st.city.as_ref().unwrap();
            let pos = (0..60)
                .flat_map(|y| (0..120).map(move |x| Pos::new(x, y)))
                .find(|q| !city.walkable(StratumKind::Street, *q))
                .unwrap();
            At {
                stratum: StratumKind::Street,
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
        assert!(st.ground.at(at.stratum, at.pos).is_empty());
        assert!(run(&mut st, at, "help").unwrap().contains("spawn"));
        assert_eq!(run(&mut st, at, "fly").unwrap_err(), "unknown command fly");
        assert_eq!(run(&mut st, at, "give x").unwrap_err(), "give <credits>");
        assert_eq!(complete(&st, "", "sp"), vec!["spawn".to_string()]);
        assert_eq!(
            complete(&st, "spawn", "co"),
            vec!["copper_coil".to_string()]
        );
        assert!(complete(&st, "give", "co").is_empty());
    }
}
