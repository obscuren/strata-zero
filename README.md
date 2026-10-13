# cyberpunk

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

## Run

```sh
cargo run --release -p cyberpunk
```

Headless generation, for scripts and tooling:

```sh
cargo run --release -p cyberpunk -- gen --seed 42 --size medium --out cities/demo.city
cargo run --release -p cyberpunk -- gen --seed 42 --size large --json --out /tmp/demo.json
```

Sizes: `small` (160x80), `medium` (240x120), `large` (320x160) or `WxH`.
Other flags: `--districts 4..12` (an upper bound; slivers merge into their
neighbours), `--water none|bay|river`, `--density 0..1`.

Needs a truecolor terminal of at least 100x30 cells (iTerm2, Ghostty, kitty,
WezTerm). Saved cities go to `./cities/*.city`.

## Keys

**Dev mode** is on by default for now (`DEV_DEFAULT` in `crates/cyberpunk/src/app.rs`)
and shows a `DEV` badge in the status bar. It unlocks the free camera, stratum
browsing and the inspector with its minimap. Press `` ` `` to flip it at runtime;
with it off you are always on foot.

**Title** `↑↓` select · `enter` confirm · `q` quit

**Configure** `↑↓` field · `←→` adjust · type digits into the seed · `enter`
on `RANDOM SEED` / `GENERATE` · `esc` back

**Generating** `space` pause · `+` `-` steps per frame · `enter` skip to the
end · `r` reseed after an error · `esc` back

**Explore** `hjkl` / arrows pan the cursor (shift: fast) · `[` `]` stratum down / up (Net on top) ·
`w` walk · `tab` deck · `i` inspector · `m` minimap (inside the inspector) · `e` legend ·
`d` digital overlay · `s` save · `r` regenerate · `g` configure · `?` keys · `esc` pause

**Walk** `hjkl` `yubn` / arrows move · `<` `>` take stairs, lift, ladder or
manhole · `enter` jack in at an adjacent terminal / jack out · `.` wait ·
`tab` deck · `c` back to the free camera (dev mode) · `esc` pause · panels as above

**Pause** (`esc` or `q`) resume · save · load · settings (dev mode, rain, fps) ·
quit to title · quit game

**Deck** (`tab`) `←→` or `1`-`7` tabs · `↑↓` rows · `space` act on a row ·
`esc` close. Tabs: quests, gear, inventory, workbench, map (what you remember of
this stratum), skills and abilities. `space` on a gear slot picks what fits from
the pack, or unequips and inspects what is worn; on an inventory item it opens
the verbs: inspect, equip, use, drop one, drop all. In a popup the letter or
`enter` fires a row and `esc` backs out one level. The readout shows hp and the
carried weight over capacity (magenta when over, and nothing more can be picked
up). Items carry perks while worn (stat, skill, sight, carry, max hp) and can
grant abilities; consumables heal when used. The starter character and every
item, ability, skill, talent, quest and recipe live in `flavor/content.ron`

## Layout

```
crates/citygen     world model + city generator (no terminal dependencies)
crates/cyberpunk   the game: ratatui app, screens, renderer, walking and jacking in, the `gen` CLI
flavor/            names.ron (names, signage glyphs) · style.ron (palettes, glyph sets)
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
