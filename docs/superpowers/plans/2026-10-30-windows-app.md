# Strata Zero on Windows and Linux Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** The window front end ships for Windows (`Strata Zero.exe` in
`strata-zero-windows-app.zip`) and Linux (`strata-zero` in
`strata-zero-linux-app.tar.gz`, drawing through Vulkan) in every GitHub Release, built and
screenshotted by CI.

**Architecture:** Everything Mac-specific in `crates/strata-zero/src/window/` becomes an
answer on a `Platform` value (fonts, chords, app folder), so all three answers are tested
on any machine. A cargo feature `app` marks the desktop-app build on every platform; on
Windows it also makes a GUI-subsystem exe with an icon resource and a message box for
fatal errors. Packaging mirrors the Mac app: a script, a dist custom job, a CI job per
platform.

**Tech Stack:** Rust 2024, winit 0.30, wgpu 30, fontdue 0.9, winresource (build-dep),
windows-sys 0.52 (Windows only), cargo-dist 0.32, GitHub Actions, bash.

**Spec:** `docs/superpowers/specs/2026-10-30-windows-app-design.md`

## Global Constraints

- Build and test with `--target-dir target/smoke`; never build into `target/release`.
- Check both front ends: clippy `-D warnings` and tests with and without
  `--features strata-zero/window` (CLAUDE.md, "Building and checking").
- Code for another OS is checked from the Mac with
  `cargo check -p strata-zero --features window --target <triple> --target-dir target/smoke`
  for `x86_64-pc-windows-msvc` and `x86_64-unknown-linux-gnu`.
- No screen changes: nothing above `Runner::frame` learns about the platform.
- The Mac app's behaviour does not change (same fonts, chords, folder).
- Names: feature `app`; `Strata Zero.exe` in `strata-zero-windows-app.zip`;
  `strata-zero` in `strata-zero-linux-app.tar.gz`; app id `strata-zero`; artifacts
  `artifacts-windows-app`, `artifacts-linux-app`; folders `%APPDATA%\Strata Zero`,
  `~/.local/share/strata-zero`.
- Commits end with `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`.

## Review Focus

1. A PC missing a fallback font (Windows without Japanese fonts, a Linux box without
   Noto): the character draws as the hollow box, nothing fails. Task 1 pins it
   (`missing_fallbacks_are_skipped`).
2. No `%APPDATA%` (or no `HOME` on Linux): the documented fallback folder, or the app
   stops with a message saying so. Task 2 pins it.
3. A non-ASCII user name in the profile path (`C:\Users\Jürgen`): the folder is built from
   `OsString`s and survives intact. Task 2 pins it.
4. Layout chords: AltGr on German (`@`), Ctrl+Alt+letter on a US layout (a chord),
   Ctrl+Shift+= (bigger), Shift+Tab (BackTab). Task 3 pins each, for Windows and Linux.
5. A fractional display scale (125 % → 18.75 px text): whole, positive cell metrics.
   Task 1 pins it (`metrics_at_fractional_sizes`).

---

### Task 1: Fira Code in the build, and fonts per platform

**Files:**
- Create: `crates/strata-zero/src/window/platform.rs`
- Modify: `crates/strata-zero/src/window/font.rs`, `crates/strata-zero/src/window/mod.rs`
  (module list, `Fonts::load()` call), `crates/strata-zero/src/window/png.rs`,
  `crates/strata-zero/src/window/raster.rs` (test helper), `CLAUDE.md`
- Already added: `crates/strata-zero/fonts/FiraCode-Regular.ttf`, `FiraCode-Bold.ttf`, `OFL.txt`

**Interfaces:**
- Produces: `pub enum Platform { Mac, Windows, Linux }`, `Platform::NATIVE` (by
  `target_os`; neither macOS nor Windows → `Linux`), in `window/platform.rs`.
- Produces: `Fonts::load() -> Fonts` (infallible), `Fonts::load_for(p: Platform, env: &dyn Fn(&str) -> Option<String>) -> Fonts`,
  `Fonts::has(&mut self, c: char) -> bool` (regular, bold or a fallback has `c`).

