# Strata Zero

A roguelike in a multi-level cyberpunk city, in Rust. The crates are laid out in
`docs/development.md` under "Layout"; game content lives in `flavor/*.ron` and is
compiled into the binary.

## Two front ends, one set of screens

The game runs in a terminal (the default) or in a window of its own (the `window` cargo
feature: `strata --window`). The `app` feature builds the desktop apps on top of it
(`Strata Zero.app`, `Strata Zero.exe`, the Linux `strata-zero`): always in the window,
with the working folder set to the player's app folder (`platform`). Both run
the same screens:
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
  - In the window, Option (Mac) and AltGr (PC) type what the keyboard layout puts there
    (`@`, `[`) with no ALT modifier, so don't build a feature on Alt+key.
  - The window keeps some chords for itself: Cmd chords on a Mac; Ctrl+=, Ctrl+-,
    Ctrl+0, F11 and Alt+Enter on Windows and Linux. Ctrl+letter stays the game's.
  - Keys outside the usual set (F13 and up, media keys) need a line in `window/keys.rs`.
- Prefer exact `Color::Rgb` colours. Named ANSI colours take the terminal's theme there
  and the xterm palette in the window.
- Box-drawing (U+2500–257F) and block (U+2580–259F) characters always work. Any other
  symbol needs a font that has it: the window carries Fira Code (`fonts/`) and falls
  back to each platform's system fonts (`window/font.rs`). A new symbol that no font has
  fails `every_drawn_character_has_a_glyph`; colour emoji don't render in the window (a
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
- The window's GPU tests skip themselves on a machine with no GPU adapter; the rest
  always run (the window carries its own font).
- `AppState::new()` has dev mode off and locked, as a player's game starts. A test or
  shot that needs the free camera, the console, the inspector or the title's debug
  starts calls `st.unlock_dev()` first (`--dev` and `STRATA_DEV=1` do it for a real
  run; see `docs/development.md`).
- Window code for another platform (behind `cfg`) never compiles on a Mac build; check
  it with `cargo check -p strata-zero --features window --target x86_64-pc-windows-msvc`
  (or `x86_64-unknown-linux-gnu`) `--target-dir target/smoke`. The Windows app build
  needs a resource compiler; from a Mac, `STRATA_NO_WINDOWS_RESOURCES=1` leaves the
  resources out so `--features app` checks too.
- On GitHub, `.github/workflows/ci.yml` runs the same checks on macOS, Linux and Windows
  for both builds, so keep shared code portable (no platform APIs outside `cfg`), and
  builds the three desktop apps; its Windows and Linux app jobs upload screenshots
  (`windows-shots`, `linux-shots`) to look at. Releases are described in
  `docs/development.md` under "Release", including the Mac app's signing.

## Seeing the window without opening it

- **Shots of real screens.** `SHOT_DIR=<dir> cargo test -p strata-zero --features window
  --target-dir target/smoke -- --ignored <shot test>` writes each shot-harness screen
  (`screens/shots.rs`) as `.svg` and as `.png`, painted as the window paints it.
  `window_shots` adds the title, as players see it and in dev mode. With `SHOT_FX=1`
  set too, it also writes `.soft.png` and `.crt.png` through the GPU effects.
- **The live window.** `STRATA_WINDOW_SHOT=<file.png>` makes it write what it shows,
  1.5 s in. `STRATA_SCREEN_FX=off|soft|crt` picks the effects; `STRATA_FONT=<file>`
  picks the font.
- **The app.** `scripts/macos-app.sh` builds `target/smoke/macos/Strata Zero.app`;
  `scripts/windows-app.sh` and `scripts/linux-app.sh` build the Windows zip and the
  Linux archive on those systems.
- **The game's folder.** `cities/` and `runs/` live in the player's app folder for the
  terminal game and the apps alike: `~/Library/Application Support/Strata Zero/` on a
  Mac, `%APPDATA%\Strata Zero` on Windows, and `~/.local/share/strata-zero` on Linux
  (`platform::Platform::files_dir`). `STRATA_DIR=.` keeps a checkout's own instead.

## Saving the run

A save is a snapshot of `AppState` (`crates/strata-zero/src/runs/`), one file per run in
`runs/`. Read `docs/saving.md` before adding state that lives through play, or touching
how a run is opened. The rules that bite:

- A new `AppState` field doesn't compile until `runs::snapshot::snapshot` and `restore`
  name it: save it, or bind it `_` with why not (UI, setting, dev, content, rebuilt).
- Saved types derive `Serialize` and `Deserialize`. No `HashSet` (use `BTreeSet`), `Rc`,
  `Cell`, `Instant` or `&'static str` in them. A cache can be `#[serde(skip)]` only if
  `runs::snapshot::after_restore` rebuilds it.
- A new field in a saved type needs `#[serde(default)]`, and renaming or reshaping one
  needs a reader for its old shape (as `net::EchoRead` does); the fixtures in
  `runs/fixtures/` must keep opening. Never regenerate them to pass a test.
- A mission or step of `flavor/missions.ron` held by position is a `runs::ids::StepRef`,
  never a bare index; the save writes ids.
- Set-up in `enter_city_at` or `missions::begin` doesn't run when a run is reopened, and
  a screen that takes story off `AppState` while open must say `holds_story()`.
- `AppState::new()` saves nowhere; tests that save use `runs::testutil::Dir`.

## The manual

The player's manual lives in `manual/`, a chapter a file, and is compiled into the game
(the MANUAL booklet, `screens/manual.rs`) and published on stratazero.io from the same
files. A change a player sees (a key, a screen, a rule) updates the chapter that
describes it in the same branch. `manual/README.md` has the format; the tests reject
Markdown the booklet can't draw, and check the Controls chapter's "On foot" keys against
the `?` key list (`screens/help.rs`).

## Design notes

Specs and plans live in `docs/superpowers/specs` and `docs/superpowers/plans`. The window
is described in `2026-10-29-window-prototype-design.md` and
`2026-10-29-window-gpu-effects-design.md`, with what a Steam build still needs; the
Windows and Linux apps in `2026-10-30-windows-app-design.md`.
