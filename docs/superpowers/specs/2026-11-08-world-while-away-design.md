# The world while you're away (#26)

Only the place you're in runs. A city you've left holds still until you come back to it.
When you do, its people must be where their day has taken them. They must not stand
where you left them, and they must not work through a queue of yesterday's errands.

The direction (from `2026-11-04-overworld-screen-design.md`, "The world while you're
away"): a short absence replays the missed time; a long one puts everyone where their
day says they'd be now. This spec adds one rule: **the catch-up is never seen.** The
replay exists for continuity, so that a man you saw walking down the street is further
down it. It is not a fast-forward for the player to watch. Sleep is the one place
where fast time shows, because there the player chose it.

## What happens today

Three kinds of city sit behind the run's clock while you're elsewhere:

- the home city, set aside in `st.away` while you're in another;
- the other cities and towns you've been in, kept whole (`Outside.kept`);
- cities built in the background and not walked into yet, with people seeded on the
  build thread (`stream.people`).

The live city also runs in jumps while you're on the overworld: each step runs its
clock for the tile's walk (`moves::step_cost`, 72 to 576 ticks) with
`PATHS_PER_FRAME` (4) paths.

A city comes back in two places:

- **Walking in** (`outside::enter` → `Stay::put`; also `outside::reopened`, which puts
  the home city back when the world can't be made again): `World::advance` runs every
  missed minute but finds no paths. People's goals
  change and their trips are queued, but nobody moves. A city built in the background
  that is more than `RUN_ON` (a day) behind is seeded again for the hour instead.
- **Back in view** (`stream::run_neighbours`): a held city whose map comes within
  `ALIVE` (20) cells of the view catches up `CATCH_UP` (60) game minutes a frame, with 4
  paths a frame. Only the city furthest behind catches up in a given frame.

`World::advance` only queues trips (`pending`). `path_some` turns them into walks and
drives, oldest first, and doesn't check whether the person still wants that trip. So
after a gap the queue holds several stale trips per person. It then drains at 4 a frame,
sending people off on errands from hours ago before the current one.

Measured on the default city (240×120, 599 people), seeded at 09:00. "Walking in"
advances the gap in one call as `Stay::put` does. "A true replay" steps a minute at a
time and finds every path as it goes:

| Away | Unmoved on walking in | Live run, unmoved | Trips queued | Queue drains in (4 a frame) | A true replay costs |
|---|---|---|---|---|---|
| 1 h | 590 | 556 | 359 | 90 frames | 27 ms |
| 6 h | 585 | 367 | 2,252 | 564 frames | 205 ms |
| 24 h | 586 | 553 | 5,777 | 1,445 frames | 776 ms |
| 72 h | 586 | 549 | 17,208 | 4,303 frames | 1.7 s |

So a short replay is cheap enough to do out of sight, and anything long has to be
placed instead of replayed.

## Goals

- A city you come back to shows its people where their day has taken them, the first
  frame you see it.
- A short absence carries on from where you left: people mid-trip are further along it
  or have arrived, and those who set off since are on their way.
- A long absence costs one pass over the people, whatever its length: no hitch on
  walking in or on coming down from the overworld.
- No stale trips: after a catch-up, nothing is left queued.
- What the city went through stays true: the dead stay dead, people the story holds stay
  held, the story's cast stays in town.

## Non-goals

- A held city's story, cameras, foes, Net and backdoors don't run while you're away.
  The story waits, as it does today; nothing here changes that.
- Off-map lives: people who are `Away` are only placed by their schedule.
- Time passing while the game is closed. A reopened run starts at its saved clock, so
  it has no gap.
- Telling the player what happened while they were gone.

## Anti-goals

- A fast-forward the player can see: people zipping about as a city comes into view.
- A hitch walking into a city, or coming home, after a long trip.
- A catch-up that moves someone the story put somewhere, or brings back someone killed.
- People heading off on errands from hours ago once you're back.

## The rule

A city whose people are behind the run's clock by a gap `g` catches up to now:

- **`g` up to an hour** (`SHORT`, 60 game minutes): **replay.** Every missed minute is
  stepped as it is today, and after each minute step the trips that minute asked for are
  found (all of them, not a frame's budget), so each trip leaves on the minute it was
  asked for. That matches the live clock, which finds about 360 paths a minute
  (`PATHS_PER_FRAME` a tick).
- **`g` over an hour: place by day.** Each person is put where their day says they'd be
  now, in one pass (see below), and the clock is set to now.

After either, the city's `pending` is empty and its tick is now.

### Never seen

- **Walking in** (`Stay::put`, from `enter` and `reopened`, and the on-foot handover,
  which goes through `enter`): the city catches up whole
  before `put` returns, so the first frame drawn is caught up. A replay costs at most
  `SHORT` worth (about 27 ms on the default city). A placement costs one pass.
- **A held city near the view** (`run_neighbours`): a city whose cells aren't on screen
  may spread a replay over frames. A city with any cell on screen that is still behind
  finishes its catch-up before the frame draws. A placement is always done in one frame.
- **Landing beside a held city**, when you come down from the overworld, come down off
  the Net overworld, or make any other move that switches to a new map screen next to one:
  - `run_neighbours` alone is too late here. A switch draws the new screen in the same
    frame, before that screen's first `update` runs (`Runner::frame`;
    `OutsideScreen::into_city` returns `ScreenCmd::Switch`).
  - So these cities catch up on the way in, before the switch (for every held city near
    where you land, generously, since the camera's view isn't known yet), or in the new
    screen's first frame before it draws. The builder picks.
- **Never seen beats the frame budget.** If several held cities land on screen behind at
  once (say you step out after 50 minutes indoors between a city and a town), each
  finishes its catch-up that frame. That costs at most a `SHORT` replay each: a rare,
  short hitch. `run_neighbours`' turns (one long catch-up a frame) apply only to cities
  off screen.
- **Built in the background and not walked into:** the same rule. A world nobody has
  touched, placed by day, comes out as seeding it for the hour would (give or take who
  roams or works the Strip where), so `RUN_ON` and the reseed give way to the rule. Seeding afresh stays acceptable where it's simpler,
  because nothing in such a world can be lost.

### The live clock in jumps

Any run of a city's clock finds the paths its people asked for in that run, not a
frame's budget:

- `clock::run`'s path budget is per tick run (`PATHS_PER_FRAME` × ticks), so an
  overworld step of 576 ticks finds up to 2,304. The cap only bounds the cost; the
  demand is a few trips a minute outside rush hour.
- A run of more than a minute finds paths after each minute step, as the replay does.
  Overworld steps are the only such runs today. Rest already runs a minute at a time
  with `PATHS_PER_MINUTE`.

So a long walk on the overworld leaves no backlog in the city you walked out of.

## Place by day

A new `sim::World` method puts a world with a history at a later tick. It builds on
`place_everyone`, which only places a freshly seeded world. For each person:

- **Dead** (`dead`): untouched.
- **Held** by the story (`held`: `keep_at_work`, `keep_where`): left exactly where they
  are, with their goal unchanged. They keep their `cut_short` mark, which is what sends
  someone `keep_where` stopped mid-trip on their way once they're let go. They also keep
  their `chat_resume`: a held person caught in a chat goes back to what they were doing
  as the chats end.
- **Everyone else:** the goal due now (`due` at the new minute of the day), placed at
  that goal's destination as `place_everyone` places it: inside a building, at a spot
  (working or loitering), or `Away`. A trip in flight is dropped; they're at its end of
  day, not on the road. Where no destination is found, someone standing or inside stays
  where they are, and someone in flight lands at the end of their trip, as an arrival
  puts them.
- **Done with a goal** (`satisfied`): honoured as the minute step honours it. If the goal
  due now is the one they're done with, they keep their current goal and are placed by
  it. Otherwise the mark is dropped.
- **The story's cast** (`in_town`): never placed off the map. One who is `Away` is
  brought back in, as the minute step's `come_back` does.

And for the world:

- **Dates** end, as a date's end has them (the worker back on the beat, the client home
  and done with the Strip), then everyone is placed. **Chats** end.
- **Cars:** a parked car stays where it is. A car in flight is parked at the kerb it was
  bound for, as a trip cut short leaves it (`park_cars_of`).
- `pending` and `displaced` are cleared, and so are `cut_short` and `chat_resume` for
  everyone who isn't held. `blocker` is unchanged: the frame sets it.
- `tick` is set to the new tick, and `minute` to its minute (`tick / TICKS_PER_MINUTE`),
  so the minute of the day is right. The minute step does not run. The open buildings
  are refreshed (`refresh_open`).
- The world's RNG is drawn from only where `dest_for` already draws from it: a roamer's
  workplace (`random_workplace`) and a beat on the Strip (`a_beat`). The RNG is part of
  the saved world, so the same world placed at the same tick comes out the same.

## Saving

Nothing new is saved: the threshold is a constant, and a catch-up never spans a save,
because it happens when a city is put in place or comes near the view. Kept cities and
the home city set aside are saved with their own tick as today, and catch up after a
reopen like any other. If a field is added to `sim::World` anyway, it takes
`#[serde(default)]`, and the fixtures in `runs/fixtures/` must keep opening
(`docs/saving.md`).

## The manual

`manual/06-the-city-and-the-world.md`, "The overworld" (next to "Your home city's story
waits for you"): a sentence saying the cities you've left carry on with their day
without you. Their people are where their day has taken them when you come back; their
story waits.

## Tests

- **sim, replay:** a world left an hour and caught up has an empty `pending`, its tick is
  now, and about as many people have moved as a live run of that hour (minute steps with
  full paths). It doesn't need to match exactly: live finds paths a tick at a time.
- **sim, place by day:** after 6 h and after 3 days:
  - nobody is `Travelling`, no car is unparked, and `pending` is empty;
  - everyone else is at the destination of the goal due now;
  - the dead are unplaced; someone held with `keep_where` or `keep_at_work` is where they
    were held, and someone `keep_where` stopped mid-trip sets off the minute after
    they're released; a cast member is never `Away`;
  - placing the same world at the same tick twice gives the same people;
  - its cost doesn't grow with the gap (one pass: 3 days costs about what 6 h does).
- **strata-zero, walking in:**
  - a kept city entered 30 minutes after leaving has replayed: people moved and nothing is
    queued;
  - one entered 6 hours later is placed;
  - coming home from another city works the same way (`st.away`).
- **strata-zero, near the view:** a held city that comes within `ALIVE` is caught up
  before any of its cells are on screen. One that lands on screen behind (put there as
  coming down from the overworld does) is drawn caught up: its tick is now when the frame
  draws.
- **strata-zero, the overworld:** an hour of overworld steps from the live city leaves
  its `pending` no larger than a minute's requests.
- The existing `RUN_ON` tests in `outside/stream.rs` are rewritten for the rule.

## Constraints

- Deterministic and portable, as the sim is today.
- The frame budget: no single catch-up on the frame thread costs more than a `SHORT`
  replay of the largest city. Several can land in one frame (see "Never seen").
- Both front ends checked (CLAUDE.md), though nothing here draws differently.

## My calls (Jeff can change any)

- `SHORT` is one game hour.
- Cars stay where they're parked after a long absence; a car in flight lands at its
  kerb. Moving each car to its owner's new place would need a free kerb per car and
  isn't worth it: seeding already parks cars at home whatever the hour.
- Dates and chats end on a placement, and someone the story holds is never moved. They
  keep what lets the story let them go again (`cut_short`, `chat_resume`).
- Never seen beats the frame budget when several cities land on screen behind at once.
- Someone in flight with nowhere to be placed lands at the end of their trip.

## Left to the builder

- Names: the placement method, the catch-up entry point, the constant.
- How a held city's replay is spread over frames out of view, and whether
  `run_neighbours`' turns (one long catch-up a frame) are still needed once a catch-up
  is at most `SHORT`.
- Whether a city built in the background is placed or seeded afresh for a long gap.
  Placed, it differs from a fresh seed only in who roams or works the Strip where.
- Where a city you land beside catches up: on the way in, before the screen switch, or in
  the new screen's first frame before it draws.
- How the minute-by-minute path finding is shared between the replay, `clock::run` and
  rest.