- [ ] **Step 1: Write the failing tests** (in `font.rs` tests)
  - `builtin_face_loads`: `Fonts::load_for(Platform::Windows, &no_env)` has `'M'`; the
    regular face lacks `'▸'`; `metrics(15.0)` has `w > 0 && h > w`.
  - `mac_families`: `families(Platform::Mac, env)` with HOME=`/Users/x` is the Nerd Font
    Mono pair under `/Users/x/Library/Fonts`, then the built-in pair; with
    STRATA_FONT=`/f.ttf` that pair comes first. `mac_fallbacks_add_geneva_and_euphemia`:
    the Mac fallbacks end with `/System/Library/Fonts/Geneva.ttf` and
    `/System/Library/Fonts/Supplemental/EuphemiaCAS.ttc`.
  - `windows_fallbacks_read_the_windows_font_folder`: with WINDIR=`D:\Win`, every
    `fallbacks(Platform::Windows, env)` file is in `D:\Win\Fonts`, `seguisym.ttf` first;
    without WINDIR, `C:\Windows\Fonts`.
  - `linux_font_index_finds_by_name`: a temp tree `a/x/DejaVuSans.ttf`,
    `b/NotoSansCJK-Regular.ttc`, `b/other.ttf`; `find_font_files(&[a, b, missing], &["NotoSansCJK-Regular.ttc", "DejaVuSans.ttf", "Nope.ttf"])`
    returns the two paths in the names' order.
  - `missing_fallbacks_are_skipped`: fallbacks of two nonexistent files: `has('M')`,
    `!has('\u{10FFFD}')`, no panic.
  - `metrics_at_fractional_sizes`: px `18.75, 22.5, 11.25` → `w ≥ 1`, `h ≥ 1`,
    `baseline < h`.
  - `every_drawn_character_has_a_glyph`: scan `CARGO_MANIFEST_DIR/src/**/*.rs` (skip
    lines whose trimmed start is `//`) and `CARGO_MANIFEST_DIR/../../flavor/*.ron`; keep
    non-ASCII chars with `shapes::shape(c, 8, 16) == None`; list those `Fonts::load()`
    lacks. Assert empty on macOS and Windows, and on Linux when `STRATA_FONT_CHECK=1`;
    otherwise print the list.
- [ ] **Step 2: Run and see them fail**
  `cargo test -p strata-zero --features window --target-dir target/smoke window::font`
  → compile errors.
- [ ] **Step 3: Implement**
  - `Face` becomes `enum Face { File { path: PathBuf, index: u32 }, Builtin(&'static [u8]) }`;
    `FIRA_REGULAR`, `FIRA_BOLD` by `include_bytes!("../../fonts/…")`.
  - `fn families(p, env) -> Vec<(Face, Face)>`: STRATA_FONT, the Nerd Font Mono pair
    (Mac only), the built-in pair.
  - `fn fallbacks(p, env) -> Vec<Face>`: Mac as today plus Geneva and Euphemia UCAS
    (`Supplemental/EuphemiaCAS.ttc`, index 0). Windows under
    `<WINDIR|C:\Windows>\Fonts`: `seguisym.ttf`, `segoeui.ttf`, `consola.ttf`,
    `cambria.ttc` (1), `gadugi.ttf`, `YuGothR.ttc`, `msgothic.ttc`, `meiryo.ttc`,
    `arial.ttf`. Linux: `find_font_files(linux_font_dirs(env), LINUX_FALLBACKS)` where the
    dirs are `/usr/share/fonts`, `/usr/local/share/fonts`, `$XDG_DATA_HOME/fonts` (else
    `$HOME/.local/share/fonts`), `$HOME/.fonts`, and `LINUX_FALLBACKS` is
    `DejaVuSans.ttf`, `DejaVuSansMono.ttf`, `NotoSansSymbols2-Regular.ttf`,
    `NotoSansMath-Regular.ttf`, `NotoSans-Regular.ttf`, `NotoSansLisu-Regular.ttf`,
    `NotoSansCanadianAboriginal-Regular.ttf`, `NotoSansCJK-Regular.ttc`,
    `NotoSansCJKjp-Regular.otf`. The Linux list is built lazily: `Fallback` gains a
    "not yet looked up" state so the walk happens the first time a fallback is needed.
    `find_font_files` walks each dir to depth 4, indexes file names, returns the paths in
    `names` order (first found per name).
  - `load()` = `load_for(Platform::NATIVE, &|k| std::env::var(k).ok())`.
  - Callers drop `?`/`unwrap` on `Fonts::load()`; the raster tests' `fonts()` returns
    `Fonts` and no test skips for want of fonts.
  - CLAUDE.md: the symbols bullet says the window carries Fira Code plus per-platform
    fallbacks and that `every_drawn_character_has_a_glyph` fails on a symbol no font has.
