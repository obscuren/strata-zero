//! Rolling a job from the city: a flat with a shard, a server, a host, a person.

use super::ui::{district_of_fixer, fixer_name};
use super::{
    DELIVER_MIN_RANGE, DELIVER_ON_SHIFT, FETCH_RANGE, HOURS_COPY, HOURS_DELIVER, HOURS_FETCH,
    HOURS_PLANT, Job, JobId, Jobs, Kind, KindTag, Objective, PAY_COPY_PER_SECURITY,
    PAY_DELIVER_BASE, PAY_FETCH, PAY_PLANT_BASE, PAY_PLANT_PER_SECURITY, PAY_VAULT_PER_SECURITY,
    PLANT_AT, State, VAULT_AT,
};
use crate::app::AppState;
use citygen::construct::{GREY_THRESHOLD, ICE_THRESHOLD};
use citygen::net::NodeKind;
use citygen::rng::hash64;
use citygen::{BuildingId, FloorPlan, Place, Pos, RoomKind};
use game::ItemId;
use sim::{PersonId, Work};

const SALT_KIND: u64 = 0x10B_0001;
const SALT_TARGET: u64 = 0x10B_0002;
const SALT_FLAT: u64 = 0x10B_0003;
const SALT_TEXT: u64 = 0x10B_0004;
const SALT_VAULT: u64 = 0x10B_0005;
/// Buildings a Fetch tries before giving up on the kind.
const FETCH_TRIES: usize = 4;

/// One seeded roll for a fixer's `counter`th offer.
pub fn roll(seed: u64, fixer: PersonId, counter: u32, salt: u64) -> u64 {
    hash64(seed, (fixer.0 as u64) ^ ((counter as u64) << 32) ^ salt)
}

fn seed_of(st: &AppState) -> u64 {
    st.city.as_ref().map_or(0, |c| c.seed)
}

pub fn fixer_spot(st: &AppState, fixer: PersonId) -> Option<Pos> {
    st.world.as_ref()?.spot_of(fixer)
}

/// The fixer whose corner is nearest `from`.
pub fn nearest_fixer(st: &AppState, from: Pos) -> Option<PersonId> {
    st.world
        .as_ref()?
        .people()
        .iter()
        .filter(|p| p.profession == "fixer")
        .filter_map(|p| match p.work {
            Work::Spot(pos) => Some((pos.manhattan(from), p.id)),
            _ => None,
        })
        .min_by_key(|(d, id)| (*d, id.0))
        .map(|(_, id)| id)
}

fn door_of(st: &AppState, b: BuildingId) -> Option<Pos> {
    st.city.as_ref()?.portals(b).first().map(|(_, p)| *p)
}

/// How a fetch's door code is to be had in `b`, in the fixer's words: from its server,
/// else from the lobby's card.
pub(crate) fn how_in(st: &AppState, b: BuildingId) -> String {
    if has_node(st, b, NodeKind::Server) {
        st.flavor.how_server()
    } else {
        st.flavor.how_card()
    }
    .to_string()
}

fn has_node(st: &AppState, b: BuildingId, kind: NodeKind) -> bool {
    st.city.as_ref().is_some_and(|c| {
        c.net
            .nodes
            .iter()
            .any(|n| n.building == Some(b) && n.kind == kind)
    })
}

fn lived_in(st: &AppState, b: BuildingId) -> bool {
    st.world
        .as_ref()
        .is_some_and(|w| w.people().iter().any(|p| p.home == Some(b)))
}

/// Whether a shift `(from, to)` in minutes of the day is on at some point within
/// `window` minutes after minute `start` of the day. A shift whose end is before its
/// start wraps past midnight.
pub fn on_shift_within(shift: (u16, u16), start: u64, window: u64) -> bool {
    minutes_on_shift_within(shift, start, window) > 0
}

/// How many minutes of a shift `(from, to)` in minutes of the day are on within `window`
/// minutes (a day at most) after minute `start` of the day. A shift whose end is before
/// its start wraps past midnight.
pub fn minutes_on_shift_within((from, to): (u16, u16), start: u64, window: u64) -> u64 {
    const DAY: u64 = 24 * 60;
    let (from, to) = (from as u64, to as u64);
    let end = if to > from { to } else { to + DAY };
    // Lay the window a day in, so the shift's previous occurrence can be tried too.
    let (s, e) = (start % DAY + DAY, start % DAY + DAY + window);
    (0..=2)
        .map(|k| {
            let (a, b) = (from + k * DAY, end + k * DAY);
            b.min(e).saturating_sub(a.max(s))
        })
        .sum()
}

