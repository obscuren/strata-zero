//! `space` at a reader slot: a popup headed with what the slot holds, whose first row
//! is its summon (cash a chip, install or run a program, sift data, read a story shard)
//! and `x Leave it`. The rows go through the rig's own job runner, so `run` and `ps` in
//! its shell see the same jobs.

use super::layout::at_box;
use super::reader::{self, Holds};
use super::room;
use crate::app::AppState;
use crate::backdoors::Kind;
use crate::interact::FREE_LETTERS;
use crate::missions::ReadAt;
use crate::missions::reading::ReadCard;
use crate::rig::jobs::{self, JobKind};
use crate::screens::popup::{Choice, Header, Pick, Popup};
use crate::ui::chrome::{CYAN, DIM};
use citygen::BuildingId;
use game::ItemId;

/// The amber a credchip's number rises in.
const AMBER: (u8, u8, u8) = (0xff, 0xb0, 0x00);

/// What a summon did.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Summoned {
    /// A chip's sum came to your credits.
    Cash(u32),
    /// A program is in the rig (the shard spent unless it was there already); whether it
    /// wants a host picked now, a seat being free and a Net backdoor held.
    Installed(&'static str, bool),
    /// A shard went to a seat to be sifted: the job's number.
    Sifting(u32),
    /// A story shard to read: its card is opened with `card`.
    Read(ItemId),
}

/// What `space` opens where you stand: the popup of the nearest reader slot's thing
/// within a step of its box. None away from the reader, at an empty slot or at one
/// holding what the reader won't take.
pub fn interact(st: &mut AppState) -> Option<Popup> {
    let pos = st
        .player
        .as_ref()
        .filter(|p| p.place == citygen::Place::Rig)?
        .pos;
    let room = room(st)?;
    let i = (0..room.slots.len())
        .filter(|i| at_box(pos, room.slots[*i]))
        .min_by_key(|i| {
            let r = room.slots[*i];
            pos.chebyshev(citygen::Pos::new(r.x + r.w / 2, r.y + r.h / 2))
        });
    let i = i?;
    let stack = st.character.reader.get(i)?.as_ref()?;
    let holds = reader::holds(st, &stack.item)?;
    let name =
        (st.content.item(&stack.item)).map_or_else(|| stack.item.0.clone(), |it| it.name.clone());
    let (glyph, fg) = reader::glyph(holds);
    let header = Header {
        glyph: Some((glyph, fg)),
        name,
        fg,
    };
    let mut rows = rows(st, i, holds, stack);
    rows.push(Choice::new('x', "Leave it", Pick::Close));
    Some(Popup::about(Some(header), None, rows, st.tick))
}

/// The summon row of slot `i`'s `stack`, greyed with its reason when it can't be done.
fn rows(st: &AppState, i: usize, holds: Holds, stack: &game::Stack) -> Vec<Choice> {
    let summon = Pick::Summon(i);
    match holds {
        Holds::Chip => {
            let credits = stack.copy.as_ref().map_or(0, |k| k.credits);
            vec![Choice::new('s', "Summon", summon).tail(format!(" · +¤{credits}"))]
        }
        Holds::Program(p) => {
            let installed = st.backdoors.jobs.programs.contains(p);
            if !installed {
                return vec![Choice::new('s', format!("Install {p}"), summon)];
            }
            let label = format!("Run {p}");
            match (seat_free(st), hosts(st).is_empty()) {
                (Err(why), _) => vec![Choice::disabled('s', label).tail(format!(" · {why}"))],
                (Ok(()), true) => {
                    vec![Choice::disabled('s', label).tail(" · no Net backdoor held")]
                }
                (Ok(()), false) => vec![Choice::new('s', label, summon)],
            }
        }
        Holds::Data => {
            let hours = if stack.item.0 == "vault_data" {
                jobs::SIFT_VAULT_HOURS
            } else {
                jobs::SIFT_HOURS
            };
            let label = format!("Sift it · {hours}h");
            match seat_free(st) {
                Ok(()) => vec![Choice::new('s', label, summon)],
                Err(why) => vec![Choice::disabled('s', label).tail(format!(" · {why}"))],
            }
        }
        Holds::Story => {
            let verb = crate::missions::reading(st, &stack.item.0, ReadAt::Rig)
                .map(|r| crate::missions::fill(st, &r.verb))
                .filter(|v| !v.is_empty())
                .unwrap_or_else(|| "Read".into());
            vec![Choice::new('s', verb, summon)]
        }
        Holds::Locked => {
            let crack = Pick::CrackChip(i);
            vec![match super::crack::crack_chance(st, i) {
                Ok(chance) => Choice::new('s', "Crack a lock", crack).tail(format!(" · {chance}%")),
                Err(why) => Choice::disabled('s', "Crack a lock").tail(format!(" · {why}")),
            }]
        }
        // Waking MASTIFF adds its row here.
        Holds::Black => Vec::new(),
    }
}