- [ ] **Step 4: Run** the crate's window tests (pass; on this Mac the coverage test must
  pass; a gap gets a Mac fallback face and a note in the spec's After review), clippy
  both builds, and the two cross-checks → exit 0.
- [ ] **Step 5: Commit** `window: Fira Code in the build, fonts per platform`

### Task 2: The `app` feature and the app folder

**Files:**
- Modify: `crates/strata-zero/Cargo.toml` (`app = ["window"]` for now),
  `crates/strata-zero/src/window/platform.rs`, `crates/strata-zero/src/window/mod.rs`
  (`run`; drop `in_bundle`, `is_bundle_path`, `app_support_dir` and their test),
  `crates/strata-zero/src/cli.rs`, `scripts/macos-app.sh`, `CLAUDE.md`

**Interfaces:**
- Consumes: `Platform`.
- Produces: `pub const IS_APP: bool = cfg!(feature = "app")` in `window/mod.rs`;
  `Platform::app_dir(self, env: &dyn Fn(&str) -> Option<OsString>) -> Option<PathBuf>`.

- [ ] **Step 1: Write the failing tests** (in `platform.rs`; expected paths built with
  `Path::join` so they hold on any host)
  - `mac_app_dir`: HOME=`/Users/x` → `/Users/x/Library/Application Support/Strata Zero`.
  - `windows_app_dir`: APPDATA=`C:\Users\x\AppData\Roaming` → that + `Strata Zero`;
    APPDATA unset, USERPROFILE=`C:\Users\x` → `C:\Users\x` + `AppData/Roaming/Strata Zero`;
    neither → `None`.
  - `linux_app_dir`: XDG_DATA_HOME=`/d` → `/d/strata-zero`; empty or unset with
    HOME=`/h` → `/h/.local/share/strata-zero`; neither → `None`.
  - `app_dir_keeps_non_ascii`: APPDATA=`C:\Users\Jürgen\AppData\Roaming` survives.
- [ ] **Step 2: Run and see them fail.**
- [ ] **Step 3: Implement** `app_dir`; `run`: when `IS_APP`, the native app dir (or bail
  "can't tell where to keep the game's files: set APPDATA (Windows) or HOME"), created and
  made the working folder; `cli.rs`: `None if cli.window || crate::window::IS_APP`;
  `macos-app.sh`: `--features app` (header too); CLAUDE.md: the `app` feature and the
  three folders replace "Inside the app".
- [ ] **Step 4: Run** tests and clippy both builds; `scripts/macos-app.sh`; start
  `target/smoke/macos/Strata Zero.app/Contents/MacOS/strata` with
  `STRATA_WINDOW_SHOT=<scratch>/mac.png` for ~4 s; the PNG shows the title screen and
  `lsof -a -p <pid> -d cwd` showed `~/Library/Application Support/Strata Zero`.
- [ ] **Step 5: Commit** `window: an app feature, and an app folder on every platform`

### Task 3: PC chords

**Files:**
- Modify: `crates/strata-zero/src/window/keys.rs`, `crates/strata-zero/src/window/mod.rs`
  (pass `Platform::NATIVE`), `CLAUDE.md` (keys bullets)

**Interfaces:**
- Produces: `translate(pressed: bool, logical: &Key, bare: &Key, mods: ModifiersState, platform: Platform) -> Input`.