/// `09:00–20:00`.
pub fn hours_text((from, to): (u16, u16)) -> String {
    format!(
        "{:02}:{:02}–{:02}:{:02}",
        from / 60,
        from % 60,
        to / 60,
        to % 60
    )
}

/// Where to find the fixer again, in their words.
pub fn where_line(st: &AppState, fixer: PersonId) -> String {
    let (from, _) = super::shift_of(st, fixer).unwrap_or((18 * 60, 4 * 60));
    st.flavor.where_line(&[
        ("district", district_of_fixer(st, fixer)),
        ("start", format!("{:02}:{:02}", from / 60, from % 60)),
    ])
}

fn blank(fixer: PersonId, kind: Kind, hours: u64, pay: u32, now: u64) -> Job {
    Job {
        id: JobId(0),
        fixer,
        kind,
        title: String::new(),
        brief: Vec::new(),
        objectives: Vec::new(),
        pay,
        hours,
        offered: now,
        taken: None,
        deadline: None,
        state: State::Offered,
        closed: None,
    }
}

/// Every job's last objective: the fixer's corner and their hours on it.
fn handin_text(st: &AppState, fixer: PersonId) -> String {
    let hours =
        super::shift_of(st, fixer).map_or_else(String::new, |s| format!(" · {}", hours_text(s)));
    format!(
        "Hand it in to {} · {} corner{hours}",
        fixer_name(st, fixer),
        district_of_fixer(st, fixer)
    )
}

/// Lays a shard in a flat of `b` and returns the Fetch kind; `None` when nobody
/// lives there or every flat is yours.
fn fetch_in(st: &mut AppState, b: BuildingId, r: u64) -> Option<Kind> {
    crate::credentials::prepare(st, b);
    let own = st
        .home
        .map(|h| (h.home.building, h.home.level, h.home.number));
    let flats: Vec<(i32, u16)> = st
        .interiors
        .homes_of(b)?
        .flats()
        .filter(|(l, n)| own != Some((b, *l, *n)))
        .collect();
    if flats.is_empty() {
        return None;
    }
    let (level, number) = flats[(r % flats.len() as u64) as usize];
    let city = st.city.as_ref()?;
    let floor = st.interiors.ensure(city, b, level)?;
    let room = floor
        .rooms
        .iter()
        .find(|r| r.kind == RoomKind::Flat && r.number == number)?;
    let tile = room
        .rect
        .iter()
        .find(|p| floor.tiles[*p].is_walkable() && floor.tiles[*p].feature.is_none())?;
    st.ground.add(
        Place::Floor { building: b, level },
        tile,
        ItemId::new("paydata"),
        1,
    );
    Some(Kind::Fetch {
        building: b,
        level,
        number,
        tile,
    })
}

fn finish_fetch(st: &AppState, mut job: Job, r_text: u64) -> Job {
    let Kind::Fetch {
        building,
        level,
        number,
        ..
    } = job.kind
    else {
        return job;
    };
    let name = crate::net::building_name(st, building);
    let how = how_in(st, building);
    let slots = [
        ("building", name.clone()),
        ("floor", FloorPlan::label(level)),
        ("number", number.to_string()),
        ("how", how),
    ];
    job.title = st.flavor.title(KindTag::Fetch, r_text, &slots);
    job.brief = st.flavor.brief(KindTag::Fetch, r_text, &slots);
    job.objectives = vec![
        Objective {
            text: format!(
                "Take the shard from {number} on {} of {name}",
                FloorPlan::label(level)
            ),
            done: false,
        },
        Objective {
            text: handin_text(st, job.fixer),
            done: false,
        },
    ];
    job
}

/// Buildings with a server whose security fits the standing.
fn servers_in_band(st: &AppState, standing: i32) -> Vec<BuildingId> {
    let Some(city) = st.city.as_ref() else {
        return Vec::new();
    };
    let mut out: Vec<BuildingId> = city
        .districts
        .iter()
        .flat_map(|d| crate::access::scannable(st, d.id))
        .collect();
    out.retain(|b| {
        let s = crate::access::security_of(st, *b);
        if standing < PLANT_AT {
            s < ICE_THRESHOLD
        } else if standing < VAULT_AT {
            s < GREY_THRESHOLD
        } else {
            true
        }
    });
    out
}

