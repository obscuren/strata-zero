# The breach splash Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Open the game on concept D, Key: a memory dump cracked for a key that is the
game's name, whose bytes fly up into the title's logo, handing over to the title with no
cut.

**Architecture:** One new screen, `screens/splash.rs`, drawn as a pure function of the
ticks since it opened (no state but `opened` and a seed). `screens::initial` opens it;
it `Switch`es to the title at tick 150 or on any key. The logo's colour and spot are
shared with the title through two small public functions so the hand-over is exact.

**Tech Stack:** Rust, ratatui buffers, crossterm keys, `citygen::rng::hash64`.

**Spec:** `docs/superpowers/specs/2026-11-04-breach-splash-design.md` (reference
animation: `docs/superpowers/specs/2026-11-04-breach-splash-mock/splash.js`, `KEYDUMP`).

## Global Constraints

- 30 ticks a second; every time in the spec is in ticks since the splash opened.
- Hand-over at tick 150; any key hands over at once (glitch transition), natural end with
  `TransitionKind::None`.
- Cyan `Rgb(0,229,255)`, magenta `Rgb(255,0,200)`, white, the greys DIM `(160,170,185)`,
  FAINT `(120,130,143)`, FAINTER `(92,102,115)`.
- Cells the splash leaves empty are reset cells (no background), like the title's.
- Only characters already drawn by the game: `▶ ◀ · │ █ ░`, `effects::GLITCH_CHARS`,
  ASCII.
- Build into `--target-dir target/smoke`; check with and without
  `--features strata-zero/window`.

## Review Focus

- **Mashing a key to skip** (Esc twice, Enter held): the second press must not quit or
  start a game. The title opened from the splash swallows keys for its first 10 ticks
  (Task 2 test `a_key_mashed_through_the_skip_does_nothing`).
- **Terminal resized mid-splash**: every frame lays out from `f.area()`; drawing at any
  size ≥ 100 × 30 never panics (Task 2 `draws_at_every_tick_and_size`).
- **A tall terminal** (300 × 80): the dump's first row stays at a non-negative row
  number (`KEY_ROW` = 1000) (same test).
- **A frame that skips ticks** (slow machine): the hand-over is `>=`, not `==` (Task 2
  `it_hands_over_at_150` updates at 152 too).
- **Nothing left behind at the hand-over**: on ticks 147–149 the splash draws the logo
  and nothing else (Task 2 `the_hand_over_is_seamless`).

---

### Task 1: The title's logo colour and spot, shared

**Files:**
- Modify: `crates/strata-zero/src/ui/logo.rs` (`Logo::render`)
- Modify: `crates/strata-zero/src/screens/title.rs` (`Title::draw`)

**Interfaces:**
- Produces: `pub fn letter_colour(tick: u64, i: usize, row: u16, accent: Rgb, steady: bool) -> Rgb`
  in `ui/logo.rs`: the colour `Logo::render` gives row `row` of letter `i` (flicker with
  `i % 7 == 3` broken unless steady; the scanline row mixed half to white).
- Produces: in `screens/title.rs`, `pub const LOGO_ACCENT: Rgb = Rgb(255, 0, 200);` and
  `pub fn logo_origin(area: Rect) -> (u16, u16)`: `(area.x + (area.width - width(TITLE)) / 2, area.y + area.height / 6)`, saturating.

- [ ] **Step 1: Test** `letter_colour_is_what_the_logo_draws` in `logo.rs`: render
  `Logo { text: "STRATA ZERO", tick: 37, accent: Rgb(255,0,200), steady: false }` into a
  66 × 5 buffer; for every non-space cell at letter `i = x / CELL_W`, row `y`, assert its
  fg is `letter_colour(37, i, y, Rgb(255,0,200), false).to_color()`. Same at tick 4.
- [ ] **Step 2:** run `cargo test -p strata-zero --target-dir target/smoke letter_colour` — fails to compile.
- [ ] **Step 3:** implement `letter_colour`; `Logo::render` calls it. `Title::draw` uses
  `logo_origin` and `LOGO_ACCENT`.
- [ ] **Step 4:** run `cargo test -p strata-zero --target-dir target/smoke title logo` — pass.
- [ ] **Step 5:** commit `logo: the title's logo colour and spot, shared`.

### Task 2: The splash screen

**Files:**
- Create: `crates/strata-zero/src/screens/splash.rs` (with its tests)
- Modify: `crates/strata-zero/src/screens/mod.rs` (`pub mod splash;`)
- Modify: `crates/strata-zero/src/screens/title.rs` (the key guard)

**Interfaces:**
- Consumes: Task 1's `letter_colour`, `logo_origin`, `LOGO_ACCENT`; `title::TITLE`,
  `logo::{glyph, CELL_W}`, `effects::GLITCH_CHARS`, `citygen::rng::hash64`.
