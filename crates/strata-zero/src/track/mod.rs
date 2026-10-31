//! Tracking: one target at a time, its walk drawn on the map. This is what a track is
//! made of: where it came from (`Source`), what that points at now (`Target`), and the
//! pure functions that turn one into the other. Spec
//! `docs/superpowers/specs/2026-10-31-tracking-and-map-design.md`, 1.

use crate::app::AppState;
use crate::jobs::{JobId, Kind, State};
use crate::missions::{RunState, Trigger, Where};
use citygen::{BuildingId, Place, Pos, StratumKind};
use route::{Leg, NoLeg};
use sim::{PersonId, Whereabouts};

pub mod listings;
pub mod overlay;
pub mod route;
pub mod ui;

/// A row of the map's listings, as a thing that can be tracked.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ListingId {
    Building(BuildingId),
    Person(PersonId),
    Job(JobId),
    Story(usize),
}

/// What a track was made from; it follows this, not the place it first pointed at.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Source {
    /// A taken job's objective `objective` (its index when tracked).
    Job { job: JobId, objective: usize },
    /// A story mission's waiting step `step` (its index when tracked).
    Story { mission: usize, step: usize },
    /// A listing from the map.
    Listing(ListingId),
    /// A tile picked on the map (or from the console), arrived within `radius`.
    Spot { place: Place, pos: Pos, radius: i32 },
}

/// Where a source points right now.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Target {
    Spot { place: Place, pos: Pos, radius: i32 },
    Building(BuildingId),
    Person(PersonId),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Resolved {
    Target(Target),
    /// The source ended; the text is why, without the label.
    Gone(String),
    /// A story step with no place to walk to.
    Nowhere,
}

/// The one thing being tracked. Made only by `track::set`.
pub struct Track {
    pub source: Source,
    /// What the pane and the log call it: "Miracorp Tattoo", "The drop", "the spot".
    pub label: String,
    walk: Walk,
}

/// The leg as last made, and what it was made from.
struct Walk {
    /// Your place and tile then.
    from: Option<(Place, Pos)>,
    /// The source's target, located.
    target: Target,
    leg: Result<Leg, NoLeg>,
    /// What comes after the leg, for the pane and the tag.
    next: Option<String>,
}

impl Walk {
    /// The leg from where you stand to a located target.
    fn new(st: &AppState, target: Target) -> Walk {
        let leg = route::leg(st, target);
        let next = leg
            .as_ref()
            .ok()
            .and_then(|l| route::next_text(st, l, target));
        Walk {
            from: here(st),
            target,
            leg,
            next,
        }
    }
}

fn here(st: &AppState) -> Option<(Place, Pos)> {
    st.player.as_ref().map(|p| (p.place, p.pos))
}

/// Tracks `source` in place of whatever was tracked, and logs it; refuses, and keeps
/// the old track, when there is nothing to walk to or no way there.
pub fn set(st: &mut AppState, source: Source) -> Result<String, String> {
    let concrete = |r: Resolved| match r {
        Resolved::Target(t) => Ok(t),
        Resolved::Gone(_) => Err("nothing to walk to".to_string()),
        Resolved::Nowhere => Err("nothing to walk to yet".to_string()),
    };
    let target = concrete(resolve(st, &source))?;
    let target = concrete(locate(st, target))?;
    let walk = Walk::new(st, target);
    match walk.leg {
        Err(NoLeg::NoWay) => return Err("no way there from here".to_string()),
        Err(NoLeg::JackedIn) => return Err("not from the net".to_string()),
        Ok(_) => {}
    }
    let label = label(st, &source);
    let text = format!("⌖ tracking {label}");
    st.track = Some(Track {
        source,
        label,
        walk,
    });
    st.say(text.clone());
    Ok(text)
}

/// Stops tracking, and logs it.
#[allow(dead_code)] // the console's `track off` calls it
pub fn stop(st: &mut AppState) {
    end(st, |label| format!("⌖ stopped tracking {label}"));
}

/// Stops `source` when it is the tracked one, else tracks it.
pub fn toggle(st: &mut AppState, source: Source) -> Result<String, String> {
    if is_tracked(st, &source) {
        Ok(end(st, |label| format!("⌖ stopped tracking {label}")).unwrap_or_default())
    } else {
        set(st, source)
    }
}

/// Whether `source` is what is tracked: any objective of the same job, any step of the
/// same story mission.
pub fn is_tracked(st: &AppState, source: &Source) -> bool {
    let Some(t) = st.track.as_ref() else {
        return false;
    };
    match (t.source, *source) {
        (Source::Job { job: a, .. }, Source::Job { job: b, .. }) => a == b,
        (Source::Story { mission: a, .. }, Source::Story { mission: b, .. }) => a == b,
        (a, b) => a == b,
    }
}

/// Drops the track and logs the line `line` makes of its label; the line, or nothing
/// when untracked.
fn end(st: &mut AppState, line: impl FnOnce(&str) -> String) -> Option<String> {
    let t = st.track.take()?;
    let text = line(&t.label);
    st.say(text.clone());
    Some(text)
}

