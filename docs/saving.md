# Saving the run

How a run is saved and brought back, which parts take part, and what a change to the game
has to do so a saved run still comes back whole. The design and its reasons are in
`docs/superpowers/specs/2026-11-04-save-the-run-design.md`; this is the working map.

The short of it: **a save is a snapshot of `AppState`.** Everything in `AppState` that is
the world or your progress is written as it stands, and new state is saved without a line
of save code, as long as it follows the rules under "Adding state" below.

## The parts

`crates/strata-zero/src/runs/`:

| File | What it holds |
|---|---|
| `mod.rs` | `RunId`, `RunMeta` (id, the tick it started on, the world minute it was born), `RunHeader` and `Death` (what the lists show), `list` / `living` / `dead` / `newest` over a runs folder, and `open`, the one way to bring a run back. |
| `snapshot.rs` | `SnapshotRef` (borrowed, written) and `Snapshot` (owned, read): the same fields. `snapshot(st)` and `restore(st, snapshot)` take `AppState` apart field by field; `after_restore(st, lost)` rebuilds what the save leaves out. |
| `file.rs` | The file: magic `SZRN`, `u16` LE `FORMAT_VERSION`, `u32` LE header length, the header (`{ run: RunHeader, body: u64 }`, MessagePack with named fields), the body (the snapshot, the same, gzipped). `write` / `write_as` (temp file then rename), `read`, `header` (never opens the body). |
| `ids.rs` | Story positions by id: `StepRef`, `with_book`, the `runs` serde module for `Missions.runs`, `read_snapshot` and `drop_lost`. The compatibility fixtures and their tests. |
| `when.rs` | When it saves: `can_save`, `save_now`, `frame` (the due save and the five-minute one), `begin_run`, `flatline`, `marked` (the SAVED mark); `EVERY`, `MARK`. |
| `describe.rs` | The header made from a state, and the strings the lists print (`place_line`, `story`, `played`, `lasted`, `credits`, `saved_at`, `clock`, `first_part`). |
| `testutil.rs` | `same` (compares two values through MessagePack, maps in any order), `round` (a round trip), `Dir` (a temp folder that removes itself). |

Elsewhere:

- `app.rs`: the `AppState` fields `saves` (where this session saves; `None` in `AppState::new()`), `run`, `save_due`, `last_save`, `save_failed`; `AppState::clear_screens()`; `Screen::holds_story()`; `Runner::save_on_exit()`.
- `cli.rs`: `start_state` is the only place that turns saving on (`saves`: `runs/` in the game's folder, which `files_dir` finds through `platform::Platform::files_dir`: the player's app folder, or `STRATA_DIR`). The terminal game and the window use the same folder, wherever they're started.
- `window/mod.rs`: the window's ways out save through `Runner::save_on_exit`.
- `screens/title.rs` (CONTINUE, HALL OF FAME, dev LOAD CITY), `screens/runs.rs` (LOAD GAME and HALL OF FAME), `screens/broadcast.rs` (`Broadcast::resume`: the run's own flags, no switches), `screens/mod.rs` (`resume_screen`), `screens/explore.rs` (`MapScreen::resumed`, the `s` key, the SAVED mark on the bottom status row, the flatline switch), `screens/pause.rs` (SAVE GAME, the ways out, `last saved · …`), `screens/flatlined.rs` (taunt or reload).
- `credentials.rs`: `run_items` / `put_back`, the run-made `keycard:*` items.
- `sim::World::rebuild`, `track::refollow`, `holds::relight`, `forget_unknown` on `game::Character`, `game::Ground` and `missions::Missions`.
- `net.rs`: `NetState` is saved whole: the Net's nodes with it (`holds`: held nodes and the verbs spent, a camera's loop among them, the feed, hijacks, lights out, a decoy; `owners`: the districts' traces; `grid::GridState`: the open Net's trail, sentinels posted, tracers' and sentinels' pace, sweeps' rings, owners on alert). Grid ICE is a `foes::Foe` with `ice`. `RollEcho` reads the first saves' `building` as its `scene` (`EchoRead`).
- Content: `flavor/flatline.ron` (the taunts), `flavor/broadcasts.ron` (`during` on a broadcast).

## The world beyond the city

`st.outside` (`outside::Outside`) holds the world: it is saved as its `params` (the overworld
itself, `world`, is `#[serde(skip)]` and made again from them by `outside::reopened` in
`after_restore`), where you are (`at`), what you've seen of the world off the live city's
map (`seen`, per 8×8 chunk: the open ground, and each city's street as you left it, which
walking into it remembers), the city or town you're in (`here`), the cities and towns
you've been into (`visited`) and the place marked on the MAP (`mark`). `built` (the other
cities' maps this session) and `blocks` (the settlements' blocks) are caches made again
when first needed.
The open ground's tiles are never saved: the seed makes them again, the same every time,
and nothing the player does changes them yet.

