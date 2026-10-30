# Story missions: authored chains on triggers, the HOLO, and the drop

Date: 2026-10-31. Designed with Jeffrey in chat; spec, plan and build run in one go at
his ask ("spec, plan and impl in one go. dont stop"). This is the "spec 2, the authored
chain" that `2026-10-18-jobs-design.md` left for later, built beside the jobs engine,
not inside it. Mockups: `2026-10-31-story-missions-mock/` (round 1 the concepts, round 2
his picks with HOLO, round 3 the glitching face; all approved).

Builds on: `2026-10-18-jobs-design.md` (fixers, the sealed package, the JOBS pane and
tab, the welcome favour this replaces), `2026-10-14-living-city-design.md` (people,
professions, shifts), `2026-10-18-interaction-design.md` (the Space window and Talk),
`2026-10-17-credentials-journal-design.md` (mails on the rig),
`2026-10-19-ranged-combat-design.md` (bodies on the map, look reading them),
`2026-10-29-window-prototype-design.md` (what draws in both front ends).

## Why

The fixers' jobs give the city work but no story. Story missions are the other half:
authored chains that flow from what happens. A new game starts one; finishing one and
letting time pass starts the next with a call; walking into a place, or talking to the
right person, moves one on. They reach you three ways: a **HOLO feed** that stops the
game for a moment that matters (the start), a **HOLO call** that rings in the corner of
your eye while you keep walking, or a conversation with someone in the street.

The first chain is the drop: Dez, the fixer nearest your flat, welcomes you to the
block and hands you a sealed package for a cook across town. When you get there the
cook is dead. Your HOLO rings: everything went wrong, the police are four minutes out,
walk, keep the package, get home. The chain stops there for now; what comes next needs
police, which the game does not have yet.

## What Jeffrey decided

- **The voice is the hustler** (round-0 text, option 1): warm, fast, slangy. The words
  of the welcome and the call are in §11.
- **The welcome is a HOLO feed (concept B):** magenta half-block brackets, a hologram
  portrait of the caller (scanlined, the chrome eye lit), the words beside it, `space`
  closes it with "take the package". Modal, at the moment the game starts.
- **The call is a card in the map's top-left corner (concept 1):** small while it
  rings (a blinking dot, `INCOMING · DEZ`, a signal glyph, `space answer · esc let it
  ring`), grown to the face and the typed lines once answered (`space next · esc hang
  up`). The game keeps running under it.
- **HOLO, not BCI.** The in-head phone is the HOLO, the genre's shared word; the frame
  reads `HOLO ▸ incoming`.
- **The face glitches** (round 3, "That's it, build it"): calm, the scanlines crawl;
  every one to three seconds a burst of two to five frames, 50–110 ms each, slips pixel
  rows a cell or two sideways, splits the colour into a magenta and a cyan fringe, and
  cuts a band to one channel or drops it dark. Whole cells only, the same in the
  terminal and the window. The feed's face glitches the same way.
- **The first chain** (playback, "It's perfect"): new game → the feed and the package;
  the walk; the cook dead where he works; the HOLO rings a moment later; Dez's lines;
  the chain stops.
