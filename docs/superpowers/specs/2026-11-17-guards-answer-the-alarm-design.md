# Guards answer the alarm (#102)

When a site's alarm goes off, its guards come for you. The floor where you were placed
closes in at once. The two nearest guards on other floors run over. A guard that gets
there and doesn't find you sweeps the rooms nearby before it goes back to its round. An
alarm raised in the Net, with no sighting behind it, sends each floor's guards to sweep
their own floor, and that is how a netrunner's jacked-in body gets found. Under a camera,
the trace stops cooling, so Ghost halves what a camera costs instead of cancelling it.

## Why

#102 asked for suspicion states, noise beyond gunshots, lockdown floor by floor, guards
closing in on an alarm, and a look at Ghost. The cloak (#36) already covers getting past a
guard unseen and choosing the first hit, and Jeff likes the fights a glimpse starts, so
suspicion and noise would mostly delay a fight people enjoy. What still has no answer is
the alarm: a radio call locks doors and lifts, and the guards keep walking their rounds.
The ambush and Vanish exist to stop that call, so the call has to cost something. Jeff cut
#102 to this and the Ghost tuning, and #102 closes with a note saying why the rest went.

## Today

- A guard is a `Foe` with a `Post` (`foes.rs`). `FoeMode` is `Idle` (calm) or
  `Hunting { seen, lost }`. Off a fight, `guards::tick` runs a calm guard's
  `round::round_step` and a hunting guard's `hunt::hunt_step`. A guard on the floor of a
  fight holds there, and the fight's turns move it (`foes::step`). Every hunting guard on
  the fight's floor gets a seat (`net::foe_seats`).
- A guard that sees you hunts and marks `spotted`. On its next turn in the fight it radios
  (`foes::radio`), and `net::lock_down` raises the alarm unless the building is already on
  alert. A guard that finds your jacked-in body also calls `lock_down`
  (`hunt::body_check`). A camera's heat that reaches 60, the Net's ICE, HELIX and the site
  Net reach `net::raise_alarm` by other ways. Every alarm passes through `raise_alarm`.
- The alarm drops hacks, wakes ICE, and shuts the site's lifts to everyone, guards
  included (`links::hops` asks `sitenet::lift_answers`). It moves no guard.
- A hunting guard that reaches where it last had you without seeing you goes calm
  (`hunt::lose_you` off a fight; `LOST_TURNS` in `foes::step` in one) and walks back to
  its round.
- Cameras warm a building's trace once a game minute (`cameras::tick`, +2, or +1 with
  Ghost). Before that, in the same clock step, `NetState::minute` cools every trace you
  aren't inside the construct of by 1 (2 with a backdoor). On a floor, then, a plain
  camera nets +1 a minute and Ghost nets 0.

## The rules

### Where the building last placed you

- `NetState.placed: BTreeMap<BuildingId, Placed>`, saved with `#[serde(default)]`.
  `Placed { level, at, minute }` is the floor and the tile, and the Net minute it was
  taken.
- It is set when:
  - a guard of the building sees you on one of its floors (`foes::look`), at your tile;
  - a floor camera of the building sees you (`cameras::tick`), at your tile. A street
    camera doesn't set it, because the street isn't one of the building's floors;
  - a guard finds your body (`hunt::body_check`), at the body.
- A placing counts for `PLACED_MINUTES` = 5 Net minutes. After that, an alarm has no
  spot. **(call)** This stops a camera glimpse from half an hour ago from steering guards
  onto an empty corridor.

### The answer

`guards::answer(st, b)` runs once per alarm, when a building that wasn't on alert goes on
alert. A new `net::alarm_goes_off(st, b) -> String` is `raise_alarm` followed by
`guards::answer`, and the places where an alarm is fresh call it instead of
`raise_alarm`:

- `heat_on`'s `Alarm`, and its `Dump` when the building wasn't on alert;
- `lock_down`, when the building wasn't on alert before it set `alert_until` (HELIX's trap
  login and `burn`, the honeypot, the radio, a found body and the site Net all come
  through here, and a repeat while on alert answers nothing);
- the key plant in HELIX (`helix/bin.rs`, its own `Alarm` event).

**(call)** The dev console's alarm stays a bare `raise_alarm`.