The street carries on past the city's edge and the open ground is walked on the city
screen, so a run saved out on the open ground is `At::City` with the player off the city's
cells (its position in the city's frame, past its edge), on foot or jacked in with the body
there (the Net self anywhere on the open Net: as far from the body as it walked or the Net
overworld dropped it); it reopens as it was, jacked in too.
The tile the Net overworld is over is the screen's, not saved: a run saved from its pause
menu reopens on the map, the Net self where it stood. `outside::reopened` checks that the
cell can still be stood on (the world under a saved run can change between builds until
1.0): a body or a player on foot whose ground is gone is put back on the city's own street
nearest it, a Net self whose cell is gone wakes on the body, each with a warning. A run
saved at `At::Ground` (the outside screen's ground, before) opens on the street at that
cell, its body there and not jacked in; if the ground there is gone, back in the city as
before.
Nothing lies on the open ground: items can't be dropped off the city's cells.

In another city of the world the home city is set aside whole in `st.away`
(`outside::Stay`: the city, its people, interiors, constructs, the Net's state, the
ground's items, foes, journal, jobs, story, track, HELIX, the mail's marks and the
keycards; its `bots` stay empty, as yours come with you), saved with the run; the city
you're in is `st.city` as ever. A save taken away holds two cities, about a quarter
bigger. Walked into on foot, a city takes over where you stand
(`outside::stream::hand_over`): your cell, your bots and what you've seen of it carry
over, and a save taken a step later opens in it. Every other city or town you've been in
is kept whole (`Outside.kept`, `outside::kept`: its people, its ground, its foes, its
jobs, its Net, its keycards, as you left them) and put back as it was when you walk in
again, after a reopen too. A save keeps each as all it holds but its map
(`kept::StayParts`), the map's fingerprint as built (`kept::Built`, made when it's first
walked into) and the cells play has changed since, by 8×8 chunk, measured at the first
save after you leave it (none today: nothing in play changes a map's cells). Reopening,
`kept::remake` (in `after_restore`) builds each map again from the world, side by side
on threads of their own, puts its changes back and runs `Stay::reopened` over it; one
this build makes otherwise (another fingerprint) or can't make is dropped and logged as
`lost in the update · what you left in {name}`. So a kept city costs a save a few KB and
a reopen its build (about 120 ms): seed 3's home run saves 1.24 MB alone and 1.26 MB
with three cities kept, which whole would make it 2.03 MB. A save from before has no
`kept` (`#[serde(default)]`): the cities it had been to are made again from the world.
A city or town held but not live whose map is in view runs its
people's day on the clock (`outside::stream::run_neighbours`: the home city set aside,
those kept, and those built in the background with the people their build seeded), so
the home city set aside is saved with its people as they've run; nothing about it is
saved besides. One behind by no more than a frame catches up (an hour) runs every
frame; of those further behind, only the one furthest behind catches up in a frame. Someone who works in
a city with no home in it lives off the map (`sim::Whereabouts::Away`), coming in and
going out by a road across the map's edge (`sim::commute`; which road is theirs is made
again from the map, not saved), and so do they when a trip of theirs has no place in the
city (`sim::Dest::Away`); a run saved before that has them where the sim left them, and they
take up the commute at their next change of goal. Someone who lives in town has a drink
the city has no place for at the first it has, else stays home. A spot worker with no home keeps to the
spot, and fixers and the story's cast never leave town: the sim's mark for the cast
isn't saved, and is made again from `missions.cast` on reopening. A break at midday
(`sim::Goal::Lunch`) and a drink after work are picked by a hash of who someone is, so
nothing about them is saved but the goal and the trip in flight. A city other than home
is seeded housed (`outside::seed_people`, `sim::SimParams::housed`): everyone who works
in it lives in it, anywhere in town with room (by a hash of who they are, not the flat
nearest their job), and once its flats are full they take another round, and another,
so only one with no housing off the Strip has commuters; its people's cars are as many
as its homes make. Who has a home is the people's own, saved
with them: a city kept in a save from before keeps its commuters.

