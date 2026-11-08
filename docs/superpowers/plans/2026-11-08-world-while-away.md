# The World While You're Away Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A city you come back to shows its people where their day has taken them, caught up before any of it is drawn. Gaps of an hour or less are replayed; longer gaps are placed by day.

**Architecture:**
- The sim gains one entry point, `World::catch_up`. It replays (each minute stepped, then every trip that minute asked for found) or places by day (one pass).
- The game calls it everywhere a city comes back:
  - walking in (`Stay::put`);
  - a held city near the view (`run_neighbours`);
  - landing beside a held city (`stream::catch_up_round`);
  - the live clock's jumps (`clock::run`).

**Tech Stack:** Rust; `crates/sim` (no terminal dependencies) and `crates/strata-zero`.

**Spec:** `docs/superpowers/specs/2026-11-08-world-while-away-design.md`

## Global Constraints

- Build into `--target-dir target/smoke`, never `target/` (CLAUDE.md).
- Both front ends are checked at the end: fmt, clippy with and without `--features strata-zero/window`, and tests with and without it (CLAUDE.md, "Building and checking").
- Nothing new is saved. If a field is added to `sim::World` anyway, it takes `#[serde(default)]`, and the fixtures in `crates/strata-zero/src/runs/fixtures/` must keep opening, unregenerated (`docs/saving.md`).
- No `HashSet` in saved types; the sim stays deterministic (no wall clock, no unseeded randomness).
- Committed text records decisions, not chat. A person named in it is "Jeff".
- Every commit ends with `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`. The last one also ends with `Closes #26`.
- `SHORT_GAP` is one game hour: `60 * TICKS_PER_MINUTE` ticks.

## Review Focus

1. **A city put back while the player's tile is still its blocker.** A city set aside keeps `blocker` from when you left it. Expect the catch-up to run with no blocker, so nobody is held a tile back on a tile you aren't standing on (Task 4 sets it to `None` before catching up; test there).
2. **A city walked into again with no time gone** (in and straight back out across a town line). Expect nothing to change: no extra minute step, no RNG draw (Task 1, `catch_up_is_nothing_when_current`).
3. **A run reopened while away, then home walked into after more than an hour.** Expect the home city to be placed by day with nothing queued, as without the reopen (Task 4, `home_after_a_reopen_is_placed`).
4. **Someone the story holds who was caught in a chat when the city was left.** Expect them to go back to what they were doing, not to stay `Chatting` or turn `Working` (Task 2, `a_held_chatter_goes_back_to_what_they_were_doing`).
5. **A held city with no people world** (built with none). Expect every catch-up path to pass it by, not panic (Task 5, `a_neighbour_with_no_people_is_passed_by`).

---

### Task 1: The replay (sim)