`answer` does nothing unless you or your body are inside the building
(`venues::inside`). Its guards only move while you are, and an alarm raised from your rig
at home shouldn't leave calls waiting for the next visit. **(call)** Only standing guards
of `b` that aren't hunting answer; a hunting guard is already after you.

**With a spot** (a placing that still counts, on `level` at `at`):

- **The spot's floor.**
  - With a fight on that floor (the radio, the usual case), every calm guard on it hunts
    at once: `Hunting { seen: at, lost: 0 }`. `seat_foes` seats each of them, as a
    gunshot does, and the ones you can't see read `FOOTSTEPS`.
  - With no fight there (a camera's alarm, a found body), they get
    `Call::Answer { level, at }` like the guards from other floors. They run there calm
    and look as they go, so a fight starts only when one of them sees you. **(call)** If
    they hunted at once, every far guard would start a fight on footsteps the moment a
    camera tipped over.
- **The other floors.** The two nearest calm guards from other floors answer. They are
  ranked by the fewest link hops to `level` their card rides, then by guard order, and a
  guard whose card reaches no way there is skipped. Each gets
  `Post.call = Some(Call::Answer { level, at })`. **(call)** Two is enough to make the
  call felt without emptying the building.
- The log warns `guards close in`.

**With no spot** (the Net's ICE, HELIX, the site Net, an old placing):

- Every calm guard of the building gets a sweep of its own floor:
  `Call::Sweep { level: its level, stops }`. The stops are near the guard (below).
- The log warns `guards sweep the floors`.

### Answering

`Post.call: Option<Call>` is saved with `#[serde(default)]`.

```rust
pub enum Call {
    /// Running to where you were placed, on its floor or another.
    Answer { level: i32, at: Pos },
    /// Checking rooms on its own floor, nearest first; the first is the next.
    Sweep { level: i32, stops: Vec<Pos> },
}
```

- A guard with a call is still `Idle`, so it looks as any calm guard does: if it sees you,
  it hunts, spots you, and a fight starts. The call is left on the post but is acted on
  only while the guard is calm. Every way back to calm passes through `lose_you`, which
  clears it, or `lost_you`, which replaces it. A stale call never resumes.
- `guards::tick` runs `call_step` for a calm guard with a call, in place of `round_step`.
- **A call ends with its alert.** On a building no longer on alert, `call_step` drops the
  call and the guard goes back to its round. This also covers a call left standing while
  you were out of the building. **(call)**
- **Answer.**
  - Off `level`, it heads for `level` the way `round_step` heads for a stop on another
    level: `links::link_to`, walking to the link's stop and taking it. A lift wait goes
    through `links::wait_for_lift`. It moves at `GUARD_RUN_TICKS`.
  - On `level`, it runs to `at` at `GUARD_RUN_TICKS`.
  - At `at`, held up beside it (someone else stands there), or with no way to `at`, it
    sweeps there: the call becomes `Call::Sweep` with stops near `at`.
  - With no link left that goes to `level`, the call ends and it goes back to its round.
- **Sweep.**
  - It walks to its first stop at `SWEEP_STEP_TICKS` = 2 × `WALK_TICKS` = 8 ticks a step,
    between a run and an amble. **(call)**
  - On the stop, it pauses `SWEEP_PAUSE_TICKS` = 30 and drops the stop.
  - A stop with no way to it is dropped, and a held tile waits a step.
  - Off its sweep's level (it shouldn't be), the call ends.
  - Out of stops, the call ends and it walks back to its round. `round.via` brings it home
    by the way it came, as it does today.
- Taking a link while answering sets `round.via` as `links::take` already does, so the way
  home works unchanged.

### Sweep stops

`sweep_stops(st, b, level, near, zone) -> Vec<Pos>` returns up to `SWEEP_ROOMS` = 3 stops.

- The rooms of the floor (`Floor.rooms`) the guard may enter: the zone is at or below its
  card's, and a private room only if `guards::opens` opens its door.
- They are taken nearest first by the distance from `near` to the room's centre. The
  room holding `near` counts too, so an open hall where it lost you gets checked. A stop
  on the guard's own tile is left out.
- The stop is the room's walkable tile nearest its centre that the guard can reach. A room
  with none is skipped.
- A floor with no such rooms gives no stops: the sweep ends at once, and the guard goes
  back to its round.
- `near` is where the guard lost you (below), or, for a sweep with no spot, where the guard
  stands.

### Losing you during an alarm

- A guard that loses you while its building is on alert sweeps around where it lost you,
  rather than going straight back to its round: `Call::Sweep` with stops near its tile.
  - Off a fight, these are `hunt_step`'s give-ups: it reached `seen`, `seen` is held, it
    has no way there, or it is held up `LOST_TURNS` times.
  - In a fight, it is `foes::step`'s `LOST_TURNS` going calm.
- That runs through a new `guards::lost_you(st, i, now)`: `lose_you`, plus the sweep when
  on alert.
- These still go through the plain `lose_you` with no sweep:
  - **Vanish.** **(call)** Vanish is the escape you paid for; the guards forget you and
    go back to their rounds. It also clears any call.
  - Leaving the building.
  - Following you by a link its card doesn't ride.
- `lose_you` clears `call`.

### Lifts carry guards during a lockdown

`links::hops` stops asking `sitenet::lift_answers`. A lift always answers a guard, to the
stops its card opens. The lockdown shuts lifts to intruders, not to security. Without
this, no guard could reach another floor by lift once the alarm is on. You still ride
only with a spoof since the alarm (`interiors::ride`, unchanged).

### The footer

A calm guard with a call reads `answering` (Answer) or `sweeping` (Sweep) in place of
`idle` in `foes::lines`: `▣ guard · sweeping · 30/30 · 6 tiles`. The map's `@` is
unchanged; hunting stays bold.

### Ghost: no cooling under a camera

- In the clock step, before `NetState::minute`, the buildings whose cameras see you now
  are collected with `cameras::seeing`. This is only when a new Net minute is due, and
  never while away, because the cameras' tick doesn't run then.
- `NetState::minute` takes that set as `watched`. In the current minute `now`, a watched
  building's trace doesn't cool. An alert that is ending still ends: the ending minute
  still runs `cool`.
- A plain camera then nets +2 a minute and Ghost +1. Ghost halves it, as its card says.
- `EYE_HEAT` and `GHOST_EYE_HEAT` stay 2 and 1.

## Saving

- `Post.call` and `NetState.placed` are new fields with `#[serde(default)]`. `Call` and
  `Placed` derive `Serialize`/`Deserialize` and hold only `i32`, `Pos`, `u64` and `Vec`.
- The fixtures in `runs/fixtures/` must keep opening, unregenerated.
- No `after_restore` work is needed: a guard reopened mid-call carries on, and a call
  whose alert has ended is dropped on its next step.

## The manual

- **07 Turns and fights, "Fights indoors":**
  - After the alarm goes off, the guards on the floor where you were last seen come for
    you, and two more from other floors run over.
  - One that gets there and doesn't find you checks the rooms nearby.
  - An alarm with no sighting behind it sends each floor's guards to check their own
    rooms, and your body if you left it jacked in.
  - Lifts still carry guards in a lockdown.
  - The footer shows `answering` and `sweeping`.
  - Vanish sends them back to their rounds.
- **08 The Net:** the line on camera heat gains that the trace doesn't cool while a camera
  sees you.
- **06 The city and the world:** the Ghost line stays true and needs no change.

## Non-goals

- Suspicion or searching states between calm and hunting, and an awareness gauge.
- Noise beyond gunshots and blasts, and a sprint.
- Lockdown spreading floor by floor.
- Guards waking a resting player.
- A second answer to a second sighting during the same alert: the alarm answers once.
- Street foes: they have no post and answer nothing.

## Calls, in short

- A placing counts for 5 Net minutes.
- The alarm answers once, only while you or your body are in the building.
- The spot's floor hunts at once in a fight there; otherwise it answers calm. Two guards
  come from other floors.
- Calls end with the alert.
- Answering runs at `GUARD_RUN_TICKS`; a sweep walks at 8 ticks a step and pauses 30 at
  each of 3 rooms.
- Losing you on alert sweeps the rooms near where the guard lost you; Vanish doesn't.
- Lifts always carry guards.
- The trace doesn't cool in a minute a camera sees you.
