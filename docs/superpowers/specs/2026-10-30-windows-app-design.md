# Strata Zero on Windows and Linux

2026-10-30. Follows `2026-10-29-window-prototype-design.md` and
`2026-10-29-window-gpu-effects-design.md`, which made the Mac app. Jeffrey: "we just did
the mac version. Now the windows renderer.. simple as that. It will be downloadable from
GH for now." Then, while this was being planned: "Perhaps we can do the linux and vulkan
build in one go as well."

## Why

The game now runs in a window of its own on the Mac (`Strata Zero.app`, on the GPU, with
the screen effects) and every GitHub Release carries it as a signed DMG. Steam is the
longer aim: Windows is most of Steam, and the Steam Deck is Linux drawing through Vulkan.
The window code was written to be portable (winit, wgpu, softbuffer, fontdue), and CI
already compiles and tests the window build on Windows and Linux; what stops a Windows or
Linux player today is everything around it:

- the fonts are read from Mac paths, so elsewhere the window doesn't open at all;
- the window's chords are Cmd chords, and a PC has no Cmd key;
- the game knows it is "the app" by finding itself inside a `.app` bundle;
- nothing builds, packages or publishes a Windows or Linux app;
- nothing has ever drawn the window through Direct3D 12 or Vulkan.

## Goals

1. **Windows.** A player downloads `strata-zero-windows-app.zip` from a GitHub Release,
   extracts it, and double-clicks `Strata Zero.exe`. The game opens in its own window,
   with no console window, looking as it does on the Mac: Fira Code, the GPU screen
   effects (Soft by default), the same colours. The exe has the game's icon in Explorer,
   the title bar and the taskbar, and "Strata Zero" as its name in Task Manager. Cities
   are saved in `%APPDATA%\Strata Zero\cities`.
2. **Linux.** A player downloads `strata-zero-linux-app.tar.gz` (x86_64), extracts it, and
   runs `strata-zero` in it, or runs `./install.sh` once to get "Strata Zero" in the
   desktop's app menu with its icon. The window draws through Vulkan (wgpu's Linux
   backend), falling back to OpenGL and then to softbuffer, on X11 and Wayland. Cities are
   saved in `~/.local/share/strata-zero/cities` (or under `$XDG_DATA_HOME`).
3. Keys work the way a PC player expects, on Windows and Linux:
   - Every game key works as it does on the Mac and in a terminal.
   - Window chords: Ctrl+= (and Ctrl++) bigger, Ctrl+- smaller, Ctrl+0 back to normal,
     F11 and Alt+Enter fullscreen. The close button (and Alt+F4) quit.
   - AltGr types what the layout puts there (`@` and `[` on a German keyboard), so the
     in-game terminal and editor can be typed into on any layout.
4. Every release carries both apps next to the Mac app, and the release notes name all
   three.
