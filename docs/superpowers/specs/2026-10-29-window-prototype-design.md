# Window prototype: Strata Zero in its own Mac window

2026-10-29. A prototype, asked for as the first step toward a Steam release.

## Why

Steam wants an `.exe`, an `.app` or a Linux binary that opens its own window; a
terminal game can't give the Steam overlay a surface, can't control its own font,
colours or fullscreen, and plays badly on a Steam Deck. The game draws through ratatui,
whose `Runner::frame` (`crates/strata-zero/src/app.rs`) is already generic over the
backend and takes crossterm `KeyEvent`s, so a native window can run every screen
unchanged if it (a) implements ratatui's `Backend` by drawing the cell grid itself and
(b) turns window key presses into the same `KeyEvent`s.

Jeffrey wants to see the outcome: the game in a Mac window, wrapped as a `.app` that
launches from Finder.

## Goals

- `strata --window` (with the cargo feature `window`) opens a native macOS window that
  runs the whole game through the existing `Runner`: every screen, every key, the 30 fps
  frame, transitions and typing, unchanged.
- The look is the terminal's, done well: a character grid from a monospace font in
  truecolour; box-drawing (U+2500–257F) and block elements (U+2580–259F) drawn as shapes
  so borders join with no seams and the half-block implant art is crisp at any size.
- The window: resizable, the grid following it as a terminal's does (the game's own
  "resize to at least 100x30" shows below that); sharp on Retina; native fullscreen
  (green button, Ctrl+Cmd+F); Cmd+Q and the close button quit; Cmd+= / Cmd+- / Cmd+0 set
  the text size.
- `scripts/macos-app.sh` builds `Strata Zero.app`. Double-clicked in Finder it opens the
  window, and cities are saved under `~/Library/Application Support/Strata Zero/`.
- PNG screenshots from the same rasterizer, so the window's look can be checked without
  a screen capture.

## Non-goals

Steamworks; Developer ID signing and notarization; Windows and Linux builds; gamepad;
mouse; a GPU renderer; bundling a font; tiles; CRT effects; any change to a screen.

## Anti-goals

- A screen that knows it is in a window.
- A terminal build that compiles winit: everything new sits behind the `window` feature,
  off by default; `cargo build` and the shipped terminal binary are unchanged.
- An app that only works when started from the repo folder.

## Design

### Code layout

`crates/strata-zero/src/window/`, compiled only with `--features window`:

| File | Job |
|---|---|
| `mod.rs` | `run(screen, st)`: the winit event loop; keys gathered between frames; one `Runner::frame` every 33 ms; present when anything changed; the Cmd shortcuts. |
| `backend.rs` | `PixelBackend`, ratatui's `Backend`: a grid of cells, the cells ratatui reports changed rasterized into a framebuffer; `size()` in cells from the pixel size and the cell size. |
| `raster.rs` | One cell into pixels: colours, modifiers, glyph coverage blended fg over bg, double-width glyphs, shapes from `shapes.rs`. Also `Buffer` → RGBA for screenshots. |
| `shapes.rs` | Box-drawing and block-element characters as rectangles: light, heavy, double, dashed, rounded corners (drawn square), halves, eighths, quadrants, shades. |
| `font.rs` | Finding the font files, a glyph cache (fontdue), the fallback chain, cell metrics. |
| `keys.rs` | A winit key event as a crossterm `KeyEvent`, or a window shortcut, or nothing. |

`app::run` keeps the terminal loop. `cli.rs` gains the `--window` flag (feature only)
and calls `window::run`.

### Calls made for the prototype (Claude's, not Jeffrey's)

1. **CPU rasterizer, softbuffer to present.** Fewer moving parts than wgpu. ratatui hands
   the backend only the changed cells, so a frame costs what changed. A Steam build
   would upload the same framebuffer to a GPU texture (the overlay needs a GPU surface);
   the rasterizer carries over.
2. **Fonts from this Mac, nothing committed.** `STRATA_FONT` (a path) first, then
   FiraCode Nerd Font Mono from `~/Library/Fonts`, SF Mono from Terminal.app, Menlo.
   Bold from the same family's bold file when there is one. Glyphs the main font lacks
   come from Apple Symbols, then Menlo. A release would embed an OFL font with its
   licence.
3. **Size.** 15 pt text by default (logical pixels; cell from the font's advance and line
   height, times the window's scale factor). The window opens at 120×40 cells, clamped
   to the screen; its minimum is 100×30 cells at the text size it opened with. Cmd+= and
   Cmd+- keep the window and change the grid, as a terminal's font size does, so a big
   enough text size can bring up the game's own "resize to at least 100x30". The text
   size is not remembered between launches.
4. **Colours.** `Rgb` as is; `Reset` foreground `#cdd4de` (the shot harness's) and
   background `#0a0b10`, the style pack's background (`flavor/style.ron`), so the street's
   frame rows, which are left on the default background, match the map tiles beside
   them; a test keeps the two equal. Named and indexed colours from the xterm palette.
5. **Modifiers.** BOLD the bold face; DIM the foreground halfway to the background;
   REVERSED swaps; UNDERLINED and CROSSED_OUT draw a line; HIDDEN draws no glyph;
   ITALIC and blink are ignored.
6. **Keys.** Pressed and repeated keys become crossterm `Press` events, as a terminal's
   key repeat does. Characters keep crossterm's convention (an upper-case letter carries
   SHIFT; Shift+Tab is `BackTab`). Cmd chords belong to the window and never reach the
   game; any other Cmd chord is dropped.
7. **The bundle.** `scripts/macos-app.sh` builds release with `--features window` under
   `target/smoke` (the house rule: never touch `target/release`), for both Mac targets
   joined by `lipo` (both are installed), into `target/smoke/macos/Strata Zero.app`
   (`Info.plist`, bundle id `io.xenohive.strata-zero`, ad-hoc signed). An executable
   inside `*.app/Contents/MacOS/` runs windowed without the flag and changes its working
   folder to `~/Library/Application Support/Strata Zero/`, so `cities/` lands there
   (the prototype's stand-in for moving `save::default_dir`).
8. **Screenshots.** The shot harness's `draw_stack` buffers rasterized to PNG by an
   ignored test, and `STRATA_WINDOW_SHOT=<path>` makes the live window write its
   framebuffer to a PNG after its first few frames (the check of the real window path,
   Retina scale included).

## Done when

- `cargo test --workspace --target-dir target/smoke` passes with and without
  `--features window`; clippy `-D warnings` both ways; `cargo fmt --check`.
- Unit tests: key mapping, colour mapping, every box-drawing and block character has a
  shape, a rasterized cell's pixels.
- PNGs of the title, the street and the ATTRIBUTES sheet, looked at.
- The `.app` opened with `open`: it stays up, and its live framebuffer dump, asked for
  with a relative path (`open --env STRATA_WINDOW_SHOT=live.png`), looks right and lands
  in `~/Library/Application Support/Strata Zero/`, which proves the working folder that
  `cities/` saves into. Saving a city through the menus from the `.app` is left for
  Jeffrey to try: driving the window's keys from outside needs an Accessibility grant.

## Toward Steam, after this

GPU present (wgpu or Metal) for the overlay; an embedded OFL font; `save::default_dir`
per platform; Developer ID signing and notarization; Steamworks (`steamworks` crate) for
achievements and cloud saves; Windows and Linux packaging; Steam Input for the Deck.