- Produces: `pub struct Splash`, `Splash::new(opened: u64, seed: u64) -> Splash`,
  `Splash::at_launch(opened: u64) -> Splash` (seed from the clock),
  `pub const HAND_OVER: u64 = 150;`, `impl Screen for Splash`.
- Produces: `Title::after_splash(st: &AppState, tick: u64) -> Title`: `Title::new` with
  keys ignored until `tick + 10`.

Constants in `splash.rs`: `FOUND = 70`, `SPAN = 560.0`, `KEY_ROW = 1000`,
`DUMP_W = 79`, `KEY = *b"STRATA ZERO"`, key row bytes
`[0x00, 0x1C, 0x7F, KEY…, 0x00, 0x00]`, byte column `12 + 3j + (j >= 8) as u16`,
address `0x00A1F000 + 16n` as `0x{:08X}`.

- [ ] **Step 1: Tests** in `splash.rs` (all with `Splash::new(0, 7)` and `st.tick` set
  before `update`/`draw`, as the runner does):
  - `it_hands_over_at_150`: `update(&mut st, &[])` is `Stay` for ticks 1–149 and a
    `Switch` with `TransitionKind::None` at 150; a fresh splash updated first at 152
    switches too.
  - `any_key_skips_to_the_title`: at tick 10 a key gives `Switch` with
    `TransitionKind::Glitch`.
  - `a_key_mashed_through_the_skip_does_nothing`: the title from the skip returns `Stay`
    for Esc at tick +3 and `Quit` for Esc at tick +12.
  - `the_key_is_found_at_75`: at 120 × 40, tick 75, the buffer text contains
    `53 54 52 41 54 41 20 5A 45 52 4F`, `STRATA ZERO` and `KEY FOUND`.
  - `the_hand_over_is_seamless`: at 120 × 40 and 100 × 30, ticks 147–149: the splash's
    buffer and `Title::new(&st, t)`'s agree on symbol and fg at every cell of the logo
    (`logo_origin`, `glyph`), and the splash has no other non-space cell.
  - `draws_at_every_tick_and_size`: 100 × 30, 120 × 40, 300 × 80, ticks 0–150, no panic.
- [ ] **Step 2:** run `cargo test -p strata-zero --target-dir target/smoke splash::` — fails.
- [ ] **Step 3:** implement, following the spec's table and `KEYDUMP` in `splash.js`
  (draw order: header, dump rows and read head, status line; then the dissolve pass over
  the area; then the flying bytes). Random numbers are `hash64` of fixed salts (the
  bytes also of `seed`) mapped to `0..1`; the flight's starts, durations, arcs and sides
  do not use the seed. A landed cell is `set_char` + `set_fg` only, as `Logo` draws.
- [ ] **Step 4:** run the tests — pass; `cargo clippy -p strata-zero --all-targets --target-dir target/smoke -- -D warnings` clean.
- [ ] **Step 5:** commit `splash: the key, cracked into the logo`.

### Task 3: Launch on the splash

**Files:**
- Modify: `crates/strata-zero/src/screens/mod.rs` (`initial` returns `Box<dyn Screen>`, the splash)
- Modify: `crates/strata-zero/src/cli.rs` (both callers)
- Modify: `crates/strata-zero/src/window/mod.rs` (`window_shots` draws `Title::new`; `SHOT_FRAME`'s comment)
- Modify: `crates/strata-zero/src/screens/shots.rs` (`splash` shot)
- Modify: `.github/workflows/ci.yml` (`SHOTS` gains `shots::splash`)
- Modify: `docs/development.md` (one line: the game opens on the splash, any key skips)

**Interfaces:**
- Consumes: Task 2's `Splash::at_launch`, `Splash::new`.
- Produces: `pub fn initial(st: &AppState) -> Box<dyn Screen>`.

- [ ] **Step 1: Test** `the_game_opens_on_the_splash` in `screens/mod.rs` tests (or
  `splash.rs`): `initial(&st)` updated with no keys stays through tick 149 and switches
  at 150.
- [ ] **Step 2:** run it — fails (the title switches nowhere).
- [ ] **Step 3:** wire `initial`, the callers, `window_shots`, the shot
  `splash` (frames 30 scan, 76 found, 96 dissolve, 118 flight; 120 × 40 and 100 × 30;
  `Splash::new(0, 7)`), CI and the doc line.
- [ ] **Step 4:** run the test; `cargo clippy` both ways (CLAUDE.md).
- [ ] **Step 5:** commit `splash: the game opens on it`.

### Task 4: Both front ends, and the frames looked at

- [ ] Run the five checks in CLAUDE.md, all green.
- [ ] `SHOT_DIR=… cargo test -p strata-zero --features window --target-dir target/smoke -- --ignored shots::splash window::tests::window_shots`; look at the PNGs against the mock's frames.
- [ ] Run the window build with `STRATA_WINDOW_SHOT` and look at the live frame.