- **Triggers are never hardcoded** ("We will very likely come up with more: door opens
  → special map created, NPC spawns, vehicle arrives"). Missions are data in a content
  file built from blocks (when, what, who); a new idea is one new block in Rust that
  every mission can then use. No mission's id appears in Rust.
- **The delivery replaces the mailed door-code favour** at new game.
- **The cast are authored characters on the city's own people:** Dez always has his
  name, face and voice and stands on the corner of the fixer the city put nearest your
  flat; Marlo is a cook across town in the same way.
- **Session only.** Story progress is not saved, like jobs; persistence comes next.
- **The fixers' random jobs stay as they are**, beside the story.

## Calls I made (Jeffrey: check these)

- **"Get home" is the last step and closes the chain.** The package stays in your pack
  as the hook for later; at your door Dez mails "lay low".
- **A call you don't answer goes to voicemail.** `esc` while it rings, or 30 seconds of
  ringing, ends it and leaves the lines as a mail on your rig; the chain moves on as if
  you had listened. `esc` while talking hangs up: the lines not yet heard are lost, the
  chain moves on. A story never stalls on a call.
- **While the call card is up, `space` and `esc` belong to it.** Everything else
  (walking, the deck, abilities) keeps working.
- **No item gates the HOLO.** Every runner has one; the look text says it rides the
  jack behind your ear, but nothing checks for the neural port, so no build can lock
  itself out of the story.
- **Story missions show in the JOBS pane and tab, apart from jobs:** the pane shows the
  open story mission above the job in hand; the tab lists STORY first.
- **The feed shows all its paragraphs at once** (as mocked); the call types its lines
  out one at a time with the log's reveal.
- **Every line a call says is logged** as it is shown (`Dez: “…”`), so the Log pane
  keeps what was said.

## Words

- **Mission**: one authored chain, an ordered list of steps. Content, never generated.
- **Step**: one trigger (**when**) and the effects that run when it fires (**then**).
  A mission waits on one step at a time.
- **Trigger**: a condition the engine checks, either on an event the game reports
  (new game, entering a place, talking to someone, a feed closing) or every tick (the
  player sees someone, time has passed).
- **Effect**: something a step does to the game: open a feed, ring a call, give an
  item, leave a body, set the objective, send a mail, log a line.
- **Role**: an authored character (`dez`, `marlo`) with a name, a title, a face and a
  **pick** rule; **casting** binds each role to a person of the sim.
- **Feed**: the modal HOLO window. **Call**: the corner card. Both are **HOLO**.
- **Body**: a person the story killed, lying at their station.

## Goals

- A `missions` module (`crates/strata-zero/src/missions/`) owns the content, the
  casting, the runtime state and the HOLO's drawing; the rest of the app reports events
  to it and reads it for display, as it does for jobs.
- `flavor/missions.ron` holds the cast and the missions, compiled in and validated at
  load like `jobs.ron`: unknown roles, items, steps or text slots fail the load.
- Triggers in this slice: `NewGame`, `Ended(step)`, `Sees`, `After`, `Enter`, `Talk`,
  `MissionDone`. Effects: `Feed`, `Call`, `Give`, `Take`, `Dead`, `Objective`, `Mail`,
  `Log`. Picks: `NearestFixer`, `Worker`. Each is one enum variant with its own
  evaluation, and §15 is the recipe for adding one.
- The HOLO feed screen and the HOLO call card, with the glitching face, drawn through
  ratatui only, in both front ends.
- Bodies: a person the story kills stops walking and lies at their station; the map
  draws them, look reads them, Talk is gone.
- The first chain, `the_drop`, as content; the welcome favour no longer runs at new
  game.
- The JOBS pane and tab show story missions.
- Dev console: `story` lists missions and steps; `story fire <mission>` fires the step
  it waits on; `story call` rings a test call; `story feed` opens a test feed.
- Shots of the feed and the call (ringing, talking, a glitch frame) in the shot harness.

## Non-goals

- Saving story progress (no persistence yet).
- Branching, choices in a feed or call, or dialogue trees. A step has one trigger and
  one way forward.
- Police, heat, a wanted level, a countdown for "four minutes out": words only.
- Any follow-up mission to the drop. The engine is ready for one; the content waits for
  the mechanics.
- Portraits for every person in the city. Only roles carry faces; a role without one
  calls with a generic silhouette.
- Voice acting, sound, a phone UI to call people back, contacts.
- Changing the fixers' jobs beyond moving the welcome favour out of new game.

## Anti-goals

- A trigger, effect or check that names a mission, a step or a role in Rust code.
- A HOLO that freezes the game while it rings or talks (only the feed is modal).
- A story that can stall: every call ends (answered, hung up, voicemail), every feed
  closes with one key, every pick has a fallback or fails loudly at new game in dev.

## 1. Content: `flavor/missions.ron`

Parsed with serde from `include_str!`, like `jobs.ron`, into `missions::content::Book`
(the RON struct name is `Story`). The shape:

```ron
// Story missions. A mission waits on one step at a time; when its trigger fires the
// step's effects run in order and the mission waits on the next. The last step closes
// the mission. Text slots: {role} the role's name, {role.title} its title,
// {role.place} where they work, {role.district} the district, {role.hours} the shift.
Story(
    cast: {
        "dez": Role(
            name: "Dez",
            title: "fixer",
            pick: NearestFixer,
            face: Some("hustler"),
        ),
        "marlo": Role(
            name: "Marlo",
            title: "cook",
            pick: Worker(professions: ["cook"], range: (30, 140)),
            face: None,
        ),
    },
    faces: {
        "hustler": Face(rows: [ /* 16 × 18 shade letters */ ], eye: (255, 0, 200)),
    },
    missions: [
        Mission(id: "the_drop", title: "The drop", from: "dez", steps: [ /* §11 */ ]),
    ],
)
```

- `cast` is keyed by role id; ids are lower snake case. `name` replaces the cast
  person's name in the world (§5). `title` is what the HOLO and the pane call them.
- `faces` are hologram portraits as shade letters (§9), keyed by name so two roles can
  share one. `face: None` calls with the built-in silhouette.
- `missions` run in file order. `from` is the role whose name heads the pane line.
- A `Step` is `Step(id: "...", when: <Trigger>, then: [<Effect>, ...])`. Step ids are
  unique within their mission.
- Text everywhere (feed paragraphs, call lines, objectives, mails, logs) takes the
  slots above, checked at load against the cast: `{marlo.place}` with no role `marlo`
  fails validation, as does an unknown field after the dot.

Validation (`Book::validate`, run by `load` and by a unit test on the shipped file):
every role a trigger, effect, slot or `from` names exists; every `Ended(step)` names an
earlier step of the same mission that opens a feed or a call; every `MissionDone`
names a mission; every item `Give`/`Take` names exists in `content.ron` (checked by a
test that has the content, since the loader does not); step ids unique; no mission
empty; a feed has at least one paragraph, a call at least one line; faces have
16-wide rows, an even row count and only known shade letters.

## 2. The engine: `crates/strata-zero/src/missions/`

`mod.rs` (state, `begin`, `send`, `tick`, firing), `content.rs` (the RON types, load,
validate, text slots), `cast.rs` (picks), `effects.rs` (applying effects),
`face.rs` (shades, tint, glitch), `feed.rs` (the feed screen), `call.rs` (the call
card: state, keys, drawing), `ui.rs` (pane, deck, console listing).

- `AppState.book: missions::Book` is loaded in `AppState::new` with
  `.expect("the missions pack is valid")`, beside `flavor`. `AppState.missions:
  missions::Missions` is the session's runtime, reset by `enter_city`/`enter_city_at`
  like `jobs`. (`st.story(..)` already names the log helper, hence `missions`.)
- `Missions` holds: `cast: BTreeMap<String, PersonId>`, `runs: Vec<Run>` (one per
  mission, in book order), `bodies: Vec<Body>`, `feeds: VecDeque<FeedView>` (opened,
  not yet shown), `call: Option<Call>` and `calls: VecDeque<Call>` (waiting their turn),
  `begun: bool`.
- `Run`: `at: usize` (the step it waits on), `since: u64` (game minute that step was
  armed), `state: RunState` (`Waiting` = nothing fired yet, not shown; `Open`; `Done {
  at: u64 }`; `Uncast` = a role it needs could not be cast), `objectives: Vec<String>`
  (each `Objective` effect pushes one; the last is current), `ended: BTreeSet<usize>`
  (steps whose feed or call has ended).
- **Firing:** `fire(st, ev: Option<&Event>)` walks the runs in order; for each run not
  `Done`/`Uncast`, if the trigger of `steps[at]` fires, the step's effects run in
  order, `at` advances, `since` = now, the run is `Open`, and after the last step it is
  `Done { at: now }`. Passes repeat until none fires (at most 16), so a trigger already
  true when its step is armed fires the same tick.
- **Events** the game reports with `missions::send(st, ev)`: `Event::NewGame`,
  `Event::Talked(PersonId)`. Everything else is a condition `fire(st, None)` checks
  each tick. `missions::tick(st)` runs in the explore update right after `jobs::tick`
  (it is frozen under menus like the rest of the world) and does `fire(st, None)` plus
  the call's ring timeout.
- **`begin(st)`** at new game: called from `MapScreen::enter_walk` right after
  `credentials::move_in` (the branch where a fresh body wakes at home). Once per city
  (`begun`); nothing without a home (the combat lot). It casts every role (§5), marks
  runs that need an uncast role `Uncast` (and in dev mode warns `story · no <role> in
  this city`), then sends `NewGame`.

## 3. Triggers

`enum Trigger` in `content.rs`, each arm evaluated in `Trigger::fires(&self, st,
run, ev) -> bool`:

| Trigger | Fires when |
|---|---|
| `NewGame` | the `NewGame` event |
| `Ended(step)` | the feed or call opened by that step of this mission has ended (closed, finished, hung up, voicemail) |
| `Sees(role, within: n)` | the role's body (or, alive, their street position) is on the player's place, `Visible` in the player's sight, and within `n` tiles (manhattan) |
| `After(minutes: n)` | `n` game minutes since this step was armed (a game minute is 90 ticks, 3 s) |
| `At(Where)` | the player is at `Home` (on their home floor, inside their flat: `st.home`'s `home.rect` contains their tile) or at `Work(role, within: n)` (within `n` tiles of the role's work tile, §6, on the street) |
| `Talk(role)` | the `Talked(person)` event for the role's person |
| `MissionDone(mission, minutes: n)` | that mission is `Done` and `n` game minutes have passed since |

`Where` is its own enum so new places (`District`, `Building`) slot in beside `Home`
and `Work`.

**Talk.** `Pick::Talk` learns the person it is about (`who: Option<PersonId>`). When
it fires and `missions::listens(st, who)` (a run waits on `Talk` for that person's
role), the popup sends `Talked(who)` and pops instead of showing the card, so whatever
the step opens (a feed, usually) shows at once. Otherwise Talk is unchanged.

## 4. Effects

`enum Effect`, applied in order by `effects::apply(st, mission, step, &effect)`:

| Effect | Does |
|---|---|
| `Feed(from: role, text: [..], close: "..")` | queues a HOLO feed (§7) from the role, its paragraphs filled; `close` is the label after `space` |
| `Call(from: role, lines: [..])` | rings a HOLO call (§8) from the role, or queues it behind the call that is up |
| `Give(item, count)` | gives the item (`Character::give`) and logs `` `<item name>` ×n · in your pack `` |
| `Take(item, count)` | takes up to `count` of it from the pack |
| `Dead(role, look: "..")` | the role's person dies (§6): the sim stops them, a body lies at their work tile; `look` is what look mode says of it |
| `Objective("..")` | the mission's current objective becomes this text (the pane and tab read it) |
| `Mail(from: role, subject, body: [..])` | a mail on the rig from the role's name |
| `Log("..")` | a story line in the log (backticks light names, as everywhere) |

Every text field is filled with the cast's slots at the moment the effect runs.

## 5. Casting

`cast::cast(st)` binds each role in `cast` (map order) to a person, never one person
to two roles, never a dead one:

- `NearestFixer`: the fixer whose corner is nearest the home building's door
  (`jobs::nearest_fixer`), else none. Every city has a fixer (the sim guarantees one).
- `Worker(professions: [..], at: [BuildingKind..], range: (min, max))`: people with one
  of the professions whose work is a `Spot` or a `Building` of a kind in `at` (empty =
  any kind); their work tile's manhattan distance from the home door must lie in
  `range`. Among those in range one is picked by a seeded roll (`hash64(seed, role
  index)`), so a city always casts the same; if none is in range, the nearest beyond
  `max`; if none, the farthest under `min`; if nobody at all, the role is uncast.

A cast person takes the role's `name` in the sim (`World::rename`), so Talk, look, the
inspector, the jobs' fixer name and the mails all say "Dez". Slots for text: `{role}`
the name, `{role.title}`, `{role.place}` (the work building's name, `the corner` for a
spot), `{role.district}` (the district of the work tile), `{role.hours}` (the shift as
`hours_text` writes it).

## 6. Bodies and the sim

- `sim::World` gains `kill(id)`, `is_dead(id)` and `rename(id, name)`. A dead person is
  skipped by `step` (no trips, no shuffle, no chats, no dates), has no `position`, is
  left out of `inside`, `person_at`, `visible` and `refresh_open`; `people()` still
  lists them (ids are indices), and callers that pick from it (the Deliver pool,
  `Placement::new`, casting) skip the dead.
- The **work tile** of a person: a `Spot` is its tile; a `Building` gives the walkable
  Street tile just outside its footprint nearest its first portal (closed buildings) or
  its footprint's centre (walk-in kinds like a noodle bar), searched ring by ring.
- `Dead(role)` records `Body { person, name, place: Street, pos: work tile, look }` in
  `missions.bodies` and calls `kill`.
- The map draws bodies as down foes are drawn: `%` in `FOE_DOWN` on any tile not
  `Unseen` (render/people.rs, the same loop, fed from `missions.bodies` for the place).
- Look reads a body like a down foe: the name as header, `%`, the line `dead`, and the
  body's `look` text as the description; "as you last saw it" when out of sight.
- Interact offers nothing on a body (the person is gone from the sim's tiles).

## 7. The HOLO feed (`missions/feed.rs`)

A `Screen` pushed over the map (`ScreenCmd::Push`, `TransitionKind::Glitch`): the map
below washes and stops, as under any window. The explore update pushes the next
queued `FeedView` whenever it runs with one waiting (so it opens on the first frame of
a new game, and after a popup closes).

Layout (round 2 of the mock), centred on the whole frame:

```
▛▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▜
▌  HOLO ▸ incoming                            ▂▄▆█ 22:00   ▐
▌                                                          ▐
▌  [face 20×9]       paragraph one, wrapped …              ▐
▌                    paragraph two …                       ▐
▌                    paragraph three …                     ▐
▌  DEZ · fixer       space take the package                ▐
▙▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▟
```

- Width `min(90, frame − 4)`; the text column starts 22 cells in and wraps at the
  width less 26; height fits the text (at least 16, at most the frame less 2, the text
  cut with `…` past that; the shipped welcome fits at 100×30).
- The frame is the **bracket frame**: a half-block border in magenta at 35 %, the
  corners `▛▀▀ ▀▀▜ ▙▄▄ ▄▄▟` and the cells under them in full magenta, on `PANEL_BG`.
  It lives in `missions/face.rs` beside the face as `bracket_frame(buf, rect)`, shared
  with the call.
- Header: `HOLO` magenta bold, ` ▸ incoming` dim, at the right `▂▄▆█` magenta and the
  clock `hh:mm` dim.
- The face (§9) at (3, 3) inside; under it the role's name upper case, magenta bold,
  and ` · <title>` dim.
- Last text row: `space` magenta bold and the feed's `close` label dim.
- `space`, `enter` or `esc` pops the feed and marks its step `Ended`.

## 8. The HOLO call (`missions/call.rs`)

Not a screen: state in `missions.call`, keys taken by the explore screen, drawn by the
explore screen over the map's top-left corner (`r.map.x + 1`, `r.map.y + 1`; the turn
column owns the top-right). The world runs on under it.

- `Call { mission, step, from: role, lines: Vec<String>, rang: u64 (clock tick),
  answered: Option<u64>, line: usize, shown: u64 (clock tick the current line began) }`.
- **Ringing** card, 36 × 5, bracket frame: `●` blinking with `○` every 15 ticks,
  `INCOMING · <NAME>` magenta bold, `▂▄▆█` at the right; `<title> · <district>` dim;
  `space` answer · `esc` let it ring.
- **Talking** card, `min(62, map width − 2)` × 13: `● <NAME> · mm:ss` (time since
  answered, real seconds) and the signal; the face at the left; the lines at the right
  wrapped to the card, the ones said so far in `TEXT` with a blank between, the
  current one typing at 60 characters a second with a magenta `▌` after it; older lines
  scroll off the top when the card is full. Last row: `space next` (or `space hang
  up` on the last line once typed) and `esc hang up`.
- **Keys**, only while a call is up, before the walk keys: `space` answers a ringing
  call, finishes the typing of a line, or moves to the next (after the last, ends the
  call); `esc` sends a ringing call to voicemail or hangs up a talking one. Every other
  key goes to the map as usual.
- Each line is logged as it starts: `` `Dez` · “<line>” ``.
- **Voicemail**: 900 clock ticks (30 s) of ringing, or `esc` while ringing, ends the
  call, mails every line to the rig (`from` the name, subject `voicemail`) and logs ``
  `Dez` · voicemail on your rig ``.
- However it ends, the step is marked `Ended` and the next queued call (if any) rings.
- Clock ticks (`clock_tick`) time the ringing and the typing, so both pause under a
  window; the face's glitch runs on the frame tick and never pauses.

## 9. The face (`missions/face.rs`)

- A face is 16 pixels wide and an even number of rows (18 for the shipped ones), each
  pixel a shade letter: `.` empty, `d` dark, `m` mid, `l` light, `h` highlight, `b`
  shadow, `e` the eye's white, `X` the chrome eye. Drawn into a 20-wide canvas (two
  empty columns each side for the slips) as half blocks (`ui::pixels::Pixels`), 9 rows
  for 18 pixels.
