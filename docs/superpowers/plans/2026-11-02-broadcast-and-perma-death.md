# The Broadcast and Perma Death Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the city generation screen with a broadcast screen that opens every NEW GAME and LOAD GAME, and add the perma death (picked on NAME) and disable-tutorial flags.

**Architecture:** Broadcasts are data in `flavor/broadcasts.ron`, loaded into `AppState` like the missions book and filled with slots from the city being entered. The broadcast screen is pushed over the wizard or the save list with an already-built `City`; `space` enters it. The cutaway art is its own `ui` file behind one `const` switch. A shared helper builds and enters cities for the dev starts.

**Tech Stack:** Rust, ratatui, crossterm, ron/serde, citygen.

**Spec:** `docs/superpowers/specs/2026-11-02-broadcast-and-perma-death-design.md` (mockups: `docs/superpowers/specs/2026-11-02-splash-perma-death-mock/gen.py`, `gen2.py`).

## Global Constraints

- Build and test into `--target-dir target/smoke`, never `target/`.
- Check both front ends: the five commands in `CLAUDE.md` (fmt, clippy ×2, test ×2).
- Exact `Color::Rgb` colours; draw only through ratatui; keys as crossterm `KeyEvent`s.
- Every non-ASCII symbol in `src/**/*.rs` and `flavor/*.ron` must pass `every_drawn_character_has_a_glyph`.
- Space confirms; enter is unbound on the broadcast screen.
- Red `(255, 40, 40)`, dark red `(28, 5, 8)`, light red `(255, 96, 112)`, amber `(255, 200, 80)`, dark cyan `(0, 172, 191)`; the rest from `ui::chrome` (`CYAN`, `MAGENTA`, `TEXT`, `DIM`, `FAINT`, `FAINTER`, `RULE`, `PANEL_BG`).
- Comments and names read like the surrounding code: short doc comments in plain voice.

## Review Focus

- A city with no district of a slot's theme, or no corp tower, or no home: the slot falls back and nothing shows `{…}` → Task 3 test `slots_fall_back_when_the_city_lacks_them`.
- A long handle (16 characters) in the banner and footer at 100×30: nothing runs past the banner's border → Task 5 test `a_long_handle_stays_inside_the_banner`.
- Pressing `space` many times in one frame batch (key repeat) jacks in once and does not skip the typed-out stage → Task 5 test `the_first_space_only_shows_it_all`.
- A terminal wider and taller than 120×40 (the window, maximised): the composition stays centred and the art ≤ 56 wide → Task 5 test `the_art_stays_narrow_on_a_wide_terminal`.
- Going back from the broadcast screen and jacking in again builds a fresh city each time and keeps the build → Task 6 test `esc_from_the_broadcast_keeps_the_build`.

---

### Task 1: The flags

**Files:**
- Modify: `crates/game/src/build.rs` (struct `Build`, `Build::blank`)
- Modify: `crates/strata-zero/src/app.rs` (struct `AppState`, `AppState::new`)

**Interfaces:**
- Produces: `game::Build.perma_death: bool`; `AppState.perma_death: bool`, `AppState.tutorial_off: bool`.

- [ ] **Step 1: Write the failing tests**

In `build.rs` tests: `perma_death_starts_on` — `Build::blank(&c).perma_death` and `Build::from_template(&c, &c.templates[0]).perma_death` are `true`.
In `app.rs` tests (or the nearest test module): `the_run_starts_with_perma_death_and_the_tutorial` — `AppState::new()` has `perma_death == true`, `tutorial_off == false`.

- [ ] **Step 2: Run them, expect compile failures**

Run: `cargo test -p game --target-dir target/smoke perma_death_starts_on`

- [ ] **Step 3: Add the fields** with doc comments ("carried through the game; nothing reads it yet").

- [ ] **Step 4: Run** `cargo test -p game -p strata-zero --target-dir target/smoke perma` — PASS.

- [ ] **Step 5: Commit** `game, app: the perma death and tutorial flags`.

### Task 2: NAME's perma death field

**Files:**
- Modify: `crates/strata-zero/src/screens/create/name.rs` (`key`, `draw`)
- Modify: `crates/strata-zero/src/screens/create/mod.rs` (`Create`, `step_key`, `footer_keys`, `go`, tests)