- [ ] **Step 1: Write the failing tests** (existing tests pass `Platform::Mac`; each new
  one runs for both `Platform::Windows` and `Platform::Linux`)
  - `pc_zoom_chords`: Ctrl+`=` → Bigger; Ctrl+Shift, logical `+`, bare `=` → Bigger;
    Ctrl+`-` → Smaller; Ctrl+`0` → ResetSize.
  - `pc_fullscreen`: F11 → Fullscreen; Alt+Enter → Fullscreen; Enter → `Game(Enter)`.
  - `pc_altgr_types`: Ctrl+Alt, logical `@`, bare `q` → `Game(Char('@'), NONE)`.
  - `pc_ctrl_alt_chord`: Ctrl+Alt, logical `a`, bare `a` → `Game(Char('a'), CONTROL | ALT)`.
  - `pc_alt_carries_alt`: Alt, logical `x` → `Game(Char('x'), ALT)`.
  - `pc_game_ctrl_letters`: Ctrl+`c`/`x`/`o` → `Game(Char(_), CONTROL)`.
  - `pc_super_does_nothing`: Super+`q` → `Nothing`.
  - `shift_tab_is_backtab` for all three platforms.
- [ ] **Step 2: Run and see them fail.**
- [ ] **Step 3: Implement**: Mac path unchanged; PC path: Super → `Nothing`; window chords
  (Ctrl with bare `=`/`+`/`-`/`0`; F11; Alt+Enter); the AltGr rule; then modifiers as held.
  CLAUDE.md: Cmd chords are the Mac's; on Windows and Linux the window takes Ctrl+=/-/0,
  F11 and Alt+Enter; Ctrl+letter stays the game's.
- [ ] **Step 4: Run** tests and clippy (window build).
- [ ] **Step 5: Commit** `window: PC chords, and AltGr typing`

### Task 4: The app on Windows and Linux: icons, names, no console, error box

**Files:**
- Create: `crates/strata-zero/build.rs`, `crates/strata-zero/src/window/fatal.rs`,
  `crates/strata-zero/windows/AppIcon.ico`, `crates/strata-zero/linux/strata-zero.png`
  (both generated)
- Modify: `crates/strata-zero/Cargo.toml`
  (`[build-dependencies] winresource = { version = "0.1", optional = true }`;
  `app = ["window", "dep:winresource", "dep:windows-sys"]`;
  `[target.'cfg(windows)'.dependencies] windows-sys = { version = "0.52", optional = true, features = ["Win32_Foundation", "Win32_UI_WindowsAndMessaging"] }`),
  `crates/strata-zero/src/main.rs`, `crates/strata-zero/src/window/mod.rs`,
  `crates/strata-zero/src/window/png.rs`

**Interfaces:**
- Produces: `window::fatal::show(message: &str)` (Windows app build: a message box
  titled "Strata Zero"; otherwise `eprintln!("strata: {message}")`) and
  `window::fatal::catch_panics()` (Windows app build: a panic hook that shows the panic,
  then calls the previous hook; elsewhere nothing).
- Produces (test-only, `png.rs`): `downsample(rgba: &[u8], from: usize, to: usize) -> Vec<u8>`
  (area average of a square RGBA image, colour weighted by alpha) and
  `write_ico(path: &Path, images: &[(u32, Vec<u8>)]) -> Result<()>` (square RGBA images
  stored as PNG; side 256 written as 0 in its entry).

- [ ] **Step 1: Write the failing tests** (`png.rs`)
  - `ico_lists_each_image`: sides `[16, 256]` → header `00 00 01 00 02 00`; entry width
    bytes 16 and 0; each entry's offset points at `89 50 4E 47`.
  - `downsample_averages`: 4×4 of two opaque colours in halves → 2×2 keeps the halves; a
    4×4 at alpha 128 keeps alpha 128 ± 1 and its colour.