Your Net self hands over the same way: walked (or dropped by the Net overworld) onto Net
cells another city or town owns, that city is the live one and your Net self is in it on
the same world cell, the deck's own state with you (`net::Carried`: the failsafe, the
cooling, the mask and its beat on the grid, the shield). The body stays in the city you left, held: the player's
`body` is its cell in the live city's frame, past the live city's own cells, as a body
left on the open ground is; a body inside at the flat's rig stands in there by the rig's
landing (the port the cord runs to), and the home city's own `Stay.player` keeps the body
as it is, the floor and all (`Stay.player` is you as each city last had you: what you'd
seen of it, and, left on the Net, your body in it). So a save jacked in after a Net
handover opens in the city your Net self is in, the body's cell in its frame. Jacking out
puts you on that cell (`net::jack_out`, as ever), and the step's handover on foot hands the
body's city back (`outside::arrive`: on the cell its kept body stands on, you're in that
body where it is, at the rig too, and the rig's route ends as a jack out ends it). The
body's city is set aside in the save, home whole and any other kept (a body left in one
stands on a stratum: only the home city has the rig). While it's held, `outside::body_stay` reads it: the port the cord runs to, the
rig's relays and its trace home (a dump on a rig run starts that trace there).

## Writing a save

1. **Something asks.**
   - `begin_run` at JACK IN (and the dev start into a fresh city; the lot never has a run).
   - `save_due`, set by `rest::wake` and `missions::run_step`, taken by `when::frame` at the end of the map's next full frame. A step that ends under a popup or the deck is saved once the map is back on top.
   - The five-minute timer in `when::frame` (`EVERY` frames), which waits while you're `fighting` (on turns on the Net, or a foe hunting you).
   - SAVE GAME and the map's `s`.
   - The ways out, through `when::save_leaving`: the pause menu's LOAD GAME and both QUIT rows, and the window's close button, Cmd+Q, the menu bar's or Dock's Quit and a logout (`Runner::save_on_exit`, from the window's close and `exiting`, once an exit). A perma death runner killed this very frame is marked flatlined there instead of saved.
2. **`when::can_save` decides.** A folder to save in, a run, a real city (not the lot), a player, not flatlined (`net.death`), not asleep (`rest`), no scene in flight (`missions.scene`). On the way out `Runner::save_on_exit` also refuses while any screen in the stack `holds_story()`. A refused save leaves the last one standing.
3. **`file::write`.** The header from `describe::header`; the body is `snapshot(st)` written inside `ids::with_book(&st.book, …)` so story positions go out as ids; written to `<id>.run.tmp`, synced, renamed over `<id>.run` (retried briefly: Windows antivirus can hold the file). A failure logs `couldn't save · <cause>` and keeps the old file; on the pause menu's ways out it also stays on the menu and says so, and a second press leaves anyway.

The title looks in `st.saves` for a run to CONTINUE, so the game's folder is set before the first screen is built; it is a whole path, so the desktop app moving into it (`window::enter_app_dir`) changes nothing for the saves.

## Bringing a run back

The order matters; every opener goes through `runs::open`:

1. **`file::read(path, &st.book)`.** Checks the magic, the version and that the file is as long as its header says, then `ids::read_snapshot` reads the body against this build's book and `drop_lost` removes whatever pointed at a mission or step the book no longer has. Returns the losses. A flatlined run is refused.
2. **`snapshot::restore`.** Puts every saved field back (`st.tick` too, so every tick stamp keeps its meaning), puts the run-made keycards back into `st.content`, clears `rest`, stands you up out of a doctor's chair (`chair` is only set while the deck's chair is open, and no screen comes back), and `clear_screens` drops what the screens remembered of another run.
3. **`snapshot::after_restore`.** Rebuilds what isn't saved: `World::rebuild` (places, labels, night lines) and the story's cast kept in town (`missions::cast::keep_in_town`, the home city set aside too), the home city set aside (`Stay::reopened`: its people's places, and what it holds that this build's content lacks), the other cities and towns kept (`outside::kept::remake`: each map built again from the world, its changes put back, `Stay::reopened` over it; one made otherwise is lost), `home` (none in another city of the world), the street's `Stratum::dark` (the districts whose lamps a lights out cut, made from `net.lights_out` by `holds::relight`, as every tick makes it), the track's walk (`track::refollow`), and the overworld (`outside::reopened`: made again from its params; you back in the city if what you stood on is gone; if the world can't be made at all while you were away, the home city put back first). Drops content ids this build lacks (`forget_unknown`) and logs every loss as `lost in the update · …`. `drop_lost` runs over the home story set aside too, and over what the kept cities hold.
4. **The screens.** `Broadcast::resume` (LOAD GAME, CONTINUE) or straight to the map (reloading after a flatline), then `MapScreen::resumed`: it respects the restored player, sets its clock to `st.tick` so the first frame doesn't run the world a minute, and never calls `missions::begin` or `enter_city` (both would start things over).

## Adding state: the checklist

Go through this whenever you add or change something that lives through play.

1. **A new `AppState` field doesn't compile** until `snapshot` and `restore` name it. Save it (add it to `SnapshotRef` and `Snapshot` and put it back in `restore`), or bind it `_` with a comment saying why not: UI, a setting, a dev switch, content, never set at a save, or rebuilt in `after_restore`.
2. **A field inside a saved type is saved automatically** by its derive. Once a version has shipped, a new field needs `#[serde(default)]`, or the old saves won't open. The fixture tests catch a missing default.
3. **A new type held by saved state** derives `Serialize` and `Deserialize`. Not in saved state: `HashSet` (it comes back in another order; use `BTreeSet`), `Rc`/`RefCell`/`Cell`, `Instant`, closures, `&'static str`, ratatui's `Color`. `HashMap` is fine.
4. **A cache or derived field** can be `#[serde(skip)]` only if something rebuilds it on load: add the rebuild to `after_restore` and say so in a comment. If nothing rebuilds it, save it. That holds for a field in another crate's saved type too: the city's `Stratum::dark` is skipped, and `after_restore` makes it again from `net.lights_out` (`holds::relight`).
   - **Renaming or reshaping a saved field** breaks the saves already written as surely as a field without a default. Read the old shape too: `#[serde(try_from = "...")]` through a struct that takes both, as `net::RollEcho` reads the first saves' `building` as `Scene::Inside` (`EchoRead`).
   - **"Once a session" is once a run now.** A flag that stops set-up happening twice (`grid.posted`: the sentinels went out) is saved with what it made (the sentinels are foes), or a reopened run makes them again beside the saved ones.
5. **Pointing into `flavor/missions.ron`** by position (a mission index, a step index) goes through `runs::ids::StepRef`, never a bare `usize`; the save writes the ids. A new saved collection keyed by mission needs the same treatment as `Missions.runs`.
6. **Holding content ids** (items, chrome, talents, skills, abilities): on load, ids this build lacks are dropped by `forget_unknown`. A new place that holds such ids either joins the right `forget_unknown` or makes every lookup survive a missing id.
7. **Adding to `st.content` during play** (as the keycards do): content isn't saved, so anything a run adds there must be saved and put back the way `credentials::run_items` / `put_back` do.
8. **Time.** Stamp with `st.tick` (frames), the world's tick, or `net.now` (game minutes); all three come back with the run. Never the wall clock in saved state.
9. **A screen that takes story off `AppState` while it is open** (the HOLO feed pops its feed; the tray popup) returns true from `Screen::holds_story()`, or keeps the story in `AppState` until it closes. Otherwise closing the window over it saves a run whose story can't move on.
10. **Something in flight that can't be saved** (a scene) belongs in `when::can_save`.
11. **Set-up done in `enter_city_at`, `MapScreen::walking`'s first placement or `missions::begin`** doesn't run when a run is reopened. If a new system starts there, its state must be saved, or rebuilt in `after_restore`.
12. **The world and the map.** The city, the floors walked into and the constructs are saved whole. If generation grows the play field a lot, the alternative is saving the seed plus what changed and regenerating the rest on load, which needs generation to stay deterministic. Size and time today (release build, the medium seed-8 morning after): about 1.0 MB on disk; a save takes about 24 ms on the frame thread (encoding 14.6 MB of MessagePack ~11 ms, gzip ~9 ms, write, sync and rename ~5 ms), under one 33 ms frame. The `#[ignore]` test `a_save_is_timed` in `runs/file.rs` measures it again (run it with `--release`); past a frame, move the gzip and the write to a thread and keep the encoding on the frame.
13. **A screen that holds story in flight** can't stop the five-minute save, the due save or SAVE GAME, because those run from the map or the pause menu over it; only the ways out see the stack. Keep story that must survive a save in `AppState`.
14. **The map screen's own state isn't saved.** The Nets' packets (`MapScreen.traffic`: each city's and town's on its lanes, and the open Net's on its trunks) are a look only, seeded afresh round the view after a reopen; a lane ride in flight (`MapScreen.riding`, Data surfer's) ends on the lane tile you were carried to. What the run needs lives in `AppState`: a camera's feed is `net.feed`, so it reopens open; the jack window over a held terminal and the HIJACK picker hold nothing (a verb is spent as it acts), and the launch splash comes before any run.
15. **State that belongs to one city** (anything keyed by its buildings, people or districts) goes in `outside::Stay` too: `take`, `fresh` and `put` name its fields, and nothing makes that exhaustive, so a new city-bound field left out of them follows you into another city. `outside::kept::StayParts` (a kept city as a save keeps it) names them too, and a new field doesn't compile until it's there. State that belongs to the runner (the character, the deck's failsafe and cooling, the mask and the shield on your Net self, the bots out, the log) stays put; `enter` carries the deck's (`net::Carried`: a new runner's field of `NetState` goes in it) and the bots across, and you on the same world cell, your Net self too with the body where it stands.
16. **A cache in a saved type** may also be rebuilt when first needed rather than in `after_restore` (`Outside.built`, `Outside.blocks`), so long as it's `#[serde(skip)]` and made from saved state only.
17. **A city's map changed in play** (a wall blown open, a stall knocked over) is saved whole for the city you're in and the home city set aside; for another city kept, as its changed cells (`kept::Built::changes`). Change the map only through `st.city` while it's live: a kept city's changes are measured once, at the first save after you leave it, and a map held but not live is taken as it was left.

## Compatibility

- `FORMAT_VERSION` (file.rs) is the file's version. Bump it only when the layout itself changes; a newer version lists as `this build can't open it`.
- The fixtures in `crates/strata-zero/src/runs/fixtures/` are runs written by v1 and must keep opening in every later build: `run-v1.run` (a calm morning after), `run-v1-busy.run` (jacked in and on turns, a foe, a bot out, a blast on a fuse, effects, a hack and a trace, a call mid-line, the tray up, a track, chrome with a fitted piece, a HELIX host, a fixer's job, a backdoor, a feed queued; `world.dates` is the one part it couldn't fill) and `run-v1-net.run` (out on the open Net beside a street camera, the Net's nodes worked: the camera held, looped and its feed open, an ad board hijacked, a street terminal held, a lamp grid cut, a district's trace at its alarm with its sentinels out and a tracer on the trail, a sweep's ring, a crack in flight outside and its echo; `net.decoy` is the one part it couldn't fill, as a decoy only lasts inside its construct). The calm and busy runs were written before the Net's nodes: every field those added defaults. `the_first_saved_run_still_opens` reads all three, restores them and plays the map a few frames; `the_busy_fixture_holds_what_the_calm_one_leaves_empty` and `the_net_fixture_holds_the_open_nets_state` check each holds what it was written to. **Never regenerate a fixture to make its test pass.** Add `#[serde(default)]` to the new field instead. A new fixture is only written for a new format version, or for state the others were written before and can't hold (as `run-v1-net.run` was for the Net's nodes), by the `#[ignore]` writers in `runs/ids.rs`.
- Story by id means a book edit is safe: an added step waits its turn, removing the step a run was waiting on ends that mission (one that never began waits on the new first step), removing a step already passed changes nothing, a removed mission is dropped, and the log says what went.

## Tests

- `cargo test -p strata-zero --target-dir target/smoke runs::` covers the file, the snapshot, the ids and when it saves; `a_run_comes_back_whole` restores a save and compares every saved field, and `the_open_nets_state_comes_back_whole` does the same for the Net's nodes. `a_looped_camera_stays_blind_after_a_reopen` and `the_lights_stay_out_after_a_reopen` play what a reopen must keep: comparing what is written can't see a skipped field made again.
- Saving off by default: `AppState::new()` saves nowhere, and tests that save use `runs::testutil::Dir`. No test writes into the crate's folder except the `#[ignore]` fixture writers.
- Shots: `save_title_shots`, `save_load_shots`, `save_hall_shots`, `save_flatline_shots`, `save_pause_shots` (see CLAUDE.md, "Seeing the window without opening it").