/// A job of `kind` from this fixer, rolled from the city; `None` when the city has
/// no target for it. The job comes back `Offered` with its id unset.
pub fn generate(st: &mut AppState, fixer: PersonId, kind: KindTag, counter: u32) -> Option<Job> {
    let seed = seed_of(st);
    let now = st.net.now(st);
    let standing = st.jobs.standing(fixer);
    let r_target = roll(seed, fixer, counter, SALT_TARGET);
    let r_text = roll(seed, fixer, counter, SALT_TEXT);
    let spot = fixer_spot(st, fixer)?;
    match kind {
        KindTag::Fetch => {
            let city = st.city.as_ref()?;
            // Somebody must live there: a flat to lay the shard in, and a lost card or
            // a server copy to get its code. By distance from the fixer's corner.
            let mut lived: Vec<(i32, BuildingId)> = city
                .buildings
                .iter()
                .filter(|x| x.kind.is_closed() && x.kind != citygen::BuildingKind::Hotel)
                .filter(|x| lived_in(st, x.id))
                .filter_map(|x| Some((door_of(st, x.id)?.manhattan(spot), x.id)))
                .collect();
            lived.sort_by_key(|(d, id)| (*d, id.index()));
            // The roll picks among the buildings in range; the ones beyond follow, nearest
            // first, so a corner with no homes near it (the Strip keeps none) still works.
            let (near, far): (Vec<_>, Vec<_>) = lived.iter().partition(|(d, _)| *d <= FETCH_RANGE);
            let mut cands: Vec<BuildingId> = near.iter().map(|(_, id)| *id).collect();
            if !cands.is_empty() {
                let start = (r_target % cands.len() as u64) as usize;
                cands.rotate_left(start);
            }
            cands.extend(far.iter().map(|(_, id)| *id));
            if cands.is_empty() {
                return None;
            }
            for (i, b) in cands.into_iter().take(FETCH_TRIES).enumerate() {
                let r_flat = roll(seed, fixer, counter, SALT_FLAT ^ i as u64);
                if let Some(k) = fetch_in(st, b, r_flat) {
                    let job = blank(fixer, k, HOURS_FETCH, PAY_FETCH, now);
                    return Some(finish_fetch(st, job, r_text));
                }
            }
            None
        }
        KindTag::Copy => {
            let servers = servers_in_band(st, standing);
            if servers.is_empty() {
                return None;
            }
            let vault =
                standing >= VAULT_AT && roll(seed, fixer, counter, SALT_VAULT).is_multiple_of(3);
            let pool: Vec<BuildingId> = if vault {
                servers
                    .iter()
                    .copied()
                    .filter(|b| has_node(st, *b, NodeKind::DataVault))
                    .collect()
            } else {
                servers
            };
            if pool.is_empty() {
                return None;
            }
            let b = pool[(r_target % pool.len() as u64) as usize];
            let sec = crate::access::security_of(st, b) as u32;
            let pay = sec
                * if vault {
                    PAY_VAULT_PER_SECURITY
                } else {
                    PAY_COPY_PER_SECURITY
                };
            let host = crate::helix::hosts::building_hostname(st, b);
            let slots = [("host", host.clone()), ("security", sec.to_string())];
            let mut job = blank(
                fixer,
                Kind::Copy { building: b, vault },
                HOURS_COPY,
                pay,
                now,
            );
            job.title = st.flavor.title(KindTag::Copy, r_text, &slots);
            job.brief = st.flavor.brief(KindTag::Copy, r_text, &slots);
            job.objectives = vec![
                Objective {
                    text: format!("Copy {host}'s {}", if vault { "vault" } else { "server" }),
                    done: false,
                },
                Objective {
                    text: handin_text(st, fixer),
                    done: false,
                },
            ];
            Some(job)
        }
        KindTag::Plant => {
            let pool: Vec<BuildingId> = servers_in_band(st, standing)
                .into_iter()
                .filter(|b| {
                    st.backdoors.held(*b).is_none() && st.backdoors.can_plant(*b, now).is_ok()
                })
                .collect();
            if pool.is_empty() {
                return None;
            }
            let b = pool[(r_target % pool.len() as u64) as usize];
            let sec = crate::access::security_of(st, b) as u32;
            let host = crate::helix::hosts::building_hostname(st, b);
            let slots = [("host", host.clone()), ("security", sec.to_string())];
            let mut job = blank(
                fixer,
                Kind::Plant { building: b },
                HOURS_PLANT,
                PAY_PLANT_BASE + sec * PAY_PLANT_PER_SECURITY,
                now,
            );
            job.title = st.flavor.title(KindTag::Plant, r_text, &slots);
            job.brief = st.flavor.brief(KindTag::Plant, r_text, &slots);
            job.objectives = vec![
                Objective {
                    text: format!("Plant a backdoor on {host}"),
                    done: false,
                },
                Objective {
                    text: handin_text(st, fixer),
                    done: false,
                },
            ];
            Some(job)
        }
        KindTag::Deliver => {
            let world = st.world.as_ref()?;
            let city = st.city.as_ref()?;
            let mine = st.home.and_then(|h| {
                st.interiors
                    .homes_of(h.home.building)?
                    .household_of(h.home.level, h.home.number)
            });
            let start = st.minute_of_day() as u64;
            let pool: Vec<(PersonId, Pos)> = world
                .people()
                .iter()
                .filter(|p| p.profession != "fixer" && p.id != fixer && Some(p.household) != mine)
                .filter(|p| !world.is_dead(p.id))
                // At their station at some point inside the window.
                .filter(|p| on_shift_within(p.shift, start, HOURS_DELIVER * 60))
                .filter_map(|p| match p.work {
                    Work::Spot(pos) => Some((p.id, pos)),
                    Work::Building(b)
                        if city.building_opt(b).is_some_and(|x| x.kind.is_closed()) =>
                    {
                        Some((p.id, door_of(st, b)?))
                    }
                    _ => None,
                })
                .filter(|(_, pos)| pos.manhattan(spot) >= DELIVER_MIN_RANGE)
                // someone from out of town is out of reach once they walk out: enough of
                // their shift in the window to walk to them
                .filter(|(id, pos)| {
                    world.comes_in_by(*id).is_none() || {
                        let walk = u64::from(sim::commute::walk_minutes(spot, *pos));
                        let shift = world.work_hours(*id).unwrap_or((0, 0));
                        let on = minutes_on_shift_within(shift, start, HOURS_DELIVER * 60);
                        on >= DELIVER_ON_SHIFT.max(walk)
                    }
                })
                .collect();
            if pool.is_empty() {
                return None;
            }
            let (to, at) = pool[(r_target % pool.len() as u64) as usize];
            let p = &world.people()[to.0 as usize];
            let place = match p.work {
                Work::Building(b) => crate::net::building_name(st, b),
                _ => "the corner".to_string(),
            };
            let hours = hours_text(world.work_hours(to).unwrap_or((0, 0)));
            let profession = world.card(to, city).profession;
            let slots = [
                ("name", p.name.clone()),
                ("profession", profession),
                ("place", place.clone()),
                ("hours", hours.clone()),
            ];
            let pay = PAY_DELIVER_BASE + (at.manhattan(spot) / 4) as u32;
            let mut job = blank(
                fixer,
                Kind::Deliver {
                    to,
                    item: ItemId::new("sealed_package"),
                },
                HOURS_DELIVER,
                pay,
                now,
            );
            job.title = st.flavor.title(KindTag::Deliver, r_text, &slots);
            job.brief = st.flavor.brief(KindTag::Deliver, r_text, &slots);
            job.objectives = vec![
                Objective {
                    text: format!("Deliver the package to {} · {place} · {hours}", p.name),
                    done: false,
                },
                Objective {
                    text: handin_text(st, fixer),
                    done: false,
                },
            ];
            Some(job)
        }
    }
}