/// Whether a seat is free to start a job in, else why not, as a row's tail says it.
fn seat_free(st: &AppState) -> Result<(), &'static str> {
    if st.backdoors.seized(st.net.now(st)) {
        return Err("held as evidence");
    }
    let seats = crate::rig::of(st).seats as usize;
    if seats == 0 {
        return Err("no core in the rig");
    }
    if st.backdoors.jobs.running.len() >= seats {
        return Err("every seat is busy");
    }
    Ok(())
}

/// The Net backdoors held and standing: what a program that wants a host can run on.
/// A key won't hold that long, so `run` refuses it and so does the list.
pub fn hosts(st: &AppState) -> Vec<BuildingId> {
    (st.backdoors.standing())
        .filter(|d| matches!(d.kind, Kind::Net { .. }))
        .map(|d| d.building)
        .collect()
}

/// Does what slot `slot` holds, by what it is. Logs what it did.
pub fn summon(st: &mut AppState, slot: usize) -> Result<Summoned, String> {
    let stack = (st.character.reader.get(slot).cloned().flatten())
        .ok_or_else(|| "that slot is empty".to_string())?;
    let holds = reader::holds(st, &stack.item).ok_or("the reader won't take that")?;
    match holds {
        Holds::Chip => {
            let credits = stack.copy.as_ref().map_or(0, |k| k.credits);
            st.character.reader[slot] = None;
            st.character.credits = st.character.credits.saturating_add(credits);
            st.say(format!(
                "summoned ¤{credits} · you carry ¤{}",
                st.character.credits
            ));
            if let Some(at) = st.player.as_ref().map(|p| p.pos) {
                let started = st.tick;
                crate::net::number(st, at, format!("+¤{credits}"), Some(AMBER), started);
            }
            Ok(Summoned::Cash(credits))
        }
        Holds::Program(p) => {
            let installed = st.backdoors.jobs.programs.contains(p);
            let wants = seat_free(st).is_ok() && !hosts(st).is_empty();
            if installed {
                seat_free(st).map_err(|why| format!("run: {why}"))?;
                if hosts(st).is_empty() {
                    return Err("run: no Net backdoor held".to_string());
                }
            } else {
                st.character.reader[slot] = None;
                st.backdoors.jobs.programs.insert(p.to_string());
                st.say(format!("INSTALLED {p} FROM SHARD"));
            }
            Ok(Summoned::Installed(p, wants))
        }
        Holds::Data => {
            seat_free(st).map_err(|why| format!("sift: {why}"))?;
            let shard = stack.item.clone();
            st.character.pull_from_reader(slot)?;
            let kind = JobKind::Sift {
                shard: shard.clone(),
            };
            let on = jobs::target(st, &kind);
            match jobs::start(st, kind.clone()) {
                Ok(n) => {
                    let (what, _) = jobs::begun(&kind, &on);
                    st.say(format!("{n} · {what}"));
                    Ok(Summoned::Sifting(n))
                }
                Err(why) => {
                    // The seat went while we looked: the shard goes back in its slot.
                    let _ = st
                        .character
                        .put_item_in_reader(&st.content.clone(), slot, &shard);
                    Err(why)
                }
            }
        }
        Holds::Story => {
            if crate::missions::reading(st, &stack.item.0, ReadAt::Rig).is_none() {
                return Err("nothing on it you can read".to_string());
            }
            Ok(Summoned::Read(stack.item))
        }
        Holds::Locked | Holds::Black => Err("the reader can't open that yet".to_string()),
    }
}

/// The HOLO card of the shard read from slot `slot`, over the map pane; counts the shard
/// read at the rig, and closing it puts the shard in the pack.
pub fn card(st: &mut AppState, slot: usize, item: &ItemId) -> Option<ReadCard> {
    ReadCard::open(st, item, ReadAt::Rig).map(|c| c.read_in_reader(slot))
}

/// The choice of a host for `prog`: each Net backdoor held, and `x Leave it`.
pub fn host_popup(st: &AppState, prog: &'static str) -> Popup {
    let mut letters = FREE_LETTERS.chars().filter(|c| *c != 's');
    let now = st.net.now(st);
    let mut rows: Vec<Choice> = hosts(st)
        .into_iter()
        .filter_map(|b| {
            let left = st.backdoors.live(b)?.left(now);
            let name = crate::net::building_name(st, b);
            let pick = Pick::RunOn { prog, b };
            Some(
                Choice::on(letters.next()?, "Run on", name, CYAN, pick)
                    .tail(format!(" · {} left", super::span(left))),
            )
        })
        .collect();
    rows.push(Choice::new('x', "Leave it", Pick::Close));
    let header = Header {
        glyph: None,
        name: format!("Run {prog} on"),
        fg: DIM,
    };
    Popup::about(Some(header), None, rows, st.tick)
}