- Holo tint: `d` 22 %, `m` 45 %, `l` 75 %, `h` 100 %, `b` 30 % of cyan (0, 229, 255);
  `e` (232, 255, 255); `X` the face's `eye` colour.
- **Calm:** scanlines; every other pixel row at 60 % (not `e` or `X`), the phase
  flipping every 4 frame ticks.
- **Bursts:** time is cut into 60-tick cycles; each cycle has one burst starting at a
  seeded offset in its first 30 ticks, of 2–5 frames each lasting 2 or 3 ticks; each
  frame picks one of the five recipes of round 3 (`gen3.py`), with row bands as
  fractions of the face's height:
  1. rows 6–9 slip right 2, with a fringe;
  2. rows 2–4 slip left 1, rows 12–14 right 1, rows 10–11 cut to (×3.0, ×0.2, ×1.0);
  3. every row slips right 1 with a fringe, rows 5–7 drop to 25 %;
  4. rows 8–10 slip left 2 with a fringe, rows 14–17 cut to (×0.2, ×1.0, ×0.4);
  5. rows 3, 4, 9, 10, 15 slip right 1, rows 6, 7, 12 left 1, rows 6–7 cut to (×2.4,
     ×0.3, ×1.2).
  A **fringe** puts a magenta pixel (80 %) on the empty pixel left of every lit pixel
  and a pale cyan (127, 246, 255) on the empty pixel right of it. A **cut** multiplies
  the channels, clamped. `face::pixels(face, tick, seed) -> Pixels` is pure, so a shot
  or a test can ask for a given frame.
