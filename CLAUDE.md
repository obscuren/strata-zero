# Strata Zero

A roguelike in a multi-level cyberpunk city, in Rust. The crates are laid out in
`docs/development.md` under "Layout"; game content lives in `flavor/*.ron` and is
compiled into the binary.

## Two front ends, one set of screens

The game runs in a terminal (the default) or in a window of its own (the `window` cargo
feature: `strata --window`, and `Strata Zero.app` on macOS). Both run the same screens:
every screen draws ratatui cells and reads crossterm `KeyEvent`s through
`app::Runner::frame`, and only the backend below that differs.

- Terminal: crossterm writes the cells to the terminal (`app::run`, `term.rs`).
- Window (`crates/strata-zero/src/window/`):
  - winit owns the window and turns key presses into crossterm `KeyEvent`s (`keys.rs`).
  - `PixelBackend` paints the changed cells into a CPU framebuffer: box and block
    characters as shapes (`shapes.rs`), everything else from fonts on the machine
    (`font.rs`).
  - wgpu uploads what changed and draws it through the screen effects (`gpu.rs`,
    `effects.wgsl`), with softbuffer as the fallback when there is no GPU.

So a new screen, panel or feature shows in the window with no change to the window
code, as long as it stays inside these lines:

- Draw everything through ratatui. Don't write to stdout, and don't rely on the
  terminal's cursor, title, bell or mouse; screens draw their own cursor as a character.
- Read keys as crossterm `KeyEvent`s.
  - In the window, Option types what the keyboard layout puts there (`@`, `[`) and
    carries no ALT modifier, so don't build a feature on Alt+key.
  - Cmd chords belong to the window.
  - Keys outside the usual set (F13 and up, media keys) need a line in `window/keys.rs`.
- Prefer exact `Color::Rgb` colours. Named ANSI colours take the terminal's theme there
  and the xterm palette in the window.
- Box-drawing (U+2500–257F) and block (U+2580–259F) characters always work. Any other
  symbol needs a monospace font that has it; colour emoji don't render in the window (a
  hollow box shows instead).
- A setting that only means something in the window (like `AppState.effects` and
  `screen_fx`) is wired on both sides: the screen offers it, the window acts on it.

## Building and checking

Check both front ends. The window code only compiles with the feature, so a change to
shared code (`AppState`, `Runner`, `ScreenCmd`, screens) can break the window build
while a plain `cargo test` stays green:

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets --target-dir target/smoke -- -D warnings
cargo clippy --workspace --all-targets --features strata-zero/window --target-dir target/smoke -- -D warnings
cargo test --workspace --target-dir target/smoke
cargo test --workspace --features strata-zero/window --target-dir target/smoke
```

- Build into `--target-dir target/smoke`, never the default `target/`. The maintainer
  plays `target/release/strata`, and macOS kills a running binary when its file is
  overwritten.
- The window's tests skip themselves on a machine with no fonts or no GPU adapter, and
  run for real on a Mac.
- On GitHub, `.github/workflows/ci.yml` runs the same checks on macOS, Linux and Windows
  for both builds, so keep shared code portable (no Mac-only APIs outside
  `#[cfg(target_os = "macos")]`). Releases are described in `docs/development.md` under
  "Release", including the Mac app's signing.

## Seeing the window without opening it

- **Shots of real screens.** `SHOT_DIR=<dir> cargo test -p strata-zero --features window
  --target-dir target/smoke -- --ignored <shot test>` writes each shot-harness screen
  (`screens/shots.rs`) as `.svg` and as `.png`, painted as the window paints it.
  `window_shots` adds the title. With `SHOT_FX=1` set too, it also writes `.soft.png`
  and `.crt.png` through the GPU effects.
- **The live window.** `STRATA_WINDOW_SHOT=<file.png>` makes it write what it shows,
  1.5 s in. `STRATA_SCREEN_FX=off|soft|crt` picks the effects; `STRATA_FONT=<file>`
  picks the font.
- **The app.** `scripts/macos-app.sh` builds `target/smoke/macos/Strata Zero.app`. Inside
  the app, the working folder (and so `cities/`) is
  `~/Library/Application Support/Strata Zero/`.

## Design notes

Specs and plans live in `docs/superpowers/specs` and `docs/superpowers/plans`. The window
is described in `2026-10-29-window-prototype-design.md` and
`2026-10-29-window-gpu-effects-design.md`, with what a Steam build still needs.