/// The track's beat in the explore update, after the story's (spec 3): cleared when
/// its source ends or its person is gone, or when you arrive (never for the story,
/// whose own trigger decides); else its leg remade when you or the target moved.
pub fn tick(st: &mut AppState) {
    let Some(t) = st.track.as_ref() else {
        return;
    };
    let (source, kept) = (t.source, t.walk.target);
    let target = match resolve(st, &source) {
        Resolved::Target(t) => t,
        Resolved::Gone(why) => {
            end(st, |label| format!("⌖ {label} · {why}"));
            return;
        }
        Resolved::Nowhere => return,
    };
    let located = match locate(st, target) {
        Resolved::Target(t) => t,
        Resolved::Gone(why) => {
            end(st, |label| format!("⌖ {label} · {why}"));
            return;
        }
        // A person out of sight for a moment (on your tile): where they were.
        Resolved::Nowhere => kept,
    };
    let arrived = match source {
        Source::Story { .. } => None,
        _ => arrival(st, target, located, &t.label),
    };
    if let Some(line) = arrived {
        end(st, |_| line);
        return;
    }
    let from = here(st);
    if st
        .track
        .as_ref()
        .is_some_and(|t| t.walk.from == from && t.walk.target == located)
    {
        return;
    }
    let walk = Walk::new(st, located);
    if let Some(t) = st.track.as_mut() {
        t.walk = walk;
    }
}

/// The line to log when you have reached `located`, where the target `target` is now.
fn arrival(st: &AppState, target: Target, located: Target, label: &str) -> Option<String> {
    let p = st.player.as_ref()?;
    let there = || format!("⌖ there · {label}");
    match (target, located) {
        (_, Target::Spot { place, pos, radius }) => {
            (p.place == place && p.pos.chebyshev(pos) <= radius).then(there)
        }
        (Target::Person(who), Target::Building(b)) => in_building(st, p.place, p.pos, b)
            .then(|| format!("⌖ {} is in here", person_name(st, who))),
        (_, Target::Building(b)) => {
            (in_building(st, p.place, p.pos, b) || at_door(st, p.place, p.pos, b)).then(there)
        }
        (_, Target::Person(_)) => None,
    }
}

/// On one of `b`'s floors, or in or beside a walk-in's footprint (a walk-in has no
/// inside of its own, and its leg ends beside it).
fn in_building(st: &AppState, place: Place, pos: Pos, b: BuildingId) -> bool {
    match place {
        Place::Floor { building, .. } => building == b,
        Place::Stratum(k) => st
            .city
            .as_ref()
            .and_then(|c| c.building_opt(b))
            .is_some_and(|x| {
                !x.kind.is_closed()
                    && k != StratumKind::Heights
                    && x.span.contains(k)
                    && x.footprint.grow(1).contains(pos)
            }),
        Place::Construct(_) => false,
    }
}

/// On or beside one of `b`'s doors on this stratum.
fn at_door(st: &AppState, place: Place, pos: Pos, b: BuildingId) -> bool {
    let Place::Stratum(k) = place else {
        return false;
    };
    st.city.as_ref().is_some_and(|c| {
        c.portals(b)
            .iter()
            .any(|(s, door)| *s == k && door.chebyshev(pos) <= 1)
    })
}

/// The leg the last tick made, for drawing.
pub fn leg_now(st: &AppState) -> Option<Result<&Leg, NoLeg>> {
    st.track
        .as_ref()
        .map(|t| t.walk.leg.as_ref().map_err(|e| *e))
}

/// The tracked target as the last tick located it.
pub fn target_now(st: &AppState) -> Option<Target> {
    st.track.as_ref().map(|t| t.walk.target)
}

/// What comes after the leg ("door, then 6F"); nothing when it arrives.
pub fn next_now(st: &AppState) -> Option<&str> {
    st.track.as_ref()?.walk.next.as_deref()
}

/// The leg's length in tiles.
pub fn distance(st: &AppState) -> Option<usize> {
    leg_now(st)?.ok().map(|l| l.path.len())
}

fn done() -> Resolved {
    Resolved::Gone("done with".to_string())
}

/// What the source points at now.
pub fn resolve(st: &AppState, source: &Source) -> Resolved {
    match *source {
        Source::Job { job, objective } => resolve_job(st, job, objective),
        Source::Story { mission, step } => resolve_story(st, mission, step),
        Source::Listing(ListingId::Building(b)) => Resolved::Target(Target::Building(b)),
        Source::Listing(ListingId::Person(p)) => Resolved::Target(Target::Person(p)),
        Source::Listing(ListingId::Job(_) | ListingId::Story(_)) => Resolved::Nowhere,
        Source::Spot { place, pos, radius } => {
            Resolved::Target(Target::Spot { place, pos, radius })
        }
    }
}

fn resolve_job(st: &AppState, id: JobId, objective: usize) -> Resolved {
    let Some(job) = st.jobs.get(id).filter(|j| j.state == State::Taken) else {
        return done();
    };
    match job.objectives.get(objective) {
        Some(o) if !o.done => {}
        _ => return done(),
    }
    if objective > 0 {
        return Resolved::Target(Target::Person(job.fixer));
    }
    Resolved::Target(match job.kind {
        Kind::Fetch {
            building,
            level,
            tile,
            ..
        } => Target::Spot {
            place: Place::Floor { building, level },
            pos: tile,
            radius: 1,
        },
        Kind::Copy { building, .. } | Kind::Plant { building } => Target::Building(building),
        Kind::Deliver { to, .. } => Target::Person(to),
    })
}