**Interfaces:**
- Consumes: `Build.perma_death`.
- Produces: `pub(crate) enum NameField { Handle, PermaDeath }`, `Create.name_field: NameField` (reset to `Handle` in `go` when entering NAME); `name::key(st: &mut AppState, field: &mut NameField, code: KeyCode)`; `footer_keys(&self)` returns the field's keys.

- [ ] **Step 1: Write the failing tests** in `create/mod.rs`:
  - `name_step_moves_to_perma_death_and_flips_it`: on NAME (`picked(&mut st, Step::Name)`), `Down` → `name_field == PermaDeath`; `Right` → `!perma_death`; `Char(' ')` → back on; `Char('x')` leaves the handle unchanged; `Up` → `Handle`; `Char('x')` appends to the handle; `Left` on the handle does not flip it.
  - `name_step_draws_perma_death_and_the_sheet_tag` at 120×40 and 100×30: text contains `perma death`, `ON · OFF`, `One life.`, `PERMA DEATH` (the sheet tag) and `[↓]perma death`; after `Down`, `Right`: contains `A flatline sends you back`, `[←→]on/off`, and not the sheet's `PERMA DEATH`.

- [ ] **Step 2: Run** `cargo test -p strata-zero --target-dir target/smoke name_step_` — FAIL.

