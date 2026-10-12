# cyberpunk

A terminal game in a full cyberpunk setting. This is the first piece: a seeded,
deterministic **city generator** and the full-screen terminal app around it.

The generator builds a multi-level city as walkable tile maps on four strata:

| Stratum | What lives there |
|---|---|
| Heights | rooftops of the tall towers, helipads, bridges between them |
| Skyways | elevated walkways, mid-rise lobbies, platforms over intersections |
| Street | districts, arterials, blocks, buildings, markets, neon |
| Undercity | tunnels under the roads, sewers, a rail line, basements, black clinics |

Every building you see at street level is the same building you stand on at
Heights. Districts have themes (corp core, night market, megablocks, combat
zone, docks, industrial, neon strip, old town) that drive street layout,
building mix, palette and signage. A Net graph of subnets, nodes and links is
anchored to the physical tiles and can be shown as a digital overlay.

Same seed and parameters always produce a byte-identical city.

## Run

```sh
cargo run --release -p city
```

Headless generation, for scripts and the game later:

```sh
cargo run --release -p city -- gen --seed 42 --size medium --out cities/demo.city
cargo run --release -p city -- gen --seed 42 --size large --json --out /tmp/demo.json
```

Sizes: `small` (160x80), `medium` (240x120), `large` (320x160) or `WxH`.
Other flags: `--districts 4..12`, `--water none|bay|river`, `--density 0..1`.

Needs a truecolor terminal of at least 100x30 cells (iTerm2, Ghostty, kitty,
WezTerm). Saved cities go to `./cities/*.city`.

## Keys

**Title** `↑↓` select · `enter` confirm · `q` quit

**Configure** `↑↓` field · `←→` adjust · type digits into the seed · `enter`
on `RANDOM SEED` / `GENERATE` · `esc` back

**Generating** `space` pause · `+` `-` steps per frame · `enter` skip to the
end · `r` reseed after an error · `esc` back

**Explore** `hjkl` / arrows pan (shift: fast) · `[` `]` stratum down / up ·
`w` walk · `i` inspector · `m` minimap · `e` legend · `d` digital overlay ·
`s` save · `r` regenerate · `g` configure · `?` keys · `q` title

**Walk** `hjkl` `yubn` / arrows move · `<` `>` take stairs, lift, ladder or
manhole · `.` wait · `tab` back to the free camera · panels as above

## Layout

```
crates/citygen   world model + generator (no terminal dependencies)
crates/city      the ratatui app and the `gen` CLI
flavor/          names.ron (names, signage glyphs) · style.ron (palettes, glyph sets)
docs/superpowers/specs   design spec
docs/superpowers/plans   implementation plan
```

## Develop

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo test -p citygen --test seeds -- --ignored   # 250-seed sweep
```
