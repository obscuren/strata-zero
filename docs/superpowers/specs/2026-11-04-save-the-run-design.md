# Saving the run · design

Issue #2 of the milestone "First public release (v0.2.0)". Approved from the mock page
(`2026-11-04-save-the-run-mock/`, `gen.py` over the throwaway `save_mock_frames` shot
test in `save_mock.diff`): "perfect". Jeffrey's words on the way there:

- "A save game shouldn't be deleted, instead it should be marked as FLATLINED and can't
  be open anymore. It should go in to the hall of [fame] (extra menu on the home screen)."
- "Most everything should be persisted. Can't we somehow snapshot the state of the game?
  For example the netrunner session is also building things like 'camera loops' which
  makes camera's later in the game be unable to detect you. Also lights in the city
  which have been turned of. Thats all state of the game and should be fully persisted
  including the things we don't have yet."
- On the flatline: with perma death, no line about the hall, "just a punky line,
  something taunty"; without, "reload last save point" and "space · load last save".

## Why

A save today is the city map and nothing else: `save::save_city` writes the `City`, and
loading one goes through `AppState::enter_city_at`, which starts the run over (missions,
journal, mail, HELIX, jobs, tracking). Someone who quits halfway through the morning
after comes back to a fresh start. Perma death is picked on NAME but nothing reads it.

Goal: a run survives quitting and crashing, comes back exactly as it was left, and a run
that ends in a flatline is remembered.

Non-goals: save slots or named saves; deleting or renaming runs from the game; moving
the save folder for terminal installs (#3); keeping settings between sessions; saving
on the lot (the dev COMBAT start).

Anti-goals: a save that drops part of the world without anyone noticing (a camera loop
undone after a load); a save from an older build that crashes a newer one; a perma
death run that quitting gets you out of.

## 1 · The snapshot

The save is the run as it stands, not a list of things someone remembered to keep.

- **The run is every field of `AppState` that is the world or your progress**: the
  city, you, the city's people where they are this minute (`sim::World`, its dice
  included), the floors walked into, the constructs, the Net and what you did to it,
  your backdoors, things on the ground, foes on you, bots out, blasts on a fuse,
  effects in flight, the journal, mail, HELIX, jobs, the story, tracking, the log, the
  clock and `tick`, `shots`, perma death and the tutorial flag.
- **Left out**: what is the screen's (the deck's remembered tabs and rows, the atlas
  view, `look_at`, `jump_to`, `drawn_you`, `repaint`), settings (`rain`, `show_fps`,
  `effects`, `screen_fx`, `cord`, `sidebar`), dev switches (`dev`, `dev_unlocked`,
  `show_eyes`, `show_light`, `free_chrome`, `force_hit`), the game's content (it comes
  with the build), and `build` (the sheet being made before JACK IN).
- **New state is saved by default.** One function turns `AppState` into the snapshot by
  destructuring it field by field with no `..`, so a field added to `AppState` doesn't
  compile until it is either in the snapshot or left out with a comment saying why. Every
  type in the snapshot derives `Serialize` and `Deserialize`, so a field added inside
  one (a camera loop on `NetState`, a lamp grid that remembers what was cut) is saved
  without a line of save code. Restoring is the same destructuring the other way.
- A part of a saved type is skipped only when it is rebuilt from the rest on load, and
  the skip says what rebuilds it. When unsure, save it.