/// The fixer's standing offer, rolled now if they have none and nothing in hand.
pub fn offer(st: &mut AppState, fixer: PersonId) -> Option<JobId> {
    if let Some(j) = st.jobs.offered(fixer) {
        return Some(j.id);
    }
    if st.jobs.taken(fixer).is_some() {
        return None;
    }
    let allowed = Jobs::allowed_for(st.jobs.standing(fixer), crate::jobs::has_deck(st));
    if allowed.is_empty() {
        return None;
    }
    let counter = st.jobs.fixer_mut(fixer).rolled;
    st.jobs.fixer_mut(fixer).rolled += 1;
    let first = (roll(seed_of(st), fixer, counter, SALT_KIND) % allowed.len() as u64) as usize;
    for i in 0..allowed.len() {
        let kind = allowed[(first + i) % allowed.len()];
        if let Some(job) = generate(st, fixer, kind, counter) {
            return Some(st.jobs.push(job));
        }
    }
    None
}

/// The first job: a neighbour's flat, the code in the mail, already taken. Once. New
/// games start with the story's drop instead; the jobs' tests still start with this.
#[cfg(test)]
pub fn first_favour(st: &mut AppState) {
    if !st.jobs.list.is_empty() {
        return;
    }
    let now = st.net.now(st);
    if st.backdoors.fixer_quiet(now) {
        return;
    }
    let Some(rig) = st.home else {
        return;
    };
    let b = rig.home.building;
    let Some(door) = door_of(st, b) else {
        return;
    };
    let Some(fixer) = nearest_fixer(st, door) else {
        return;
    };
    let seed = seed_of(st);
    let Some(kind) = fetch_in(st, b, hash64(seed, b.index() as u64 ^ 0xFA50)) else {
        return;
    };
    let Kind::Fetch { level, number, .. } = kind else {
        return;
    };
    let code = citygen::rng::door_code(seed, b, level, number);
    let household = st
        .interiors
        .homes_of(b)
        .and_then(|h| h.household_of(level, number));
    st.journal.learn(crate::credentials::Code {
        building: b,
        level,
        number,
        household,
        code,
        source: crate::credentials::Source::Mail,
        noted_by: None,
        at: now,
    });
    let mut job = finish_fetch(st, blank(fixer, kind, HOURS_FETCH, PAY_FETCH, now), 0);
    job.title = "A favour".to_string();
    let first = fixer_name(st, fixer);
    job.brief = vec![
        format!(
            "Welcome to the block, choom. {number} on {} belongs to a guy who owes me.",
            FloorPlan::label(level)
        ),
        format!("Door code {code:04}. There's a shard in there with my name on it."),
        "Bring it.".to_string(),
    ];
    let mut body = job.brief.clone();
    body.push(where_line(st, fixer));
    body.push(first.clone());
    let id = st.jobs.push(job);
    st.jobs.take(id, now);
    st.journal.mails.push(crate::credentials::Mail {
        from: first,
        subject: "a favour".to_string(),
        body,
        read: false,
        at: now,
        job: Some(id),
        story: None,
    });
    // In the inbox as the game starts: the map has nothing to announce.
    st.mail_seen = st.journal.mails.len();
}