/// Starts `prog` on the backdoor at `b` in a free seat, as `run` does; its first lines.
pub fn run_on(st: &mut AppState, prog: &str, b: BuildingId) -> Result<String, String> {
    let kind = match prog {
        "sniff" => JobKind::Sniff(b),
        "keepalive" => JobKind::Keepalive(b),
        _ => return Err("run: no such job · ls /opt/jobs".to_string()),
    };
    let on = jobs::target(st, &kind);
    let n = jobs::start(st, kind.clone())?;
    let (what, _) = jobs::begun(&kind, &on);
    Ok(format!("{n} · {what}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::{Screen, ScreenCmd};
    use crate::missions::reading::tests::reader as reading_state;
    use crate::rigroom::layout::front;
    use crate::rigroom::testutil::{at_the_rig, hold_a_backdoor};
    use crate::screens::testutil::key;
    use crossterm::event::KeyCode;

    /// Jacked in at a rig with a Civitas cabinet: three reader slots and two seats.
    fn in_the_room() -> AppState {
        let mut st = at_the_rig();
        let c = st.content.clone();
        crate::rig::fit_new_rack(&mut st.character, &c, "rack_civitas");
        crate::net::jack_in_here(&mut st).expect("jacked in");
        st
    }

    /// `item` in reader slot `slot`, with a copy sum of `credits` for a chip.
    fn load(st: &mut AppState, slot: usize, item: &str, credits: u32) {
        let c = st.content.clone();
        let id = ItemId::new(item);
        st.character.give_item(&c, id.clone(), 1);
        let copy = (st.character.inventory.iter_mut().rev())
            .find(|s| s.item == id)
            .and_then(|s| {
                if let Some(k) = s.copy.as_mut() {
                    k.credits = credits;
                }
                s.copy_id()
            });
        match copy {
            Some(copy) => st.character.put_in_reader(&c, slot, copy),
            None => st.character.put_item_in_reader(&c, slot, &id),
        }
        .expect("in the reader");
    }

    /// You at the front of slot `slot`'s box.
    fn stand_at(st: &mut AppState, slot: usize) {
        let at = front(room(st).unwrap().slots[slot]);
        st.player.as_mut().unwrap().pos = at;
    }

    fn rows_of(p: &Popup) -> Vec<String> {
        p.choices()
            .iter()
            .map(|c| format!("{}{}", c.label, c.tail))
            .collect()
    }

    fn fire(st: &mut AppState, p: &mut Popup, key_char: char) -> ScreenCmd {
        p.update(st, &[key(KeyCode::Char(key_char))])
    }

    #[test]
    fn summoning_a_chip_cashes_it() {
        let mut st = in_the_room();
        st.character.credits = 320;
        load(&mut st, 0, "credchip", 340);
        stand_at(&mut st, 0);
        let fx = st.fx.len();
        let popup = interact(&mut st).expect("a popup at the slot");
        assert_eq!(rows_of(&popup), ["Summon · +¤340", "Leave it"]);
        assert_eq!(summon(&mut st, 0), Ok(Summoned::Cash(340)));
        assert_eq!(st.character.credits, 660);
        assert!(st.character.reader[0].is_none());
        assert_eq!(st.fx.len(), fx + 1, "a number rises");
        let said = st.log.latest().unwrap().text.clone();
        assert_eq!(said, "summoned ¤340 · you carry ¤660");
    }

    #[test]
    fn summoning_a_program_installs_it_and_offers_a_host() {
        let mut st = in_the_room();
        let net = hold_a_backdoor(&mut st, 0);
        let city = st.city.as_ref().unwrap();
        let seed = city.seed;
        let other = (city.buildings.iter().map(|b| b.id))
            .find(|b| *b != net && st.backdoors.live(*b).is_none())
            .unwrap();
        st.backdoors.plant_key(other, "ops", 100, seed, 0);
        load(&mut st, 1, "prog_keepalive", 0);
        assert!(!st.backdoors.jobs.programs.contains("keepalive"));
        stand_at(&mut st, 1);
        let popup = interact(&mut st).unwrap();
        assert_eq!(rows_of(&popup), ["Install keepalive", "Leave it"]);
        assert_eq!(
            summon(&mut st, 1),
            Ok(Summoned::Installed("keepalive", true))
        );
        assert!(st.character.reader[1].is_none(), "the shard is spent");
        assert!(st.backdoors.jobs.programs.contains("keepalive"));
        assert!(
            st.log
                .iter()
                .any(|e| e.text == "INSTALLED keepalive FROM SHARD")
        );
        // The picker lists the Net backdoor, not the key.
        let mut hosts_popup = host_popup(&st, "keepalive");
        let picks: Vec<_> = (hosts_popup.choices().iter())
            .filter_map(|c| match &c.pick {
                Pick::RunOn { prog, b } => Some((*prog, *b)),
                _ => None,
            })
            .collect();
        assert_eq!(picks, [("keepalive", net)]);
        let first = hosts_popup.choices()[0].key;
        let cmd = fire(&mut st, &mut hosts_popup, first);
        assert!(matches!(cmd, ScreenCmd::Pop { .. }));
        assert!(
            (st.backdoors.jobs.running.iter()).any(|j| j.kind == JobKind::Keepalive(net)),
            "{:?}",
            st.backdoors.jobs.running
        );
    }

    #[test]
    fn an_installed_program_runs_without_spending_the_shard() {
        let mut st = in_the_room();
        let net = hold_a_backdoor(&mut st, 0);
        st.backdoors.jobs.programs.insert("keepalive".into());
        load(&mut st, 1, "prog_keepalive", 0);
        stand_at(&mut st, 1);
        let popup = interact(&mut st).unwrap();
        assert_eq!(rows_of(&popup), ["Run keepalive", "Leave it"]);
        assert_eq!(
            summon(&mut st, 1),
            Ok(Summoned::Installed("keepalive", true))
        );
        assert!(st.character.reader[1].is_some(), "the shard stays");
        let line = run_on(&mut st, "keepalive", net).unwrap();
        assert!(line.contains("keepalive on") && line.contains("its hold stays full"));
    }

    #[test]
    fn summoning_paydata_sifts_it() {
        let mut st = in_the_room();
        load(&mut st, 2, "paydata", 0);
        stand_at(&mut st, 2);
        let popup = interact(&mut st).unwrap();
        assert_eq!(rows_of(&popup), ["Sift it · 6h", "Leave it"]);
        let n = match summon(&mut st, 2) {
            Ok(Summoned::Sifting(n)) => n,
            other => panic!("{other:?}"),
        };
        assert!(st.character.reader[2].is_none());
        assert!(
            (st.backdoors.jobs.running.iter())
                .any(|j| j.n == n && matches!(j.kind, JobKind::Sift { .. }))
        );
        assert_eq!(st.character.count(&ItemId::new("paydata")), 0);
        // Every seat busy: the row is greyed and the shard stays.
        load(&mut st, 2, "paydata", 0);
        let seats = crate::rig::of(&st).seats as usize;
        while st.backdoors.jobs.running.len() < seats {
            jobs::start(&mut st, JobKind::Rent).expect("a seat");
        }
        let popup = interact(&mut st).unwrap();
        let sift = &popup.choices()[0];
        assert!(!sift.enabled);
        assert_eq!(rows_of(&popup)[0], "Sift it · 6h · every seat is busy");
        assert!(summon(&mut st, 2).is_err());
        assert!(st.character.reader[2].is_some(), "the shard stays");
    }

    #[test]
    fn summoning_the_ledger_opens_its_reading() {
        let mut st = reading_state(in_the_room(), &["hacking"]);
        let shard = ItemId::new("ledger_shard");
        let c = st.content.clone();
        st.character.put_item_in_reader(&c, 0, &shard).unwrap();
        stand_at(&mut st, 0);
        let popup = interact(&mut st).unwrap();
        assert_eq!(rows_of(&popup), ["Read", "Leave it"]);
        assert_eq!(summon(&mut st, 0), Ok(Summoned::Read(shard.clone())));
        let mut card = card(&mut st, 0, &shard).expect("the HOLO card");
        assert!(st.missions.read.contains("ledger_shard"));
        assert_eq!(st.character.count(&shard), 0);
        let cmd = card.update(&mut st, &[key(KeyCode::Char(' '))]);
        assert!(matches!(cmd, ScreenCmd::Pop { .. }));
        assert_eq!(
            st.character.count(&shard),
            1,
            "the story finds it in the pack"
        );
        assert!(st.character.reader[0].is_none());
    }

    #[test]
    fn space_away_from_the_reader_does_nothing() {
        let mut st = in_the_room();
        load(&mut st, 0, "credchip", 340);
        // On the landing, off every box.
        assert!(interact(&mut st).is_none());
        // At an empty slot too.
        stand_at(&mut st, 1);
        assert!(interact(&mut st).is_none());
    }
}