- [ ] **Step 2: Run and see them fail.**
- [ ] **Step 3: Implement**
  - `downsample`, `write_ico`; `app_icon` (still ignored) draws through
    `fn icon_rgba(size: usize, inset: f32, radius: f32, edge: f32) -> Vec<u8>`: `icon.png`
    as now (1024, 100, 180, 10); the PC art (1024, 24, 160, 12) as `icon.ico` (256, 64,
    48, 32, 24, 16) and `strata-zero.png` (256). Run it, copy to
    `crates/strata-zero/windows/AppIcon.ico` and `crates/strata-zero/linux/strata-zero.png`,
    and look at both.
  - `build.rs`: with `feature = "app"`, `CARGO_CFG_TARGET_OS == "windows"` and no
    `STRATA_NO_WINDOWS_RESOURCES`: `winresource::WindowsResource::new()`,
    `.set_icon("windows/AppIcon.ico")`, ProductName and FileDescription "Strata Zero",
    `.compile()` (panics with its error); `rerun-if-changed` the icon,
    `rerun-if-env-changed` the variable.
  - `main.rs`: `#![cfg_attr(all(windows, feature = "app"), windows_subsystem = "windows")]`;
    with the window feature, `window::fatal::catch_panics()` first and an `Err` from
    `cli::run()` is shown with `window::fatal::show(&format!("{e:#}"))` when `IS_APP`.
  - `window/mod.rs` `open()`: Windows app build: `Icon::from_resource(1, None)` as the
    window icon and taskbar icon (errors ignored). Linux (`cfg(all(unix, not(target_os = "macos")))`):
    `WindowAttributesExtWayland::with_name(attrs, "strata-zero", "strata-zero")` (the same
    field X11 reads) and a window icon decoded from
    `include_bytes!("../../linux/strata-zero.png")` (`Icon::from_rgba`; errors ignored).
- [ ] **Step 4: Run** tests; clippy both builds; cross-checks for both triples with
  `--features window`, and
  `STRATA_NO_WINDOWS_RESOURCES=1 cargo clippy -p strata-zero --features app --target x86_64-pc-windows-msvc --target-dir target/smoke -- -D warnings`.
- [ ] **Step 5: Commit** `window: the desktop app on Windows and Linux`

### Task 5: Packaging, releases, release notes, CI

**Files:**
- Create: `scripts/windows-app.sh`, `scripts/linux-app.sh`,
  `crates/strata-zero/linux/strata-zero.desktop`, `crates/strata-zero/linux/install.sh`,
  `.github/workflows/build-windows-app.yml`, `.github/workflows/build-linux-app.yml`
- Rename + modify: `scripts/mac-app-notes.sh` → `scripts/app-notes.sh`,
  `.github/workflows/mac-app-notes.yml` → `.github/workflows/app-notes.yml`
- Modify: `dist-workspace.toml`, `.github/workflows/release.yml` (`dist generate`),
  `.github/workflows/ci.yml`, `README.md`

**Interfaces:**
- Produces: `target/smoke/windows/strata-zero-windows-app.zip` (`Strata Zero/` with
  `Strata Zero.exe`, `LICENSE.txt`, `OFL.txt`) and
  `target/smoke/linux/strata-zero-linux-app.tar.gz` (`strata-zero/` with `strata-zero`,
  `strata-zero.desktop`, `strata-zero.png`, `install.sh`, `LICENSE`, `OFL.txt`).

- [ ] **Step 1: scripts.** Both `set -euo pipefail`, cd to the repo root, `--icon` runs
  the `app_icon` test and copies both icons, then
  `cargo build --release -p strata-zero --features app --target-dir target/smoke` and
  stage + archive. Windows zips with
  `powershell -NoProfile -Command "Compress-Archive -Force -Path '<dir>' -DestinationPath '<zip>'"`
  (`cygpath -w` paths). Linux: `tar -czf` from `target/smoke/linux`. `install.sh`:
  `--uninstall` or install as the spec says, writing the desktop file with
  `Exec=$HOME/.local/bin/strata-zero` and `Icon=strata-zero`; refreshes
  `update-desktop-database` / `gtk-update-icon-cache` only when present. The desktop file:
  `Type=Application`, `Name=Strata Zero`, `Comment=A roguelike in a multi-level cyberpunk city`,
  `Exec=strata-zero`, `Icon=strata-zero`, `Terminal=false`, `Categories=Game;RolePlaying;`,
  `StartupWMClass=strata-zero`. shellcheck clean.