fn resolve_story(st: &AppState, mission: usize, step: usize) -> Resolved {
    let live = st
        .missions
        .runs
        .get(mission)
        .is_some_and(|r| r.state == RunState::Open && r.at == step);
    let Some(when) = st
        .book
        .missions
        .get(mission)
        .and_then(|m| m.steps.get(step))
        .map(|s| &s.when)
        .filter(|_| live)
    else {
        return done();
    };
    let street = Place::Stratum(StratumKind::Street);
    let target = match when {
        Trigger::Sees { role, .. } | Trigger::Talk(role) => {
            crate::missions::person_of(st, role).map(Target::Person)
        }
        Trigger::At(Where::Home) => flat_tile(st).map(|(place, pos)| Target::Spot {
            place,
            pos,
            radius: 0,
        }),
        Trigger::At(Where::Work { role, within }) => crate::missions::person_of(st, role)
            .and_then(|p| crate::missions::cast::work_tile(st, p))
            .map(|pos| Target::Spot {
                place: street,
                pos,
                radius: *within,
            }),
        _ => None,
    };
    target.map_or(Resolved::Nowhere, Resolved::Target)
}

/// A `Person` target made concrete: the tile they stand on, the building they are in,
/// or where they lay. Other targets pass through.
pub fn locate(st: &AppState, t: Target) -> Resolved {
    let Target::Person(id) = t else {
        return Resolved::Target(t);
    };
    let Some(world) = st.world.as_ref() else {
        return Resolved::Nowhere;
    };
    if world.is_dead(id) {
        return match st.missions.bodies.iter().find(|b| b.person == id) {
            Some(b) => Resolved::Target(Target::Spot {
                place: b.place,
                pos: b.pos,
                radius: 1,
            }),
            None => Resolved::Gone(format!("{} is gone", person_name(st, id))),
        };
    }
    if let Some(pos) = world.pos_of(id) {
        return Resolved::Target(Target::Spot {
            place: Place::Stratum(StratumKind::Street),
            pos,
            radius: 1,
        });
    }
    match world.people().get(id.0 as usize).map(|p| &p.whereabouts) {
        Some(Whereabouts::Inside(b)) => Resolved::Target(Target::Building(*b)),
        _ => Resolved::Nowhere,
    }
}

fn person_name(st: &AppState, id: PersonId) -> String {
    st.world
        .as_ref()
        .and_then(|w| w.people().get(id.0 as usize))
        .map_or_else(|| "someone".to_string(), |p| p.name.clone())
}

/// What the pane and the log call a source.
pub fn label(st: &AppState, source: &Source) -> String {
    match *source {
        Source::Job { job, .. } | Source::Listing(ListingId::Job(job)) => st
            .jobs
            .get(job)
            .map_or_else(|| "a job".to_string(), |j| j.title.clone()),
        Source::Story { mission, .. } | Source::Listing(ListingId::Story(mission)) => st
            .book
            .missions
            .get(mission)
            .map_or_else(|| "the story".to_string(), |m| m.title.clone()),
        Source::Listing(ListingId::Building(b)) => {
            if crate::clinic::is_clinic(st, b) {
                crate::clinic::name(st, b)
            } else {
                crate::net::building_name(st, b)
            }
        }
        Source::Listing(ListingId::Person(p)) => person_name(st, p),
        Source::Spot { place, pos, .. } => match district_at(st, place, pos) {
            Some(d) => format!("{d} · spot"),
            None => "spot".to_string(),
        },
    }
}

/// The name of the district a tile lies in.
fn district_at(st: &AppState, place: Place, pos: Pos) -> Option<String> {
    let city = st.city.as_ref()?;
    let id = match place {
        Place::Stratum(k) => city.tile(k, pos)?.district?,
        Place::Floor { building, .. } | Place::Construct(building) => {
            city.building_opt(building)?.district
        }
    };
    city.district_opt(id).map(|d| d.name.clone())
}

