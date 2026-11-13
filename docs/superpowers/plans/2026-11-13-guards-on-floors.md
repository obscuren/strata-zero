# Guards on Floors Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Guards in a mission copy walk their rounds (some across floors), see you by the floor's light, open the doors their card opens, chase you through doors and up stairs and lifts, radio the alarm, wake to gunshots, find the body you left jacked in, and drop their zone card when downed; and every fight that works on the street works on a floor (#33).

**Architecture:**
- Foes read the map through `MapRef` (`foes::map_of`), never `place.stratum()`: sight, lines, steps and the player's side of a fight (shots, misses, pushes, chains, blasts, throws, the engaged step, the aimed walk) all work on a floor. One door rule, `guards::opens`, says which doors a foe's card opens; a floor's exits are walls to every foe.
- A new `guards` module beside `foes` holds what only guards do: the calm round on the clock (`guards::tick` from `clock::run`), taking links, following you across floors, hearing shots, finding your body and dropping a card. Its state is saved inside `foes::Post`.
- citygen's mission layer gives about one guard in three a stop on another level from a random stream of its own; the turn engine seats every hunting guard on your floor, and a guard that saw you radios the alarm on its next turn.

**Tech Stack:** Rust workspace (`crates/citygen`, `crates/strata-zero`); serde + rmp-serde (`to_vec_named`) for saves; ratatui; `rand_chacha::ChaCha8Rng` seeded through `citygen::rng::hash64`.

**Spec:** `docs/superpowers/specs/2026-11-13-guards-on-floors-design.md`. Every "(call)" in it is decided; this plan settles what it leaves to the builder.

## Global Constraints

- Build and test only with `--target-dir target/smoke`, never the default `target/`. The five checks from CLAUDE.md, both front ends: `cargo fmt --all --check`; `cargo clippy --workspace --all-targets --target-dir target/smoke -- -D warnings`; the same with `--features strata-zero/window`; `cargo test --workspace --target-dir target/smoke`; the same with `--features strata-zero/window`.
- Saved types: `#[serde(default)]` on every new field, new enum variants at the end, no `HashSet`/`Rc`/`Cell`/`Instant`/`&'static str` in saved types; `runs/fixtures/` keep opening and are never regenerated.
- Fixed player-facing words, verbatim: `guard spots you`, `guard radios it in · ALARM`, `a guard found you · jacked out`, the epitaph `a guard in {building}` (from `a {name} in {building}`). Counts on screens are digits.
- Values from the spec: `GUARD_DARK_REACH` 3, guard sight 8 (the goon's `GOON_SIGHT`), `GUARD_STEP_TICKS` = 3 × `net::WALK_TICKS`, `HEAR_RADIUS` 12, about 1 guard in 3 with a cross-floor stop.
- Anti-goals, never: a guard through a locked door it has no card for or through a wall; a guard seeing through a wall or a shut door, or across a dark room from far off; a floor fight playing differently from the street in a way the map doesn't explain; a guard, its alarm or its body in the plain building after the mission; an `Actor::Foe` index pointing at the wrong foe (nothing removes a foe during an engagement).
- Street foes keep their sight rule (no light). Only a floor reads light.
- citygen randomness comes from a `ChaCha8Rng` seeded through `hash64`; no `HashMap`/`HashSet` iteration decides anything laid.
- Existing tests that stand in a copy and run the clock or the frame may now meet guards that look, walk and find your body. Where such a test is about something else, it clears the building's guards first with `guards::testutil::no_guards(&mut st, b)`; never change a game rule to make an old test pass.
- A change a player sees updates its manual chapter in the same branch; manual Markdown stays within what the booklet draws (`manual/README.md`). No new glyphs: guards are `@` in foe red, as today.
- Commit messages: `area: a lowercase sentence (#33)` in the repo's style, ending with the line `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`; the final task's message ends with `Closes #33` before that line. Committed docs record decisions, not chat; a person in them is "Jeff".
- `AppState::new()` has dev mode off; a test needing a dev start calls `st.unlock_dev()` (the sitenet test fixtures already do).

## Rulings

Where the spec leaves a choice to the builder, or the code forces one, this plan decides:

1. **Ruling (the map):** `foes::map_of(st, place) -> Option<MapRef<'_>>` is `MapRef::for_place(city, &st.interiors, &st.constructs, place, st.minute_of_day())` for a stratum (no `beyond`) or a drawn floor, and None for a construct, a site's Net or a floor not drawn. With no map there is no line, no sight and no step. Every fight function takes a `&MapRef` (or builds one with `map_of`) in place of `(&City, Place)`.
2. **Ruling (doors):** `guards::opens(post: Option<&Post>, floor: &Floor, from: Pos, to: Pos) -> bool` answers a step from `from` onto `to`: a lock on `to` opens when `lock.free_from == Some(from)`, or when `post` is Some, `lock.ways.has(Ways::CARD)` and `lock.zone <= post.zone`; a private room's door (`room.private && room.kind != RoomKind::Home && !room.rect.contains(from)`) opens when `post` is Some and `room.zone <= post.zone`; any other tile opens. No alarm term: cards open doors in an alarm. `foes::can_step(map, post, from, to)` is `map.walkable(to)` on a stratum; on a floor also `floor.exit_at(to).is_none()` (an exit is a wall to every foe) and `guards::opens(post, floor, from, to)`. A foe with no post (a console goon) passes plain doors and a lock's free side only.
3. **Ruling (forced steps):** a push and a step out of a live blast end only on tiles `can_step(map, None, from, to)` allows: on a floor a body is never thrown onto an exit or through a shut door.
4. **Ruling (sight):** on a floor, `foes::sees` is `foe.standing() && foe.pos.chebyshev(to) <= foe.sight && sight::sees(foe.pos, to, GUARD_DARK_REACH, blocks, lit, |_| false)` where `blocks(p)` is `map.tile(p).is_none_or(foes::blocks_sight)` and `lit(p)` is `LIT_SIGHT` when `map.light(p) >= GUARD_LIT` else 0. `GUARD_LIT` is 100, where the exposure bands' `lit` begins (`EXPOSURE_BANDS`); a floor's `light` includes its ambient, so the street's emitted threshold (40) would light nearly every room. It holds for every foe on a floor; a stratum keeps `line_clear` and no light.
5. **Ruling (saved state):** `foes::Post` gains `home: Option<i32>` (its round's level; None in an old save, filled by `after_restore` with the level it stands on), `stop: Option<(i32, Pos)>`, `round: Round`, `follow: Option<(usize, i32)>` (the link you took off its floor and the level you went to), `spotted: bool` (it has seen you since it last radioed) and `radioed: bool`, each `#[serde(default)]`. `Round { leg: usize, next: u64, lift: Option<(usize, i32)>, via: Option<usize> }` (derives `Default`) lives in `foes.rs` beside `Post`: the goal it walks to by index into `guards::goals`, the clock tick it next acts at, the lift it waits for (link, level), and the link it took to its stop (so it comes back the same way). `Post::new(building, zone, patrol)` builds one with the rest defaulted; every literal uses it.
6. **Ruling (the round):** `guards::goals(post, home) -> Vec<(i32, Pos)>` is the patrol out (`patrol[0..n]` on `home`), back (indices `n-2` down to `1`, ends excluded; empty when `n < 3`), then the stop if any; `round.leg` wraps to 0, the post. On its tick a calm guard: standing on its goal, takes the next leg and pauses `GUARD_PAUSE_TICKS` (90, three seconds at 30 fps); else steps one tile toward it (on another level, toward the link `guards::link_to` picks) and waits `GUARD_STEP_TICKS` (12), and a step that lands on its goal takes the next leg and pauses at once. The step is `foes::step_toward` round the tiles held (you, your body, other standing foes); with no way round them, the step of the way that ignores them, and when that tile is held it waits a step, never sharing. A goal (or a link) with no way at all is skipped (next leg, wait a step), so a round with nothing reachable stands where it is. Calm steps ignore `stunned` (a stun counts turns).
7. **Ruling (when guards move):** `guards::tick` runs from `clock::run` after `venues::tick`. `guards::tick` visits every building that has a foe with a `post`, in `BuildingId` order, taken from `st.foes` (not from `interiors.copies()`: the ward has no copy). A building's guards move only while `venues::inside(st, b)` (made `pub(crate)`: you on one of its floors, on its Net, or your body on one of its floors); elsewhere its hunting guards turn calm (follow cleared) and every guard holds. A calm or hunting guard on the floor of an engagement holds (the turn engine moves it); only guards on the fight's floor hold, every other guard of the building moves on, a fight on the site's Net included. A floor a guard walks on is drawn with `Interiors::ensure` when needed.
8. **Ruling (links):** citygen gets `FloorPlan::hops(&self, link, from) -> Vec<i32>`: none by a crawl, a lift's `rides`, else the stair's, escalator's or ladder's `next_stop` up and down. `guards::hops(st, post, plan, link, from)` keeps a lift's hops only when `sitenet::lift_answers(st, b, link)` and `venues::ride_card(plan, link, to) <= post.zone`; stairs, escalators and ladders ask no card (no ladder asks Tech in this build; a crawl, which wants a crawler, is never a guard's). `guards::link_to(st, i, to_level) -> Option<(usize, Pos)>` is `round.via` when it still hops there (the way back from a stop is the way it came), else the link one hop from the guard's level to `to_level` nearest by its A* walk to the link's stop tile (ties: link order); taking a link to the stop sets `via`, coming back clears it. A saved `round.lift`, `round.via` or `follow` names a link by index into a plan rebuilt from code: on restore (`after_restore`) and again on use, one whose link no longer stops on the guard's level (`hops` empty) is dropped (`lift`/`via`/`follow` to None), so a later build's link order can't misroute a guard. Taking it: standing on the stop tile, a stair, escalator or ladder moves the guard at once; a lift sets `round.lift`, waits `GUARD_LIFT_TICKS` (60), and moves when it answers (re-checked every step while it doesn't). It lands on the other level's stop tile, or the first free walkable tile of `at.neighbors8()`; with none free it waits. You land the same way (`interiors::ride`/`climb` never put you on a foe).
9. **Ruling (the stop):** `GuardPost.stop: Option<Placed>` (`#[serde(default)]`). `MissionLayer::cross_floors` runs last in `lay`, after `stand_devices`, from `ChaCha8Rng::seed_from_u64(hash64(hash64(layout.seed, inputs.seed), STOP_SALT))` with `STOP_SALT = 0x5709_0033`, so nothing laid before it moves. On the kept plan (`FloorPlan::of_venue(layout).kept(&self.kept)`), a guard is eligible when some other kept level M has a link with M in `hops(link, level)` and `level` in `hops(link, M)`, both stops' `card` at most its zone when that link is a lift, and a spot of a room of its zone on M not in `used`. `n = max(1, guards.len() / 3)` of the eligible (in guard order, shuffled), each to a level among its eligible ones and a free spot there (both by the rng, from sorted lists); each stop goes into `used`.
10. **Ruling (seats):** on a floor, `net::initiative` and `net::seat_foes` seat `foes::in_memory` and every standing guard of that floor that hunts (`foes::hunting_on(st, place)`); at each round's wrap on a floor every standing calm guard of the floor looks first. A foe out of your sight memory (`foes::unseen(st, i)`) is left out of `net::turn_readout`, its turn's banner reads `FOOTSTEPS` / `wait`, and its actions don't wait `PACE_TICKS`.
11. **Ruling (radio):** `foes::look` sets `post.spotted` when a guard sees you, and warns `guard spots you` when that sight turns it from calm to hunting. `foes::turn_start`, after the stun check and before it looks, makes a guard with `spotted && !radioed` radio: `radioed = true`, `spotted = false`, and unless `st.net.alert(b, now)` already, `st.warn("guard radios it in · ALARM")` then `net::lock_down(st, b)` (the alarm with the alert on; `raise_alarm` alone sets no alert). A guard stunned when that turn comes loses its call: `radioed = true`, no alarm. A guard downed first never has the turn.
12. **Ruling (following):** `interiors::ride` and `climb` call `guards::you_left(st, b, from_level, link, to)` before you land: every standing guard of that floor that hunts gets `follow = Some((link, to))`. Off a fight, a hunting guard moves every `GUARD_RUN_TICKS` (= `net::WALK_TICKS`): with a follow, to the link's stop tile on its level, then takes it if `guards::hops` allows (a lift after its wait), landing with `mode = Hunting { seen: where it landed, lost: 0 }` and no follow; if not, it turns calm. With no follow, toward `seen`; at `seen`, or with no way there, it turns calm. A guard that sees you clears its follow. Calm, it walks back into its round.
13. **Ruling (noise):** `guards::hear(st, place, at, toward)` turns every standing calm guard on `place` (a floor only) within `HEAR_RADIUS` (Chebyshev, walls ignored) of `at` to `Hunting { seen: toward, lost: 0 }`, not spotted. Called by `combat::volley` (your shot: `at` = `toward` = your tile), `combat::fire_at_you` (`at` = `toward` = the guard's tile), `blast::land` (`at` the blast's tile, `toward` the thrower) and `blast::detonate` (`at` the centre, `toward` as it wakes its footprint).
14. **Ruling (your body):** each `guards::tick`, while you're jacked in with `player.body` on a floor of b, the first standing guard of that floor (index order) that `foes::sees` the body's tile finds it: `radioed = true`; `net::lock_down(st, b)` unless already on alert; `net::dump(st, "a guard found you · jacked out")` (its log reads `DUMPED · a guard found you · jacked out`); the guard `Hunting { seen: body tile, lost: 0 }`. The body panel (`sitenet::body_lines`) needs no new words: it reads the guards where they stand.
15. **Ruling (the card):** `combat::wound_foe`, when it downs a foe with a post of zone above 0, calls `guards::drop_card(st, i)`: `credentials::zone_card_id(b, zone)` added to `st.ground` at the foe's tile and pushed onto the copy's `left` through a new `Interiors::left_mut(b)` (it doesn't drop the site's Net cache: cards change no Net).
16. **Ruling (the epitaph):** `combat::epitaph(st, name, place)`: on a floor `a {name} in {building}` (`net::building_name`), on any stratum `a {name} on the street` as today.
17. **Ruling (your side on a floor):** the engaged step on a floor runs `MapScreen::doorway` first, as the calm step does (an exit leaves and the fight ends with the place; a held lock or private door warns and costs nothing). `open_at` accepts a floor's walkable tiles. The aimed walk on a floor paths over the floor's map round foes, never onto an exit or a door shut to you (`track::route::shut_to_you`, the rule `walk_past` uses, made `pub(crate)`), and each walked tile asks `venues::step_onto` (a held lock stops the walk).
18. **Ruling (tests):** a hand-made two-level building, "the ward" (`guards::testutil::ward`), gives the rules exact coordinates; integration tests use the DEVELOPER office copy (`sitenet::testutil::office_copy`). The ward is building `WARD = BuildingId(0)` on the lot (`City::lot(1)`, which has no buildings), its plan and floors put by hand (`Interiors::put_plan` and a new `#[cfg(test)] Interiors::put_floor`), no venue and no copy.