- `face: None` draws the face named `silhouette` from the book, which validation
  requires.

## 10. The JOBS pane and tab

- The pane (`missions::ui::pane_lines`, put before `jobs::ui::pane_lines` where the
  sidebar's `jobs` lines are built) shows each `Open` run: ` THE DROP · Dez · story`
  (title magenta bold, the rest dim) and ` » <current objective>`. With a story line
  shown, the jobs' `no job on · find a fixer` line stays as it is. `JOBS_CAP` goes from
  4 to 5.
- The deck's JOBS tab lists, when any run is `Open` or `Done`, a `STORY` header row,
  the runs (`  <title>` open, `✓ <title>` done), a `JOBS` header, then the jobs as
  today. With no story yet the tab is exactly as today. `Detail::Mission(usize)` shows
  the title, `from <name> · story · open|done`, then the objectives: earlier ones `☑`
  dim, the current `☐`.

## 11. The first chain: `the_drop`

```ron
Mission(id: "the_drop", title: "The drop", from: "dez", steps: [
    Step(id: "welcome", when: NewGame, then: [
        Feed(from: "dez", close: "take the package", text: [
            "Ey, welcome to the block, choom. Word on the wire says you got what it takes. Preem. Let's find out.",
            "Got a package needs walking across town. Nothing heavy, nothing hot. Goes to {marlo}, cooks noodles at {marlo.place} in {marlo.district}. Into his hands. Not the counter, not his cousin. Hands.",
            "Do it clean, there's more where that came from. Don't open it. Don't get cute. Don't be late.",
        ]),
    ]),
    Step(id: "package", when: Ended("welcome"), then: [
        Give(item: "sealed_package", count: 1),
        Dead(role: "marlo", look: "Face down on the pavement, apron still on. Not breathing."),
        Objective("Hand the package to {marlo} · {marlo.place} · {marlo.district}"),
    ]),
    Step(id: "body", when: Sees(role: "marlo", within: 6), then: [
        Log("`{marlo}` is dead"),
    ]),
    Step(id: "ring", when: After(minutes: 1), then: [
        Call(from: "dez", lines: [
            "Choom. Choom, you there? Whatever you're looking at, don't touch it.",
            "{marlo} was meant to be breathing when you got there. Somebody got there first.",
            "Badges are four minutes out, and they'll love you for this. Walk, don't run. Keep the package. Get home.",
        ]),
    ]),
    Step(id: "run", when: Ended("ring"), then: [
        Objective("Get home · keep the package"),
    ]),
    Step(id: "home", when: At(Home), then: [
        Mail(from: "dez", subject: "lay low", body: [
            "You made it home. Good.",
            "Keep the package somewhere they won't look, and don't open it.",
            "Lay low. I'll call.",
        ]),
        Log("`The drop` · home · `{dez}` will call"),
    ]),
]),
```

Cast: `dez` = `NearestFixer`, face `hustler` (the round-3 face, magenta eye); `marlo`
= `Worker(professions: ["noodle_cook"], at: [], range: (30, 140))`, no face (he never
calls). Faces: `hustler` and `silhouette` (a plain head and shoulders in `d`/`m`, no
features).

## 12. What goes

- `credentials::move_in` no longer calls `jobs::first_favour`; the welcome mail and its
  door code are gone from new game. `first_favour` and `first_fetch` become
  `#[cfg(test)]` and `jobs::testutil::state_home` calls `first_favour` after `move_in`,
  so the jobs tests keep their Fetch. Tests and shots that met the welcome through
  `MapScreen::walking` (the keypad shot, the rig's mail test, the offer stack, the
  interact test) call it themselves or stop assuming it.
- Nothing else of jobs changes.

## 13. Dev console

`story` lists the cast (`dez → <person id> <generated name>`) and every mission with its
state and the step it waits on (`the_drop · open · waits on ring: After(1)`). `story fire
<mission>` runs the waiting step's effects as if its trigger fired. `story feed` queues a
test feed from the first role with a face; `story call` rings a test call from it.
`COMMANDS` and `help` learn `story`.

## 14. Testing

- Content: the shipped book parses and validates; each refusal (unknown role, unknown
  slot or slot field, `Ended` naming a step that opens neither, an unknown mission, a
  duplicate step id, a bad face) is its own error; every item named exists in
  `content.ron`.
- Casting: on seed 8 both roles cast, Dez is `nearest_fixer` of the home door, Marlo a
  noodle cook in range; the same city casts the same; over seeds 1–16 (medium) the drop
  casts every time.
- Engine, through `send`/`tick` on a real city: the welcome queues a feed; closing it
  gives the package, kills Marlo (dead in the sim: no position, not inside, not at his
  tile), sets the objective; standing within 6 with the tile visible runs `body`; a
  minute later the call rings; answering and stepping logs each line; voicemail after 900
  ticks mails the lines; hanging up ends it; `At(Home)` mails "lay low" and closes the
  mission. A test book exercises `Talk`, `MissionDone` with minutes, `At(Work)`, `Take`.
- Sim: a killed person is skipped by `step`, `position`, `inside`, `person_at`,
  `visible`; `rename` renames.
- Drawing: the body as `%` once seen; look reads it; the feed at 100×30 and 120×40 keeps
  every word of the welcome; the call card fits a 100×30 map; `face::pixels` is pure and
  a burst frame differs from a calm one; the glyph test passes.
- Pane and tab show the story; the console's `story` lines.
- Shots: `holo_feed`, `holo_call_ringing`, `holo_call_talking`, `holo_call_glitch` (a
  burst frame), each at 120×40 and 100×30.
- Both front ends: fmt, clippy and tests with and without `strata-zero/window`, into
  `target/smoke`.

## 15. Adding a block (the recipe)

- **A trigger** (say "a door opens"): a variant on `Trigger` (and on `Where` if it is a
  place), its arm in `Trigger::fires`, its names in `validate`; if it is an event, a
  variant on `Event` and one `missions::send` where the game does it (the doorway in
  explore). A test with a small book.
- **An effect** (say "a vehicle arrives", "an NPC spawns", "make a map"): a variant on
  `Effect`, its arm in `effects::apply`, its names in `validate`. A test.
- **A pick** (say "someone in this district"): a variant on `Pick` and its arm in
  `cast::pick`.
- No mission id, step id or role id ever appears in Rust outside tests.