/// The first job's flat and tile, for the tests and shots that used the favour.
#[cfg(test)]
pub fn first_fetch(st: &AppState) -> Option<(BuildingId, i32, u16, Pos)> {
    st.jobs.list.first().and_then(|j| match j.kind {
        Kind::Fetch {
            building,
            level,
            number,
            tile,
        } => Some((building, level, number, tile)),
        _ => None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::AppState;
    use crate::jobs::testutil::{a_fixer, state_home};
    use crate::jobs::{
        COPY_AT, Kind, KindTag, PAY_COPY_PER_SECURITY, PAY_DELIVER_BASE, PAY_FETCH, PAY_PLANT_BASE,
        PAY_PLANT_PER_SECURITY, PLANT_AT, State,
    };
    use citygen::Place;
    use game::ItemId;

    #[test]
    fn the_same_counter_gives_the_same_job() {
        let mut a = state_home();
        let mut b = state_home();
        let fx = a_fixer(&a);
        let ja = generate(&mut a, fx, KindTag::Fetch, 3).expect("a fetch");
        let jb = generate(&mut b, fx, KindTag::Fetch, 3).expect("a fetch");
        assert_eq!(
            (&ja.kind, &ja.title, &ja.brief),
            (&jb.kind, &jb.title, &jb.brief)
        );
        let jc = generate(&mut a, fx, KindTag::Fetch, 4).expect("a fetch");
        assert!(
            jc.kind != ja.kind || jc.title != ja.title,
            "a new counter rolls afresh"
        );
    }

    #[test]
    fn fetch_lays_a_shard_in_a_flat_that_is_not_yours() {
        let mut st = state_home();
        let fx = a_fixer(&st);
        let home = st.home.unwrap().home;
        for counter in 0..6 {
            let j = generate(&mut st, fx, KindTag::Fetch, counter).expect("a fetch");
            let Kind::Fetch {
                building,
                level,
                number,
                tile,
            } = j.kind
            else {
                panic!("{:?}", j.kind)
            };
            assert!((building, level, number) != (home.building, home.level, home.number));
            let place = Place::Floor { building, level };
            assert!(
                st.ground
                    .at(place, tile)
                    .iter()
                    .any(|s| s.item == ItemId::new("paydata"))
            );
            assert_eq!(j.pay, PAY_FETCH);
            assert_eq!(j.objectives.len(), 2);
            assert!(
                j.objectives[0].text.starts_with("Take the shard from"),
                "{}",
                j.objectives[0].text
            );
            assert!(!j.brief.iter().any(|l| l.contains('{')), "{:?}", j.brief);
            assert!(!j.title.contains('{'), "{}", j.title);
        }
    }

    #[test]
    fn copy_and_plant_pick_servers_in_the_standing_band() {
        let mut st = state_home();
        let fx = a_fixer(&st);
        st.jobs.fixer_mut(fx).standing = COPY_AT;
        let j = generate(&mut st, fx, KindTag::Copy, 0).expect("a copy");
        let Kind::Copy { building, vault } = j.kind else {
            panic!()
        };
        assert!(!vault);
        let sec = crate::access::security_of(&st, building);
        assert!(sec < citygen::construct::ICE_THRESHOLD, "{sec}");
        assert_eq!(j.pay, sec as u32 * PAY_COPY_PER_SECURITY);
        st.jobs.fixer_mut(fx).standing = PLANT_AT;
        let p = generate(&mut st, fx, KindTag::Plant, 0).expect("a plant");
        let Kind::Plant { building } = p.kind else {
            panic!()
        };
        let sec = crate::access::security_of(&st, building);
        assert!(sec < citygen::construct::GREY_THRESHOLD, "{sec}");
        assert_eq!(p.pay, PAY_PLANT_BASE + sec as u32 * PAY_PLANT_PER_SECURITY);
        assert_eq!(
            p.objectives[1].text,
            format!(
                "Hand it in to {} · {} corner · 18:00–04:00",
                crate::jobs::ui::fixer_name(&st, fx),
                crate::jobs::ui::district_of_fixer(&st, fx)
            )
        );
        // Every kind hands in the same way.
        st.jobs.fixer_mut(fx).standing = 0;
        for k in [KindTag::Fetch, KindTag::Copy, KindTag::Deliver] {
            st.jobs.fixer_mut(fx).standing = COPY_AT;
            let j = generate(&mut st, fx, k, 0).expect("a job");
            assert_eq!(
                j.objectives.last().unwrap().text,
                p.objectives[1].text,
                "{k:?}"
            );
        }
    }

    #[test]
    fn deliver_names_someone_far_enough_with_their_hours() {
        let mut st = state_home();
        let fx = a_fixer(&st);
        let j = generate(&mut st, fx, KindTag::Deliver, 0).expect("a deliver");
        let Kind::Deliver { to, item } = j.kind.clone() else {
            panic!()
        };
        assert_eq!(item, ItemId::new("sealed_package"));
        let world = st.world.as_ref().unwrap();
        let p = &world.people()[to.0 as usize];
        assert_ne!(p.profession, "fixer");
        assert!(
            j.objectives[0]
                .text
                .starts_with(&format!("Deliver the package to {}", p.name)),
            "{}",
            j.objectives[0].text
        );
        assert!(
            j.objectives[0].text.contains('–'),
            "hours as 09:00–20:00: {}",
            j.objectives[0].text
        );
        assert!(j.pay >= PAY_DELIVER_BASE);
    }

    #[test]
    fn the_dead_get_no_deliveries() {
        let mut st = state_home();
        let fx = a_fixer(&st);
        let j = generate(&mut st, fx, KindTag::Deliver, 0).expect("a deliver");
        let Kind::Deliver { to, .. } = j.kind else {
            panic!()
        };
        st.world.as_mut().unwrap().kill(to);
        for counter in 0..=30 {
            if let Some(j) = generate(&mut st, fx, KindTag::Deliver, counter)
                && let Kind::Deliver { to: named, .. } = j.kind
            {
                assert_ne!(named, to, "counter {counter}");
            }
        }
    }

    #[test]
    fn offer_keeps_one_standing_offer_per_fixer() {
        let mut st = state_home();
        let fx = a_fixer(&st);
        // The welcome may be this fixer's; clear it so they can offer.
        let welcome = st.jobs.active().unwrap().id;
        st.jobs.drop_job(welcome, 0);
        st.jobs.fixer_mut(fx).standing = 0;
        let a = offer(&mut st, fx).expect("an offer");
        let b = offer(&mut st, fx).expect("the same offer");
        assert_eq!(a, b);
        assert_eq!(st.jobs.get(a).unwrap().state, State::Offered);
        assert!(
            matches!(
                st.jobs.get(a).unwrap().kind,
                Kind::Fetch { .. } | Kind::Deliver { .. }
            ),
            "standing 0 offers fetch or deliver"
        );
        st.jobs.take(a, 0);
        assert_eq!(offer(&mut st, fx), None, "one job in hand, no second offer");
    }

    #[test]
    fn the_first_favour_is_a_taken_fetch_with_its_mail() {
        // state_home runs first_favour after move_in.
        let mut st = state_home();
        let j = st.jobs.active().expect("the favour in hand").clone();
        let Kind::Fetch {
            building,
            level,
            number,
            tile,
        } = j.kind
        else {
            panic!()
        };
        let home = st.home.unwrap().home;
        assert_eq!(building, home.building);
        assert!((level, number) != (home.level, home.number));
        assert_eq!(first_fetch(&st), Some((building, level, number, tile)));
        let place = Place::Floor { building, level };
        assert!(
            st.ground
                .at(place, tile)
                .iter()
                .any(|s| s.item == ItemId::new("paydata"))
        );
        assert_eq!(st.journal.mails.len(), 1);
        assert_eq!(st.journal.mails[0].subject, "a favour");
        let code = st
            .journal
            .code_for(building, level, number)
            .expect("the code");
        assert_eq!(code.source, crate::credentials::Source::Mail);
        assert!(
            st.journal.mails[0]
                .body
                .iter()
                .any(|l| l.contains(&format!("Door code {:04}", code.code)))
        );
        assert!(
            st.journal.mails[0]
                .body
                .iter()
                .any(|l| l.contains("I hold the corner on"))
        );
        assert_eq!(
            st.journal.mails[0].from,
            crate::jobs::ui::fixer_name(&st, j.fixer)
        );
        assert_eq!(j.title, "A favour");
        // A load wakes you again: nothing doubles.
        first_favour(&mut st);
        crate::credentials::move_in(&mut st);
        assert_eq!(st.journal.mails.len(), 1);
        assert_eq!(st.jobs.list.len(), 1);
        assert_eq!(
            st.ground.at(place, tile).iter().map(|s| s.qty).sum::<u32>(),
            1
        );
    }

    #[test]
    fn fetch_never_picks_your_own_flat() {
        let mut st = state_home();
        let fx = a_fixer(&st);
        let home = st.home.unwrap().home;
        for counter in 0..20 {
            if let Some(j) = generate(&mut st, fx, KindTag::Fetch, counter)
                && let Kind::Fetch {
                    building,
                    level,
                    number,
                    ..
                } = j.kind
            {
                assert!(
                    (building, level, number) != (home.building, home.level, home.number),
                    "counter {counter}"
                );
            }
        }
    }

    #[test]
    fn first_favour_on_a_city_without_a_strip() {
        // A four-district city without a Strip or a Market (the sim's sweep proves
        // such seeds exist); the fallback fixer carries the welcome.
        for seed in [1u64, 2, 3, 4, 5, 6, 7, 11, 13, 17] {
            let mut st = AppState::new();
            let city = citygen::Generator::run(citygen::GenParams {
                seed,
                size: citygen::SizePreset::Custom { w: 160, h: 80 },
                districts: 4,
                ..Default::default()
            })
            .unwrap();
            let plain = !city.districts.iter().any(|d| {
                matches!(
                    d.theme,
                    citygen::world::Theme::NeonStrip | citygen::world::Theme::NightMarket
                )
            });
            st.enter_city(city);
            let Some(rig) = st.home else {
                continue;
            };
            crate::credentials::prepare(&mut st, rig.home.building);
            crate::credentials::move_in(&mut st);
            first_favour(&mut st);
            if plain {
                assert!(
                    st.jobs.active().is_some(),
                    "seed {seed}: the welcome needs a fixer"
                );
                return;
            }
        }
        panic!("no swept seed had a megablock and no Strip; widen the sweep");
    }

    #[test]
    fn deliver_names_someone_on_shift_inside_the_window() {
        let mut st = state_home();
        let fx = a_fixer(&st);
        let start = st.minute_of_day() as u64;
        for counter in 0..30 {
            let j = generate(&mut st, fx, KindTag::Deliver, counter).expect("a deliver");
            let Kind::Deliver { to, .. } = j.kind else {
                panic!()
            };
            let shift = st.world.as_ref().unwrap().work_hours(to).unwrap();
            assert!(
                on_shift_within(shift, start, crate::jobs::HOURS_DELIVER * 60),
                "counter {counter}: {shift:?} is never on inside {start} + 8h"
            );
        }
        assert!(
            on_shift_within((1080, 240), 1320, 480),
            "a night beat at 22:00"
        );
        assert!(
            !on_shift_within((360, 840), 1320, 480),
            "the 06–14 shift at 22:00"
        );
        assert!(
            on_shift_within((360, 840), 1320, 600),
            "but it is at 22:00 + 10h"
        );
        assert!(on_shift_within((480, 1080), 600, 60), "mid-shift");
    }

    /// Minutes of shift `(from, to)` inside `window` minutes after minute `start` of the
    /// day, counted a minute at a time.
    fn minutes_on(shift: (u16, u16), start: u64, window: u64) -> u64 {
        let (from, to) = (u64::from(shift.0), u64::from(shift.1));
        (start..start + window)
            .filter(|m| {
                let m = m % (24 * 60);
                if from < to {
                    (from..to).contains(&m)
                } else {
                    m >= from || m < to
                }
            })
            .count() as u64
    }

    #[test]
    fn deliver_to_a_commuter_leaves_an_hour_of_their_shift() {
        // someone who comes in from out of town is in reach only on their shift: a
        // package for them leaves at least an hour of it inside the job's window, at
        // whatever hour the fixer rolls it
        let mut st = state_home();
        let fx = a_fixer(&st);
        let people = sim::People::load().unwrap();
        let window = crate::jobs::HOURS_DELIVER * 60;
        let mut commuters = 0;
        for minute in (0..24 * 60).step_by(25) {
            let city = st.city.as_ref().unwrap();
            let w = sim::World::seed(city, &people, &sim::SimParams::default(), minute, 0);
            st.world = Some(w);
            let start = u64::from(minute);
            for counter in 0..20 {
                let Some(j) = generate(&mut st, fx, KindTag::Deliver, counter) else {
                    continue;
                };
                let Kind::Deliver { to, .. } = j.kind else {
                    panic!()
                };
                let w = st.world.as_ref().unwrap();
                if w.comes_in_by(to).is_none() {
                    continue;
                }
                commuters += 1;
                let shift = w.work_hours(to).unwrap();
                let on = minutes_on(shift, start, window);
                assert!(
                    on >= 60,
                    "at {minute}: {} comes in {shift:?}, {on} minutes of it inside the job",
                    w.people()[to.0 as usize].name
                );
            }
        }
        assert!(commuters >= 10, "{commuters} packages for commuters");
    }

    #[test]
    fn every_fixer_can_offer_a_fetch() {
        let mut st = state_home();
        let fixers: Vec<sim::PersonId> = st
            .world
            .as_ref()
            .unwrap()
            .people()
            .iter()
            .filter(|p| p.profession == "fixer")
            .map(|p| p.id)
            .collect();
        assert!(fixers.len() >= 2, "seed 8 has a Market and a Strip");
        for fx in fixers {
            let j = generate(&mut st, fx, KindTag::Fetch, 0);
            assert!(j.is_some(), "fixer {fx:?} rolls no fetch");
        }
    }

    #[test]
    fn no_net_jobs_without_a_deck() {
        let mut st = state_home();
        let fx = a_fixer(&st);
        st.jobs.fixer_mut(fx).standing = crate::jobs::VAULT_AT;
        st.character.equipment.remove(&game::Slot::Deck);
        assert!(!crate::jobs::has_deck(&st));
        assert_eq!(
            Jobs::allowed_for(crate::jobs::VAULT_AT, false),
            vec![KindTag::Fetch, KindTag::Deliver]
        );
        for _ in 0..12 {
            let welcome: Vec<_> = st.jobs.list.iter().map(|j| j.id).collect();
            for id in welcome {
                st.jobs.drop_job(id, 0);
            }
            st.jobs.list.retain(|j| j.state != State::Offered);
            if let Some(id) = offer(&mut st, fx) {
                let k = st.jobs.get(id).unwrap().kind.tag();
                assert!(matches!(k, KindTag::Fetch | KindTag::Deliver), "{k:?}");
            }
        }
        st.character
            .equipment
            .insert(game::Slot::Deck, ItemId::new("pawnshop_deck"));
        assert!(crate::jobs::has_deck(&st));
        assert_eq!(
            Jobs::allowed_for(crate::jobs::VAULT_AT, true),
            Jobs::allowed(crate::jobs::VAULT_AT)
        );
        assert!(Jobs::allowed_for(crate::jobs::VAULT_AT, true).contains(&KindTag::Plant));
    }
}