The ward, level 0 (`L`), 32 × 7, and level 1 (`2F`) the same frame with one open hall:

```
     0         1         2         3
     01234567890123456789012345678901
y0   ################################
y1   #.S.........LL.#........#......#
y2   #..............#........#......#
y3   X..............K........P......#
y4   #..............#........#......#
y5   #..............#........#......#
y6   ################################
```

- `#` Wall, `.` Floor, `S` Stair at (2,1), `LL` Lift bank at (12,1)-(13,1), `X` an exit door at (0,3) onto the Street (`Exit { at: (0,3), stratum: Street, landing: (6,8), door: (6,9) }`), `K` a card lock door at (15,3) (`DoorLock { at: (15,3), number: 1, zone: 1, kind: LockKind::Card, ways: Ways::CARD, free_from: Some((16,3)) }`), `P` a private room's door at (24,3).
- Rooms: 0 the west hall `Rect(1,1,14,5)` `Hall` zone 0; 1 the east hall `Rect(16,1,8,5)` `Corridor` zone 1; 2 `Rect(25,1,6,5)` `Office`, private, number 7, zone 2, door (24,3).
- Light (and glow, grey of the same value): 150 at x ≤ 15 and x ≥ 24; 10 at x 16..=23 (the east hall is dark).
- Level 1: walls on the border, Floor inside, `S` (2,1), `LL` (12,1)-(13,1), one room `Rect(1,1,30,5)` `Hall` zone 0, light 150 everywhere, no exits, no locks.
- Plan: `kind: Office`, `theme: Theme::CorpCore`, `size: 32×7`, `footprint: Rect(0,0,32,7)`, `levels: [0, 1]`, `links: [Stair { stops: [(0,(2,1)), (1,(2,1))] }, Lift { stops: [(0,(12,1), card 0), (1,(12,1), card 1)] }]` (every stop `on` and `off`), nothing else.

## Review Focus

1. **You arrive by stair or lift on a landing a guard stands on** (waiting for the lift, or crossing): you land on the nearest free walkable tile beside it, never on its tile (Task 6, `you_land_beside_a_guard_on_the_landing`).
2. **A guard arrives by a link onto a landing you or another foe hold:** it lands beside, else waits; it never shares a tile (Task 6, `a_guard_lands_beside_whoever_holds_the_landing`).
3. **A save taken mid-round** (a guard pausing, waiting for a lift, following you, having radioed) reopens with the same state, and the guards carry on from it (Task 8, `a_guards_round_comes_back_whole`).
4. **The card a downed guard dropped, picked up before the job ends:** the copy's end takes only what still lies there; the card stays in your pack (Task 11, `the_jobs_end_takes_a_dropped_card_and_a_picked_up_guard_card_stays_in_your_pack`).
5. **A calm guard whose next step is your tile or your body's:** it waits, never shares, and walks on once you move (Task 5, `a_guard_waits_for_you_to_clear_its_way`).

## Files

citygen (`crates/citygen/src/`):
- `interior/plan.rs`: `FloorPlan::hops`.
- `interior/venue/mission.rs`: `GuardPost.stop`, `STOP_SALT`, `MissionLayer::cross_floors`; `interior/venue/mission/tests.rs`.

strata-zero (`crates/strata-zero/src/`):
- `foes.rs`: `map_of`, `line_clear`, `sees`, `can_step`, `step_toward`, `step_out`, `look`, `turn_start`, `step`, `hunting_on`, `unseen`, `GUARD_DARK_REACH`, `GUARD_LIT`, `Post` (new fields, `Post::new`), `Round`.
- `guards.rs` (new; `mod guards;` in `main.rs`): constants, `tick`, re-exports. `guards/doors.rs` (`opens`), `guards/round.rs` (`goals`, the calm round), `guards/links.rs` (`hops`, `link_to`, taking a link, landing), `guards/hunt.rs` (`you_left`, the off-fight hunt, `hear`, the body, `drop_card`), `guards/testutil.rs` (the ward, `no_guards`), each with its tests.
- `grid.rs`, `combat.rs`, `blast.rs`, `render/fx/shapes.rs`, `net.rs`, `interiors.rs`, `venues.rs`, `venues/copy.rs`, `track/route.rs`, `clock.rs`, `screens/explore.rs`, `screens/console.rs`, `runs/snapshot.rs`, `runs/file.rs`, `sitenet/panel.rs` (`Post::new`), `venues/tests/mod.rs`, `screens/shots.rs`.

Docs: `docs/saving.md`, `manual/06-the-city-and-the-world.md`, `manual/07-turns-and-fights.md`, `manual/08-the-net.md`.

Test commands used throughout:
- `cargo test -p citygen --target-dir target/smoke [-- <filters>]`
- `cargo test -p strata-zero --target-dir target/smoke [-- <filters>]`
- Green at a task's end: `cargo test --workspace --target-dir target/smoke && cargo check --workspace --all-targets --features strata-zero/window --target-dir target/smoke`

---

### Task 1: Foes on any map, and the door rule

**Files:**
- Modify: `crates/strata-zero/src/foes.rs` (`line_clear`, `sees`, `step_toward`, `step_out`, `look`, `step`; new `map_of`, `can_step`; tests)
- Create: `crates/strata-zero/src/guards.rs`, `crates/strata-zero/src/guards/doors.rs`, `crates/strata-zero/src/guards/testutil.rs`; `main.rs`: `mod guards;`
- Modify: `crates/strata-zero/src/interiors.rs` (`#[cfg(test)] put_floor`)
- Modify (callers of the new signatures): `grid.rs` (`sees`, `step_toward` calls :640, :707, `line_clear` :858), `combat.rs` (`reach` :92, `chain_foes` :658), `blast.rs` (`footprint`'s line check and `landing`'s, blast.rs:340), `render/fx/shapes.rs` (`sees`)

**Interfaces:**
- Produces:
  ```rust
  // foes.rs
  pub fn map_of(st: &AppState, place: Place) -> Option<MapRef<'_>>;
  pub fn line_clear(map: &MapRef, from: Pos, to: Pos) -> bool;
  pub fn sees(map: &MapRef, foe: &Foe, to: Pos) -> bool;
  pub fn can_step(map: &MapRef, post: Option<&Post>, from: Pos, to: Pos) -> bool;
  pub fn step_toward(map: &MapRef, from: Pos, goal: Pos, avoid: &[Pos], post: Option<&Post>) -> Option<Pos>;
  // guards/doors.rs, re-exported as guards::opens
  pub fn opens(post: Option<&Post>, floor: &Floor, from: Pos, to: Pos) -> bool;
  // grid.rs
  pub fn sees(map: &MapRef, foe: &Foe, to: Pos, masked: bool) -> bool;
  // interiors.rs
  #[cfg(test)] pub(crate) fn put_floor(&mut self, b: BuildingId, floor: Floor);
  // guards/testutil.rs
  pub(crate) const WARD: BuildingId; // BuildingId(0)
  pub(crate) fn ward() -> AppState;           // the lot, the ward's plan and both floors, you on L at (6,3), sight refreshed, no foes
  pub(crate) fn on(level: i32) -> Place;      // Place::Floor { building: WARD, level }
  pub(crate) fn stand(st: &mut AppState, level: i32, at: Pos); // you on foot there, sight refreshed
  pub(crate) fn post(zone: u8, patrol: &[Pos]) -> Post;        // building WARD
  pub(crate) fn guard(st: &mut AppState, level: i32, zone: u8, patrol: &[Pos]) -> usize; // "guard", '@', at patrol[0]; its index
  pub(crate) fn goon(st: &mut AppState, level: i32, at: Pos) -> usize;
  pub(crate) fn advance(st: &mut AppState, n: u64); // as foes::tests::advance
  pub(crate) fn logged(st: &AppState, needle: &str) -> bool; // a log line contains it
  ```

- [ ] **Step 1: Write the ward and the failing tests**

Build `guards::testutil::ward` exactly as Ruling 18 draws it. In `guards/doors.rs` tests:

```rust
#[test]
fn a_guards_card_opens_its_zones_doors() {
    let st = ward();
    let (_, floor) = st.interiors.floor(WARD, 0).unwrap();
    let z = |zone| post(zone, &[Pos::new(2, 3)]);
    let (west, lock, east) = (Pos::new(14, 3), Pos::new(15, 3), Pos::new(16, 3));
    assert!(!opens(Some(&z(0)), floor, west, lock), "zone 0 has no card for zone 1");
    assert!(opens(Some(&z(1)), floor, west, lock));
    assert!(opens(Some(&z(0)), floor, east, lock), "its free side");
    assert!(!opens(None, floor, west, lock) && opens(None, floor, east, lock));
    let (outside, door, inside) = (Pos::new(23, 3), Pos::new(24, 3), Pos::new(25, 3));
    assert!(!opens(Some(&z(1)), floor, outside, door));
    assert!(opens(Some(&z(2)), floor, outside, door));
    assert!(opens(Some(&z(0)), floor, inside, door), "out of a room is free");
    assert!(opens(None, floor, Pos::new(5, 3), Pos::new(6, 3)), "no door");
}
```