- [ ] **Step 2: release jobs.** `build-windows-app.yml` (runner `windows-2022`, bash,
  `git config --global core.autocrlf false`, checkout, script, upload
  `artifacts-windows-app`) and `build-linux-app.yml` (`ubuntu-22.04`, script, upload
  `artifacts-linux-app`), headers in the style of `build-mac-app.yml`.
- [ ] **Step 3: dist.** `global-artifacts-jobs = ["./build-mac-app", "./build-windows-app", "./build-linux-app"]`,
  `post-announce-jobs = ["./app-notes"]`; `dist generate`; `dist plan --tag=v0.1.0` → OK.
- [ ] **Step 4: notes.** `app-notes.sh` keeps the Mac section's text and adds:
  - Windows (`strata-zero-windows-app.zip`): "Strata Zero in a window of its own, for
    64-bit Windows 10 and 11. Unzip it and run `Strata Zero.exe`; your cities are kept in
    `%APPDATA%\Strata Zero`. It isn't signed yet, so Windows SmartScreen asks the first
    time: choose More info, then Run anyway."
  - Linux (`strata-zero-linux-app.tar.gz`): "Strata Zero in a window of its own, for
    64-bit Linux (X11 or Wayland, drawn through Vulkan where there is a driver). Unpack it
    and run `strata-zero`, or run `./install.sh` once to add Strata Zero to your app menu;
    your cities are kept in `~/.local/share/strata-zero`."
  Each is added only when the release has the asset and the notes don't link it; on top,
  Mac, Windows, Linux. Dry runs with the gh wrapper: v0.1.0 as is ("already link") and
  with empty notes (only the Mac section).
- [ ] **Step 5: ci.yml.** `windows-app` (`windows-2022`, bash, autocrlf off, rust-cache
  key `windows-app`) and `linux-app` (`ubuntu-22.04`, rust-cache `linux-app`; first
  `sudo apt-get install -y mesa-vulkan-drivers xvfb fonts-dejavu-core fonts-noto-core fonts-noto-cjk`).
  Each: clippy `--features strata-zero/app -- -D warnings`; Linux also
  `STRATA_FONT_CHECK=1 WGPU_BACKEND=vulkan cargo test -p strata-zero --features window`;
  the script; shots
  `SHOT_DIR=<dir> SHOT_FX=1 cargo test -p strata-zero --features window --target-dir target/smoke -- --ignored window::tests::window_shots shots::attributes_sheet shots::the_rig_at_home shots::skill_cards shots::combat_bloom_mid shots::terminal_on_the_street`;
  the live shot (the staged app with `STRATA_WINDOW_SHOT=<dir>/live.png`, under
  `timeout 20` and on Linux `xvfb-run -a`; then `test -s <dir>/live.png`); upload the
  archive and the shots dir (`if: always()` for the shots).
- [ ] **Step 5b: Mac bundle**: `scripts/macos-app.sh` copies `crates/strata-zero/fonts/OFL.txt`
  into `Contents/Resources/`.
- [ ] **Step 6: README** Install (both archives; SmartScreen; `install.sh`), Run (the
  window on Windows and Linux, the PC chords, the folders), Release (the two jobs, the
  notes), Checks (the two app jobs, their screenshots).
- [ ] **Step 7: Commit** `ci: the Windows and Linux apps in every release, built and shot on every push`

### Task 6: Seen on Windows and Linux

- [ ] **Step 1:** Whole-branch review by a fresh reviewer (code, scripts, workflows); fix
  what holds up.
- [ ] **Step 2:** Push the branch (`git push -u origin worktree-windows-app`) and open a
  draft PR (`gh pr create --draft -R obscuren/strata-zero …`).
- [ ] **Step 3:** Watch the run; fix and push until green.
- [ ] **Step 3b:** The release path once: `pr-run-mode = "upload"` in `dist-workspace.toml`,
  `dist generate`, push; the PR's Release run builds all targets and the three custom
  jobs. Then remove the line, `dist generate`, push.
- [ ] **Step 4:** `gh run download` the shots and archives; look at every PNG (title,
  sheets, ▰▱, katakana, effects, the live window) for both platforms and check both
  archives' layout. Tune fallbacks if a glyph is a box.
- [ ] **Step 5:** Spec "After review" with the calls made; memory; the finishing menu.
