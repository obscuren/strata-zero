# Strata Zero

A terminal roguelike in a full cyberpunk setting. You will play it on foot
through a multi-level city, and later from inside the Net as well.

The first subsystem is in place: a seeded, deterministic city generator and the
full-screen terminal app that lets you generate, explore and walk its cities.

The generator builds a city as walkable tile maps on four strata:

| Stratum | What lives there |
|---|---|
| Net | cyberspace: a dotted grid, a construct on every building, district hubs, link lanes, ICE at the gates |
| Heights | rooftops of the tall towers, helipads, bridges between them |
| Skyways | elevated walkways, mid-rise lobbies, platforms over intersections |
| Street | districts, arterials, blocks, buildings, markets, neon |
| Undercity | tunnels under the roads, sewers, a rail line, basements, black clinics |

Every building you see at street level is the same building you stand on at
Heights. Districts have themes (corp core, night market, megablocks, combat
zone, docks, industrial, neon strip, old town) that drive street layout,
building mix, palette and signage. A Net graph of subnets, nodes and links is
anchored to the physical tiles and can be shown as a digital overlay.

The same graph is cyberspace. Jack in at any street terminal (walk up to it and
press `enter`): your body stays at the port and you emerge in the Net at the
same spot, in a wireframe world laid out one to one with the city. `enter`
again jacks out. In Explore mode `]` past Heights shows the Net.

Same seed and parameters always produce a byte-identical city.

The city is lived in. Every building is staffed and every bed is slept in: the
`sim` crate seeds a population at jack-in, with professions of its own
(shopkeepers, noodle cooks, barkeeps, corp drones, line workers, couriers,
ripperdocs, sex workers, fixers, door muscle, monks, gangers) and a daily
routine for each, so a shop is open when its keeper is behind the counter and
the night shift at the factory relieves the day shift at ten. People walk to
work along the sidewalks, and the ones who own a car take it from the kerb
when the commute is long and park it near the door. The Neon Strip is the
red-light district: its workers hold the sidewalk in front of the clubs from
nine at night and take clients into the hotels. Off shift, people head for the
bar they are known at, queue at the noodle bars at the rush, and stop for a
word (`…`) when they meet. Nobody waits for you to be looking: the whole city
moves on the clock, and only the people inside the screen's bubble are drawn;
on foot you see the ones in your sight, and a parked car stays in memory like
a pile. Walk into someone and you swap places, so a crowd never blocks the
way; `space` lists whoever stands beside you, to talk. The clock is the city's
own: it runs while you walk and stops in the free camera and in every menu, so
the overview is a still of the city, and the inspector reads whoever is under
the cursor there: who they are, what they are doing, what comes next for them,
where they live and work, and where their car is.

## Run

```sh
cargo run --release -p strata-zero
```

Headless generation, for scripts and tooling:

```sh
cargo run --release -p strata-zero -- gen --seed 42 --size medium --out cities/demo.city
cargo run --release -p strata-zero -- gen --seed 42 --size large --json --out /tmp/demo.json
```

Sizes: `small` (160x80), `medium` (240x120), `large` (320x160) or `WxH`.
Other flags: `--districts 4..12` (an upper bound; slivers merge into their
neighbours), `--water none|bay|river`, `--density 0..1`.

Needs a truecolor terminal of at least 100x30 cells (iTerm2, Ghostty, kitty,
WezTerm). Saved cities go to `./cities/*.city`.

## Keys

The game captures the mouse, so dragging over it does not select text (in iTerm2,
⌥-drag still selects). Mouse controls are not here yet; its events are dropped.

