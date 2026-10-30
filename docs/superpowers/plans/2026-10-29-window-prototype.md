# Window Prototype Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** `strata --window` and a Finder-launchable `Strata Zero.app` that run the whole game in a native Mac window, drawn by our own cell rasterizer.

**Architecture:** A ratatui `Backend` (`PixelBackend`) keeps a cell grid and rasterizes changed cells into a `u32` framebuffer (glyphs from fontdue, box/block characters as shapes). A winit loop calls the existing `Runner::frame` every 33 ms with winit keys turned into crossterm `KeyEvent`s and presents the framebuffer through softbuffer. Everything sits behind the cargo feature `window`.

**Tech Stack:** Rust 2024 workspace, ratatui 0.29, crossterm 0.28, winit 0.30, softbuffer 0.4, fontdue 0.9, png 0.18.

**Spec:** `docs/superpowers/specs/2026-10-29-window-prototype-design.md`

## Global Constraints

- New dependencies are optional and enabled only by `[features] window = [...]` in `crates/strata-zero/Cargo.toml`; `cargo build` without the feature compiles no winit.
- No screen module changes; `app::Runner::frame` is the only entry the window uses.
- Builds and runs use `--target-dir target/smoke`; never `target/release`.
- `Reset` fg `#cdd4de`, `Reset` bg `#0a0b10` (the style pack's background); default text 15 logical px; window opens at 120×40 cells clamped to the monitor; minimum 100×30 cells.
- Font chain: `STRATA_FONT`, `~/Library/Fonts/FiraCodeNerdFontMono-Regular.ttf` (bold `…-Bold.ttf`), `/System/Applications/Utilities/Terminal.app/Contents/Resources/Fonts/SF-Mono-Regular.otf` (bold `SF-Mono-Bold.otf`), `/System/Library/Fonts/Menlo.ttc` (index 0, bold index 1); glyph fallbacks `/System/Library/Fonts/Apple Symbols.ttf`, then Menlo.
- Bundle: `target/smoke/macos/Strata Zero.app`, id `io.xenohive.strata-zero`, executable `strata`, ad-hoc signed, universal (`aarch64-apple-darwin` + `x86_64-apple-darwin`).
- Commits end with `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`.

## Review Focus

1. Window shrunk below 100×30 cells, or to zero pixels while minimised: no panic (softbuffer needs non-zero sizes); the game's "resize to at least" line shows. Task 4 test `zero_and_tiny_sizes_never_panic`.
2. Moving the window to a monitor with another scale factor, or Cmd+=/−: the glyph cache is rebuilt at the new pixel size and the whole grid repaints. Task 4 test `resize_cells_repaints_everything`.
3. A character no font has: a hollow box, never a panic and never an empty cell. Task 3 test `missing_glyph_draws_a_box`.
4. A double-width glyph (`⚡`) and the cell after it: the wide glyph spans both cells and survives a redraw of either one. Task 4 test `wide_glyph_spans_two_cells`.
5. Held keys: repeats arrive as `Press`, releases never reach the game. Task 1 test `repeat_is_press_release_is_nothing`.

---

### Task 1: The feature, its dependencies, and key mapping

**Files:**
- Modify: `crates/strata-zero/Cargo.toml` (optional deps `winit = "0.30"`, `softbuffer = "0.4"`, `fontdue = "0.9"`, `png = "0.18"`; feature `window`)
- Modify: `crates/strata-zero/src/main.rs` (`#[cfg(feature = "window")] mod window;`)
- Create: `crates/strata-zero/src/window/mod.rs` (module docs and `mod` lines only for now)
- Create: `crates/strata-zero/src/window/keys.rs`

**Interfaces:**
- Produces: `pub enum Input { Game(KeyEvent), Window(Shortcut), Nothing }`, `pub enum Shortcut { Quit, Fullscreen, Bigger, Smaller, ResetSize }`, `pub fn translate(pressed: bool, logical: &Key, bare: &Key, mods: ModifiersState) -> Input` (winit `Key`; `bare` is `key_without_modifiers()`; `pressed` is false for releases).

- [ ] **Step 1: Write the failing tests** in `keys.rs`:
  - `letters_and_shift`: `Character("a")` → `Game(Char('a'), NONE)`; `Character("A")` with SHIFT → `Char('A')`, `SHIFT`.
  - `named_keys`: Space → `Char(' ')`; Enter, Escape, Tab, Backspace, Delete, arrows, Home, End, PageUp, PageDown, F5 → their crossterm codes; Shift+Tab → `BackTab` with `SHIFT`.
  - `ctrl_and_alt_use_the_bare_key`: Ctrl with logical `"\u{3}"` and bare `"c"` → `Char('c')`, `CONTROL`; Alt with logical `"å"` and bare `"a"` → `Char('a')`, `ALT`.
  - `cmd_chords_are_the_window_s`: Cmd+Q → `Window(Quit)`; Cmd+Ctrl+F → `Window(Fullscreen)`; Cmd+`=` and Cmd+`+` → `Bigger`; Cmd+`-` → `Smaller`; Cmd+`0` → `ResetSize`; Cmd+K → `Nothing`.
  - `repeat_is_press_release_is_nothing`: `pressed = false` → `Nothing`; every `Game` event has `kind == KeyEventKind::Press`.
- [ ] **Step 2:** `cargo test -p strata-zero --features window --target-dir target/smoke window::keys` → fails to compile (no `translate`).
- [ ] **Step 3:** Implement `translate`.
- [ ] **Step 4:** Same command → PASS. `cargo build -p strata-zero --target-dir target/smoke` (no feature) still builds.
- [ ] **Step 5:** Commit `feat(window): a window feature and keys for it`.

### Task 2: Box-drawing and block characters as shapes

**Files:**
- Create: `crates/strata-zero/src/window/shapes.rs`

**Interfaces:**
- Produces: `pub fn shape(c: char, w: usize, h: usize) -> Option<Vec<u8>>` — a `w*h` coverage mask (row-major, 0–255) for U+2500–U+259F, `None` for anything else.

Box characters are four arms (left, right, up, down), each `None | Light | Heavy | Double`, from a table over U+2500–U+254B and U+2550–U+256C; dashed ones (U+2504–U+250B, U+254C–U+254F) are light or heavy arms with gaps; rounded corners U+256D–U+2570 are drawn as light corners; diagonals U+2571–U+2573 as lines; half arms U+2574–U+257F. Light stroke = `max(1, round(h/14))` px, heavy = 2× light, double = two light strokes with one light gap. Strokes are centred on the cell's middle row and column so neighbours join.

- [ ] **Step 1: Write the failing tests:**
  - `every_box_and_block_char_has_a_shape`: for every `c` in `'\u{2500}'..='\u{259F}'`, `shape(c, 10, 20)` is `Some` with `len() == 200` and at least one non-zero byte, except U+2591–U+2593 which are checked by `shades_are_partial`.
  - `horizontal_line_spans_the_cell`: `shape('─', 10, 20)` has full coverage in column 0 and column 9 on the middle row and none in row 0.
  - `lines_meet_at_the_centre`: in `shape('┼', 10, 20)` the middle row and middle column are full end to end.
  - `full_and_half_blocks`: `█` all 255; `▀` rows 0..10 are 255 and 10..20 are 0; `▄` the reverse; `▌` left half.
  - `shades_are_partial`: `░`, `▒`, `▓` cover every pixel with 64, 128, 192.
  - `not_a_shape`: `shape('a', 10, 20)` is `None`.
- [ ] **Step 2:** `cargo test … window::shapes` → fails.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** PASS.
- [ ] **Step 5:** Commit `feat(window): box and block characters drawn as shapes`.

### Task 3: Fonts and the cell rasterizer

**Files:**
- Create: `crates/strata-zero/src/window/font.rs`
- Create: `crates/strata-zero/src/window/raster.rs`

**Interfaces:**
- Produces (`font.rs`): `pub struct Fonts` with `pub fn load() -> anyhow::Result<Fonts>` (the chain in Global Constraints), `pub fn metrics(&self, px: f32) -> CellMetrics`, `pub fn glyph(&mut self, c: char, bold: bool, px: f32) -> Option<&Glyph>` (cached by `(c, bold, px bits)`; `None` when no font has `c`); `pub struct CellMetrics { pub w: usize, pub h: usize, pub baseline: usize }`; `pub struct Glyph { pub left: i32, pub top: i32, pub w: usize, pub h: usize, pub alpha: Vec<u8> }` (`left`/`top` relative to the cell's top-left).
- Produces (`raster.rs`): `pub struct Frame { pub w: usize, pub h: usize, pub px: Vec<u32> }` (0x00RRGGBB); `pub fn rgb(c: Color, fallback: u32) -> u32`; `pub const FG: u32 = 0xcdd4de; pub const BG: u32 = 0x0a0b10;` `pub fn paint(frame: &mut Frame, fonts: &mut Fonts, m: CellMetrics, px: f32, col: u16, row: u16, cell: &Cell, span: u16)` (span 2 for a wide glyph); `pub fn buffer_to_rgba(buf: &Buffer, fonts: &mut Fonts, px: f32) -> (usize, usize, Vec<u8>)` for PNGs.

`paint` fills the background, then the coverage from `shapes::shape` or the font glyph, blended `fg` over `bg`; modifiers as the spec's call 5; text coverage lifted by `a' = 255 * (a/255)^0.8` so thin strokes read as on macOS.

- [ ] **Step 1: Write the failing tests** (`raster.rs`; they skip with a printed note when `Fonts::load()` fails):
  - `default_bg_is_the_style_pack_s`: `BG` equals `AppState::new().style.background` as `0xRRGGBB`.
  - `colours`: `rgb(Color::Rgb(1,2,3), 0)` = `0x010203`; `rgb(Color::Reset, FG)` = `FG`; `rgb(Color::Red, 0)` = `0xcd0000`; `rgb(Color::Indexed(196), 0)` = `0xff0000`.
  - `block_fills_with_fg`, `space_fills_with_bg`, `reversed_swaps`, `dim_halves_toward_bg` (`█` fg `Rgb(200,100,0)` bg `Rgb(0,0,0)` with DIM → `0x643200`).
  - `letter_has_ink`: painting `M` puts at least one fg-coloured pixel inside the cell and none outside it.
  - `missing_glyph_draws_a_box`: `'\u{10FFFD}'` paints fg pixels on the cell's border and bg in its centre.
- [ ] **Step 2:** fails. **Step 3:** implement both files. **Step 4:** PASS.
- [ ] **Step 5:** Commit `feat(window): fonts and a cell rasterizer`.

### Task 4: `PixelBackend`

**Files:**
- Create: `crates/strata-zero/src/window/backend.rs`

**Interfaces:**
- Consumes: Task 3.
- Produces: `pub struct PixelBackend`; `pub fn new(fonts: Fonts, px: f32, width: usize, height: usize) -> PixelBackend` (pixel size); `pub fn resize(&mut self, width: usize, height: usize)`; `pub fn set_px(&mut self, px: f32)`; `pub fn frame(&self) -> &Frame`; `pub fn take_dirty(&mut self) -> bool`; `pub fn cell_px(&self) -> (usize, usize)`; `impl Backend for PixelBackend` (`size()` = pixels / cell, at least 1×1; `clear` repaints everything as `Reset`; cursor calls are no-ops).

A shadow grid of `Cell`s lets a redraw of either half of a wide glyph repaint the whole glyph. `resize`/`set_px` rebuild the grid and the frame; ratatui's `autoresize` then redraws every cell.

- [ ] **Step 1: Write the failing tests:**
  - `size_is_pixels_over_cells`: 10× by 3× the cell size gives `Size { width: 10, height: 3 }`.
  - `draw_through_a_terminal`: a `ratatui::Terminal<PixelBackend>` drawing a red `█` at (2,1) colours that cell's pixels and sets `take_dirty()` once.
  - `zero_and_tiny_sizes_never_panic`: `resize(0,0)`, `resize(3,3)`, then `terminal.draw(app::draw_too_small)` succeeds.
  - `resize_cells_repaints_everything`: after `set_px(30.0)` and a draw, a cell drawn before is drawn again at the new size (its pixels are at the new cell's coordinates).
  - `wide_glyph_spans_two_cells`: drawing `"⚡x"` then redrawing only cell 1 keeps fg ink in cell 0's right half region (the glyph is repainted).
- [ ] **Step 2:** fails. **Step 3:** implement. **Step 4:** PASS.
- [ ] **Step 5:** Commit `feat(window): a ratatui backend that paints pixels`.

### Task 5: The window loop, the flag, the bundle mode

**Files:**
- Modify: `crates/strata-zero/src/window/mod.rs` (`pub fn run(screen: Box<dyn Screen>, st: AppState) -> anyhow::Result<()>`)
- Modify: `crates/strata-zero/src/cli.rs` (`#[cfg(feature = "window")] #[arg(long)] window: bool`; `None` command runs `window::run` when `window || window::in_bundle()`)
- Modify: `crates/strata-zero/src/app.rs` (`Runner` and `FRAME` already public within the crate; make `Runner::new` reachable if it is not)
- Create: `crates/strata-zero/src/window/png.rs` (`pub fn write_png(path: &Path, w: usize, h: usize, rgba: &[u8]) -> anyhow::Result<()>`; `Frame` → RGBA helper)

**Interfaces:**
- Produces: `pub fn in_bundle() -> bool` (current exe path contains `.app/Contents/MacOS/`); `pub fn app_support_dir() -> PathBuf` (`$HOME/Library/Application Support/Strata Zero`).

The loop (winit 0.30 `ApplicationHandler`): create the window in `resumed` (title `Strata Zero`, inner size 120×40 cells in logical px clamped to the monitor, min 100×30 cells); `ControlFlow::WaitUntil(next)`; on `ResumeTimeReached` run `runner.frame(&mut term, &keys)` (exit when it returns false), then `request_redraw` if `take_dirty()`; on `RedrawRequested` copy the frame into the softbuffer surface (skip at zero size) and present; `Resized` and `ScaleFactorChanged` resize the backend (px = 15 × scale × text zoom); shortcuts: Quit exits, Fullscreen toggles `Fullscreen::Borderless(None)`, Bigger/Smaller step the zoom by 1.1× within 0.6–2.5, ResetSize sets 1.0. With `STRATA_WINDOW_SHOT=<path>` the frame is written as PNG on frame 45 (1.5 s, after the title's typing). In a bundle, `run` first creates and enters `app_support_dir()`.

- [ ] **Step 1:** `in_bundle_detects_app_paths` test over a pure helper `fn is_bundle_path(p: &Path) -> bool`.
- [ ] **Step 2:** Implement; `cargo build -p strata-zero --features window --target-dir target/smoke --release`.
- [ ] **Step 3: Run it:** `STRATA_WINDOW_SHOT=<scratch>/live.png target/smoke/release/strata --window`, wait, check the PNG (title screen, Retina pixel size) and quit.
- [ ] **Step 4:** Commit `feat(window): strata --window runs the game in a Mac window`.

### Task 6: PNG shots of the real screens

**Files:**
- Modify: `crates/strata-zero/src/screens/shots.rs` (`draw_stack` becomes `pub(crate)`; with the feature, `shot_stack` also writes `<name>.png` through `raster::buffer_to_rgba`)
- Create: ignored test `window_shots` in `crates/strata-zero/src/window/mod.rs` writing `title.png`, `street.png` and `attributes.png` (120×40) to `SHOT_DIR`.

- [ ] **Step 1:** Write the test; run `SHOT_DIR=<scratch>/shots cargo test -p strata-zero --features window --target-dir target/smoke window_shots -- --ignored`.
- [ ] **Step 2:** Look at the three PNGs; fix what reads wrong (seams, baseline, weight).
- [ ] **Step 3:** Commit `test(window): PNG shots through the rasterizer`.

### Task 7: The `.app`

**Files:**
- Create: `scripts/macos-app.sh`
- Create: `crates/strata-zero/macos/Info.plist`
- Modify: `README.md` (a short "In a Mac window" section)

The script: release builds with `--features window --target-dir target/smoke` for both targets, `lipo -create` into `Contents/MacOS/strata`, copies `Info.plist` with the workspace version substituted for `@VERSION@`, `codesign --force -s -`, prints the path.

- [ ] **Step 1:** Run the script; `file` on the binary says universal (arm64 + x86_64).
- [ ] **Step 2:** `open --env STRATA_WINDOW_SHOT=live.png "target/smoke/macos/Strata Zero.app"`; after 5 s `pgrep -f "Strata Zero.app"` finds it and `~/Library/Application Support/Strata Zero/live.png` exists and looks right; quit it with `osascript -e 'quit app "Strata Zero"'`.
- [ ] **Step 3:** Commit `feat(window): Strata Zero.app for macOS`.

### Task 8: Whole check

- [ ] `cargo test --workspace --target-dir target/smoke`; the same with `--features strata-zero/window`; clippy `-D warnings` both ways; `cargo fmt --check`.
- [ ] One fresh reviewer over the branch; fix what holds up.