/// Your flat's floor and the walkable tile inside the flat nearest its door.
pub(crate) fn flat_tile(st: &AppState) -> Option<(Place, Pos)> {
    let rig = st.home.as_ref()?;
    let home = rig.home;
    let (_, floor) = st.interiors.floor(home.building, home.level)?;
    let r = home.rect;
    let pos = (r.y..r.bottom())
        .flat_map(|y| (r.x..r.right()).map(move |x| Pos::new(x, y)))
        .filter(|p| floor.tiles.get(*p).is_some_and(|t| t.is_walkable()))
        .min_by_key(|p| (p.manhattan(home.door), p.y, p.x))?;
    Some((rig.place(), pos))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::{Screen, ScreenCmd};
    use crate::jobs::testutil::{a_fixer, state_home};
    use crate::jobs::{KindTag, generate};
    use crate::render::map::{MapRef, TileMap};
    use crate::screens::explore::MapScreen;
    use citygen::Terrain;
    use std::collections::{HashMap, VecDeque};

    /// A job of this kind, pushed and taken; none when the city offers none.
    fn taken(st: &mut AppState, tag: KindTag) -> Option<JobId> {
        let fixer = a_fixer(st);
        st.jobs.fixer_mut(fixer).standing = 10;
        let job = (0..40).find_map(|n| generate::generate(st, fixer, tag, n))?;
        let id = st.jobs.push(job);
        assert!(st.jobs.take(id, 0));
        Some(id)
    }

    fn job_source(job: JobId, objective: usize) -> Source {
        Source::Job { job, objective }
    }

    #[test]
    fn job_objective_zero_resolves_by_kind() {
        let mut st = state_home();
        let mut seen = 0;
        for tag in [
            KindTag::Fetch,
            KindTag::Copy,
            KindTag::Plant,
            KindTag::Deliver,
        ] {
            let Some(id) = taken(&mut st, tag) else {
                continue;
            };
            seen += 1;
            let want = match st.jobs.get(id).unwrap().kind.clone() {
                Kind::Fetch {
                    building,
                    level,
                    tile,
                    ..
                } => Target::Spot {
                    place: Place::Floor { building, level },
                    pos: tile,
                    radius: 1,
                },
                Kind::Copy { building, .. } | Kind::Plant { building } => {
                    Target::Building(building)
                }
                Kind::Deliver { to, .. } => Target::Person(to),
            };
            assert_eq!(resolve(&st, &job_source(id, 0)), Resolved::Target(want));
        }
        assert!(seen >= 2, "at least Fetch and Deliver are on offer");
    }

    #[test]
    fn job_hand_in_resolves_to_the_fixer() {
        let mut st = state_home();
        let id = taken(&mut st, KindTag::Fetch).unwrap();
        let fixer = st.jobs.get(id).unwrap().fixer;
        assert_eq!(
            resolve(&st, &job_source(id, 1)),
            Resolved::Target(Target::Person(fixer))
        );
    }

    #[test]
    fn job_not_taken_or_objective_ticked_is_gone() {
        let mut st = state_home();
        let id = taken(&mut st, KindTag::Fetch).unwrap();
        st.jobs
            .list
            .iter_mut()
            .find(|j| j.id == id)
            .unwrap()
            .objectives[0]
            .done = true;
        assert_eq!(resolve(&st, &job_source(id, 0)), done());
        assert!(matches!(
            resolve(&st, &job_source(id, 1)),
            Resolved::Target(_)
        ));
        st.jobs.list.retain(|j| j.id != id);
        assert_eq!(resolve(&st, &job_source(id, 0)), done());
        assert_eq!(resolve(&st, &job_source(id, 1)), done());
    }

    /// The first step, over every mission, whose trigger `f` accepts.
    fn step_of(st: &AppState, f: impl Fn(&Trigger) -> bool) -> (usize, usize) {
        st.book
            .missions
            .iter()
            .enumerate()
            .find_map(|(m, x)| x.steps.iter().position(|s| f(&s.when)).map(|i| (m, i)))
            .expect("a step with that trigger")
    }

    fn story(mission: usize, step: usize) -> Source {
        Source::Story { mission, step }
    }

    #[test]
    fn story_steps_resolve_by_trigger() {
        let mut st = state_home();
        crate::missions::begin(&mut st);
        st.missions.feeds.clear();
        crate::missions::end(&mut st, 0, 0);
        let (m, sees) = step_of(&st, |t| matches!(t, Trigger::Sees { .. }));
        let Trigger::Sees { role, .. } = st.book.missions[m].steps[sees].when.clone() else {
            unreachable!()
        };
        let who = crate::missions::person_of(&st, &role).unwrap();
        assert_eq!(st.missions.runs[m].at, sees, "the story waits on Sees");
        assert_eq!(
            resolve(&st, &story(m, sees)),
            Resolved::Target(Target::Person(who))
        );
        // A step the run is not waiting on is gone.
        assert_eq!(resolve(&st, &story(m, sees + 1)), done());

        // The `After` step has no place to walk to.
        let (_, ring) = step_of(&st, |t| matches!(t, Trigger::After { .. }));
        assert!(crate::missions::force(&mut st, m));
        assert_eq!(st.missions.runs[m].at, ring);
        assert_eq!(resolve(&st, &story(m, ring)), Resolved::Nowhere);

        let (_, home) = step_of(&st, |t| matches!(t, Trigger::At(Where::Home)));
        while st.missions.runs[m].at < home {
            assert!(crate::missions::force(&mut st, m));
        }
        let rig = st.home.unwrap();
        let (place, pos) = flat_tile(&st).unwrap();
        assert_eq!(place, rig.place());
        assert!(rig.home.rect.contains(pos));
        assert_eq!(
            resolve(&st, &story(m, home)),
            Resolved::Target(Target::Spot {
                place,
                pos,
                radius: 0
            })
        );
        assert_eq!(resolve(&st, &story(m, sees)), done());
    }

    #[test]
    fn listing_resolves_to_building_or_person() {
        let st = state_home();
        let b = st.home.unwrap().home.building;
        let p = a_fixer(&st);
        assert_eq!(
            resolve(&st, &Source::Listing(ListingId::Building(b))),
            Resolved::Target(Target::Building(b))
        );
        assert_eq!(
            resolve(&st, &Source::Listing(ListingId::Person(p))),
            Resolved::Target(Target::Person(p))
        );
        assert_eq!(
            resolve(&st, &Source::Listing(ListingId::Job(JobId(0)))),
            Resolved::Nowhere
        );
        assert_eq!(
            resolve(&st, &Source::Listing(ListingId::Story(0))),
            Resolved::Nowhere
        );
    }

    #[test]
    fn locate_follows_a_person() {
        let mut st = state_home();
        let fixer = a_fixer(&st);
        let spot = st.world.as_ref().unwrap().spot_of(fixer).unwrap();
        st.world.as_mut().unwrap().park(fixer, spot);
        assert_eq!(
            locate(&st, Target::Person(fixer)),
            Resolved::Target(Target::Spot {
                place: Place::Stratum(StratumKind::Street),
                pos: spot,
                radius: 1
            })
        );
        let (indoors, b) = st
            .world
            .as_ref()
            .unwrap()
            .people()
            .iter()
            .find_map(|p| match p.whereabouts {
                Whereabouts::Inside(b) => Some((p.id, b)),
                _ => None,
            })
            .expect("somebody is indoors");
        assert_eq!(
            locate(&st, Target::Person(indoors)),
            Resolved::Target(Target::Building(b))
        );
        // Spots and buildings pass through.
        let t = Target::Building(b);
        assert_eq!(locate(&st, t), Resolved::Target(t));
        // Killed without a body.
        let name = person_name(&st, fixer);
        st.world.as_mut().unwrap().kill(fixer);
        assert_eq!(
            locate(&st, Target::Person(fixer)),
            Resolved::Gone(format!("{name} is gone"))
        );
    }

    #[test]
    fn locate_finds_a_story_body() {
        let mut st = state_home();
        crate::missions::begin(&mut st);
        st.missions.feeds.clear();
        crate::missions::end(&mut st, 0, 0);
        let body = st.missions.bodies[0].clone();
        assert_eq!(
            locate(&st, Target::Person(body.person)),
            Resolved::Target(Target::Spot {
                place: body.place,
                pos: body.pos,
                radius: 1
            })
        );
    }

    #[test]
    fn labels_read_the_source() {
        let mut st = state_home();
        let id = taken(&mut st, KindTag::Fetch).unwrap();
        let title = st.jobs.get(id).unwrap().title.clone();
        assert_eq!(label(&st, &job_source(id, 0)), title);
        let p = a_fixer(&st);
        assert_eq!(
            label(&st, &Source::Listing(ListingId::Person(p))),
            person_name(&st, p)
        );
        let spot = Source::Spot {
            place: Place::Stratum(StratumKind::Street),
            pos: Pos::new(5, 5),
            radius: 1,
        };
        assert!(label(&st, &spot).ends_with("spot"));
        let nowhere = Source::Spot {
            place: Place::Stratum(StratumKind::Street),
            pos: Pos::new(-1, -1),
            radius: 1,
        };
        assert_eq!(label(&st, &nowhere), "spot");
    }

    #[test]
    fn a_new_city_clears_the_track() {
        let mut st = state_home();
        let (place, pos, radius) = (Place::Stratum(StratumKind::Street), Pos::new(5, 5), 1);
        st.track = Some(Track {
            source: Source::Spot { place, pos, radius },
            label: "the spot".to_string(),
            walk: Walk::new(&st, Target::Spot { place, pos, radius }),
        });
        let city = st.city.clone().unwrap();
        st.enter_city_at(city, 0);
        assert!(st.track.is_none());
    }

    const STREET: Place = Place::Stratum(StratumKind::Street);

    fn put(st: &mut AppState, place: Place, pos: Pos) {
        let p = st.player.as_mut().unwrap();
        p.place = place;
        p.pos = pos;
    }

    /// The home city, awake, you on the street outside the megablock's door.
    pub(super) fn outside() -> AppState {
        let mut st = state_home();
        MapScreen::walking(&mut st);
        let b = st.home.unwrap().home.building;
        let city = st.city.as_ref().unwrap();
        st.interiors.ensure(city, b, 0).unwrap();
        let (_, floor) = st.interiors.floor(b, 0).unwrap();
        let landing = floor
            .exits
            .iter()
            .find(|e| e.stratum == StratumKind::Street)
            .unwrap()
            .landing;
        put(&mut st, STREET, landing);
        st
    }

    pub(super) fn here(st: &AppState) -> Pos {
        st.player.as_ref().unwrap().pos
    }

    /// Steps from `from` to every street tile outside buildings a walk reaches.
    pub(super) fn reach(st: &AppState, from: Pos) -> HashMap<Pos, u32> {
        let map = MapRef::stratum_of(st.city.as_ref().unwrap(), StratumKind::Street);
        let open = |p: Pos| {
            map.tile(p).is_some_and(|t| {
                t.is_walkable() && t.terrain != Terrain::Door && t.building.is_none()
            })
        };
        let mut d = HashMap::from([(from, 0)]);
        let mut q = VecDeque::from([from]);
        while let Some(p) = q.pop_front() {
            for n in p.neighbors4() {
                if open(n) && !d.contains_key(&n) {
                    d.insert(n, d[&p] + 1);
                    q.push_back(n);
                }
            }
        }
        d
    }

    /// A street tile exactly `n` steps from where you stand.
    pub(super) fn steps_off(st: &AppState, n: u32) -> Pos {
        let mut at: Vec<Pos> = reach(st, here(st))
            .into_iter()
            .filter(|(_, k)| *k == n)
            .map(|(p, _)| p)
            .collect();
        at.sort();
        at[0]
    }

    fn street_spot(pos: Pos, radius: i32) -> Source {
        Source::Spot {
            place: STREET,
            pos,
            radius,
        }
    }

    fn last(st: &AppState) -> String {
        st.log.latest().unwrap().text.clone()
    }

    fn tracked(st: &AppState) -> Option<Source> {
        st.track.as_ref().map(|t| t.source)
    }

    #[test]
    fn set_replaces_and_logs() {
        let mut st = outside();
        let (a, b) = (
            street_spot(steps_off(&st, 20), 1),
            street_spot(steps_off(&st, 40), 1),
        );
        let said = set(&mut st, a).unwrap();
        assert_eq!(said, format!("⌖ tracking {}", label(&st, &a)));
        assert_eq!(last(&st), said);
        assert_eq!(tracked(&st), Some(a));
        let said = set(&mut st, b).unwrap();
        assert_eq!(tracked(&st), Some(b));
        assert_eq!(said, format!("⌖ tracking {}", label(&st, &b)));
        assert_eq!(last(&st), said);
        // The leg is made with it.
        assert_eq!(distance(&st), Some(40));
        assert_eq!(
            target_now(&st),
            Some(Target::Spot {
                place: STREET,
                pos: steps_off(&st, 40),
                radius: 1
            })
        );
        assert_eq!(next_now(&st), None);
    }

    #[test]
    fn toggle_on_the_tracked_one_stops() {
        let mut st = outside();
        let a = street_spot(steps_off(&st, 20), 1);
        let name = label(&st, &a);
        assert!(toggle(&mut st, a).is_ok());
        assert!(is_tracked(&st, &a));
        assert_eq!(toggle(&mut st, a), Ok(format!("⌖ stopped tracking {name}")));
        assert_eq!(last(&st), format!("⌖ stopped tracking {name}"));
        assert!(st.track.is_none());
        // Stopping nothing says nothing.
        let n = st.log.len();
        stop(&mut st);
        assert_eq!(st.log.len(), n);
        // A job is tracked by any of its objectives, the story by any of its steps.
        let id = taken(&mut st, KindTag::Fetch).unwrap();
        set(&mut st, job_source(id, 0)).unwrap();
        assert!(is_tracked(&st, &job_source(id, 1)));
        assert!(!is_tracked(&st, &a));
        toggle(&mut st, job_source(id, 1)).unwrap();
        assert!(st.track.is_none());
        crate::missions::begin(&mut st);
        st.missions.feeds.clear();
        crate::missions::end(&mut st, 0, 0);
        let at = st.missions.runs[0].at;
        set(&mut st, story(0, at)).unwrap();
        assert!(is_tracked(&st, &story(0, at + 3)));
        assert!(!is_tracked(&st, &job_source(id, 0)));
    }

    #[test]
    fn set_refuses_and_keeps_the_old() {
        let mut st = outside();
        let a = street_spot(steps_off(&st, 20), 1);
        set(&mut st, a).unwrap();
        // The story waiting on its ring has nowhere to walk to.
        crate::missions::begin(&mut st);
        st.missions.feeds.clear();
        crate::missions::end(&mut st, 0, 0);
        let (_, ring) = step_of(&st, |t| matches!(t, Trigger::After { .. }));
        assert!(crate::missions::force(&mut st, 0));
        assert_eq!(st.missions.runs[0].at, ring);
        let n = st.log.len();
        assert_eq!(
            set(&mut st, story(0, ring)),
            Err("nothing to walk to yet".to_string())
        );
        assert_eq!(tracked(&st), Some(a));
        // A step it is not waiting on is done with.
        assert_eq!(
            set(&mut st, story(0, ring + 1)),
            Err("nothing to walk to".to_string())
        );
        // A spot walled off.
        let to = steps_off(&st, 30);
        let city = st.city.as_mut().unwrap();
        for p in to.neighbors8() {
            city.tile_mut(StratumKind::Street, p).unwrap().terrain = Terrain::Wall;
        }
        assert_eq!(
            set(&mut st, street_spot(to, 1)),
            Err("no way there from here".to_string())
        );
        assert_eq!(tracked(&st), Some(a));
        // Jacked in.
        let at = here(&st);
        let b = st.home.unwrap().home.building;
        put(&mut st, Place::Construct(b), Pos::new(1, 1));
        assert_eq!(
            set(&mut st, street_spot(at, 1)),
            Err("not from the net".to_string())
        );
        assert_eq!(tracked(&st), Some(a));
        assert_eq!(st.log.len(), n, "a refusal logs nothing here");
    }

    #[test]
    fn arriving_at_a_spot_clears() {
        let mut st = outside();
        let to = steps_off(&st, 30);
        let a = street_spot(to, 2);
        let name = label(&st, &a);
        set(&mut st, a).unwrap();
        let near = reach(&st, here(&st));
        let at = |d: i32| {
            let mut v: Vec<Pos> = near
                .keys()
                .copied()
                .filter(|p| p.chebyshev(to) == d)
                .collect();
            v.sort();
            v[0]
        };
        put(&mut st, STREET, at(3));
        tick(&mut st);
        assert!(st.track.is_some(), "three off is not there");
        put(&mut st, STREET, at(2));
        tick(&mut st);
        assert!(st.track.is_none());
        assert_eq!(last(&st), format!("⌖ there · {name}"));
    }

    #[test]
    fn arriving_at_a_building_clears() {
        let mut st = outside();
        let start = here(&st);
        let clinic = st
            .city
            .as_ref()
            .unwrap()
            .buildings
            .iter()
            .map(|b| b.id)
            .find(|b| crate::clinic::is_clinic(&st, *b))
            .unwrap();
        let src = Source::Listing(ListingId::Building(clinic));
        let name = label(&st, &src);
        set(&mut st, src).unwrap();
        tick(&mut st);
        assert!(st.track.is_some());
        // Beside its door.
        let city = st.city.as_ref().unwrap();
        let fp = city.building(clinic).footprint;
        let door = city
            .portals(clinic)
            .into_iter()
            .find(|(k, _)| *k == StratumKind::Street)
            .unwrap()
            .1;
        let beside = door
            .neighbors4()
            .find(|p| !fp.contains(*p) && city.walkable(StratumKind::Street, *p))
            .unwrap();
        put(&mut st, STREET, beside);
        tick(&mut st);
        assert!(st.track.is_none());
        assert_eq!(last(&st), format!("⌖ there · {name}"));
        // On one of its floors: the home megablock, from down the street.
        put(&mut st, STREET, start);
        let far = steps_off(&st, 40);
        put(&mut st, STREET, far);
        let home = st.home.unwrap().home.building;
        let src = Source::Listing(ListingId::Building(home));
        let name = label(&st, &src);
        set(&mut st, src).unwrap();
        tick(&mut st);
        assert!(st.track.is_some());
        let lift = st.interiors.floor(home, 0).unwrap().0.lift.unwrap();
        put(
            &mut st,
            Place::Floor {
                building: home,
                level: 0,
            },
            lift,
        );
        tick(&mut st);
        assert!(st.track.is_none());
        assert_eq!(last(&st), format!("⌖ there · {name}"));
    }

    #[test]
    fn a_person_inside_clears_on_entering() {
        let mut st = outside();
        let home = st.home.unwrap().home.building;
        let indoors: Vec<(PersonId, BuildingId)> = st
            .world
            .as_ref()
            .unwrap()
            .people()
            .iter()
            .filter_map(|p| match p.whereabouts {
                Whereabouts::Inside(b) if b != home => Some((p.id, b)),
                _ => None,
            })
            .collect();
        let city = st.city.as_ref().unwrap();
        let (who, b, door) = indoors
            .into_iter()
            .find_map(|(who, b)| {
                let closed = city.building(b).kind.is_closed();
                let door = city
                    .portals(b)
                    .into_iter()
                    .find(|(k, _)| *k == StratumKind::Street)?;
                (closed && !crate::clinic::is_clinic(&st, b)).then_some((who, b, door))
            })
            .expect("somebody is in a building with a street door");
        let src = Source::Listing(ListingId::Person(who));
        set(&mut st, src).unwrap();
        assert_eq!(target_now(&st), Some(Target::Building(b)));
        // Beside the door is not in.
        let city = st.city.as_ref().unwrap();
        let fp = city.building(b).footprint;
        let beside = door
            .1
            .neighbors4()
            .find(|p| !fp.contains(*p) && city.walkable(StratumKind::Street, *p))
            .unwrap();
        put(&mut st, STREET, beside);
        tick(&mut st);
        assert!(st.track.is_some());
        crate::interiors::enter(&mut st, b, door).unwrap();
        tick(&mut st);
        assert!(st.track.is_none());
        assert_eq!(last(&st), format!("⌖ {} is in here", person_name(&st, who)));
    }

    #[test]
    fn a_dropped_job_clears_once() {
        let mut st = outside();
        let id = taken(&mut st, KindTag::Fetch).unwrap();
        let title = st.jobs.get(id).unwrap().title.clone();
        set(&mut st, job_source(id, 0)).unwrap();
        tick(&mut st);
        assert!(st.track.is_some());
        assert!(st.jobs.drop_job(id));
        let n = st.log.len();
        tick(&mut st);
        assert!(st.track.is_none());
        assert_eq!(st.log.len(), n + 1);
        assert_eq!(last(&st), format!("⌖ {title} · done with"));
        tick(&mut st);
        assert_eq!(st.log.len(), n + 1);
    }

    #[test]
    fn a_ticked_objective_clears() {
        let mut st = outside();
        let id = taken(&mut st, KindTag::Fetch).unwrap();
        let job = st.jobs.get(id).unwrap().clone();
        let Kind::Fetch {
            building,
            level,
            tile,
            ..
        } = job.kind
        else {
            unreachable!()
        };
        set(&mut st, job_source(id, 0)).unwrap();
        assert!(next_now(&st).unwrap().starts_with("door"));
        tick(&mut st);
        assert!(st.track.is_some());
        crate::jobs::send(
            &mut st,
            crate::jobs::Event::Taken {
                place: Place::Floor { building, level },
                pos: tile,
            },
        );
        tick(&mut st);
        assert!(st.track.is_none());
        assert_eq!(last(&st), format!("⌖ {} · done with", job.title));
    }

    #[test]
    fn a_story_step_moving_on_clears_and_arrival_does_not() {
        let mut st = outside();
        crate::missions::begin(&mut st);
        st.missions.feeds.clear();
        crate::missions::end(&mut st, 0, 0);
        let at = st.missions.runs[0].at;
        assert_eq!(st.book.missions[0].steps[at].id, "body");
        let title = st.book.missions[0].title.clone();
        set(&mut st, story(0, at)).unwrap();
        let body = st.missions.bodies[0].clone();
        let city = st.city.as_ref().unwrap();
        let beside = body
            .pos
            .neighbors4()
            .find(|p| city.walkable(StratumKind::Street, *p))
            .unwrap();
        put(&mut st, body.place, beside);
        tick(&mut st);
        assert!(st.track.is_some(), "the story's own trigger decides");
        let minute = st.minute_of_day();
        let city = st.city.as_ref().unwrap();
        st.player
            .as_mut()
            .unwrap()
            .update_fov(&MapRef::stratum_at(city, StratumKind::Street, minute), 0);
        for _ in 0..4 {
            if st.missions.runs[0].at != at {
                break;
            }
            crate::missions::tick(&mut st);
        }
        assert_ne!(st.missions.runs[0].at, at, "the body is seen");
        tick(&mut st);
        assert!(st.track.is_none());
        assert_eq!(last(&st), format!("⌖ {title} · done with"));
    }

    #[test]
    fn a_dead_person_without_a_body_clears() {
        let mut st = outside();
        let fixer = a_fixer(&st);
        let spot = st.world.as_ref().unwrap().spot_of(fixer).unwrap();
        st.world.as_mut().unwrap().park(fixer, spot);
        let src = Source::Listing(ListingId::Person(fixer));
        set(&mut st, src).unwrap();
        tick(&mut st);
        assert!(st.track.is_some());
        let name = person_name(&st, fixer);
        st.world.as_mut().unwrap().kill(fixer);
        tick(&mut st);
        assert!(st.track.is_none());
        assert_eq!(last(&st), format!("⌖ {name} · {name} is gone"));
    }

    #[test]
    fn a_person_on_your_tile_is_where_they_were() {
        let mut st = outside();
        let fixer = a_fixer(&st);
        let spot = st.world.as_ref().unwrap().spot_of(fixer).unwrap();
        st.world.as_mut().unwrap().park(fixer, spot);
        set(&mut st, Source::Listing(ListingId::Person(fixer))).unwrap();
        let was = target_now(&st);
        // You stand on their tile: the world has no tile for them.
        st.world.as_mut().unwrap().set_blocker(Some(spot));
        assert_eq!(locate(&st, Target::Person(fixer)), Resolved::Nowhere);
        tick(&mut st);
        assert!(st.track.is_some(), "not a clear");
        assert_eq!(target_now(&st), was);
        put(&mut st, STREET, spot);
        tick(&mut st);
        assert!(st.track.is_none());
        assert_eq!(last(&st), format!("⌖ there · {}", person_name(&st, fixer)));
    }

    #[test]
    fn no_way_from_here_keeps_the_track() {
        let mut st = outside();
        let at = here(&st);
        let to = steps_off(&st, 30);
        let a = street_spot(to, 1);
        set(&mut st, a).unwrap();
        let b = st.home.unwrap().home.building;
        put(&mut st, Place::Construct(b), Pos::new(1, 1));
        let n = st.log.len();
        tick(&mut st);
        assert_eq!(tracked(&st), Some(a));
        assert_eq!(leg_now(&st), Some(Err(NoLeg::JackedIn)));
        assert_eq!(distance(&st), None);
        // Walled off while you were away: no way from here, still tracked.
        let city = st.city.as_mut().unwrap();
        for p in to.neighbors8() {
            city.tile_mut(StratumKind::Street, p).unwrap().terrain = Terrain::Wall;
        }
        put(&mut st, STREET, at);
        tick(&mut st);
        assert_eq!(tracked(&st), Some(a));
        assert_eq!(leg_now(&st), Some(Err(NoLeg::NoWay)));
        assert_eq!(st.log.len(), n, "keeping it says nothing");
    }

    #[test]
    fn the_leg_is_remade_only_when_you_or_the_target_move() {
        let mut st = outside();
        let to = steps_off(&st, 30);
        set(&mut st, street_spot(to, 1)).unwrap();
        tick(&mut st);
        assert_eq!(distance(&st), Some(30));
        // The way shut, nobody moved: the leg stands.
        let city = st.city.as_mut().unwrap();
        for p in to.neighbors8() {
            city.tile_mut(StratumKind::Street, p).unwrap().terrain = Terrain::Wall;
        }
        tick(&mut st);
        assert_eq!(distance(&st), Some(30));
        // A step: it is made again.
        let step = st.track.as_ref().unwrap().walk.leg.as_ref().unwrap().path[0];
        put(&mut st, STREET, step);
        tick(&mut st);
        assert_eq!(leg_now(&st), Some(Err(NoLeg::NoWay)));
    }

    #[test]
    fn flatlining_clears() {
        let mut st = outside();
        let mut s = MapScreen::walking(&mut st);
        let a = street_spot(steps_off(&st, 20), 1);
        set(&mut st, a).unwrap();
        s.update(&mut st, &[]);
        assert_eq!(tracked(&st), Some(a), "a frame keeps it");
        let hp = st.character.hp as u8;
        crate::combat::wound_you(&mut st, "a goon on the street", hp);
        let n = st.log.len();
        assert!(matches!(s.update(&mut st, &[]), ScreenCmd::Switch { .. }));
        assert!(st.track.is_none());
        assert_eq!(st.log.len(), n, "a flatline says nothing of it");
    }
}