- [ ] **Step 3: Implement.** Draw per the spec's NAME section and `gen.py`'s `name_step` (label two rows under the rules' last line; the description wrapped to `left.width`; the tag right-aligned at `inner.right() - 2` on the name row). `{HANDLE}` is `b.handle.trim().to_uppercase()`, `your runner` when blank. The cursor `▮` only while the handle has focus; `▸` on the focused field.

- [ ] **Step 4: Run** the two tests and `cargo test -p strata-zero --target-dir target/smoke create::` — PASS.

- [ ] **Step 5: Commit** `create: perma death on NAME`.

### Task 3: Broadcasts as data

**Files:**
- Create: `flavor/broadcasts.ron` (the spec's three broadcasts, copy exact)
- Create: `crates/strata-zero/src/broadcasts.rs`; add `mod broadcasts;` to `main.rs`
- Modify: `crates/strata-zero/src/app.rs` (`AppState.broadcasts`, loaded in `new` like `book`)
- Modify: `docs/development.md` (the `flavor/` line gains `broadcasts.ron (what the city says as you drop in)`)

**Interfaces:**
- Produces:
  - `#[derive(Deserialize)] pub enum When { NewGame, LoadGame }`
  - `pub struct Broadcast { pub id: String, pub when: When, pub tag: Vec<(String, Option<(u8, u8, u8)>)>, pub shout: String, pub paras: Vec<String>, pub sign: String, pub sign_colour: (u8, u8, u8) }`
  - `pub struct Broadcasts(pub Vec<Broadcast>)` with `load() -> Result<Broadcasts, BroadcastError>`, `parse(&str) -> Result<…>`, `pick(&self, when: When, seed: u64) -> &Broadcast` (the matching ones, index `hash64(seed, 0xB40ADCA5) % n`).
  - `pub enum BroadcastError { Parse(String), Empty(String), UnknownSlot { id: String, slot: String }, Duplicate(String), Missing(When) }` (thiserror, messages prefixed `broadcasts:`).
  - `pub struct Aired { pub tag: Vec<(String, Option<Color>)>, pub shout: String, pub paras: Vec<String>, pub sign: String, pub sign_colour: Color }`
  - `pub fn air(b: &Broadcast, city: &City, handle: &str) -> Aired`
  - `pub fn fill(text: &str, city: &City, handle: &str) -> String`
  - `pub fn loud(name: &str) -> String`
- Slots (validated by name, case-insensitive): `city`, `city.loud`, `you`, `home`, `corp`, `market`, `zone`, `blocks`, `docks`, `core`, `strip`, `industry`, `oldtown`. A slot written in capitals (`{CITY}`) fills `to_uppercase()`; `{CITY.LOUD}` is the same as `{city.loud}`.

- [ ] **Step 1: Write the failing tests** in `broadcasts.rs`:
  - `the_shipped_broadcasts_load`: `Broadcasts::load()` is `Ok`; ids `static66`, `nitewire`, `block_pa`; `pick(When::NewGame, s).id == "static66"` for seeds 1..20; `LoadGame` picks only `nitewire` or `block_pa`, and the same seed picks the same one.
  - `bad_books_are_refused`: a `{nope}` slot → `UnknownSlot`; two `id: "a"` → `Duplicate`; an empty `shout` → `Empty`; a book with only `NewGame` → `Missing(LoadGame)`.
  - `loud_holds_the_last_two_vowels_before_a_consonant`: `HOLLOWSIDE` → `HOLLOOOOWSIIIIDE`; `Newreach` → `NEEEEWREAAAACH`; `Ashreach` → `AAAASHREAAAACH`; `Greymarch` → `GREEEEYMAAAARCH` (Y counts as a consonant; a vowel run is held on its last letter).
  - `slots_fill_from_the_city`: on `Generator::run` of a small city (`GenParams { seed: 2, size: SizePreset::Custom { w: 80, h: 40 }, districts: 4, ..Default::default() }`), `fill("{city} {CITY} {you}", …, "Vex")` is `"<name> <NAME> Vex"`; `fill("{market}", …)` is one of the city's district names; no output contains `{`.
  - `slots_fall_back_when_the_city_lacks_them`: on `City::lot(1)` (no districts of most themes, no corp tower, no home), every slot fills without `{` and `{corp}` is `Orodyne`, `{home}` is `the block` when there is no district at all.

- [ ] **Step 2: Run** `cargo test -p strata-zero --target-dir target/smoke broadcasts::` — FAIL.

- [ ] **Step 3: Implement.** Seeded picks: for a theme slot, the districts of that theme (else all) indexed by `hash64(city.seed, salt) % n` with one salt per slot name; `{home}` from `citygen::home(city)` → `city.building_opt(home.building)?.name`, else the `blocks` slot's district, else `the block`; `{corp}` from buildings with `kind.is_corp()` and a name, else `Orodyne`. The `loud` algorithm:

```text
upper = name.to_uppercase(); vowels = AEIOU
groups = maximal vowel runs whose next char exists and is not a vowel
take the last two such groups; for each, repeat its last letter 3 more times
```

- [ ] **Step 4: Run** the tests — PASS. Also `cargo test -p strata-zero --features window --target-dir target/smoke every_drawn_character_has_a_glyph` — PASS.

- [ ] **Step 5: Commit** `broadcasts: what the city says as you drop in`.

### Task 4: The cutaway art

**Files:**
- Create: `crates/strata-zero/src/ui/cutaway.rs`; add `pub mod cutaway;` to `ui/mod.rs`

**Interfaces:**
- Produces: `pub const BANDS: [&str; 5] = ["HEIGHTS", "SKYWAYS", "STREET", "UNDERCITY", "NET"]`; `pub fn band_rows(rows: u16) -> [u16; 5]` (each band's first cell row: pixel rows `0, 30%, 50%, 70%, 86%` of `2 * rows`, halved); `pub fn draw(buf: &mut Buffer, area: Rect, seed: u64)`.

- [ ] **Step 1: Write the failing tests:**
  - `the_cutaway_fills_its_area_and_no_more`: draw into `Rect::new(5, 3, 48, 22)` of a 60×30 buffer filled with `PANEL_BG`; every cell outside the rect keeps `PANEL_BG` and `" "`; every cell inside has a non-panel background or a half block.
  - `the_cutaway_is_the_same_for_a_seed`: two draws with seed 7 are equal; seed 8 differs.
  - `band_rows_split_the_strata`: `band_rows(22) == [0, 6, 11, 15, 18]`.

- [ ] **Step 2: Run** `cargo test -p strata-zero --target-dir target/smoke cutaway` — FAIL.

- [ ] **Step 3: Implement** by porting `cutaway_art` from round 2's `gen.py` (the version with back and front tower rows) onto `ui::pixels::Pixels` and its `fold`, with `rand_chacha::ChaCha8Rng::seed_from_u64(seed)` (or the crate's existing small rng helper) in place of Python's `random`. Fill every pixel (the sky gradient first) so the area is opaque.

- [ ] **Step 4: Run** — PASS.

- [ ] **Step 5: Commit** `ui: the city cut open`.

### Task 5: The broadcast screen

**Files:**
- Create: `crates/strata-zero/src/screens/broadcast.rs`; add `pub mod broadcast;` to `screens/mod.rs`

**Interfaces:**
- Consumes: `broadcasts::{When, air, Aired}`, `AppState.broadcasts`, `ui::cutaway::{draw, band_rows, BANDS}`, `ui::logo::Logo`, `title::TITLE` and the subtitle (make `title::SUBTITLE` `pub`), `ui::chrome::reveal`, `create::{keys_line, wrap}`, `missions::call::{TYPE_PER_SECOND, TICKS_PER_SECOND}`, `screens::enter_built_city` (Task 6 — until then, inline the three lines and swap in Task 6).
- Produces:
  - `pub const CUTAWAY: bool = true;`
  - `pub enum Entry { NewGame, Load { path: String } }`
  - `pub struct Broadcast { city: Option<City>, entry: Entry, aired: Aired, handle: String, seed: u64, opened: u64, full: bool, pub perma_death: bool, pub tutorial_off: bool }`
  - `Broadcast::new_game(st: &AppState, city: City) -> Self` (perma death from `st.build`, the tutorial from `st.tutorial_off`, handle from the build), `Broadcast::load(st: &AppState, city: City, path: String) -> Self` (both from `st`, handle from `st.character`).
  - `impl Screen for Broadcast`.

- [ ] **Step 1: Write the failing tests** (a small city as in Task 3; `st.build = Some(Build::from_template(...))` for a new game):
  - `the_first_space_only_shows_it_all`: right after opening, `update(&[space, space, space])` returns `Stay` and the broadcast is full; the next `space` returns `Switch`.
  - `space_enters_the_city_with_the_build_and_the_flags`: new game; flip `p` and `t`; space ×2 → `Switch`, `st.city` is the city, `st.character.handle == "VEX"`, `st.build.is_none()`, `!st.perma_death`, `st.tutorial_off`.
  - `a_load_starts_from_the_runs_flags_and_says_so`: with `st.perma_death = false`, `Broadcast::load(…, "cities/x.city")` shows `PERMA DEATH IS OFF`; space ×2 → `!st.perma_death`, the log's last line contains `loaded cities/x.city`.
  - `esc_goes_back_and_enter_does_nothing`: `Esc` → `Pop`; `Enter` → `Stay`.
  - `the_screen_draws_the_broadcast_the_toggles_and_the_banner` at 120×40 once full: contains `ON AIR`, `WAKE UP!`, `[ ] DISABLE TUTORIAL`, `JACK IN`, `PERMA DEATH IS ON`, `last chance`, `HEIGHTS` and `NET ─`; after `t` and `p`: `[✓] DISABLE TUTORIAL`, `PERMA DEATH IS OFF`, `turn it on`.
  - `every_broadcast_fits_both_sizes`: for each shipped broadcast, for seeds `[2, 77, 1234]` (small cities), at 100×30 and 120×40: the last broadcast row is above the toggles' row (expose `fn layout(&self, area: Rect) -> Layout` with `text_end` and `toggles_y`, or assert on the drawn buffer that the toggles' row holds only the toggles), and at 100×30 every paragraph is drawn (none dropped); at 100×30 there is no `HEIGHTS`.
  - `the_art_stays_narrow_on_a_wide_terminal`: at 220×60 the art's columns (cells right of the `│` rule) are ≤ 56 wide and the composition's left and right margins differ by ≤ 1.
  - `a_long_handle_stays_inside_the_banner`: handle `ABCDEFGHIJKLMNOP` at 100×30: the banner's right border `║` sits at its column on every banner row.

- [ ] **Step 2: Run** `cargo test -p strata-zero --target-dir target/smoke broadcast::` — FAIL.

- [ ] **Step 3: Implement** per the spec's "The broadcast screen" and `gen2.py`'s `splash`/`broadcast`: logo row 2 (1 under 36 rows), subtitle only from 36 rows, toggles' row `h − 9`, banner rows `h − 7..h − 4`, footer `h − 2`; art when `CUTAWAY && w ≥ 120 && h ≥ 36`: column 50 wide, gutter 13, art `min(56, …)`, the whole centred; art off: column `min(76, w − 12)` centred and vertically centred. Typed characters: `(st.tick − opened) * TYPE_PER_SECOND / TICKS_PER_SECOND`, shown with `reveal` over the composed lines (`p = typed / total`), full when `self.full`. Fit: paragraphs lose their blank rows first, then trailing paragraphs drop. Keys: `space` (full, then jack in), `t`, `p`, `esc` (`Pop` with `SlideUp`). Jack in: new game makes the character (`Character::from_build`, `st.build.take()`), a load calls `st.say(format!("loaded {path}"))` after entering; both copy the flags and enter the city.

- [ ] **Step 4: Run** the tests — PASS.

- [ ] **Step 5: Commit** `screens: the broadcast`.

### Task 6: The way in, and the generation screen goes

**Files:**
- Modify: `crates/strata-zero/src/screens/mod.rs` (helpers; drop `pub mod generating;`)
- Delete: `crates/strata-zero/src/screens/generating.rs`
- Modify: `crates/strata-zero/src/screens/create/mod.rs` (`jack_in`, `footer_keys` for `Summary`), `create/summary.rs` (its `[enter] JACK IN` line → `[enter] next`)
- Modify: `crates/strata-zero/src/screens/files.rs` (load pushes the broadcast)
- Modify: `crates/strata-zero/src/screens/title.rs` (`debug_game`), `configure.rs` (GENERATE), `explore.rs` (`r`)
- Modify: tests that expected the generation screen (`create` `summary_wears_the_name_and_jacks_in_a_template`, `files` `save_then_load_round_trip_through_screens`, configure's and title's), `helix/hosts.rs` if it names the screen

**Interfaces:**
- Produces in `screens/mod.rs`:
  - `pub fn build_city(params: citygen::GenParams) -> anyhow::Result<citygen::City>` (`Generator::run`)
  - `pub fn enter_built_city(st: &mut AppState, city: citygen::City) -> ScreenCmd` (`st.params = city.params.clone()`, `st.enter_city(city)`, `Switch { to: explore_screen(st), fx: Glitch }`)
  - `pub fn start_city(st: &mut AppState, params: citygen::GenParams) -> ScreenCmd` (build then enter, or `st.warn(e)` and `Stay`)

- [ ] **Step 1: Write / change the failing tests:**
  - `summary_wears_the_name_and_jacks_in_a_template`: `Enter` on the sheet returns `Push` (not `Switch`), `st.build` is still `Some`, `st.city` is `None`; the sheet reads `[enter]next`, not `JACK IN`.
  - `esc_from_the_broadcast_keeps_the_build`: sheet `Enter` → push; broadcast `Esc` → `Pop`; sheet `Enter` again pushes a new broadcast whose city seed differs (fresh params) and the build is still there.
  - `save_then_load_round_trip_through_screens`: load's `Enter` returns `Push`; space ×2 on the pushed screen returns `Switch` and `st.city` is the loaded city.
  - `the_dev_starts_skip_the_broadcast`: title debug start (dev unlocked) returns `Switch` with `st.city` set; configure GENERATE likewise.

- [ ] **Step 2: Run** `cargo test -p strata-zero --target-dir target/smoke` — the changed tests FAIL.

- [ ] **Step 3: Implement**; delete `generating.rs`; grep `generating` and `Generating` to zero hits in `crates/`. FLATLINED (`screens/flatlined.rs`) also sets `st.perma_death = true` and `st.tutorial_off = false` where it swaps in the fresh VEX (test: extend its existing test or add `a_flatline_resets_the_flags`).

- [ ] **Step 4: Run** `cargo test -p strata-zero --target-dir target/smoke` — PASS.

- [ ] **Step 5: Commit** `screens: NEW GAME and LOAD GAME go in through the broadcast; the generation screen goes`.

### Task 7: Shots and the full checks

**Files:**
- Modify: `crates/strata-zero/src/screens/shots.rs`

- [ ] **Step 1: Add an ignored shot test** `broadcast_and_perma_death`: NAME with perma death on (120×40, 100×30) and on its field turned off (120×40); the broadcast screen, full, for each shipped broadcast at 120×40 (seed chosen so `pick` lands on each; or build `Broadcast` with a named one through a `#[cfg(test)]` constructor) and STATIC 66 at 100×30; STATIC 66 with perma death off and the tutorial ticked.

- [ ] **Step 2: Render and look:** `SHOT_DIR=<scratch> cargo test -p strata-zero --features window --target-dir target/smoke -- --ignored broadcast_and_perma_death`; open the PNGs and compare with the mockups.

- [ ] **Step 3: Run the five checks in `CLAUDE.md`** — all clean.

- [ ] **Step 4: Commit** `shots: the broadcast and perma death`, with the spec, plan and mock generators (`gen.py`, `gen2.py`, `round1.html`, `round2.html`).