**Files:**
- Create: `crates/sim/src/world/away.rs` (a child module of `world`, so it reaches `World`'s private fields and methods)
- Modify: `crates/sim/src/world.rs`: add `mod away;` and `pub use away::{CatchUp, SHORT_GAP};` near the top.
- Test: `crates/sim/src/world/away.rs` (`#[cfg(test)] mod tests`)

**Interfaces:**
- Produces:
  - `pub const SHORT_GAP: u64`: the longest gap, in ticks, a world replays (`60 * TICKS_PER_MINUTE`).
  - `#[derive(Clone, Copy, Debug, PartialEq, Eq)] pub enum CatchUp { Current, Replayed, Placed }`
  - `impl World { pub fn replay_to(&mut self, city: &City, tick: u64); pub fn catch_up(&mut self, city: &City, tick: u64) -> CatchUp }`
  - All three are re-exported from the crate root (`sim::SHORT_GAP`, `sim::CatchUp`), as `lib.rs` already does `pub use world::*`.

- [ ] **Step 1: Write the failing tests**

Test helpers (in `away.rs`'s test module):
- `fn city() -> City`: the sim tests' city, `Generator::run(GenParams { seed: 6, size: SizePreset::Custom { w: 120, h: 60 }, districts: 8, ..Default::default() })`.
- `fn morning(c: &City) -> World`: `World::seed(c, &People::load().unwrap(), &SimParams::default(), 7 * 60, 0)`.
- `fn places(w: &World) -> Vec<Whereabouts>`
- `fn moved(w: &World, before: &[Whereabouts]) -> usize`: how many people's whereabouts differ.

```rust
#[test]
fn an_hour_replayed_leaves_nothing_queued() {
    let c = city();
    let mut w = morning(&c);
    w.replay_to(&c, SHORT_GAP);
    assert_eq!(w.tick(), SHORT_GAP);
    assert_eq!(w.pending(), 0);
}

#[test]
fn an_hour_replayed_moves_people_as_a_live_hour_does() {
    let c = city();
    let mut live = morning(&c);
    let before = places(&live);
    for t in 1..=SHORT_GAP {
        live.advance(&c, t);
        live.path_some(&c, 4);
    }
    let mut bare = morning(&c);
    bare.advance(&c, SHORT_GAP);
    let mut replayed = morning(&c);
    replayed.replay_to(&c, SHORT_GAP);
    let (live, bare, replayed) = (
        moved(&live, &before),
        moved(&bare, &before),
        moved(&replayed, &before),
    );
    assert!(replayed * 10 >= live * 9, "replayed {replayed}, live {live}");
    assert!(replayed > 2 * bare, "replayed {replayed}, a bare advance {bare}");
}

#[test]
fn catch_up_is_nothing_when_current() {
    let c = city();
    let mut w = morning(&c);
    let before = places(&w);
    assert_eq!(w.catch_up(&c, 0), CatchUp::Current);
    assert_eq!(w.tick(), 0);
    assert_eq!(places(&w), before);
}

#[test]
fn catch_up_replays_up_to_an_hour() {
    let c = city();
    let mut w = morning(&c);
    assert_eq!(w.catch_up(&c, SHORT_GAP), CatchUp::Replayed);
    assert_eq!((w.tick(), w.pending()), (SHORT_GAP, 0));
}
```

- [ ] **Step 2: Run the tests to make sure they fail**

Run: `cargo test -p sim --target-dir target/smoke away`
Expected: FAIL to compile (`replay_to`, `catch_up`, `SHORT_GAP` and `CatchUp` are not defined).

- [ ] **Step 3: Implement `SHORT_GAP`, `CatchUp`, `replay_to` and `catch_up` in `away.rs`**

`replay_to` steps one minute boundary at a time and finds every queued trip after each, so a trip leaves on the minute it was asked for. It does nothing when `tick <= self.tick()`.

```rust
while self.tick < tick {
    let next = (self.tick / TICKS_PER_MINUTE + 1) * TICKS_PER_MINUTE;
    self.advance(city, next.min(tick));
    self.path_some(city, usize::MAX);
}
```

`catch_up` decides by the gap `tick - self.tick()`:
- `tick <= self.tick()`: `Current`, and nothing is touched;
- a gap up to and including `SHORT_GAP`: `replay_to`, then `Replayed`;
- longer: for now, `replay_to` as well, returning `Replayed`. Task 2 swaps in `place_at` and `Placed`.

Each item gets a doc comment in the module's voice. The module doc says what a catch-up is for: a city you come back to is caught up before it's seen; a short gap is replayed, a long one placed by day.

- [ ] **Step 4: Run the tests to make sure they pass**

Run: `cargo test -p sim --target-dir target/smoke away`
Expected: 4 passed.

- [ ] **Step 5: Commit**

```bash
git add crates/sim/src/world.rs crates/sim/src/world/away.rs
git commit -m "sim: a world replays a missed hour, finding every trip on its minute"
```

---

### Task 2: Place by day (sim)

**Files:**
- Modify: `crates/sim/src/world/away.rs`
- Modify: `crates/sim/src/world.rs`: pull the step's "Dates ending: both come out" loop body out into `fn date_over(&mut self, city: &City, d: Date)`, and call it from `step` and from `place_at`.
- Test: `crates/sim/src/world/away.rs`

**Interfaces:**
- Consumes: Task 1's `CatchUp`, `SHORT_GAP` and `catch_up`.
- Produces: `impl World { pub fn place_at(&mut self, city: &City, tick: u64) }`. `catch_up` now returns `Placed` for a gap over `SHORT_GAP`.

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn catch_up_places_past_an_hour() {
    let c = city();
    let mut w = morning(&c);
    assert_eq!(w.catch_up(&c, SHORT_GAP + 1), CatchUp::Placed);
    assert_eq!((w.tick(), w.pending()), (SHORT_GAP + 1, 0));
}

#[test]
fn placed_people_are_where_their_day_says() {
    for hours in [6u64, 72] {
        let c = city();
        let mut w = morning(&c);
        // ten minutes of morning, so some trips are in flight to drop
        for t in 1..=10 * TICKS_PER_MINUTE {
            w.advance(&c, t);
            w.path_some(&c, 4);
        }
        let at = w.tick() + hours * 60 * TICKS_PER_MINUTE;
        w.place_at(&c, at);
        assert_eq!((w.tick(), w.pending()), (at, 0));
        assert!(w.cars().iter().all(|car| car.parked && car.bound.is_none()));
        let minute = w.minute_of_day();
        for (i, p) in w.people().iter().enumerate() {
            assert!(!matches!(p.whereabouts, Whereabouts::Travelling(_)), "{} on the road", p.name);
            if w.satisfied.contains_key(&p.id) {
                continue;
            }
            assert_eq!(p.goal, w.due(i, minute), "{}", p.name);
            match (p.goal, p.home, p.work) {
                (Goal::Home, Some(h), _) => assert_eq!(p.whereabouts, Whereabouts::Inside(h), "{}", p.name),
                (Goal::Work, _, Work::Building(b)) => assert_eq!(p.whereabouts, Whereabouts::Inside(b), "{}", p.name),
                (Goal::Work, _, Work::Spot(s)) => assert_eq!(
                    p.whereabouts,
                    Whereabouts::At { pos: s, activity: Activity::Working },
                    "{}", p.name
                ),
                _ => {}
            }
        }
    }
}

#[test]
fn placing_is_one_pass_whatever_the_gap() {
    // the same world placed at the same hour two days apart comes out the same
    let c = city();
    let (mut a, mut b) = (morning(&c), morning(&c));
    let six = 6 * 60 * TICKS_PER_MINUTE;
    let day = 24 * 60 * TICKS_PER_MINUTE;
    a.place_at(&c, six);
    b.place_at(&c, six + 2 * day);
    assert_eq!(a.minute_of_day(), b.minute_of_day());
    assert!(a.people() == b.people());
}

#[test]
fn the_dead_the_held_and_the_cast_keep_what_the_story_gave_them() {
    let c = city();
    let mut w = morning(&c);
    for t in 1..=20 * TICKS_PER_MINUTE {
        w.advance(&c, t);
        w.path_some(&c, 4);
    }
    let on_foot = |p: &Person| matches!(&p.whereabouts,
        Whereabouts::Travelling(t) if t.legs.iter().all(|l| l.car.is_none()));
    let walker = w.people().iter().find(|p| on_foot(p)).unwrap().id;
    w.keep_where(walker);
    let held_at = w.people()[walker.0 as usize].whereabouts.clone();
    let worker = w.people().iter()
        .find(|p| p.id != walker && matches!(p.work, Work::Building(_)))
        .unwrap().id;
    w.keep_at_work(worker);
    let worked_at = w.people()[worker.0 as usize].whereabouts.clone();
    let dead = w.people().iter().find(|p| ![walker, worker].contains(&p.id)).unwrap().id;
    let dead_at = w.people()[dead.0 as usize].whereabouts.clone();
    w.kill(dead);
    // a commuter, kept in town: off the map at night but for the story
    let cast = w.people().iter()
        .find(|p| p.home.is_none() && ![walker, worker, dead].contains(&p.id) && w.commuter(p.id.0 as usize))
        .unwrap().id;
    w.keep_in_town(cast);
    // fourteen hours on: the evening, when commuters have gone home
    w.place_at(&c, w.tick() + 14 * 60 * TICKS_PER_MINUTE);
    assert_eq!(w.people()[walker.0 as usize].whereabouts, held_at);
    assert_eq!(w.people()[worker.0 as usize].whereabouts, worked_at);
    assert_eq!(w.people()[worker.0 as usize].goal, Goal::Work);
    assert!(w.is_dead(dead));
    assert_eq!(w.people()[dead.0 as usize].whereabouts, dead_at);
    assert_ne!(w.people()[cast.0 as usize].whereabouts, Whereabouts::Away);
    // let go, the walker stopped mid-trip asks for a trip the next minute
    w.release(walker);
    let t = w.tick();
    w.advance(&c, t + TICKS_PER_MINUTE);
    assert!(w.pending.iter().any(|r| r.who == walker), "sent on once released");
}

#[test]
fn a_held_chatter_goes_back_to_what_they_were_doing() {
    let c = city();
    let mut w = morning(&c);
    let id = w.people().iter()
        .find(|p| matches!(p.whereabouts, Whereabouts::At { .. }))
        .unwrap().id;
    w.keep_where(id);
    let Whereabouts::At { pos, activity } = w.people()[id.0 as usize].whereabouts else { unreachable!() };
    // caught in a chat as the city was left
    w.chat_resume.insert(id, activity);
    w.people[id.0 as usize].whereabouts = Whereabouts::At { pos, activity: Activity::Chatting };
    w.chats.push((pos, w.minute + 1));
    w.place_at(&c, w.tick() + 3 * 60 * TICKS_PER_MINUTE);
    assert_eq!(w.people()[id.0 as usize].whereabouts, Whereabouts::At { pos, activity });
    assert!(w.chats.is_empty() && w.chat_resume.is_empty());
}
```

- [ ] **Step 2: Run the tests to make sure they fail**

Run: `cargo test -p sim --target-dir target/smoke away`
Expected: FAIL to compile (`place_at` is not defined).

- [ ] **Step 3: Implement `place_at(&mut self, city: &City, tick: u64)` in `away.rs` and switch `catch_up` to it past `SHORT_GAP`**

The steps, in order. The order matters: dates end before anyone is placed, and chats are resolved before `chat_resume` is cleared.

1. Set the clock with no minute step: `self.tick = tick; self.minute = tick / TICKS_PER_MINUTE;`. Clear `pending`.
2. The displaced go back. Take `blocker` out, call `settle()`, put `blocker` back, then clear `displaced`.
3. Chats end. Everyone `Chatting` gets the activity in their `chat_resume` entry, or `Working` without one, as the step does. Then clear `chats` and `chat_resume`.
4. Dates end. Take every date out of `dates` and pass each to `date_over`, the step's own date end. The worker goes back on the beat; the client goes to the hotel door, home-bound and done with the Strip.
5. Place each person `i` who is neither dead nor held. Let `minute` be the new minute of the day.
   - A traveller's cars first: `park_cars_of(i)`.
   - The goal is `g = due(i, minute)`. If `satisfied[i] == g`, they keep their current goal. Otherwise the `satisfied` entry is removed and their goal is `g`.
   - Someone `Away` with no road out (`way_out(i).is_none()`, the story's cast): the first of `[goal, Goal::Home, Goal::Work]` that `dest_for` gives a destination for, `arrive`d at at once, as `come_back` would without the walk in.
   - Anyone else: `dest_for(city, i, goal)`. With `Some(d)`, `arrive(i, d, goal)`. With `None`, someone `Travelling` arrives at their trip's `dest` for its `goal`, and anyone else stays where they are.
   - Remove `i` from `cut_short`.
6. `refresh_open()`.

The held, the dead and their `cut_short` marks are not touched. Don't call `place_everyone`. It overwrites the held and the dead and leaves travellers' cars in flight; `place_at` is what a world with a history needs.

- [ ] **Step 4: Run the tests to make sure they pass**

Run: `cargo test -p sim --target-dir target/smoke`
Expected: all pass, the step's date tests included now that it calls `date_over`.

- [ ] **Step 5: Commit**

```bash
git add crates/sim/src/world.rs crates/sim/src/world/away.rs
git commit -m "sim: a world long left is placed by its day, the held and the dead kept"
```

---

### Task 3: The live clock in jumps

**Files:**
- Modify: `crates/strata-zero/src/clock.rs`: `PATHS_PER_FRAME` doc and `run`.
- Modify: `crates/strata-zero/src/rest.rs`: delete `PATHS_PER_MINUTE` (line 55); line 125 passes `crate::clock::PATHS_PER_FRAME`.
- Test: `crates/strata-zero/src/clock.rs` (`mod tests`)

**Interfaces:**
- Consumes: `sim::World::catch_up` (Task 2).
- Produces: `clock::run(st, Some(n), paths)` now takes `paths` per tick run.
  - `n` up to a minute (`sim::TICKS_PER_MINUTE`): `advance` `n` ticks, then `path_some(paths * n)`.
  - `n` over a minute: `world.catch_up(city, world.tick() + n)`, which replays up to an hour and places beyond it. Only tests run more than an hour in one call; overworld steps run at most 576 ticks.

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn an_hour_of_overworld_steps_leaves_the_city_nothing_queued() {
    // a flat tile's walk at a time (72 ticks), the live city run on as the overworld runs it
    let mut st = crate::outside::tests::home_run();
    for _ in 0..50 {
        run(&mut st, Some(72), PATHS_PER_FRAME);
    }
    assert_eq!(st.world.as_ref().unwrap().pending(), 0);
    // and a mountain river's walk (576 ticks) at a time
    for _ in 0..7 {
        run(&mut st, Some(576), PATHS_PER_FRAME);
    }
    assert_eq!(st.world.as_ref().unwrap().pending(), 0);
}
```

- [ ] **Step 2: Run the test to make sure it fails**

Run: `cargo test -p strata-zero --target-dir target/smoke clock::tests::an_hour_of_overworld_steps`
Expected: FAIL, with `pending()` above 0 (4 paths a step can't keep up with the evening at 22:00). If it passes, the test proves nothing: run it from the morning instead, moving the clock on to 07:00 with `run` first, until it fails.

- [ ] **Step 3: Implement the per-tick budget and the catch-up for runs over a minute in `clock::run`**

`PATHS_PER_FRAME`'s doc says it's per tick of the clock run. `paths.saturating_mul(n as usize)`. Rest's minute runs keep their 360 paths a minute: `PATHS_PER_FRAME` × 90.

- [ ] **Step 4: Run the tests to make sure they pass**

Run: `cargo test -p strata-zero --target-dir target/smoke clock:: rest::`
Expected: all pass.

Then run the whole crate: `cargo test -p strata-zero --target-dir target/smoke`.
- Tests that run the clock for hours in one call now place the live city instead of stepping it.
- A test that compared people with a bare `advance` over the same run is fixed by comparing with `catch_up` over it.
- A test of `RUN_ON` or the neighbours' `CATCH_UP` that breaks here is left for Tasks 4 and 5, which rewrite them. Note them in the commit message.

- [ ] **Step 5: Commit**

```bash
git add crates/strata-zero/src/clock.rs crates/strata-zero/src/rest.rs
git commit -m "clock: paths found per tick run, and a run over a minute caught up"
```

---

### Task 4: Walking in

**Files:**
- Modify: `crates/strata-zero/src/outside/mod.rs`:
  - `Stay::put` (lines 216-222): `w.rebuild`, then `w.set_blocker(None)`, then `w.catch_up(&city, now)`. It replaces the `advance`.
  - `enter`'s `Next::Fresh` arm: no `RUN_ON` filter on `take_people`. A build's people of any age are put in place, and `put` catches them up.
  - Delete `RUN_ON` (line 644) and its doc.
- Modify: `crates/strata-zero/src/outside/stream.rs` tests: rewrite `a_streamed_citys_people_are_ready_when_you_walk_in` and `a_streamed_city_entered_a_day_on_is_seeded_afresh`; drop `RUN_ON` from the `builds` module's `use`.
- Test: `crates/strata-zero/src/outside/mod.rs` (`mod tests`)

**Interfaces:**
- Consumes: `sim::World::catch_up`, `sim::World::set_blocker`, `sim::SHORT_GAP`.
- Produces: nothing new. `outside::RUN_ON` is gone; Task 5 removes its last use, in `run_neighbours`.

- [ ] **Step 1: Write the failing tests**

In `outside/mod.rs` tests, next to `time_runs_on_through_another_city_and_home`:

```rust
/// Home's people, set aside while you're in another city `minutes` of the clock.
fn home_after(minutes: u64) -> AppState {
    let mut st = home_run();
    let (s, _) = beside_another(&st);
    enter(&mut st, s).unwrap();
    crate::clock::run(&mut st, Some(minutes * sim::TICKS_PER_MINUTE), crate::clock::PATHS_PER_FRAME);
    let home = st.outside.as_ref().unwrap().home();
    enter(&mut st, home).unwrap();
    st
}

#[test]
fn home_after_half_an_hour_has_replayed_it() {
    let st = home_after(30);
    let w = st.world.as_ref().unwrap();
    assert_eq!(w.tick(), st.clock_tick());
    assert_eq!(w.pending(), 0, "every trip asked for found");
}

#[test]
fn home_after_six_hours_is_placed() {
    let st = home_after(6 * 60);
    let w = st.world.as_ref().unwrap();
    assert_eq!(w.tick(), st.clock_tick());
    assert_eq!(w.pending(), 0);
    assert!(w.people().iter().all(|p| !matches!(p.whereabouts, sim::Whereabouts::Travelling(_))));
}

#[test]
fn a_city_walked_back_into_has_no_blocker_from_when_you_left() {
    let mut st = home_run();
    let (s, _) = beside_another(&st);
    enter(&mut st, s).unwrap();
    let home = st.outside.as_ref().unwrap().home();
    st.away.as_mut().unwrap().world.as_mut().unwrap().set_blocker(Some(Pos::new(3, 3)));
    crate::clock::run(&mut st, Some(10 * sim::TICKS_PER_MINUTE), crate::clock::PATHS_PER_FRAME);
    enter(&mut st, home).unwrap();
    assert_eq!(st.world.as_ref().unwrap().blocker(), None);
}

#[test]
fn home_after_a_reopen_is_placed() {
    let mut st = home_run();
    let (s, _) = beside_another(&st);
    enter(&mut st, s).unwrap();
    crate::clock::run(&mut st, Some(3 * 60 * sim::TICKS_PER_MINUTE), crate::clock::PATHS_PER_FRAME);
    let mut st = reopen(st, "home-after-a-reopen");
    let home = st.outside.as_ref().unwrap().home();
    enter(&mut st, home).unwrap();
    let w = st.world.as_ref().unwrap();
    assert_eq!((w.tick(), w.pending()), (st.clock_tick(), 0));
}
```

`World::blocker(&self) -> Option<Pos>` is a new one-line getter in `crates/sim/src/world.rs`, beside `set_blocker`.

In `stream.rs`'s `builds` tests:
- `a_streamed_citys_people_are_ready_when_you_walk_in`: the clock runs 40 minutes before you walk in. Build `want` the same way, but instead of `want.advance(&city, now)` call `want.set_blocker(None); want.catch_up(&city, now);`, and still assert `w.people() == want.people()`.
- `a_streamed_city_entered_a_day_on_is_seeded_afresh` is renamed `a_streamed_city_entered_hours_on_is_placed`. It runs the clock `3 * 60` minutes, not `RUN_ON + 60`. Its `want` is the build's people placed: `old.rebuild(..); old.set_blocker(None); old.place_at(&city, now)`, and it asserts `w.people() == old.people()` and `w.pending() == 0`.

- [ ] **Step 2: Run the tests to make sure they fail**

Run: `cargo test -p strata-zero --target-dir target/smoke outside::`
Expected: the four new tests FAIL (`pending()` above 0 after walking in; `blocker()` undefined until added); the two rewritten build tests FAIL.

- [ ] **Step 3: Implement the changes to `Stay::put`, `enter` and `RUN_ON`, and add `World::blocker`**

`put`'s doc becomes: "its people caught up to `now` unseen (`sim::World::catch_up`: an hour or less replayed, longer placed by day)". `enter`'s doc drops "if that was no more than `RUN_ON` ago".

- [ ] **Step 4: Run the tests to make sure they pass**

Run: `cargo test -p strata-zero --target-dir target/smoke outside::`
Expected: all pass, apart from `run_neighbours` tests that still reference `RUN_ON`'s behaviour. Those are rewritten in Task 5; leave them failing, and say so in the commit message.

- [ ] **Step 5: Commit**

```bash
git add crates/sim/src/world.rs crates/strata-zero/src/outside/mod.rs crates/strata-zero/src/outside/stream.rs
git commit -m "outside: a city walked into is caught up unseen, replayed or placed"
```

---

### Task 5: Neighbours near the view

**Files:**
- Modify: `crates/strata-zero/src/outside/stream.rs`:
  - `run_neighbours` (lines 751-838) and its doc;
  - `CATCH_UP` and `BEHIND` (lines 748-755) are replaced by `REPLAY_PER_FRAME`;
  - a new private `with_held`.
- Test: `crates/strata-zero/src/outside/stream.rs` (`handover` tests: rewrite the neighbour tests at lines 2761-2919)

**Interfaces:**
- Consumes: `sim::World::{catch_up, replay_to, place_at}`, `sim::SHORT_GAP`.
- Produces:
  - `pub const REPLAY_PER_FRAME: u64 = 15;`: game minutes a held city off screen replays a frame.
  - `fn with_held<R>(st: &mut AppState, s: Settlement, f: impl FnOnce(&City, &mut sim::World) -> R) -> Option<R>`: the people of held city `s` (the home city set aside, a kept city, or a build's people), as `run_neighbours` finds them today. None for the live city or one with no people.
  - `run_neighbours(st, view, paths, step) -> usize` keeps its signature; `paths` is now per tick.

The rule for each held city whose map comes within `ALIVE` of `view` (cities further out still hold still), with `behind = now - world.tick()`:
- `behind <= step`: keeping time. `advance(city, now)`, then `path_some(city, paths * step)`.
- Else, if any of its map is on screen (`view.intersects(map)`, without the `ALIVE` band): `catch_up(city, now)`, the whole catch-up this frame.
- Else, if `behind > SHORT_GAP`: `place_at(city, now)`.
- Else: `replay_to(city, min(now, tick + REPLAY_PER_FRAME * TICKS_PER_MINUTE))`.

Each city runs every frame it's near; there are no turns. A placement is one pass, and a replay frame is at most 15 minutes. The built-and-not-walked-into branch that re-seeded past `RUN_ON` goes, because placement covers it. `set_blocker(None)` stays before each. The return value counts the cities run.

- [ ] **Step 1: Rewrite the neighbour tests**

`back_beside_it(minutes)` now returns `(AppState, Settlement, Rect, Rect)`: the 200×60 view centred on `x` as before, which shows part of the city's map, and an off-screen view. The off-screen view is a 10×10 rect centred 15 cells back from `x`, opposite `d`: `x.step(d.opposite())` fifteen times. Its near edge is 11 cells from the map, so the map lies in the `ALIVE` band but not in the view. The rewritten tests:

```rust
#[test]
fn a_neighbour_off_screen_replays_a_while_a_frame() {
    let (mut st, s, _, view) = back_beside_it(40);
    let t0 = crate::outside::people_of(&st, s).unwrap().tick();
    let now = st.clock_tick();
    let ticks = |st: &AppState| crate::outside::people_of(st, s).unwrap().tick();
    assert_eq!(run_neighbours(&mut st, view, 4, 1), 1);
    assert_eq!(ticks(&st), t0 + REPLAY_PER_FRAME * sim::TICKS_PER_MINUTE);
    run_neighbours(&mut st, view, 4, 1);
    assert_eq!(ticks(&st), t0 + 2 * REPLAY_PER_FRAME * sim::TICKS_PER_MINUTE);
    run_neighbours(&mut st, view, 4, 1);
    assert_eq!(ticks(&st), now, "caught up");
    assert_eq!(crate::outside::people_of(&st, s).unwrap().pending(), 0);
    // out of view it holds still
    let far = Rect::new(view.x - 2000, view.y, 200, 60);
    crate::clock::run(&mut st, Some(90), crate::clock::PATHS_PER_FRAME);
    assert_eq!(run_neighbours(&mut st, far, 4, 90), 0);
    assert_eq!(ticks(&st), now);
}

#[test]
fn a_neighbour_on_screen_is_caught_up_before_it_draws() {
    let (mut st, s, view, _) = back_beside_it(40);
    run_neighbours(&mut st, view, 4, 1);
    let w = crate::outside::people_of(&st, s).unwrap();
    assert_eq!((w.tick(), w.pending()), (st.clock_tick(), 0));
}

#[test]
fn a_neighbour_hours_behind_is_placed_in_a_frame() {
    // the old a_neighbours_people_a_day_behind_are_seeded_afresh, at 150 minutes
    let (mut st, s, _, view) = back_beside_it(150);
    let count = crate::outside::people_of(&st, s).unwrap().people().len();
    run_neighbours(&mut st, view, 4, 1);
    let w = crate::outside::people_of(&st, s).unwrap();
    assert_eq!((w.tick(), w.pending()), (st.clock_tick(), 0));
    assert_eq!(w.minute_of_day(), st.minute_of_day());
    assert_eq!(w.people().len(), count);
}
```

- `neighbours_behind_catch_up_one_a_frame` becomes `neighbours_on_screen_all_catch_up_in_a_frame`. The setup is unchanged (a 250 and b 200 minutes behind, `all` in view). One `run_neighbours(&mut st, all, 4, 1)` returns 2 and leaves both at `st.clock_tick()`.
- `a_neighbour_a_little_behind_keeps_time_while_another_catches_up` becomes `a_neighbour_days_behind_is_placed_beside_one_keeping_time`. The setup is unchanged. One frame leaves both at the clock; the next frame (after `clock::run(Some(1))`) runs both and leaves both at the clock.
- `a_long_frame_runs_every_neighbour_that_kept_time` stays as it is.
- New, for Review Focus 5:

```rust
#[test]
fn a_neighbour_with_no_people_is_passed_by() {
    let mut st = home_run();
    let (s, _, _) = beside_a_city(&mut st);
    // built, its people never seeded
    assert!(crate::outside::people_of(&st, s).is_none());
    let all = Rect::new(-100_000, -100_000, 200_000, 200_000);
    assert_eq!(run_neighbours(&mut st, all, 4, 1), 0);
}
```

(`beside_a_city` builds the city and seeds no people; `seed_its_people` is what gives it some.)

- [ ] **Step 2: Run the tests to make sure they fail**

Run: `cargo test -p strata-zero --target-dir target/smoke outside::stream::tests::handover`
Expected: the rewritten tests FAIL (60 minutes a frame, and turns, today); `REPLAY_PER_FRAME` is undefined.

- [ ] **Step 3: Implement `with_held`, `REPLAY_PER_FRAME` and the rule in `run_neighbours`**

The doc on `run_neighbours` restates the rule in the module's voice and points to the spec. Delete `CATCH_UP` and `BEHIND`.

- [ ] **Step 4: Run the tests to make sure they pass**

Run: `cargo test -p strata-zero --target-dir target/smoke outside::`
Expected: all pass.

- [ ] **Step 5: Commit**

```bash
git add crates/strata-zero/src/outside/stream.rs
git commit -m "outside: neighbours catch up unseen: off screen a while a frame, on screen at once"
```

---

### Task 6: Landing beside a held city

**Files:**
- Modify: `crates/strata-zero/src/outside/stream.rs`: add `catch_up_round` and `LANDING`.
- Modify: `crates/strata-zero/src/outside/moves.rs`: `into_city` (line 241) calls `catch_up_round(st)` last.
- Modify: `crates/strata-zero/src/screens/outside.rs`: `net_up_key` (line 124). Each branch that returns its `Pop` back to the map calls `crate::outside::stream::catch_up_round(st)` first: `W`, `Enter` on `Ok`, and `J`.
- Modify: `crates/strata-zero/src/screens/explore.rs`: in `MapScreen::update` (around lines 3655-3700), the `run_neighbours` call moves after the "The story moved you" block that may `follow_player`. Its `view` is then the camera this frame draws. It still runs only in `Mode::Walk`, with `carries_on`, and not during a rest.
- Test: `crates/strata-zero/src/outside/stream.rs` (`handover` tests) and `crates/strata-zero/src/screens/outside.rs` tests.

**Interfaces:**
- Consumes: `with_held` (Task 5); `sim::World::catch_up`.
- Produces:
  - `pub const LANDING: i32 = 256;`: cells round where you land that a held city's map must come within to be caught up on landing. It's wider than any view the window or a terminal shows.
  - `pub fn catch_up_round(st: &mut AppState)`: every held city whose map comes within `LANDING` of the player's cell in the live frame (`st.player`'s `pos`, which is the Net self's cell when jacked in) is caught up whole to the clock (`catch_up`). No player, or no world beyond, does nothing.

- [ ] **Step 1: Write the failing tests**

In `stream.rs`'s `handover` tests:

```rust
#[test]
fn coming_down_beside_a_city_finds_it_caught_up() {
    let mut st = home_run();
    let (s, x, _) = beside_a_city(&mut st);
    seed_its_people(&mut st, s);
    crate::clock::run(&mut st, Some(40 * sim::TICKS_PER_MINUTE), crate::clock::PATHS_PER_FRAME);
    crate::outside::moves::into_city(&mut st, x);
    let w = crate::outside::people_of(&st, s).unwrap();
    assert_eq!((w.tick(), w.pending()), (st.clock_tick(), 0));
}
```

In `screens/outside.rs` tests, beside `j_from_the_net_overworld_jacks_out`:

```rust
#[test]
fn back_down_from_the_net_overworld_finds_neighbours_caught_up() {
    use crate::outside::stream::tests::handover::{beside_a_city, seed_its_people};
    let (mut st, _m, mut up) = up_here();
    let (s, x, _) = beside_a_city(&mut st);
    st.player.as_mut().unwrap().pos = x;
    seed_its_people(&mut st, s);
    crate::clock::run(&mut st, Some(40 * sim::TICKS_PER_MINUTE), crate::clock::PATHS_PER_FRAME);
    assert!(matches!(up.update(&mut st, &[key(KeyCode::Char('W'))]), ScreenCmd::Pop { .. }));
    let w = crate::outside::people_of(&st, s).unwrap();
    assert_eq!(w.tick(), st.clock_tick());
}
```

If `beside_a_city` needs a city other than the one `up_here`'s pump is beside, use `beside_a_city_where` with a filter.

- [ ] **Step 2: Run the tests to make sure they fail**

Run: `cargo test -p strata-zero --target-dir target/smoke coming_down_beside back_down_from_the_net`
Expected: FAIL, with the city's tick 40 minutes behind the clock.

- [ ] **Step 3: Implement `LANDING` and `catch_up_round`, add the three calls, and move the `run_neighbours` call in `explore.rs`**

- [ ] **Step 4: Run the tests to make sure they pass**

Run: `cargo test -p strata-zero --target-dir target/smoke`
Expected: all pass.

- [ ] **Step 5: Commit**

```bash
git add crates/strata-zero/src/outside/stream.rs crates/strata-zero/src/outside/moves.rs crates/strata-zero/src/screens/outside.rs crates/strata-zero/src/screens/explore.rs
git commit -m "outside: a city you land beside is caught up before the map draws"
```

---

### Task 7: The manual, both front ends, and closing #26

**Files:**
- Modify: `manual/06-the-city-and-the-world.md`, "The overworld": the paragraph that ends "Your home city's story waits for you while you're away, and a call that couldn't reach you rings again when you come back."
- Modify: `docs/superpowers/specs/2026-11-08-world-while-away-design.md`: a short "Built" note at the end naming the rulings this plan made: no turns in `run_neighbours`, `REPLAY_PER_FRAME` = 15, `LANDING` = 256, and a live clock run over an hour is placed.

- [ ] **Step 1: The manual line**

After "...rings again when you come back.", add: `Cities you've left carry on with their day without you: come back and their people are where the hour has taken them.` Keep it in the chapter's plain second person, and keep it to Markdown the booklet draws (`manual/README.md`).

- [ ] **Step 2: Run the manual's tests**

Run: `cargo test -p strata-zero --target-dir target/smoke manual`
Expected: all pass.

- [ ] **Step 3: Check both front ends (CLAUDE.md)**

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets --target-dir target/smoke -- -D warnings
cargo clippy --workspace --all-targets --features strata-zero/window --target-dir target/smoke -- -D warnings
cargo test --workspace --target-dir target/smoke
cargo test --workspace --features strata-zero/window --target-dir target/smoke
```

Expected: no formatting diff, no warnings, every test passes on both.

- [ ] **Step 4: Commit, closing the issue**

```bash
git add manual/06-the-city-and-the-world.md docs/superpowers/specs/2026-11-08-world-while-away-design.md
git commit -m "manual: cities you've left carry on with their day

Closes #26"
```