**Dev mode** is on by default for now (`DEV_DEFAULT` in `crates/strata-zero/src/app.rs`)
and shows a `DEV` badge in the header. It unlocks the free camera, stratum
browsing, the console and the inspect and legend sections. Press `` ` `` to flip it at runtime;
with it off you are always on foot.

**Title** `↑↓` select · `enter` confirm (new game opens the chooser; load game
opens the save list) · `q` quit

**New game** picks a template (VEX, KESSLER) or a custom build, then walks six
steps: ORIGIN, PROFESSION, STATS, SKILLS, NAME, SUMMARY. `↑↓` choose · `enter`
next · `esc` back. On STATS `→` buys a point and `←` refunds one; a point's cost
follows your origin's and profession's affinity. On SKILLS `→` `←` unfold and
fold a category and `space` buys or refunds it or a skill with SP. SUMMARY ends
on `enter`, JACK IN, which builds the character and generates a city. Unspent
points stay with you and can be spent from the deck.

**Configure** `↑↓` field · `←→` adjust · type digits into the seed · `enter`
on `RANDOM SEED` / `GENERATE` · `esc` back

**Generating** `space` pause · `+` `-` steps per frame · `enter` skip to the
end · `r` reseed after an error · `esc` back

**Explore** `hjkl` / arrows pan the cursor (shift: fast) · `[` `]` stratum down / up (Net on top) ·
`w` walk · `tab` deck · `F1`-`F4` panes (here, quest, map, log; `F5` `F6` inspect and legend in dev mode) · `v` the same as a popup · `m` messages ·
`d` digital overlay · `s` save · `r` regenerate · `g` configure · `:` dev console (dev mode) · `?` keys · `esc` pause

**Walk** `hjkl` `yubn` / arrows move · `<` `>` take stairs, lift, ladder or
manhole · `space` interact: take what lies here, jack in at an adjacent terminal
or out again, take the stairs, look · `1`-`5` fire the ability in that slot ·
`a` the full ability list · walking into someone swaps places with them, and
they step back once you move on · `enter` jack shortcut · `.` wait · `tab` deck ·
`F1`-`F4` panes · `v` view popup · `m` messages · `d` digital overlay · `s` save ·
`?` keys · `c` back to the free camera (dev mode) · `:` dev
console (dev mode: `spawn`, `drop`, `give`, `heal`, `xp`, `focus`, `sp`, `pts`, `who`
counts the people out, inside and driving, `clock hh:mm` jumps the day there,
`clear`, `help`, with Tab completion and history) · `esc` pause

The play HUD frames the map on one grid. The top band carries the handle,
level, credits and dev readouts, then HP, FOCUS and XP bars that run the width
of the map column; on the right, where you are and the clock, the CARRY bar and
the pane tabs. The right column holds the panes you toggle with `F1`-`F4` (here,
quest with its objective pips, the minimap with coordinates, the stamped log;
`F5` `F6` inspect and legend in dev mode), separated by thin rules. The footer
shows STATUS (on foot, jacked in, camera; overburdened in magenta), HERE (a
summary of the tile) and the verbs, then the ABILITIES row with numbered slots.
Every result and refusal lands in the log, and `m` shows the whole history. With
the log pane hidden the newest line shows briefly over the map instead.

Items lie where they are dropped: a pile shows its kind's icon in amber (`≡`
when mixed), dimmed once out of sight, and the HERE section lists what is on the
focused tile. A scan paints the map around you into memory; a ping lights the
terminals and stairs in range for a while.

**Pause** (`esc` or `q`) resume · save game · load game · settings (dev mode,
rain, fps) · quit to title · quit game

**Deck** (`tab`) `←→` or `1`-`9` tabs · `↑↓` rows · `space` act on a row ·
`esc` close; it reopens where you left it. Tabs: skills (the category board:
`enter` folds and unfolds, `space` offers an unlock priced in SP), stats (the
five stats and your pools; `space` buys a point outright), gear, inventory, workbench
(WRENCH unlocks it), cyberdeck (the jacked deck and its programs), quests,
journal (empty for now) and map (what you remember of this stratum). Every
list wraps at both ends. `space` on a gear slot picks what fits from the pack,
or unequips and inspects what is worn; on an inventory item it opens the
verbs: inspect, equip, use, drop one, drop all. In a popup the letter or
`enter` fires a row and `esc` backs out one level. The readout shows hp and the
carried weight over capacity (magenta when over, and nothing more can be picked
up). Items carry perks while worn (stat, sight, carry, max hp) and can grant
abilities; consumables heal when used. The templates, origins, professions,
categories and every item, ability, skill, talent, quest and recipe live in
`flavor/content.ron`.

## Layout

```
crates/citygen       world model + city generator (no terminal dependencies)
crates/game          characters, items, abilities, the build rules (no terminal dependencies)
crates/sim           the living city: people, places, paths and the world's minute
crates/strata-zero   the game: ratatui app, screens, renderer, walking and jacking in, the `gen` CLI
flavor/              names.ron (names, handles, signage glyphs) · style.ron (palettes, glyph sets) · content.ron (game content) · people.ron (professions and their lines)
docs/superpowers/specs   design specs
docs/superpowers/plans   implementation plans
```

## Develop

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo test -p citygen --test seeds -- --ignored   # 250-seed sweep
```