In `foes.rs` tests (the existing street tests move to the new signatures with `map_of(&st, STREET).unwrap()`; `a_wall_blocks_sight`'s construct case becomes `assert!(map_of(&st, Place::Construct(BuildingId(0))).is_none())`):

```rust
#[test]
fn a_floors_walls_and_doors_block_the_line() {
    use crate::guards::testutil::*;
    let st = ward();
    let map = map_of(&st, on(0)).unwrap();
    assert!(line_clear(&map, Pos::new(2, 3), Pos::new(14, 3)));
    assert!(!line_clear(&map, Pos::new(14, 3), Pos::new(17, 3)), "the lock's door");
    assert!(line_clear(&map, Pos::new(15, 3), Pos::new(18, 3)), "from the doorway");
    assert!(map_of(&st, Place::Floor { building: WARD, level: 5 }).is_none());
}

#[test]
fn a_foe_on_a_floor_walks_round_walls_and_never_out_an_exit() {
    use crate::guards::testutil::*;
    let st = ward();
    let map = map_of(&st, on(0)).unwrap();
    assert_eq!(step_toward(&map, Pos::new(2, 3), Pos::new(10, 3), &[], None), Some(Pos::new(3, 3)));
    assert_eq!(step_toward(&map, Pos::new(2, 3), Pos::new(0, 3), &[], None), None, "the exit");
    assert_eq!(step_toward(&map, Pos::new(14, 3), Pos::new(20, 3), &[], None), None, "no card");
    assert_eq!(step_toward(&map, Pos::new(14, 3), Pos::new(20, 3), &[], Some(&post(0, &[]))), None);
    assert_eq!(step_toward(&map, Pos::new(14, 3), Pos::new(20, 3), &[], Some(&post(1, &[]))), Some(Pos::new(15, 3)));
    assert_eq!(step_toward(&map, Pos::new(16, 3), Pos::new(10, 3), &[], Some(&post(0, &[]))), Some(Pos::new(15, 3)), "getting out is free");
}
```

In `combat.rs` tests:

```rust
#[test]
fn a_shot_across_a_floor_has_its_line() {
    use crate::guards::testutil::*;
    let mut st = ward();
    let near = goon(&mut st, 0, Pos::new(12, 3));
    let far = goon(&mut st, 0, Pos::new(18, 3));
    assert_eq!(reach(&st, near, 12), Ok(6));
    assert_eq!(reach(&st, far, 12), Err("no line".to_string()));
}
```

- [ ] **Step 2: Run them to make sure they fail**

Run: `cargo test -p strata-zero --target-dir target/smoke -- foes:: guards:: combat::tests::a_shot_across`
Expected: FAIL to compile (`map_of`, `guards`, `put_floor` not defined).

- [ ] **Step 3: Implement**

- `map_of` per Ruling 1; `line_clear` over `map.tile` with `blocks_sight`, a missing tile blocking; `sees` (no light yet: Task 3 adds the floor's) and `look`/`step`/`step_out` build their map with `map_of`. `can_step` and the door rule per Ruling 2; `step_toward` runs `astar(map.size(), …)` with `can_step(map, post, p, n) && (!avoid.contains(&n) || n == goal)`. `foes::step` passes `foe.post.as_ref()`. `step_out` filters with `can_step(map, None, foe.pos, p)` (Ruling 3).
- `grid.rs`, `combat::reach`, `chain_foes` and `render/fx/shapes.rs::sees` take their map from `map_of` (or the map they hold). `blast::footprint` keeps its signature this task and checks lines over `MapRef::stratum_at(city, k, 0)`; `blast::landing` (it calls `foes::line_clear(city, place, …)`, blast.rs:340) moves to the map the same way, over the stratum map, so Task 1 compiles. Task 2 then moves both onto `map_of`.
- `guards.rs`: `mod doors; pub use doors::opens; #[cfg(test)] pub(crate) mod testutil;`.

- [ ] **Step 4: Run the tests**

Run: `cargo test -p strata-zero --target-dir target/smoke -- foes:: guards:: grid:: combat:: blast::`
Expected: PASS, every street test as before. Then the green line.

- [ ] **Step 5: Commit**

```bash
git add crates/strata-zero
git commit -m "foes: see, step and shoot over any map, floors by their doors (#33)" -m "Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 2: Your side of a fight on a floor

**Files:**
- Modify: `crates/strata-zero/src/combat.rs` (`miss_impact`, `push_foe`, `fire_at_you`, new `epitaph`; tests)
- Modify: `crates/strata-zero/src/blast.rs` (`footprint`, `landing`, `land`, `live_footprints`, `detonate`; tests)
- Modify: `crates/strata-zero/src/render/fx/shapes.rs` (`open_ground`, `sees`; test)
- Modify: `crates/strata-zero/src/screens/explore.rs` (`open_at`, `engaged_step`, `street_path`, `walk_step`, `street_walk_step`; tests)
- Modify: `crates/strata-zero/src/track/route.rs` (`shut_to_you`, out of `walk_past`)
- Modify: `crates/strata-zero/src/screens/console.rs` (`foe`/`dummy`; test)

**Interfaces:**
- Consumes: Task 1's `map_of`, `line_clear`, `can_step`, `guards::testutil`.
- Produces:
  ```rust
  pub fn miss_impact(map: &MapRef, from: Pos, target: Pos, range: i32) -> Pos;            // combat
  pub(crate) fn epitaph(st: &AppState, name: &str, place: Place) -> String;              // combat
  pub fn footprint(map: &MapRef, centre: Pos, radius: i32) -> Vec<(Pos, i32)>;          // blast
  pub(crate) fn shut_to_you(st: &AppState, plan: &FloorPlan, floor: &Floor, from: Pos, to: Pos) -> bool; // track::route
  ```

- [ ] **Step 1: Write the failing tests**

`combat.rs` tests (`push_foe` and `miss_impact` are reachable from the module's tests):

```rust
#[test]
fn a_miss_on_a_floor_stops_at_the_door() {
    use crate::guards::testutil::*;
    let st = ward();
    let map = foes::map_of(&st, on(0)).unwrap();
    assert_eq!(miss_impact(&map, Pos::new(6, 3), Pos::new(12, 3), 12), Pos::new(15, 3));
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
```

`blast.rs` tests:

```rust
#[test]
fn a_blast_on_a_floor_stops_at_walls_and_doors() {
    use crate::guards::testutil::*;
    let st = ward();
    let map = foes::map_of(&st, on(0)).unwrap();
    let tiles: Vec<Pos> = footprint(&map, Pos::new(13, 3), 3).into_iter().map(|(p, _)| p).collect();
    assert!(tiles.contains(&Pos::new(14, 3)) && tiles.contains(&Pos::new(13, 1)));
    assert!(!tiles.contains(&Pos::new(15, 3)) && !tiles.contains(&Pos::new(16, 3)));
}

#[test]
fn a_throw_lands_on_a_floor() {
    let st = crate::guards::testutil::ward();
    assert_eq!(landing(&st, 8, Pos::new(10, 3)), Ok(()));
    // The door is in sight with a clear way to it: the refusal is the landing's.
    assert_eq!(landing(&st, 12, Pos::new(15, 3)), Err("no landing".to_string()));
}
```

`render/fx/shapes.rs` tests: `a_throw_shape_reads_a_floor`: on the ward's L map, `open_ground(&map, (10,3))` and `sees(&map, (6,3), (10,3))` are true; `open_ground(&map, (15,3))` is false.

`screens/console.rs` tests: `foe_and_dummy_spawn_on_a_floor`: on the ward, `run(&mut st, at, "foe 4")` is `Ok("a goon at 10,3")` and `run(&mut st, at, "dummy 4")` is `Err("no room there")` (the goon stands there); `st.foes[0].place == on(0)`.

`screens/explore.rs` tests (`MapScreen::walking(&mut st)` on the ward, `key(...)` as the file's tests use):

```rust
#[test]
fn an_engaged_step_on_a_floor_spends_a_move() {
    let mut st = crate::guards::testutil::ward();
    let mut s = MapScreen::walking(&mut st);
    crate::net::engage(&mut st, crate::guards::testutil::on(0), true);
    let moves = st.net.engaged.as_ref().unwrap().moves;
    s.update(&mut st, &[key(KeyCode::Char('l'))]);
    assert_eq!(st.player.as_ref().unwrap().pos, Pos::new(7, 3));
    assert_eq!(st.net.engaged.as_ref().unwrap().moves, moves - 1);
}

#[test]
fn an_engaged_step_onto_a_lock_that_holds_costs_nothing() {
    use crate::guards::testutil::*;
    let mut st = ward();
    stand(&mut st, 0, Pos::new(14, 3));
    let mut s = MapScreen::walking(&mut st);
    crate::net::engage(&mut st, on(0), true);
    let moves = st.net.engaged.as_ref().unwrap().moves;
    s.update(&mut st, &[key(KeyCode::Char('l'))]);
    assert_eq!(st.player.as_ref().unwrap().pos, Pos::new(14, 3));
    assert_eq!(st.net.engaged.as_ref().unwrap().moves, moves);
    assert!(st.log.iter().any(|e| e.text.starts_with("locked · ")));
}

#[test]
fn the_aimed_walk_on_a_floor_goes_round_what_holds() {
    let mut st = crate::guards::testutil::ward();
    let s = MapScreen::walking(&mut st);
    let path = s.street_path(&st, Pos::new(6, 3), Pos::new(12, 5)).expect("a way");
    assert_eq!(path.last(), Some(&Pos::new(12, 5)));
    assert!(s.street_path(&st, Pos::new(6, 3), Pos::new(20, 3)).is_none(), "the lock holds you");
    assert!(s.street_path(&st, Pos::new(6, 3), Pos::new(0, 3)).is_none(), "never onto the exit");
}
```

- [ ] **Step 2: Run them to make sure they fail**

Run: `cargo test -p strata-zero --target-dir target/smoke floor`
Expected: FAIL (signatures, then assertions: "no line", `open_step` false on floors).

- [ ] **Step 3: Implement**

- `miss_impact`, `footprint`, `landing`, the throw shapes read `map.tile` (Ruling 1); callers make their map with `foes::map_of` (`volley` and `fire_at_you` compute each round's impact in a block before wounding). `push_foe` steps only where `can_step(map, None, pos, next)` (Ruling 3). `fire_at_you` wounds you with `epitaph(st, &name, place)` (Ruling 16).
- `open_at` on a floor: the floor's map walkable. `engaged_step` on a floor calls `self.doorway(st, dir)` first and returns its command when it has one (Ruling 17). `street_path` on a floor: `astar` over the floor's map with `map.walkable(n) && floor.exit_at(n).is_none() && foes::foe_at(st, place, n).is_none() && !route::shut_to_you(st, plan, floor, p, n)`. `walk_step` sends a floor's walk to `street_walk_step`, which on a floor asks `venues::step_onto` before each tile and stops the walk with its warning on `Held`.
- `shut_to_you` is `walk_past`'s `shut` closure lifted out unchanged; `walk_past` calls it.
- Console `foe`/`dummy`: `open` is `foes::map_of(st, place).is_some_and(|m| m.walkable(at))` and nobody standing there.

- [ ] **Step 4: Run the tests**

Run: `cargo test -p strata-zero --target-dir target/smoke -- combat:: blast:: shapes console explore`
Expected: PASS. Then the green line.

- [ ] **Step 5: Commit**

```bash
git add crates/strata-zero
git commit -m "combat: fights on a floor, from a step to a throw, and the epitaph names the building (#33)" -m "Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 3: Guards awake, seeing by the floor's light

**Files:**
- Modify: `crates/strata-zero/src/foes.rs` (`Post` fields, `Post::new`, `Round`, `GUARD_DARK_REACH`, `GUARD_LIT`, `sees` on floors; tests)
- Modify: `crates/strata-zero/src/venues.rs` (`guard_foes`), `venues/tests/mod.rs` (`guards_are_inert_foes_at_their_posts` becomes `guards_are_awake_foes_at_their_posts`), `sitenet/panel.rs` (its `Post` literal)
- Modify: `crates/strata-zero/src/runs/snapshot.rs` (`after_restore`), `runs/file.rs` (test)
- Modify: `crates/strata-zero/src/guards/testutil.rs` (`post` through `Post::new`; `no_guards`), `docs/saving.md`

**Interfaces:**
- Consumes: Task 1.
- Produces:
  ```rust
  pub const GUARD_DARK_REACH: i32 = 3;
  pub const GUARD_LIT: u8 = 100;
  #[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
  pub struct Round { pub leg: usize, pub next: u64, pub lift: Option<(usize, i32)>, pub via: Option<usize> }
  pub struct Post { pub building, pub zone, pub patrol,
      #[serde(default)] pub home: Option<i32>, #[serde(default)] pub stop: Option<(i32, Pos)>,
      #[serde(default)] pub round: Round, #[serde(default)] pub follow: Option<(usize, i32)>,
      #[serde(default)] pub spotted: bool, #[serde(default)] pub radioed: bool }
  impl Post { pub fn new(building: BuildingId, zone: u8, patrol: Vec<Pos>) -> Post; }
  pub(crate) fn no_guards(st: &mut AppState, b: BuildingId); // guards::testutil: drops b's posted foes
  ```

- [ ] **Step 1: Write the failing tests**

`foes.rs` tests:

```rust
#[test]
fn a_foe_on_a_floor_sees_by_its_light() {
    use crate::guards::testutil::*;
    let st = ward();
    let map = map_of(&st, on(0)).unwrap();
    let mut g = Foe::goon(on(0), Pos::new(2, 3));
    assert!(sees(&map, &g, Pos::new(10, 3)), "lit at 8");
    assert!(!sees(&map, &g, Pos::new(11, 3)), "past its sight");
    g.pos = Pos::new(17, 3);
    assert!(sees(&map, &g, Pos::new(20, 3)), "dark within 3");
    assert!(!sees(&map, &g, Pos::new(21, 3)), "dark at 4");
    g.pos = Pos::new(14, 3);
    assert!(!sees(&map, &g, Pos::new(17, 3)), "through the shut door");
    g.pos = Pos::new(15, 3);
    assert!(sees(&map, &g, Pos::new(12, 3)) && sees(&map, &g, Pos::new(18, 3)), "a doorway sees both rooms");
}
```

`venues/tests/mod.rs`: `guards_are_awake_foes_at_their_posts` as the old test, but `assert!(!f.inert && !f.down && !f.hunting())` and `f.post == Some(Post { home: Some(g.level), ..Post::new(b, g.zone, g.patrol.clone()) })`; and in an `office_copy(SITE_NET_SECURITY)`, `combat::wound_foe(&mut st, i, 255)` downs a guard.

`runs/file.rs` tests:

```rust
#[test]
fn old_guards_wake_after_a_reopen() {
    let (mut a, b) = crate::sitenet::testutil::office_copy(190);
    for f in a.foes.iter_mut().filter(|f| f.post.is_some()) {
        f.inert = true;                       // as a save from before holds them
        f.post.as_mut().unwrap().home = None;
    }
    let dir = Dir::new("old-guards");
    let st = reopened(&write(&dir.0, &running(a)).unwrap());
    let guards: Vec<_> = st.foes.iter().filter(|f| f.post.as_ref().is_some_and(|p| p.building == b)).collect();
    assert!(!guards.is_empty());
    for f in guards {
        let Place::Floor { level, .. } = f.place else { panic!("{f:?}") };
        assert!(!f.inert, "{f:?}");
        assert_eq!(f.post.as_ref().unwrap().home, Some(level));
    }
}
```

- [ ] **Step 2: Run them to make sure they fail**

Run: `cargo test -p strata-zero --target-dir target/smoke -- a_foe_on_a_floor_sees guards_are_awake old_guards_wake`
Expected: FAIL (`Post::new`, `home` not defined).

- [ ] **Step 3: Implement**

- `Post` and `Round` per Ruling 5; `sees` per Ruling 4 (a `MapRef::Floor` arm; others `line_clear`). `guard_foes` builds `Post { home: Some(g.level), ..Post::new(b, g.zone, g.patrol.clone()) }` and no longer sets `inert`. `after_restore`, after the cities are back: every foe with a post gets `inert = false` and, when `home` is None, the level of its `place`.
- `docs/saving.md`, "Sites and copies": guards are foes with a `post` that now carries their round (`home`, `stop`, `round`, `follow`, `spotted`, `radioed`, all defaulting); `after_restore` wakes the guards of a save from before (`inert` off, `home` from where each stands); `Foe.post`'s new fields join the "New fields default" list.
- Run the whole strata-zero suite; a test about something else that now meets an awake guard calls `no_guards` (Global Constraints).

- [ ] **Step 4: Run the tests**

Run: `cargo test -p strata-zero --target-dir target/smoke -- foes:: venues:: runs::`
Expected: PASS, the fixtures still open. Then the green line.

- [ ] **Step 5: Commit**

```bash
git add crates/strata-zero docs/saving.md
git commit -m "foes: guards wake, take damage and see you by a floor's light (#33)" -m "Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 4: A stop on another floor

**Files:**
- Modify: `crates/citygen/src/interior/plan.rs` (`FloorPlan::hops`; test)
- Modify: `crates/citygen/src/interior/venue/mission.rs` (`GuardPost.stop`, `STOP_SALT`, `cross_floors`, `lay`); `mission/tests.rs`
- Modify: `crates/citygen/src/interior/floor.rs` (the shared private-door helper `Room::shut_from`)
- Modify: `crates/strata-zero/src/track/route.rs` (`Access::hops` calls `FloorPlan::hops`; `shut_to_you` uses `Room::shut_from`), `guards/doors.rs` (`opens` uses `Room::shut_from`)
- Modify: `crates/strata-zero/src/venues.rs` (`guard_foes` carries the stop); `venues/tests/mod.rs`; `docs/saving.md`

**Interfaces:**
- Produces:
  ```rust
  impl FloorPlan { pub fn hops(&self, link: usize, from: i32) -> Vec<i32>; }
  pub struct GuardPost { pub level, pub at, pub zone, pub patrol, #[serde(default)] pub stop: Option<Placed> }
  const STOP_SALT: u64 = 0x5709_0033;
  fn cross_floors(&mut self, layout: &VenueLayout, used: &mut BTreeSet<(i32, Pos)>, rng: &mut ChaCha8Rng);
  impl Room { pub fn shut_from(&self, from: Pos) -> bool; } // private && kind != Home && !rect.contains(from): the one private-door check
  // strata-zero: Foe.post.stop == GuardPost.stop.map(|s| (s.level, s.at))
  ```

- [ ] **Step 1: Write the failing tests**

In `mission/tests.rs` (`layers()` and `builtin_layers()` as they are):

```rust
/// The kept plan of a laid layer.
fn kept_plan(x: &Laid) -> FloorPlan {
    FloorPlan::of_venue(&x.layout).kept(&x.layer.kept)
}

/// Whether link `k` of `plan` joins `a` and `b` both ways for a guard of `zone`.
fn joins(plan: &FloorPlan, k: usize, a: i32, b: i32, zone: u8) -> bool {
    let card = |l: i32| plan.links[k].stop(l).map_or(u8::MAX, |s| s.card);
    plan.hops(k, a).contains(&b)
        && plan.hops(k, b).contains(&a)
        && (plan.links[k].kind != LinkKind::Lift || (card(a) <= zone && card(b) <= zone))
}

#[test]
fn a_stop_is_a_spot_of_its_zone_one_link_away() {
    let mut any = 0;
    for x in layers().iter().chain(builtin_layers()) {
        let plan = kept_plan(x);
        for g in &x.layer.guards {
            let Some(s) = g.stop else { continue };
            any += 1;
            assert!(s.level != g.level && x.layer.kept.contains(&s.level), "{}: {g:?}", x.what);
            let lv = x.layout.level(s.level).unwrap();
            assert!(lv.rooms.iter().any(|r| r.zone == g.zone && r.spots.contains(&s.at)), "{}: {g:?}", x.what);
            assert!((0..plan.links.len()).any(|k| joins(&plan, k, g.level, s.level, g.zone)), "{}: {g:?}", x.what);
        }
    }
    assert!(any > 0, "some guard somewhere has a stop");
}

#[test]
fn about_one_guard_in_three_has_a_stop() {
    for x in layers().iter().chain(builtin_layers()) {
        let stops = x.layer.guards.iter().filter(|g| g.stop.is_some()).count();
        assert!(stops <= (x.layer.guards.len() / 3).max(1), "{}", x.what);
        // At least one where a guard could have one.
        let used = occupied(&x.layer);
        let plan = kept_plan(x);
        let could = x.layer.guards.iter().any(|g| {
            x.layer.kept.iter().filter(|&&m| m != g.level).any(|&m| {
                (0..plan.links.len()).any(|k| joins(&plan, k, g.level, m, g.zone))
                    && x.layout.level(m).unwrap().rooms.iter()
                        .any(|r| r.zone == g.zone && r.spots.iter().any(|p| !used.contains(&(m, *p))))
            })
        });
        assert_eq!(stops > 0, could, "{}", x.what);
    }
}

#[test]
fn no_stop_stands_on_a_post_a_device_or_another_stop() {
    for x in layers() {
        let stops: Vec<(i32, Pos)> = x.layer.guards.iter().filter_map(|g| g.stop).map(|s| (s.level, s.at)).collect();
        let mut seen = BTreeSet::new();
        for s in &stops {
            assert!(seen.insert(*s), "{}: two stops at {s:?}", x.what);
            assert!(!x.layer.guards.iter().any(|g| (g.level, g.at) == *s), "{}", x.what);
            assert!(!x.layer.devices.iter().any(|d| (d.level, d.at) == *s), "{}", x.what);
            assert!(!x.layer.containers.iter().any(|c| (c.level, c.at) == *s), "{}", x.what);
        }
    }
}

#[test]
fn a_guard_post_saved_before_reads_with_no_stop() {
    #[derive(serde::Serialize)]
    struct Old { level: i32, at: Pos, zone: u8, patrol: Vec<Pos> }
    let old = Old { level: 2, at: Pos::new(3, 4), zone: 1, patrol: vec![Pos::new(3, 4)] };
    let g: GuardPost = rmp_serde::from_slice(&rmp_serde::to_vec_named(&old).unwrap()).unwrap();
    assert_eq!((g.level, g.zone, g.stop), (2, 1, None));
}
```

`occupied(layer)` (a test helper) is the objective, every card, code, post, device and container as `(level, at)`: exactly what `lay` has put into `used` when `cross_floors` starts (check `leave_keys`, `post_guards` and `stand_devices` and match them). `the_same_inputs_lay_the_same_layer` (existing) covers that the stop is drawn the same way every time. In `venues/tests/mod.rs`, `guards_are_awake_foes_at_their_posts` has its Task 3 equality rewritten to carry the stop (`stop: g.stop.map(|s| (s.level, s.at))` in the expected `Post`; Step 3).

- [ ] **Step 2: Run them to make sure they fail**

Run: `cargo test -p citygen --target-dir target/smoke mission::tests`
Expected: FAIL to compile (`hops`, `stop` not defined).

- [ ] **Step 3: Implement**

`FloorPlan::hops` per Ruling 8 (citygen's half); `track::route::Access::hops` (route.rs:355) calls `FloorPlan::hops` and filters lifts by its own card/answer rule, so the branches live once. The private-door check is one helper, `Room::shut_from(from)` in citygen (`interior/floor.rs`, beside `Room`), and `guards::opens` (Ruling 2) and `track::route::shut_to_you` (Task 2; the rule `doorway` and `walk_past` use) both call it; none repeats the predicate. `cross_floors` per Ruling 9, called last in `lay` with its own rng; `guard_foes` copies the stop, and Task 3's `guard_foes` equality test (`guards_are_awake_foes_at_their_posts`) is updated here to `f.post == Some(Post { home: Some(g.level), stop: g.stop.map(|s| (s.level, s.at)), ..Post::new(b, g.zone, g.patrol.clone()) })`, since Task 3's version (stop None) fails for any guard given a stop. `docs/saving.md`: `GuardPost.stop` joins the copy's layer notes (laid once, saved, `#[serde(default)]`: a layer saved before has no stops).

- [ ] **Step 4: Run the tests**

Run: `cargo test -p citygen --target-dir target/smoke mission && cargo test -p strata-zero --target-dir target/smoke venues::`
Expected: PASS. Then the green line.

- [ ] **Step 5: Commit**

```bash
git add crates/citygen crates/strata-zero docs/saving.md
git commit -m "venue: about one guard in three gets a stop on another floor, from a stream of its own (#33)" -m "Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 5: Calm rounds on the clock

**Files:**
- Modify: `crates/strata-zero/src/guards.rs` (constants, `tick`), create `guards/round.rs`
- Modify: `crates/strata-zero/src/clock.rs` (call `guards::tick`), `venues.rs`/`venues/copy.rs` (`inside` made `pub(crate)`, re-exported as `venues::inside`)
- Modify: `crates/strata-zero/src/guards/testutil.rs` (`run`, `run_until`), `sitenet/testutil.rs` (`stand_on_floor` made `pub(crate)`)

**Interfaces:**
- Consumes: Tasks 1, 3 (`step_toward`, `Post`, `Round`).
- Produces:
  ```rust
  pub const GUARD_STEP_TICKS: u64 = 3 * crate::net::WALK_TICKS;
  pub const GUARD_RUN_TICKS: u64 = crate::net::WALK_TICKS;
  pub const GUARD_PAUSE_TICKS: u64 = 90;
  pub const GUARD_LIFT_TICKS: u64 = 60;
  pub const HEAR_RADIUS: i32 = 12;
  pub fn tick(st: &mut AppState);                              // guards.rs, from clock::run
  pub fn goals(post: &Post, home: i32) -> Vec<(i32, Pos)>;     // round.rs
  pub(crate) fn round_step(st: &mut AppState, i: usize, now: u64); // round.rs: one calm action
  pub(crate) fn avoid(st: &AppState, i: usize) -> Vec<Pos>;     // you, your body, other standing foes on its place
  pub(crate) fn home(foe: &Foe) -> i32;                         // post.home, else the level it stands on
  // guards/testutil.rs
  pub(crate) fn run(st: &mut AppState, n: u64);                 // n ticks, each `advance(st, 1)` then `guards::tick`
  pub(crate) fn run_until(st: &mut AppState, max: u64, done: impl Fn(&AppState) -> bool); // panics past `max`
  ```

- [ ] **Step 1: Write the failing tests**

In `guards/round.rs` tests (`run` and `run_until` from the testutil):

```rust
#[test]
fn goals_walk_the_patrol_out_and_back_then_the_stop() {
    let (a, b, c, d, s) = (Pos::new(1, 1), Pos::new(2, 1), Pos::new(3, 1), Pos::new(4, 1), Pos::new(9, 9));
    let mut p = post(0, &[a, b, c, d]);
    assert_eq!(goals(&p, 0), vec![(0, a), (0, b), (0, c), (0, d), (0, c), (0, b)]);
    p.stop = Some((1, s));
    assert_eq!(goals(&p, 0).last(), Some(&(1, s)));
    assert_eq!(goals(&post(0, &[a]), 0), vec![(0, a)]);
}

#[test]
fn a_guard_walks_its_round_a_step_every_twelve_ticks_and_pauses() {
    let mut st = ward();
    stand(&mut st, 1, Pos::new(20, 3)); // in the building, out of its sight
    let i = guard(&mut st, 0, 0, &[Pos::new(2, 3), Pos::new(10, 3)]);
    run(&mut st, 1);                     // at its post: the pause
    run(&mut st, GUARD_PAUSE_TICKS);
    assert_eq!(st.foes[i].pos, Pos::new(3, 3));
    run(&mut st, GUARD_STEP_TICKS - 1);
    assert_eq!(st.foes[i].pos, Pos::new(3, 3), "not yet");
    run(&mut st, 1);
    assert_eq!(st.foes[i].pos, Pos::new(4, 3));
    run(&mut st, 6 * GUARD_STEP_TICKS);
    assert_eq!(st.foes[i].pos, Pos::new(10, 3), "there");
    run(&mut st, GUARD_PAUSE_TICKS);
    assert_eq!(st.foes[i].pos, Pos::new(9, 3), "paused, and on its way back");
}

#[test]
fn guards_hold_while_you_are_out_of_the_building() {
    let mut st = ward();
    let i = guard(&mut st, 0, 0, &[Pos::new(2, 3), Pos::new(10, 3)]);
    st.player.as_mut().unwrap().place = Place::Stratum(citygen::StratumKind::Street);
    run(&mut st, 400);
    assert_eq!(st.foes[i].pos, Pos::new(2, 3));
}

#[test]
fn a_calm_guard_holds_in_a_fight_on_its_floor_and_walks_in_one_elsewhere() {
    let mut st = ward();
    stand(&mut st, 1, Pos::new(20, 3));
    let i = guard(&mut st, 0, 0, &[Pos::new(2, 3), Pos::new(10, 3)]);
    crate::net::engage(&mut st, on(0), true);   // a fight on its floor (as when you were there)
    run(&mut st, 400);
    assert_eq!(st.foes[i].pos, Pos::new(2, 3));
    crate::net::disengage(&mut st);
    crate::net::engage(&mut st, on(1), true);
    run(&mut st, GUARD_PAUSE_TICKS + GUARD_STEP_TICKS + 1);
    assert_ne!(st.foes[i].pos, Pos::new(2, 3));
}

#[test]
fn an_unreachable_spot_is_skipped_and_nothing_reachable_stands() {
    let mut st = ward();
    stand(&mut st, 1, Pos::new(20, 3));
    let i = guard(&mut st, 0, 0, &[Pos::new(2, 3), Pos::new(20, 3), Pos::new(6, 3)]);
    run(&mut st, 1 + GUARD_PAUSE_TICKS + 2 * GUARD_STEP_TICKS);
    assert_eq!(st.foes[i].pos.y, 3);
    assert!(st.foes[i].pos.x > 2 && st.foes[i].pos.x <= 6, "on to (6,3), past the east hall");
    let j = guard(&mut st, 0, 0, &[Pos::new(20, 4)]);
    st.foes[j].pos = Pos::new(10, 5);
    run(&mut st, 300);
    assert_eq!(st.foes[j].pos, Pos::new(10, 5), "nothing it can reach");
}

#[test]
fn a_guard_waits_for_you_to_clear_its_way() {
    let mut st = ward();
    stand(&mut st, 0, Pos::new(15, 3));                       // in the only doorway west
    let i = guard(&mut st, 0, 1, &[Pos::new(18, 3), Pos::new(10, 3)]);
    st.foes[i].post.as_mut().unwrap().round.leg = 1;          // on its way west
    run(&mut st, 6 * GUARD_STEP_TICKS);
    assert_eq!(st.foes[i].pos, Pos::new(16, 3), "it waits at the door, never shares");
    stand(&mut st, 0, Pos::new(12, 5));
    run(&mut st, 3 * GUARD_STEP_TICKS);
    assert!(st.foes[i].pos.x < 15, "through once you're clear");
}

#[test]
fn a_guard_on_a_floor_not_drawn_yet_walks() {
    let (mut st, b) = crate::sitenet::testutil::office_copy(crate::venues::SITE_NET_SECURITY);
    let i = (st.foes.iter()).position(|f| {
        f.post.as_ref().is_some_and(|p| p.building == b && p.patrol.len() > 1)
            && matches!(f.place, Place::Floor { level, .. } if level != 0)
    }).expect("a walking guard off L");
    let Place::Floor { level, .. } = st.foes[i].place else { unreachable!() };
    st.interiors.drop_floors(b);
    let city = st.city.clone().unwrap();
    let floor = st.interiors.ensure(&city, b, 0).unwrap();
    let door = floor.doorstep(&floor.exits[0]);
    crate::sitenet::testutil::stand_on_floor(&mut st, b, 0, door); // you on L, inside
    let from = st.foes[i].pos;
    run(&mut st, 1 + GUARD_PAUSE_TICKS + GUARD_STEP_TICKS);
    assert!(st.interiors.floor(b, level).is_some(), "drawn for it");
    assert_ne!(st.foes[i].pos, from);
}
```

(`sitenet::testutil::stand_on_floor` is made `pub(crate)`.)

- [ ] **Step 2: Run them to make sure they fail**

Run: `cargo test -p strata-zero --target-dir target/smoke guards::round`
Expected: FAIL to compile (`tick`, `goals`, constants not defined).

- [ ] **Step 3: Implement**

- `guards::tick` per Ruling 7: the buildings of `st.foes`' posts in `BuildingId` order (not `interiors.copies()`: the ward has no copy); for each guard of a building you're in, not down, not held by a fight on its floor, with `now >= round.next` (`now = st.clock_tick()`): calm → `round::round_step`. Hunting guards and the body arrive in Tasks 8 and 10.
- `round_step` per Ruling 6 for goals on its own level; a goal on another level is skipped this task (Task 6 takes links). It makes its map with `foes::map_of` after `st.interiors.ensure(city, b, level)`.
- `clock::run` calls `crate::guards::tick(st)` after `crate::venues::tick(st)`.
- Run the strata-zero suite; tests that stand in a copy and run the clock and are about something else call `no_guards`.

- [ ] **Step 4: Run the tests**

Run: `cargo test -p strata-zero --target-dir target/smoke guards::`
Expected: PASS. Then the green line.

- [ ] **Step 5: Commit**

```bash
git add crates/strata-zero
git commit -m "guards: walk their rounds on the clock while you or your body are in the building (#33)" -m "Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 6: Taking the stairs and the lifts

**Files:**
- Create: `crates/strata-zero/src/guards/links.rs`; modify `guards/round.rs` (goals on another level)
- Modify: `crates/strata-zero/src/interiors.rs` (`ride`, `climb`: land beside a foe; tests)

**Interfaces:**
- Consumes: Task 4 (`FloorPlan::hops`), Task 5 (`round_step`, `avoid`, constants).
- Produces:
  ```rust
  pub fn hops(st: &AppState, post: &Post, plan: &FloorPlan, link: usize, from: i32) -> Vec<i32>;
  pub(crate) fn link_to(st: &AppState, i: usize, to: i32) -> Option<(usize, Pos)>;  // (link, its stop tile on the guard's level)
  pub(crate) fn take(st: &mut AppState, i: usize, link: usize, to: i32, now: u64);  // stairs at once; a lift sets round.lift
  pub(crate) fn arrive(st: &mut AppState, i: usize, link: usize, to: i32) -> bool;  // false: no free tile, it waits
  pub(crate) fn free_landing(st: &AppState, place: Place, at: Pos) -> Option<Pos>;  // `at`, else the first free walkable neighbors8 tile
  ```

- [ ] **Step 1: Write the failing tests**

In `guards/links.rs` tests:

```rust
#[test]
fn a_guard_takes_the_stairs_to_its_stop_and_back() {
    let mut st = ward();
    stand(&mut st, 0, Pos::new(28, 3));                    // in the private room, out of sight
    let i = guard(&mut st, 0, 0, &[Pos::new(4, 3)]);
    st.foes[i].post.as_mut().unwrap().stop = Some((1, Pos::new(20, 3)));
    run_until(&mut st, 2000, |st| st.foes[i].place == on(1));
    assert_eq!(st.foes[i].pos, Pos::new(2, 1), "off the stairs");
    run_until(&mut st, 2000, |st| st.foes[i].pos == Pos::new(20, 3));
    // Back the way it came: the stairs, though 2F's lift (card 0 down to L) stands nearer.
    run_until(&mut st, 3000, |st| st.foes[i].place == on(0));
    assert_eq!(st.foes[i].pos, Pos::new(2, 1));
    run_until(&mut st, 3000, |st| st.foes[i].pos == Pos::new(4, 3));
}

#[test]
fn a_guard_waits_for_the_lift_its_card_rides() {
    let mut st = ward();
    stand(&mut st, 0, Pos::new(28, 3));
    let i = guard(&mut st, 0, 1, &[Pos::new(12, 3)]);
    st.foes[i].post.as_mut().unwrap().stop = Some((1, Pos::new(20, 3)));
    run_until(&mut st, 600, |st| st.foes[i].post.as_ref().unwrap().round.lift.is_some());
    assert_eq!((st.foes[i].place, st.foes[i].pos), (on(0), Pos::new(12, 1)));
    run(&mut st, GUARD_LIFT_TICKS);
    assert_eq!((st.foes[i].place, st.foes[i].pos), (on(1), Pos::new(12, 1)));
}

#[test]
fn a_zone_0_card_never_takes_the_carded_lift_and_an_alarm_sends_a_guard_to_the_stairs() {
    let mut st = ward();
    stand(&mut st, 0, Pos::new(28, 3));
    let plan = st.interiors.plan_of(WARD).unwrap().clone();
    assert!(hops(&st, &post(0, &[]), &plan, 1, 0).is_empty(), "the lift's 2F asks zone 1");
    assert_eq!(hops(&st, &post(1, &[]), &plan, 1, 0), vec![1]);
    crate::net::lock_down(&mut st, WARD);
    assert!(hops(&st, &post(1, &[]), &plan, 1, 0).is_empty(), "the lift doesn't answer");
    let i = guard(&mut st, 0, 1, &[Pos::new(12, 3)]);
    assert_eq!(link_to(&st, i, 1).map(|(k, _)| k), Some(0), "the stairs");
}

#[test]
fn a_lift_that_stops_answering_keeps_a_guard_waiting() {
    let mut st = ward();
    stand(&mut st, 0, Pos::new(28, 3));
    let i = guard(&mut st, 0, 1, &[Pos::new(12, 1)]);
    st.foes[i].post.as_mut().unwrap().round.lift = Some((1, 1));
    crate::net::lock_down(&mut st, WARD);
    run(&mut st, 4 * GUARD_LIFT_TICKS);
    assert_eq!(st.foes[i].place, on(0));
}

#[test]
fn a_guard_never_takes_a_crawl() {
    let st = ward();
    let mut plan = st.interiors.plan_of(WARD).unwrap().clone();
    plan.links[0].kind = citygen::LinkKind::Crawl;
    assert!(hops(&st, &post(2, &[]), &plan, 0, 0).is_empty());
}

#[test]
fn a_guard_lands_beside_whoever_holds_the_landing() {
    let mut st = ward();
    stand(&mut st, 1, Pos::new(2, 1));                       // you on 2F's stair landing
    let i = guard(&mut st, 0, 0, &[Pos::new(2, 1)]);
    assert!(arrive(&mut st, i, 0, 1));
    assert_eq!(st.foes[i].place, on(1));
    assert_eq!(st.foes[i].pos.chebyshev(Pos::new(2, 1)), 1);
}
```

`run_until(st, max, done)` (testutil) runs a tick at a time until `done` holds, panicking past `max` ticks.

In `interiors.rs` tests:

```rust
#[test]
fn you_land_beside_a_guard_on_the_landing() {
    use crate::guards::testutil::*;
    let mut st = ward();
    stand(&mut st, 0, Pos::new(2, 1));
    let i = guard(&mut st, 1, 0, &[Pos::new(2, 1)]);
    assert_eq!(climb(&mut st, true).unwrap(), "Entering `2F`");
    let me = st.player.as_ref().unwrap().pos;
    assert_ne!(me, st.foes[i].pos);
    assert_eq!(me.chebyshev(Pos::new(2, 1)), 1);
}
```

- [ ] **Step 2: Run them to make sure they fail**

Run: `cargo test -p strata-zero --target-dir target/smoke -- guards::links interiors::tests::you_land_beside`
Expected: FAIL to compile, then on the guard staying on L.

- [ ] **Step 3: Implement**

Ruling 8. `round_step`'s goal on another level: on its link's stop tile, `take`; else `link_to` and a step toward it, or skip the leg when there's none. A pending `round.lift` comes first in `round_step`: when due and the lift answers, `arrive`; else wait a step. `ride` and `climb` land you on `free_landing`.

- [ ] **Step 4: Run the tests**

Run: `cargo test -p strata-zero --target-dir target/smoke -- guards:: interiors::`
Expected: PASS. Then the green line.

- [ ] **Step 5: Commit**

```bash
git add crates/strata-zero
git commit -m "guards: take the stairs and the lifts their card rides to a stop on another floor (#33)" -m "Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 7: Hunting on floors and the radio

**Files:**
- Modify: `crates/strata-zero/src/foes.rs` (`look`, `turn_start`; new `hunting_on`, `unseen`; `settle`/`pace` of its tests move out)
- Modify: `crates/strata-zero/src/guards/testutil.rs` (`settle`; `pace` stays out of the export so it can't shadow `net::pace`)
- Modify: `crates/strata-zero/src/net.rs` (`initiative`, `seat_foes`, `pace`, `turn_readout`, `turn_view`; tests)
- Modify: `crates/strata-zero/src/screens/explore.rs` (`turn_prompt`'s `Actor::Foe` arm; test)

**Interfaces:**
- Consumes: Task 3 (`Post.spotted`, `radioed`), Task 1 (`step` with the post's doors).
- Produces:
  ```rust
  pub fn hunting_on(st: &AppState, place: Place) -> Vec<usize>; // standing guards of `place` that hunt, by index
  pub fn unseen(st: &AppState, i: usize) -> bool;               // not in your sight memory
  ```

- [ ] **Step 1: Write the failing tests**

In `net.rs` tests (the ward; `settle` as `foes::tests` has it, moved to `guards::testutil`; `pace` is not exported, the unseen-pace test calls `crate::net::pace`):

```rust
#[test]
fn every_hunting_guard_on_your_floor_takes_a_seat() {
    use crate::guards::testutil::*;
    let mut st = ward();
    let seen = guard(&mut st, 0, 0, &[Pos::new(10, 3)]);
    let woken = guard(&mut st, 0, 1, &[Pos::new(20, 3)]);
    st.foes[woken].mode = crate::foes::FoeMode::Hunting { seen: Pos::new(16, 3), lost: 0 };
    assert!(crate::foes::look_all(&mut st));
    engage(&mut st, on(0), false);
    let order = &st.net.engaged.as_ref().unwrap().order;
    assert!(order.contains(&Actor::Foe(seen)) && order.contains(&Actor::Foe(woken)));
    let readout = turn_readout(&st).unwrap();
    assert_eq!(readout.matches("guard").count(), 1, "{readout}");
}

#[test]
fn an_unseen_guard_acts_without_the_pace() {
    use crate::guards::testutil::*;
    let mut st = ward();
    let woken = guard(&mut st, 0, 1, &[Pos::new(20, 3)]);
    st.foes[woken].mode = crate::foes::FoeMode::Hunting { seen: Pos::new(16, 3), lost: 0 };
    engage(&mut st, on(0), false);
    while st.net.engaged.as_ref().unwrap().actor() != Actor::Foe(woken) {
        end_turn(&mut st);
    }
    crate::net::pace(&mut st); // explicitly net's, not a testutil copy: no tick passed
    assert_eq!(st.foes[woken].pos, Pos::new(19, 3));
}

#[test]
fn an_unseen_guard_is_not_in_the_turn_column_until_seen() {
    use crate::guards::testutil::*;
    let mut st = ward();
    let woken = guard(&mut st, 0, 1, &[Pos::new(20, 3)]);
    st.foes[woken].mode = crate::foes::FoeMode::Hunting { seen: Pos::new(16, 3), lost: 0 };
    engage(&mut st, on(0), false);
    assert!(crate::foes::unseen(&st, woken));
    assert!(!turn_view(&st).iter().any(|c| c.actor == Actor::Foe(woken)), "not before you see it");
    stand(&mut st, 0, Pos::new(16, 3)); // the doorway sees the east hall
    assert!(!crate::foes::unseen(&st, woken));
    assert!(turn_view(&st).iter().any(|c| c.actor == Actor::Foe(woken)), "once seen");
}

#[test]
fn a_guard_that_sees_you_radios_on_its_next_turn() {
    use crate::guards::testutil::*;
    let mut st = ward();
    let i = guard(&mut st, 0, 0, &[Pos::new(10, 3)]);
    assert!(crate::foes::look_all(&mut st));
    assert!(logged(&st, "guard spots you"));
    engage(&mut st, on(0), false);
    let now = st.net.now(&st);
    assert!(!st.net.alert(WARD, now), "not before its turn");
    while st.net.engaged.as_ref().is_some_and(|e| e.actor() != Actor::Foe(i)) {
        end_turn(&mut st);
    }
    assert!(logged(&st, "guard radios it in · ALARM"));
    assert!(st.net.alert(WARD, st.net.now(&st)));
    assert!(st.foes[i].post.as_ref().unwrap().radioed);
}

#[test]
fn drop_it_before_its_turn_and_nobody_hears() {
    use crate::guards::testutil::*;
    let mut st = ward();
    let i = guard(&mut st, 0, 0, &[Pos::new(10, 3)]);
    crate::foes::look_all(&mut st);
    engage(&mut st, on(0), true);
    crate::combat::wound_foe(&mut st, i, 255);
    for _ in 0..4 { end_turn(&mut st); }
    assert!(!st.net.alert(WARD, st.net.now(&st)) && !logged(&st, "radios"));
}

#[test]
fn a_guard_woken_by_noise_or_stunned_does_not_radio() {
    use crate::guards::testutil::*;
    let mut st = ward();
    stand(&mut st, 0, Pos::new(3, 5));
    let woken = guard(&mut st, 0, 1, &[Pos::new(20, 3)]);
    st.foes[woken].mode = crate::foes::FoeMode::Hunting { seen: Pos::new(16, 3), lost: 0 };
    let stunned = guard(&mut st, 0, 0, &[Pos::new(8, 5)]);
    crate::foes::look_all(&mut st);
    st.foes[stunned].stunned = 1;
    engage(&mut st, on(0), true);
    for _ in 0..6 { end_turn(&mut st); }
    assert!(!st.net.alert(WARD, st.net.now(&st)));
    assert!(st.foes[stunned].post.as_ref().unwrap().radioed, "the stun spent its call");
    assert!(!st.foes[woken].post.as_ref().unwrap().radioed);
}

#[test]
fn a_guard_radios_once_ever_and_not_into_an_alarm() {
    use crate::guards::testutil::*;
    let mut st = ward();
    let i = guard(&mut st, 0, 0, &[Pos::new(10, 3)]);
    lock_down(&mut st, WARD);
    let lines = st.log.iter().count();
    crate::foes::look_all(&mut st);
    engage(&mut st, on(0), true);
    while st.net.engaged.as_ref().is_some_and(|e| e.actor() != Actor::Foe(i)) { end_turn(&mut st); }
    assert!(st.foes[i].post.as_ref().unwrap().radioed);
    assert!(!st.log.iter().skip(lines).any(|e| e.text.contains("radios")));
}

#[test]
fn a_calm_guard_looks_each_round_and_joins() {
    use crate::guards::testutil::*;
    let mut st = ward();
    engage(&mut st, on(0), true);
    let i = guard(&mut st, 0, 0, &[Pos::new(10, 3)]);
    end_turn(&mut st); // the round wraps
    assert!(st.net.engaged.as_ref().unwrap().order.contains(&Actor::Foe(i)));
    assert!(st.foes[i].hunting());
}
```

In `screens/explore.rs` tests: `the_banner_hides_an_unseen_actor`: the ward, a woken guard at (20,3) as above, engaged, its turn: `s.turn_prompt(&st) == Some(("FOOTSTEPS".to_string(), "wait".to_string()))`; a seen one's reads `THE GUARD ACTS`.

- [ ] **Step 2: Run them to make sure they fail**

Run: `cargo test -p strata-zero --target-dir target/smoke -- net::tests:: explore::tests::the_banner`
Expected: FAIL (`hunting_on` not defined; then the unseen guard never seated, no radio).

- [ ] **Step 3: Implement**

Rulings 10 and 11. `foes::look`: a guard that sees you sets `spotted`, clears `follow`, and warns `guard spots you` when it was calm. `foes::turn_start`: after the stun (a guard stunned with `spotted && !radioed` sets `radioed`), the radio, then `look`. `initiative` and `seat_foes` on a `Place::Floor` add `hunting_on` (deduped, in index order); `seat_foes` on a floor first runs `foes::look` on every standing calm guard of the place. `pace` skips the `PACE_TICKS` wait for an unseen `Actor::Foe`. `turn_readout` and `net::turn_view` (the chips `render/turns.rs` draws, with HP) skip an unseen foe as they skip a bot gone. (Adjust the `turn_view` test's chip field names to the real type.)

- [ ] **Step 4: Run the tests**

Run: `cargo test -p strata-zero --target-dir target/smoke -- net:: foes:: explore::`
Expected: PASS. Then the green line.

- [ ] **Step 5: Commit**

```bash
git add crates/strata-zero
git commit -m "guards: every hunting guard on your floor takes turns, and one that saw you radios the alarm (#33)" -m "Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 8: Following you across floors, never out of the building

**Files:**
- Create: `crates/strata-zero/src/guards/hunt.rs` (`you_left`, `hunt_step`)
- Modify: `crates/strata-zero/src/guards.rs` (`tick`: hunting guards; out of the building they turn calm), `interiors.rs` (`ride`, `climb` call `you_left`)
- Modify: `crates/strata-zero/src/runs/file.rs` (tests), `runs/snapshot.rs` (`after_restore` drops stale links)

**Interfaces:**
- Consumes: Tasks 5, 6, 7.
- Produces:
  ```rust
  pub(crate) fn you_left(st: &mut AppState, b: BuildingId, level: i32, link: usize, to: i32);
  pub(crate) fn hunt_step(st: &mut AppState, i: usize, now: u64); // one off-fight hunting action
  ```

- [ ] **Step 1: Write the failing tests**

`guards/hunt.rs` tests:

```rust
#[test]
fn a_hunting_guard_follows_you_up_the_stairs() {
    let mut st = ward();
    stand(&mut st, 0, Pos::new(2, 1));
    let i = guard(&mut st, 0, 0, &[Pos::new(6, 3)]);
    crate::foes::look_all(&mut st);
    crate::net::engage(&mut st, on(0), true);
    crate::interiors::climb(&mut st, true).unwrap();
    assert_eq!(st.foes[i].post.as_ref().unwrap().follow, Some((0, 1)));
    crate::net::disengage(&mut st);
    stand(&mut st, 1, Pos::new(28, 3));            // walked off out of its way
    run_until(&mut st, 400, |st| st.foes[i].place == on(1));
    assert!(matches!(st.foes[i].mode, crate::foes::FoeMode::Hunting { seen, .. } if seen == st.foes[i].pos));
    assert_eq!(st.foes[i].post.as_ref().unwrap().follow, None);
}

#[test]
fn a_guard_whose_card_wont_ride_the_lift_loses_you_there() {
    let mut st = ward();
    stand(&mut st, 1, Pos::new(28, 3));                       // you rode the lift up
    let i = guard(&mut st, 0, 0, &[Pos::new(10, 3)]);
    st.foes[i].mode = crate::foes::FoeMode::Hunting { seen: Pos::new(12, 1), lost: 0 };
    st.foes[i].post.as_mut().unwrap().follow = Some((1, 1)); // as `you_left` marks it
    run_until(&mut st, 400, |st| !st.foes[i].hunting());
    assert_eq!(st.foes[i].place, on(0));
    assert_eq!(st.foes[i].post.as_ref().unwrap().follow, None);
}

#[test]
fn out_the_door_the_guards_lose_you() {
    let mut st = ward();
    let i = guard(&mut st, 0, 0, &[Pos::new(10, 3)]);
    crate::foes::look_all(&mut st);
    let exit = *st.interiors.floor(WARD, 0).unwrap().1.exit_at(Pos::new(0, 3)).unwrap();
    crate::interiors::leave(&mut st, &exit).unwrap();
    crate::guards::tick(&mut st);
    assert!(!st.foes[i].hunting());
    assert_eq!(st.foes[i].place, on(0), "it never leaves the building");
}

#[test]
fn a_woken_guard_off_your_floor_walks_to_the_shot_and_gives_up() {
    let mut st = ward();
    stand(&mut st, 1, Pos::new(28, 3));
    let i = guard(&mut st, 0, 0, &[Pos::new(2, 3)]);
    st.foes[i].mode = crate::foes::FoeMode::Hunting { seen: Pos::new(6, 3), lost: 0 };
    run(&mut st, 4 * GUARD_RUN_TICKS);
    assert_eq!(st.foes[i].pos, Pos::new(6, 3));
    run(&mut st, GUARD_RUN_TICKS);
    assert!(!st.foes[i].hunting(), "nobody there: back to its round");
}
```

`runs/file.rs` tests:

```rust
#[test]
fn a_guards_round_comes_back_whole() {
    let (mut a, b) = crate::sitenet::testutil::office_copy(190);
    let i = a.foes.iter().position(|f| f.post.as_ref().is_some_and(|p| p.building == b)).unwrap();
    let p = a.foes[i].post.as_mut().unwrap();
    p.round = crate::foes::Round { leg: 2, next: 12345, lift: Some((1, 3)), via: Some(1) };
    p.follow = Some((0, 2));
    p.radioed = true;
    p.spotted = true;
    a.foes[i].mode = crate::foes::FoeMode::Hunting { seen: a.foes[i].pos, lost: 1 };
    let dir = Dir::new("guard-round");
    let a = running(a);
    let st = reopened(&write(&dir.0, &a).unwrap());
    assert_eq!(st.foes[i], a.foes[i]);
}

#[test]
fn a_saved_link_that_no_longer_stops_on_the_level_is_dropped() {
    let (mut a, b) = crate::sitenet::testutil::office_copy(190);
    let i = a.foes.iter().position(|f| f.post.as_ref().is_some_and(|p| p.building == b)).unwrap();
    let p = a.foes[i].post.as_mut().unwrap();
    p.round.lift = Some((99, 1));      // a link the plan no longer has
    p.round.via = Some(99);
    p.follow = Some((99, 1));
    let dir = Dir::new("guard-stale-links");
    let st = reopened(&write(&dir.0, &running(a)).unwrap());
    let r = &st.foes[i].post.as_ref().unwrap();
    assert_eq!((r.round.lift, r.round.via, r.follow), (None, None, None));
}
```

- [ ] **Step 2: Run them to make sure they fail**

Run: `cargo test -p strata-zero --target-dir target/smoke -- guards::hunt a_guards_round_comes_back`
Expected: FAIL (`you_left` not defined; the guard never follows; the stale links stay).

- [ ] **Step 3: Implement**

Ruling 12; `after_restore` and `guards::link_to`/`take` drop a `lift`, `via` or `follow` whose link no longer stops on the guard's level (Ruling 8). In `guards::tick`, a building you're out of turns its hunting guards calm and clears their follows before holding; a hunting guard you're not fighting on its floor takes `hunt_step` when due. `ride` and `climb` call `you_left` with the link under you before landing.

- [ ] **Step 4: Run the tests**

Run: `cargo test -p strata-zero --target-dir target/smoke -- guards:: runs:: interiors::`
Expected: PASS. Then the green line.

- [ ] **Step 5: Commit**

```bash
git add crates/strata-zero
git commit -m "guards: follow you through the floors their card rides, and lose you at the street door (#33)" -m "Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 9: Shots and blasts are heard

**Files:**
- Modify: `crates/strata-zero/src/guards/hunt.rs` (`hear`; tests)
- Modify: `crates/strata-zero/src/combat.rs` (`volley`, `fire_at_you`), `blast.rs` (`land`, `detonate`)

**Interfaces:**
- Produces:
  ```rust
  pub fn hear(st: &mut AppState, place: Place, at: Pos, toward: Pos); // re-exported as guards::hear
  ```

- [ ] **Step 1: Write the failing tests**

`guards/hunt.rs` tests:

```rust
#[test]
fn a_shot_on_a_floor_wakes_the_calm_guards_within_twelve() {
    let mut st = ward();
    st.character.skills.insert(game::SkillId::new("handguns"));
    let target = goon(&mut st, 0, Pos::new(10, 3));
    let near = guard(&mut st, 0, 1, &[Pos::new(18, 3)]);   // 12, behind the wall
    let far = guard(&mut st, 0, 1, &[Pos::new(19, 3)]);    // 13
    let above = guard(&mut st, 1, 0, &[Pos::new(6, 3)]);
    crate::net::engage(&mut st, on(0), true);
    crate::net::stay(&mut st);
    crate::combat::fire(&mut st, target).unwrap();
    assert_eq!(st.foes[near].mode, crate::foes::FoeMode::Hunting { seen: Pos::new(6, 3), lost: 0 });
    assert!(!st.foes[near].post.as_ref().unwrap().spotted, "heard, not seen");
    assert!(!st.foes[far].hunting() && !st.foes[above].hunting());
}

#[test]
fn a_guards_shot_wakes_the_guards_near_it() {
    let mut st = ward();
    let shooter = guard(&mut st, 0, 0, &[Pos::new(10, 3)]);
    let near = guard(&mut st, 0, 1, &[Pos::new(22, 3)]);   // 12 from the shooter
    crate::combat::fire_at_you(&mut st, shooter);
    assert_eq!(st.foes[near].mode, crate::foes::FoeMode::Hunting { seen: Pos::new(10, 3), lost: 0 });
}

#[test]
fn a_blast_on_a_floor_is_heard_within_twelve() {
    let mut st = ward();
    let near = guard(&mut st, 0, 2, &[Pos::new(25, 3)]);   // 12 from (13,3)
    let far = guard(&mut st, 0, 2, &[Pos::new(26, 3)]);
    let satchel = st.content.item(&game::ItemId::new("satchel")).unwrap().clone();
    let (_, _, blast) = satchel.throw().unwrap();
    st.live.push(crate::blast::Live {
        place: on(0), pos: Pos::new(13, 3), blast, fx: Vec::new(), name: satchel.name.clone(),
        turns: 2, lands: 0, wakes: Some(Pos::new(6, 3)),
    });
    crate::blast::land(&mut st);
    assert_eq!(st.foes[near].mode, crate::foes::FoeMode::Hunting { seen: Pos::new(6, 3), lost: 0 });
    assert!(!st.foes[far].hunting());
}
```

A street test in `combat.rs` stays as it is: `hear` does nothing off a floor.

- [ ] **Step 2: Run them to make sure they fail**

Run: `cargo test -p strata-zero --target-dir target/smoke -- guards::hunt::tests::a_shot guards::hunt::tests::a_guards_shot guards::hunt::tests::a_blast`
Expected: FAIL (`hear` not defined; then guards stay calm).

- [ ] **Step 3: Implement**

Ruling 13: `hear` and its four callers.

- [ ] **Step 4: Run the tests**

Run: `cargo test -p strata-zero --target-dir target/smoke -- guards:: combat:: blast::`
Expected: PASS. Then the green line.

- [ ] **Step 5: Commit**

```bash
git add crates/strata-zero
git commit -m "guards: a gun or a blast on a floor wakes the guards within twelve tiles (#33)" -m "Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 10: A guard finds your body

**Files:**
- Modify: `crates/strata-zero/src/guards/hunt.rs` (`body_check`; tests, `a_guard_that_finds_your_body…` and `the_body_panel_follows_a_guard_as_it_walks`), `guards.rs` (`tick` calls it)

**Interfaces:**
- Produces:
  ```rust
  pub(crate) fn body_check(st: &mut AppState); // from guards::tick, after the guards move
  ```

- [ ] **Step 1: Write the failing tests**

`guards/hunt.rs` tests (the office copy; `kiosk_body` places a guard where it sees the body):

```rust
/// Site `b`'s first guard moved onto a tile of the body's floor two tiles from it with a
/// clear line, lit or not (2 is within the dark reach), its round held so it stands
/// there; its index.
fn guard_by_the_body(st: &mut AppState, b: BuildingId) -> usize {
    let (place, at) = st.player.as_ref().unwrap().body.unwrap();
    let map = crate::foes::map_of(st, place).unwrap();
    let spot = (at.x - 2..=at.x + 2).flat_map(|x| (at.y - 2..=at.y + 2).map(move |y| Pos::new(x, y)))
        .filter(|p| p.chebyshev(at) == 2 && map.walkable(*p) && crate::foes::line_clear(&map, *p, at))
        .find(|p| crate::foes::foe_at(st, place, *p).is_none())
        .expect("a tile beside the body");
    let i = st.foes.iter().position(|f| f.post.as_ref().is_some_and(|p| p.building == b)).unwrap();
    st.foes[i].place = place;
    st.foes[i].pos = spot;
    st.foes[i].post.as_mut().unwrap().round.next = u64::MAX;
    i
}

#[test]
fn a_guard_that_finds_your_body_dumps_you_and_raises_the_alarm() {
    use crate::sitenet::testutil::{jacked_in_at_the_kiosk, office_copy};
    let (mut st, b) = office_copy(190);
    jacked_in_at_the_kiosk(&mut st, b);
    let (_, at) = st.player.as_ref().unwrap().body.unwrap();
    let i = guard_by_the_body(&mut st, b);
    crate::guards::tick(&mut st);
    assert!(!st.player.as_ref().unwrap().jacked_in());
    assert!(st.log.iter().any(|e| e.text == "DUMPED · a guard found you · jacked out"));
    assert!(st.net.alert(b, st.net.now(&st)));
    assert_eq!(st.character.focus, 0);
    assert_eq!(st.foes[i].mode, crate::foes::FoeMode::Hunting { seen: at, lost: 0 });
    assert!(st.foes[i].post.as_ref().unwrap().radioed);
}

#[test]
fn the_body_panel_follows_a_guard_as_it_walks() {
    use crate::foes::{Foe, Post};
    use crate::sitenet::testutil::{jacked_in_at_the_kiosk, office_copy};
    let (mut st, b) = office_copy(190);
    jacked_in_at_the_kiosk(&mut st, b);
    let (place, at) = st.player.as_ref().unwrap().body.unwrap();
    let Place::Floor { level, .. } = place else { panic!("a body on a floor") };
    // One guard, walking between the tile of the body's floor farthest from it and a
    // tile beside that: far past its sight.
    let (far, next) = {
        let (_, floor) = st.interiors.floor(b, level).unwrap();
        let open = |p: Pos| floor.tiles.get(p).is_some_and(|t| t.terrain == citygen::Terrain::Floor);
        let far = (floor.tiles.iter()).map(|(p, _)| p).filter(|p| open(*p))
            .max_by_key(|p| (p.chebyshev(at), p.y, p.x)).unwrap();
        let next = far.neighbors4().filter(|p| open(*p)).min_by_key(|p| p.chebyshev(at)).unwrap();
        (far, next)
    };
    no_guards(&mut st, b);
    st.foes.push(Foe {
        name: "guard".to_string(),
        glyph: '@',
        post: Some(Post { home: Some(level), ..Post::new(b, 1, vec![far, next]) }),
        ..Foe::goon(place, far)
    });
    let mut seen = std::collections::BTreeSet::new();
    for _ in 0..400 {
        advance(&mut st, 1);
        crate::guards::tick(&mut st);
        let d = st.foes[st.foes.len() - 1].pos.chebyshev(at);
        let lines = crate::sitenet::body_lines(&st);
        assert!(lines[1].contains(&format!("nearest {d} ")), "{lines:?}");
        seen.insert(d);
    }
    assert!(st.player.as_ref().unwrap().jacked_in(), "never near enough to find you");
    assert!(seen.len() > 1, "the line followed it");
}
```

- [ ] **Step 2: Run them to make sure they fail**

Run: `cargo test -p strata-zero --target-dir target/smoke -- guards::hunt::tests::a_guard_that_finds guards::hunt::tests::the_body_panel`
Expected: `a_guard_that_finds…` FAILS (you stay jacked in); `the_body_panel_follows_a_guard_as_it_walks` already passes after Task 5 (the panel reads guards where they stand) and is here as a guard against regression.

- [ ] **Step 3: Implement**

Ruling 14. Run the strata-zero suite; sitenet tests that leave a body on a floor while the clock runs and are about something else call `no_guards`.

- [ ] **Step 4: Run the tests**

Run: `cargo test -p strata-zero --target-dir target/smoke -- guards:: sitenet::`
Expected: PASS. Then the green line.

- [ ] **Step 5: Commit**

```bash
git add crates/strata-zero
git commit -m "guards: a guard that finds the body you left jacked in pulls you out and raises the alarm (#33)" -m "Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 11: A downed guard drops its card

**Files:**
- Modify: `crates/strata-zero/src/guards/hunt.rs` (`drop_card`; tests), `combat.rs` (`wound_foe` calls it), `interiors.rs` (`left_mut`)

**Interfaces:**
- Produces:
  ```rust
  pub(crate) fn drop_card(st: &mut AppState, i: usize);
  pub(crate) fn left_mut(&mut self, b: BuildingId) -> Option<&mut Vec<(Place, Pos, ItemId)>>; // Interiors
  ```

- [ ] **Step 1: Write the failing tests**

```rust
/// Site `b`'s first guard of a zone above public, and its zone.
fn a_carded_guard(st: &AppState, b: BuildingId) -> (usize, u8) {
    (st.foes.iter().enumerate())
        .find_map(|(i, f)| f.post.as_ref().filter(|p| p.building == b && p.zone > 0).map(|p| (i, p.zone)))
        .expect("a carded guard")
}

#[test]
fn a_downed_guard_leaves_its_zone_card() {
    let (mut st, b) = crate::sitenet::testutil::office_copy(190);
    let (i, zone) = a_carded_guard(&st, b);
    let (place, at) = (st.foes[i].place, st.foes[i].pos);
    crate::combat::wound_foe(&mut st, i, 255);
    let id = crate::credentials::zone_card_id(b, zone);
    assert!(st.ground.at(place, at).iter().any(|s| s.item == id));
    assert!(st.interiors.copy(b).unwrap().left.contains(&(place, at, id)));
}

#[test]
fn a_public_guard_drops_nothing() {
    let (mut st, b) = crate::sitenet::testutil::office_copy(190);
    let (i, _) = a_carded_guard(&st, b);
    st.foes[i].post.as_mut().unwrap().zone = 0;
    let (place, at) = (st.foes[i].place, st.foes[i].pos);
    let left = st.interiors.copy(b).unwrap().left.len();
    crate::combat::wound_foe(&mut st, i, 255);
    assert!(st.ground.at(place, at).is_empty());
    assert_eq!(st.interiors.copy(b).unwrap().left.len(), left);
}

#[test]
fn the_jobs_end_takes_a_dropped_card_and_a_picked_up_guard_card_stays_in_your_pack() {
    let (mut st, b) = crate::sitenet::testutil::office_copy(190);
    let (i, zone) = a_carded_guard(&st, b);
    let (place, at) = (st.foes[i].place, st.foes[i].pos);
    crate::combat::wound_foe(&mut st, i, 255);
    let id = crate::credentials::zone_card_id(b, zone);
    let mut other = (st.foes.iter().enumerate())
        .filter(|(j, f)| *j != i && f.post.as_ref().is_some_and(|p| p.building == b && p.zone > 0));
    let (j, _) = other.next().expect("a second carded guard");
    let (jp, ja, jz) = (st.foes[j].place, st.foes[j].pos, st.foes[j].post.as_ref().unwrap().zone);
    crate::combat::wound_foe(&mut st, j, 255);
    let jid = crate::credentials::zone_card_id(b, jz);
    assert_eq!(st.ground.take(jp, ja, &jid, 1), 1);
    st.character.give(jid.clone(), 1);                       // picked up
    crate::venues::end(&mut st, b).unwrap();                 // you're outside: it ends now
    assert!(st.ground.at(place, at).iter().all(|s| s.item != id), "the one left lying goes");
    assert_eq!(st.character.count(&jid), 1, "the one you took stays");
    assert!(!st.foes.iter().any(|f| f.post.as_ref().is_some_and(|p| p.building == b)));
}
```

(`venues::end` marks the copy ending and runs `venues::tick`, which finishes it with you outside, where `office_copy` leaves you.)

- [ ] **Step 2: Run them to make sure they fail**

Run: `cargo test -p strata-zero --target-dir target/smoke -- guards::hunt::tests::a_downed guards::hunt::tests::a_public guards::hunt::tests::the_jobs_end`
Expected: FAIL (nothing on the ground).

- [ ] **Step 3: Implement**

Ruling 15.

- [ ] **Step 4: Run the tests**

Run: `cargo test -p strata-zero --target-dir target/smoke -- guards:: venues:: combat::`
Expected: PASS. Then the green line.

- [ ] **Step 5: Commit**

```bash
git add crates/strata-zero
git commit -m "guards: a downed guard leaves its zone card, and the job's end takes it with the rest (#33)" -m "Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 12: The manual, the shots and the checks

**Files:**
- Modify: `manual/06-the-city-and-the-world.md` ("Hotels and offices": a paragraph on guards), `manual/07-turns-and-fights.md` (after the opening: fights indoors and the radio), `manual/08-the-net.md` ("Jacking in": a guard can find your body)
- Modify: `crates/strata-zero/src/screens/shots.rs` (`guards_shots`)

**Interfaces:**
- Consumes: everything above.

- [ ] **Step 1: The manual**

What each chapter says, in the manual's voice (`manual/README.md`), the game's words in backticks:
- 06, after "Hotels and offices"' second paragraph: on a job, guards (`@`) walk their rounds, some up and down the stairs and lifts their card rides; each carries its zone's card and opens the doors that card opens; a dark corner hides you past 3 tiles, a lit room doesn't, and a door blocks their sight as it blocks yours; they stop walking while you're out of the building; a downed guard leaves its card on the floor, which you can pick up.
- 07, after the opening paragraphs: fights work inside as on the street, floor by floor; a guard who spots you (`guard spots you`) radios it in on its next turn (`guard radios it in · ALARM`) unless you drop it or stun it first; a shot or a blast wakes the guards within 12 tiles even through walls; a guard you haven't seen still takes its turn (`FOOTSTEPS`); taking the stairs or a lift ends the fight, but a guard whose card lets it follows you; out of the building, they give up.
- 08, "Jacking in", after the `BODY` paragraph: a guard who sees your body pulls you out (`a guard found you · jacked out`), raises the alarm, and is hunting you as you wake; the `BODY` line shows the nearest guard as it walks.

Run: `cargo test -p strata-zero --target-dir target/smoke manual`
Expected: PASS (the booklet draws it; the Controls chapter is unchanged).

- [ ] **Step 2: The shots**

`guards_shots` (`#[test] #[ignore]`, returns without `SHOT_DIR`), from `office_copy(SITE_NET_SECURITY)`, each at 120 × 40 and 100 × 30 through `sitenet::testutil::drawn` and `write_svg`, as `site_net_shot` writes:
- `guards_round_{w}x{h}`: you on L in a dark spot of a room a walking guard's round passes, the clock run until it stands 3 to 6 tiles from you.
- `guards_fight_{w}x{h}`: you on a floor in a guard's sight, engaged, after its radio: the turn readout and `guard radios it in · ALARM` in the log.
- `guards_body_{w}x{h}`: jacked in at the lobby kiosk with a guard on L, the site Net's panel showing `BODY` and the nearest guard.

Run: `SHOT_DIR=<a scratch dir> cargo test -p strata-zero --features window --target-dir target/smoke -- --ignored guards_shots`
Expected: PASS, six `.svg` and six `.png` written; look at them: the guard `@` in foe red, the log lines whole at 100 × 30.

- [ ] **Step 3: The five checks**

Run:
```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets --target-dir target/smoke -- -D warnings
cargo clippy --workspace --all-targets --features strata-zero/window --target-dir target/smoke -- -D warnings
cargo test --workspace --target-dir target/smoke
cargo test --workspace --features strata-zero/window --target-dir target/smoke
```
Expected: all pass, `every_drawn_character_has_a_glyph` and the fixtures among them.

- [ ] **Step 4: Commit**

```bash
git add manual crates/strata-zero
git commit -m "manual: guards on their rounds, fights indoors, and a guard finding your body (#33)" -m "Closes #33" -m "Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```