5. CI builds both apps on every push to main and every pull request, and shows them
   working without a PC of either kind:
   - screenshots of game screens painted by the window code, plain and through the
     effects, drawn by Direct3D 12 (WARP, Microsoft's software renderer) on Windows and
     by Vulkan (lavapipe, Mesa's software renderer) on Linux;
   - the real app started on each (on Linux under a virtual X display), with the live
     window's own screenshot written after 1.5 s;
   - all kept as downloadable artifacts.
6. Every symbol the game draws has a glyph on the Mac, on Windows, and on a Linux desktop
   with the common fonts, checked by a test rather than by eye.

## Non-goals

- Code signing on Windows. The exe is unsigned for now, so SmartScreen warns on first
  run ("More info" → "Run anyway"); the release notes say so. Signing (Azure Trusted
  Signing or a certificate) is for later, with Steam.
- Installers (MSI, setup.exe, .deb, AppImage, Flatpak). An archive is enough for a GitHub
  download; the game keeps its files in the player's own folders, so it can live
  anywhere.
- ARM builds of either app (Windows on ARM, ARM64 Linux). The terminal game already
  covers ARM64 Linux.
- Steamworks, controllers, saving settings.
- Changing how the game looks on Jeffrey's Mac.

## Anti-goals

- A console window opening behind the game on Windows, or flashing on start.
- Anything screen-specific for a platform. The screens stay one set for both front ends
  and every platform; the work is below `Runner::frame`.
- The Mac app getting worse: it must open, draw and save exactly as it does now.

## Design

### One notion of the platform

`window/platform.rs` holds `Platform { Mac, Windows, Linux }` and the running one
(`Platform::NATIVE`; anything that isn't macOS or Windows counts as Linux). Fonts, chords
and the app folder are answers on a `Platform`, so every platform's answer is tested on
any machine; only the code that calls the operating system is behind `cfg`.

### Fonts: Fira Code inside the build

- Fira Code 6.2 Regular and Bold (SIL Open Font License 1.1) live in
  `crates/strata-zero/fonts/` with `OFL.txt`, and the window build carries them with
  `include_bytes!`. Fira Code is the face the Mac draws with today (Jeffrey's Mac has the
  Nerd Font build of it; the game uses none of the Nerd icons, which sit in the Private
  Use Area).
- A face is either a file on disk or bytes in the binary. The monospace families are
  tried in order: `STRATA_FONT`, then the platform's list, then the built-in Fira Code,
  which always loads. The Mac's list is FiraCode Nerd Font Mono from `~/Library/Fonts`,
  so Jeffrey's Mac draws exactly as before; SF Mono and Menlo, which every other Mac got,
  give way to the built-in Fira Code, so all players see the face the game was made with.
  Windows and Linux have no list and get the built-in face. `Fonts::load` can no longer
  fail.
- Fallbacks for characters the monospace face lacks are per platform:
  - Mac: as today (Apple Symbols, Menlo, Arial Unicode), then Geneva and Euphemia UCAS,
    which have the six characters no Mac fallback had (the bike tile's ꝺ, the sticker
    letters ꓘ ꓤ ꓭ ᗡ, and Ԁ): they were hollow boxes on the Mac until now.
  - Windows, from `%WINDIR%\Fonts`: Segoe UI Symbol (shapes, arrows, dingbats, braille),
    Segoe UI and Consolas (IPA, Latin extensions, Lisu), Cambria Math, Gadugi (Canadian
    syllabics), then a Japanese face for the halfwidth katakana (Yu Gothic, MS Gothic or
    Meiryo, whichever exists).
  - Linux: font files differ in place by distribution, so the fallbacks are named by
    file (DejaVu Sans, DejaVu Sans Mono, Noto Sans Symbols 2, Noto Sans Math, Noto Sans,
    the Noto faces for Lisu and Canadian syllabics, Noto Sans CJK) and found by walking
    the usual font folders (`/usr/share/fonts`, `/usr/local/share/fonts`,
    `$XDG_DATA_HOME/fonts` or `~/.local/share/fonts`, `~/.fonts`) once, the first time a
    character needs a fallback.
- Fira Code lacks 123 of the 226 non-ASCII characters the game uses (the bars ▰ ▱, the
  small triangles ▸ ◂, the katakana, the upside-down letters). A test collects every
  character the game can draw (from `flavor/*.ron` and the string text of the sources,
  less ASCII and the box/block ranges that are drawn as shapes) and checks the font chain
  has each one. It asserts on macOS and Windows; on Linux it asserts when
  `STRATA_FONT_CHECK=1` (set by the CI job that installs the common desktop fonts) and
  otherwise reports what is missing. The fallback lists are tuned until it passes.
- The window tests that skipped on a machine with no fonts now always run.

### "The app": a cargo feature

- A new cargo feature `app` (it implies `window`) means "this binary is the desktop app".
  An app build always opens the window (no `--window`), and makes the player's app folder
  its working folder before anything else, so `cities/` lands there:
  - macOS: `~/Library/Application Support/Strata Zero`;
  - Windows: `%APPDATA%\Strata Zero` (else `%USERPROFILE%\AppData\Roaming\Strata Zero`);
  - Linux: `$XDG_DATA_HOME/strata-zero`, else `~/.local/share/strata-zero`.
  With none of those variables set, the app stops and says why.
- The Mac app moves to the same feature: `scripts/macos-app.sh` builds with
  `--features app`, and the `.app` bundle-path check (`in_bundle`) goes. A plain
  `strata --window` (feature `window` only) keeps working from the folder it was started
  in, on every platform.
- On Windows, an app build:
  - is a GUI program (`windows_subsystem = "windows"`), so no console opens;
  - carries an icon and version information as Windows resources (ProductName and
    FileDescription "Strata Zero", the version from Cargo), put in by `build.rs` with the
    `winresource` crate as an optional build-dependency of the `app` feature;
  - gives its window the same icon from that resource (title bar and taskbar);
  - shows a fatal error, and a panic, in a message box, since there is no console.
- On Linux, the window (any window build) is named `strata-zero` (the Wayland app id and
  the X11 class), which ties it to `strata-zero.desktop`, and has the game's icon on X11
  (Wayland takes the icon from the desktop file).

### Keys: Mac chords and PC chords

- `keys::translate` takes the `Platform`. The Mac is exactly today's behaviour (Cmd
  chords; Option types the layout's character). Windows and Linux share the PC chords:
  - Ctrl+= or Ctrl++ bigger, Ctrl+- smaller, Ctrl+0 back to normal; F11 and Alt+Enter
    fullscreen. There is no quit chord: Alt+F4 arrives as a close request.
  - AltGr: winit already reports the right Alt key on an AltGr layout as no modifier at
    all, but Windows also treats left Ctrl+Alt as AltGr. So Ctrl+Alt with a character
    key whose character differs from the bare key's is AltGr typing that character, with
    no modifiers; Ctrl+Alt with any other key is a Ctrl+Alt chord.
  - Alt with a key otherwise carries the ALT modifier, as a terminal reports it. No
    screen reads Alt chords (CLAUDE.md), so this changes nothing in the game.
  - Chords with the Windows (Super) key do nothing.
- The game's own Ctrl chords are letters only (Ctrl+C, K, O, U, W, X in the editor and
  terminal) and its F-keys are F1–F6, so the PC chords take nothing from it.

### Icons

- The `app_icon` test (ignored, run on demand) draws the Mac icon as now and also the PC
  icon: the same art on a fuller square (Windows and Linux icons don't leave the macOS
  margin). It writes `crates/strata-zero/windows/AppIcon.ico` (PNG images at 16, 24, 32,
  48, 64 and 256 pixels) and `crates/strata-zero/linux/strata-zero.png` (256 pixels).
  Both are committed, as `AppIcon.icns` is. The Linux PNG is also built into window
  builds for the X11 window icon.

### Packaging

- `scripts/windows-app.sh` (bash; on Windows it runs under Git Bash, as CI's
  `shell: bash` does) builds `--release --features app` into `target/smoke`, stages
  `target/smoke/windows/Strata Zero/` with `Strata Zero.exe`, the game's `LICENSE` and the
  font's `OFL.txt`, and zips it to `target/smoke/windows/strata-zero-windows-app.zip`.
  `--icon` regenerates the icons.
- `scripts/linux-app.sh` builds the same way, stages `target/smoke/linux/strata-zero/`
  with the binary as `strata-zero`, `strata-zero.desktop`, `strata-zero.png`,
  `install.sh`, `LICENSE` and `OFL.txt`, and makes
  `target/smoke/linux/strata-zero-linux-app.tar.gz`. `install.sh` copies the binary to
  `~/.local/bin/strata-zero`, the icon to
  `~/.local/share/icons/hicolor/256x256/apps/strata-zero.png` and the desktop file
  (pointing at the installed binary) to `~/.local/share/applications/`; `install.sh
  --uninstall` removes them. The binary is `strata-zero`, not `strata`, so it never
  shadows the terminal game's `strata` on the PATH.
- Built on the runners dist builds the terminal game on (`windows-2022`,
  `ubuntu-22.04`), so the Linux app runs on the same glibc range.

### Releases and CI

- `.github/workflows/build-windows-app.yml` and `build-linux-app.yml`, dist
  global-artifacts jobs like `build-mac-app.yml`, run the scripts and hand the archives
  to the release as `artifacts-windows-app` and `artifacts-linux-app`.
- Release notes: `scripts/mac-app-notes.sh` becomes `scripts/app-notes.sh` (workflow
  `app-notes.yml`): for each desktop app on the release that the notes don't yet link, it
  adds a section on top, in the order Mac, Windows, Linux. The Windows text says it is
  unsigned and how to get past SmartScreen; the Linux text names `install.sh`.
- CI (`ci.yml`) gains `windows-app` and `linux-app` jobs. Each runs clippy with the `app`
  feature, builds its archive, runs the shot tests with `SHOT_DIR` and `SHOT_FX`, starts
  the app itself with `STRATA_WINDOW_SHOT` for a live screenshot (Linux: under `xvfb-run`;
  both: stopped after a few seconds, and the job fails if no screenshot was written), and
  uploads the archive and the screenshots. The Linux job first installs Mesa's Vulkan
  drivers, `xvfb` and the common desktop fonts, runs the window tests with
  `STRATA_FONT_CHECK=1` and `WGPU_BACKEND=vulkan`, so the GPU tests and the effects run
  through Vulkan there.

### Rendering

No change to the drawing code. wgpu presents through Direct3D 12 or Vulkan on Windows and
Vulkan or OpenGL on Linux, with softbuffer as the fallback, exactly as the Mac uses Metal.

## Checking it

- On the Mac: unit tests for the PC chords, the app folder, the `.ico` writer, the
  built-in font, the Linux font index and glyph coverage; clippy and tests with and
  without the window feature; `cargo check` of the window build for
  `x86_64-pc-windows-msvc` and `x86_64-unknown-linux-gnu` for code a Mac build never
  compiles; the Mac app rebuilt with the `app` feature, opened, and seen to save into
  Application Support.
- On Windows and Linux: the draft PR's CI, where the `check` matrix runs every test, and
  the app jobs build the archives and the screenshots. The screenshots are looked at
  before this is called done.

## After review

A builder read this before planning. Its three questions were settled as the author's
calls (Jeffrey can overturn any of them):

- **Mac players' face.** Only Jeffrey's Mac has Fira Code installed; other Macs drew
  with SF Mono. The built-in Fira Code now comes before SF Mono and Menlo (which drop
  out), so every Mac player sees Fira Code. Jeffrey's Mac is unchanged.
- **Mac glyph gaps.** Six characters had no glyph on any Mac (they drew as boxes), so the
  coverage test could not pass there. Geneva and Euphemia UCAS join the Mac's fallbacks.
- **How far before done.** The draft PR's CI green, its screenshots looked at, and the
  release path run once on the PR: dist's `pr-run-mode = "upload"` on the branch for one
  push, so the three custom release jobs (Mac, Windows, Linux) build and upload there,
  then back to the default. A broken custom job would otherwise block the next real
  release, since dist's host job needs all of them. The notes job only runs on a real
  release; its new sections are checked by dry runs. Merging and pushing main stay
  Jeffrey's call.

Also from the review: the Mac app now carries Fira Code too, so `OFL.txt` goes into the
`.app`'s Resources as well as into both archives.

After the build, a second review and the PR's CI found, and the branch fixed:

- The exe linked the C runtime as a DLL (`VCRUNTIME140.dll`), which a fresh Windows
  lacks; it is now built in (`+crt-static`, as dist builds the terminal game), and
  `scripts/windows-app.sh` fails if the exe still imports it.
- Windows PowerShell's `Compress-Archive` wrote `\` into the zip's paths; Windows' own
  `tar` writes the zip instead.
- `install.sh` built an icon cache in the player's folder, which can hide icons other
  programs add later; it touches the folder instead.
- On Linux, fallbacks are also asked of fontconfig, for distributions that name or
  place fonts differently; Noto Sans Symbols joined the list (⌗).
- winit needs `libxkbcommon-x11` for an X11 window: every desktop has it, the CI runner
  didn't.
- GitHub retires the `macos-14` runner, dist's default for Apple Silicon, on
  2026-11-02; a brownout cancelled the first release dry run, and that build now runs
  on `macos-15`.

Seen working on GitHub's runners: the real `Strata Zero.exe` on Windows (Direct3D 12
on WARP) and `strata-zero` on Linux (Vulkan on lavapipe, under Xvfb), each writing its
live screenshot; every screen through Soft and CRT; and the whole release, the three app
jobs included, built on the PR with dist's upload mode.

Not done here: fontdue reads every glyph of a fallback face when it loads it, so the
first katakana spark costs a few hundred MB for a CJK face on Windows and Linux (the Mac
already pays this with Arial Unicode). Worth fixing before a Steam Deck build.

## Left to the builder

The exact fallback lists (driven by the coverage test), the PC icon's margins, how the
character scan tells strings from comments (a heuristic is fine), the message box's
wording, the release-notes wording, and how long CI lets the live app run.