- `st.tick` is restored with the run, so everything stamped with a tick (effects, the
  log's reveal, the mail pill, the rest) keeps its meaning.

## 2 · The file

- One file per run: `runs/<id>.run` beside `cities/` in the working folder (#3 moves both
  for terminal installs). `<id>` is 16 hex digits, made when the run starts.
- Layout: the magic `SZRN`, a little-endian `u16` format version (starting at 1), a
  little-endian `u32` header length, the header, then the body.
  - **The header** is what the lists show, so listing never opens a body: run id,
    handle, profession, level, city name and seed, the day and minute, where you stand
    (the line the card shows, `Velform Group Plaza 18F · at home`), the story (the
    mission's title and its objective), perma death, money, chrome count and names,
    talent count, seconds played, when it was saved (Unix seconds), and the death when
    there is one (cause, day and minute, Unix seconds).
  - **The body** is the snapshot.
  - Both are MessagePack with named fields (`rmp-serde`), the body gzipped (`flate2`,
    fast). Measured on seed 8 (release): the city alone is 8.2 MB raw, 726 KB gzipped,
    about 13 ms to write and 16 ms to read.
- Written to `<id>.run.tmp`, then renamed over the old file, so a crash mid-write keeps
  the last good save.
- **Older saves open in newer builds.** Every field added to a saved type after this
  ships carries `#[serde(default)]`. A run file written by this build is checked in as a
  fixture, and a test loads it in every later build. A header this build can't read, or
  a newer format version, shows its row dim in LOAD GAME as `this build can't open it`.

## 3 · When it saves

- When the run starts: JACK IN, once you stand in the city.
- When you wake (a rest ends), when a story step ends, on SAVE GAME, and on QUIT TO
  TITLE and QUIT GAME.
- When the window closes (its close button, Cmd+Q), if the run can be saved right then;
  otherwise the last save stands.
- Every five minutes of play, when the run can be saved (see the builder rulings).
- The run can be saved when a city is entered and isn't the lot, you aren't flatlined,
  you aren't asleep (`st.rest`), and no screen holds story in flight (a scene, a film, a
  call). The deck, the terminal, a popup or the pause menu over the map don't stop it.
- Each save puts the SAVED mark up. A failed write puts a warning in the log
  (`couldn't save · <reason>`) and keeps the old file.

## 4 · The title

- Rows: CONTINUE (only when a run that isn't flatlined exists), NEW GAME, the dev
  starts in dev mode, LOAD GAME, HALL OF FAME, QUIT. The cursor starts on CONTINUE when
  it is there.
- While CONTINUE is selected, two rows under the menu, centred: the newest run's line,
  `VEX · NETRUNNER · Saintburg · day 2 07:40 · Bring it to Coz` (the handle cyan and
  bold, the objective dim, the rest faint).
- CONTINUE opens that run as LOAD GAME would.
- NEW GAME with runs already saved starts another run; nothing is overwritten.
- In dev mode, LOAD CITY after LOAD GAME opens today's `.city` list (the old LOAD GAME),
  and the pause menu gains SAVE CITY. Players never see either.

## 5 · LOAD GAME

- The files panel, retitled `LOAD GAME`. A header row (faint): RUNNER, BUILD, LVL, CITY,
  CLOCK, THE STORY. One row per run that isn't flatlined, newest saved first; `PERMA`
  (magenta, bold) at the right of a perma death run. The selected row teal with `▸`.
- Under a rule, the selected run's card: `VEX · NETRUNNER · level 4` with `PERMA DEATH`
  at the right; `Saintburg · day 2 07:40 · Velform Group Plaza 18F · at home`;
  `The ledger · Bring it to Coz · the Brass Lamp · Ironside`;
  `¤ 1,240 · 2 chrome · 6 talents · played 3h 12m · saved today 10:44`.
- Keys: `↑↓ select · space continue · esc back`. Empty: `No runs yet.`
- Opening a run plays a LoadGame broadcast, and JACK IN on it puts you where you stood.
- From the pause menu, LOAD GAME saves the run you're in first; it is in the list.

## 6 · HALL OF FAME

- The same panel, `HALL OF FAME`. Header: RUNNER, BUILD, LVL, CITY, LASTED, THE END. One
  row per flatlined run, newest death first. LASTED is city time from the run's start to
  the death (`3 days`, `19 hours`, `40 min`); THE END is the cause, cut to fit.
- The card: `MOTH · NETRUNNER · level 6` with `FLATLINED` (red, bold) at the right; the
  cause, day, clock and date (pink); `Saintburg · lasted 3 days · played 5h 40m`;
  `got as far as  The ledger · Bring it to Coz`; `left behind    ¤ 2,340 · neural port ·
  target lens · 9 talents`.
- Keys: `↑↓ select · esc back`. Nothing opens a run from here.
- Empty: `Nobody you ran has flatlined. Yet.`

## 7 · The pause menu and the mark

- The rows stay. SAVE GAME saves the run on the spot (no name to type). LOAD GAME opens
  LOAD GAME after saving. Both QUIT rows save first.
- Under the rows, faint: `last saved · just now`, `· 1 min ago`, `· N min ago`,
  `· N h ago`, by the wall clock.
- The SAVED mark: ` :: SAVED` (teal, bold) after the place line on the map's bottom
  status row, for three seconds after a save.

## 8 · The flatline

- **Perma death on**: the run is saved one last time, marked with the death (cause, day,
  clock, date), and from then on is only in the HALL OF FAME. The screen: the FLATLINED
  logo, the cause, `VEX · level 1`, then one taunt (pink) and `any key · title`. The
  taunts live in `flavor/flatline.ron`, one picked by the run's id:
  `somebody's already wearing your jacket.`, `your rent's still due.`,
  `should've stayed in bed.`, `the street forgot you before you hit the floor.`,
  `the ICE didn't even slow down.`, `they'll sell your chrome by morning.`
- **Perma death off**: nothing is marked. The screen: the logo, the cause, the runner,
  then `reload last save point · day 2 07:40 · at home` (dim, from the save's header) and
  `space · load last save     esc · title`. Space loads that save; esc goes to the
  title, where CONTINUE offers it.

## 9 · The courier broadcast

Broadcasts gain an optional `during: Some("<mission id>")`. A LoadGame broadcast with
`during` fits only while that mission is under way (started, not done), and when one
fits it is picked over the others. `courier` gets `during: Some("morning_after")`, so
it airs on a load while its warning ("don't open it") still holds.

## Calls I made

- MessagePack with named fields, gzipped, over postcard: postcard can't open a save
  once a struct gains a field; MessagePack with `#[serde(default)]` can. The city costs
  726 KB on disk.
- The snapshot by exhaustive destructuring rather than moving the run's fields into a
  struct of their own: the compiler still forces every new field to be classified, and
  the thousands of `st.<field>` uses stay as they are.
- A flatlined run leaves LOAD GAME for the HALL OF FAME (Jeffrey agreed the mock).
- A save in the middle of a scene, a film, a call or a sleep isn't taken; the last one
  stands. Those are short, and a screen in flight isn't part of the run.
- The runs folder holds runs only; `.city` files stay for dev and the command line.
- Time played is frames, not wall time: the run keeps the `tick` it started on, and
  since `tick` is restored with the run, played = (`tick` − start) / 30 seconds.

## Builder rulings

A builder read this before planning and asked three things. My answers:

- **The broadcast's switches on a loaded run.** Today the LoadGame broadcast offers the
  same `p` (perma death) and `t` (tutorial) switches as a new game, and JACK IN writes
  them into the run. On a run that is opened (CONTINUE, LOAD GAME), the broadcast shows
  the run's own flags and offers no switches, so quitting and reopening can't turn perma
  death off. "space · load last save" on the perma-off flatline goes straight back into
  the city, with no broadcast.
- **Content that changes between builds.** The story's state points into
  `flavor/missions.ron` by position: missions in book order, steps by index, and
  `(mission, step)` pairs in calls, feeds, scenes, the tray and the news. A build that
  adds or reorders a step would make an old run skip or re-fire steps (mail sent twice,
  an item given twice). So the save writes story positions **by mission id and step id**,
  translated through the built-in book on the way out and on the way back in. The engine
  keeps its indices while it runs; one small saved type for a mission and step reference
  (`StepRef`) does the translating wherever such a pair is saved. On load, anything whose
  id this build no longer has (a mission, a step, an item, a piece of chrome, a talent)
  is dropped, and the log says so (`lost in the update · …`). An ended step whose id
  still exists never fires again. Nothing crashes on a missing id.
- **Other ways a session ends, and crashes.** The window's close button and Cmd+Q save
  (the run permitting). The terminal build has no quit key during play, and closing the
  terminal kills the process without the game knowing. So instead of catching signals,
  the game also saves **every five minutes of play** (9,000 frames) when the run can be
  saved; that bounds what a crash or a killed terminal loses. Nothing saves from a
  panic: a state that panicked could panic again on every load and leave a run that never
  opens, and the write being atomic already keeps the last good save. Killing the
  process to escape a perma death fight isn't policed, any more than copying the file is.
