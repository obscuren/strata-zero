//! Tracking: one target at a time, its walk drawn on the map. This is what a track is
//! made of: where it came from (`Source`), what that points at now (`Target`), and the
//! pure functions that turn one into the other. Spec
//! `docs/superpowers/specs/2026-10-31-tracking-and-map-design.md`, 1.

use crate::app::AppState;
use crate::jobs::{JobId, Kind, State};
use crate::missions::{RunState, Trigger, Where};
use citygen::{BuildingId, Place, Pos, StratumKind};
use sim::{PersonId, Whereabouts};

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
    use crate::jobs::testutil::{a_fixer, state_home};
    use crate::jobs::{KindTag, generate};

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
        st.track = Some(Track {
            source: Source::Spot {
                place: Place::Stratum(StratumKind::Street),
                pos: Pos::new(5, 5),
                radius: 1,
            },
            label: "the spot".to_string(),
        });
        let city = st.city.clone().unwrap();
        st.enter_city_at(city, 0);
        assert!(st.track.is_none());
    }
}
