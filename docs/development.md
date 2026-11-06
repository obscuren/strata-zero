# Developing Strata Zero

How the code is laid out, how to run and check it from a checkout, and how releases are
made. To play, see the [README](../README.md).

## Layout

```
crates/citygen       world model + city generator + interiors (no terminal dependencies)
crates/game          characters, items, abilities, the build rules (no terminal dependencies)
crates/sim           the living city: people, places, paths and the world's minute
crates/strata-zero   the game: ratatui app, screens, renderer, walking and jacking in, the `gen` CLI
flavor/              names.ron (names, handles, signage glyphs) · style.ron (palettes, glyph sets) · content.ron (game content) · people.ron (professions and their lines) · broadcasts.ron (what the city says as you drop in)
manual/              the player's manual, a chapter a file (format in manual/README.md): the game's MANUAL booklet and stratazero.io/manual
docs/superpowers/specs   design specs
docs/superpowers/plans   implementation plans
```

## Run from a checkout

```sh
cargo run --release -p strata-zero
```

Headless generation, for scripts and tooling:

```sh
cargo run --release -p strata-zero -- gen --seed 42 --size medium --out cities/demo.city
cargo run --release -p strata-zero -- gen --seed 42 --size large --json --out /tmp/demo.json
```

Sizes: `small` (160x80), `medium` (240x120), `large` (320x160) or `WxH`.
Other flags: `--districts 4..12` (an upper bound; slivers merge into their
neighbours), `--water none|bay|river`, `--density 0..1`.

Needs a truecolor terminal of at least 100x30 cells (iTerm2, Ghostty, kitty,
WezTerm). Runs and saved cities go to the game's folder (below).

### Where the game keeps its files

Runs (`runs/*.run`) and dev mode's saved cities (`cities/*.city`) are kept in the
player's app folder, the same one for the terminal game, `--window` and the desktop
apps, wherever the game is started from:

| Platform | Folder |
|---|---|
| Mac | `~/Library/Application Support/Strata Zero/` |
| Windows | `%APPDATA%\Strata Zero\` |
| Linux | `$XDG_DATA_HOME/strata-zero/`, else `~/.local/share/strata-zero/` |

`STRATA_DIR=<folder>` keeps them in another folder instead, and `STRATA_DIR=.` in the
one the game is started in: a checkout's own `cities/` and `runs/`, beside the sources
(both are in `.gitignore`):

```sh
STRATA_DIR=. cargo run --release -p strata-zero
STRATA_DIR=. target/release/strata
```

It's a variable rather than a flag so it can be set once for a shell and reaches
`cargo run`, `--window` and scripts alike. The game never guesses from a `runs/` or
`cities/` it finds in the working folder: a player's own folders can be called that.
Like `STRATA_DEV`, it reaches the desktop app only when a shell starts it.
`platform::Platform::files_dir` picks the folder and `cli::start_state` hands it to
`AppState.saves` and `AppState.cities`.

Terminal builds before v0.2.0 kept both folders in the working folder (the desktop
apps already used the app folder). To carry on with runs saved by one, move its `runs/`
(and `cities/`) into the folder above, or start the game with `STRATA_DIR` naming the
folder they're in.

Overworlds, many seeds on one page in the atlas look, to open in a browser:

```sh
cargo run --release -p strata-zero -- world --seeds 0..12 --out world-sheet.html
```

`--seed N` for one; `--size small|medium|large|WxH` (160x52, 320x104, 440x144; medium is
the game's); `--cities 3..16` (9 by default). The generator is `citygen::overworld`; its design is
`docs/superpowers/specs/2026-11-04-world-generator-design.md`.

One level down, the walkable ground of the tiles round one overworld tile, drawn as the
street draws it beside the overworld round it:

```sh
cargo run --release -p strata-zero -- world --seed 0 --detail 108,30 --span 10x4 --out detail.html
```

`--detail TX,TY` needs `--seed`; `--span WxH` counts overworld tiles (8×8 detail tiles
each). The ground is `citygen::overworld::detail`; its design is
`docs/superpowers/specs/2026-11-04-world-tiles-design.md`.

### The window

The `window` feature builds the window front end; `--window` opens it:

```sh
cargo run --release -p strata-zero --features window -- --window
scripts/macos-app.sh   # target/smoke/macos/Strata Zero.app, universal, ad-hoc signed
scripts/windows-app.sh # on Windows (Git Bash): target/smoke/windows/strata-zero-windows-app.zip
scripts/linux-app.sh   # on Linux: target/smoke/linux/strata-zero-linux-app.tar.gz
```

The scripts build the `app` feature: the desktop app, which always opens the window
and works in the game's folder (the player's app folder, as above). The window carries
Fira Code (`crates/strata-zero/fonts/`, SIL Open Font License) and takes the symbols
Fira Code lacks from each platform's own fonts. On a Mac the window's chords are Cmd
chords; on Windows and Linux F11 or Alt+Enter is fullscreen and Ctrl+= / Ctrl+- /
Ctrl+0 set the text size. It draws through wgpu: Metal on a Mac, Direct3D 12 or Vulkan on Windows,
Vulkan or OpenGL on Linux.

`STRATA_FONT=<file>` picks the font, `STRATA_SCREEN_FX=off|soft|crt` the screen
effects.

`STRATA_WINDOW_SHOT=<file.png>` writes what the window shows 1.5 s in. The designs,
and what a Steam build still needs, are in
`docs/superpowers/specs/2026-10-29-window-prototype-design.md`,
`docs/superpowers/specs/2026-10-29-window-gpu-effects-design.md` and
`docs/superpowers/specs/2026-10-30-windows-app-design.md`.

### Dev mode

Dev mode is off in every build, and locked: no `DEV` badge, no free camera, no `:`
console, no inspect or legend panes, and only NEW GAME, LOAD GAME and QUIT on the
title. Start the game with `--dev`, or with `STRATA_DEV=1` set, to unlock it:

```sh
cargo run --release -p strata-zero -- --dev
STRATA_DEV=1 target/release/strata
open -a "Strata Zero" --args --dev   # the Mac app
```

It then starts on, `` ` `` flips it, Settings has a row for it, `?` lists its keys,
and the title has NEW GAME (DEBUG) and NEW GAME · COMBAT while it is on. The
overworld and the Net overworld take the keys that need no street: `` ` ``, the `:`
console (`tp X Y` moves you over the world there) and F5's inspector of the tile.
The game opens on a five-second splash (`screens/splash.rs`) before the title, in
dev mode too; any key skips it.
`STRATA_DEV` reaches the game only when a shell starts it: the Mac app opened from
the Finder or the Dock never sees it.

## Develop

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo clippy --workspace --all-targets --features strata-zero/window -- -D warnings
cargo test --workspace
cargo test --workspace --features strata-zero/window
cargo test -p citygen --test seeds -- --ignored   # 250-seed sweep
```

The window front end only compiles with its feature, so check both; `CLAUDE.md` has
more on keeping screens working in both front ends.

## Release

Bump `version` in the workspace `Cargo.toml`, commit, tag `vX.Y.Z` and push
the tag to GitHub. GitHub runs `.github/workflows/release.yml`, built by
[dist](https://opensource.axo.dev/cargo-dist/), and publishes a GitHub Release
with:

- the terminal game for every target in `dist-workspace.toml` (macOS on Apple
  Silicon and Intel, Linux on x86_64 and ARM64, Windows on x86_64), with the
  shell and PowerShell installers;
- `strata-zero-mac-app.dmg`: `Strata Zero.app`, the window build, universal,
  signed with a Developer ID and notarized. dist runs
  `.github/workflows/build-mac-app.yml` for it as a custom job: it builds with
  `scripts/macos-app.sh`, then signs, wraps, notarizes and staples with
  `scripts/macos-package.sh`, the only step that sees the secrets;
- `strata-zero-windows-app.zip` and `strata-zero-linux-app.tar.gz`: the window build
  for x86_64 Windows and Linux, from `.github/workflows/build-windows-app.yml` and
  `build-linux-app.yml` with `scripts/windows-app.sh` and `scripts/linux-app.sh`. The
  Windows exe isn't signed yet.

dist's notes list only the files dist builds, so once the release is out
`.github/workflows/app-notes.yml` puts the three apps at the top of them with
`scripts/app-notes.sh <tag>`, which can also be run by hand.

After changing `dist-workspace.toml`, run `dist generate`.

Every archive and app in the release carries `THIRD-PARTY-NOTICES.txt` beside
`LICENSE`: the licence notices of the crates the game is built from and of Fira Code,
which their licences ask to travel with it. `scripts/third-party.sh` writes it with
[cargo-about](https://github.com/EmbarkStudios/cargo-about) from `about.toml`, the
licences a dependency may carry. After adding or updating a dependency, run it and
commit the file.

### Signing the Mac app

The release needs five repository secrets (GitHub › Settings › Secrets and
variables › Actions):

| Secret | What it holds |
|---|---|
| `MACOS_CERTIFICATE` | A "Developer ID Application" certificate with its private key, exported from Keychain Access as `.p12`, in base64 (`base64 -i cert.p12 \| pbcopy`) |
| `MACOS_CERTIFICATE_PASSWORD` | The password the `.p12` was exported with |
| `APPLE_API_KEY` | The text of an App Store Connect API key, `AuthKey_….p8`, used to notarize |
| `APPLE_API_KEY_ID` | That key's ID |
| `APPLE_API_ISSUER` | The issuer ID shown above the list of keys |

To get them:

1. Join the Apple Developer Program.
2. Create a Developer ID Application certificate (Xcode › Settings › Accounts ›
   Manage Certificates, or developer.apple.com › Certificates), then export it with
   its private key from Keychain Access.
3. In App Store Connect › Users and Access › Integrations, create a team API key with
   the Developer role. Its `.p8` downloads only once.

Without the secrets the release stops at the Mac app. To release anyway with an app
signed ad hoc (players then allow it once in System Settings › Privacy & Security),
set the repository variable `MAC_APP_UNSIGNED` to `1`. Locally,
`STRATA_SIGN_IDENTITY="Developer ID Application: …"` plus the three `APPLE_API_*`
variables make `scripts/macos-app.sh --dmg` sign and notarize the same way (it hands
the packaging to `scripts/macos-package.sh`).

### Checks

`.github/workflows/ci.yml` runs on every push to main and every pull request:

- rustfmt;
- the crates' licences: each one that `about.toml` accepts, and
  `THIRD-PARTY-NOTICES.txt` up to date (`scripts/third-party.sh --check`);
- clippy and the tests on macOS, Linux and Windows, for both the terminal and the
  window build;
- the Mac app, built unsigned;
- the Windows and Linux apps, built as a release builds them, then seen working: a
  handful of screens drawn through the GPU and its effects (Direct3D 12 on WARP,
  Vulkan on lavapipe, both software renderers), and the app itself started for a
  screenshot of its window (on Linux under a virtual display). The archives and the
  screenshots are the run's artifacts (`windows-app`, `windows-shots`, `linux-app`,
  `linux-shots`). On Linux every symbol's glyph is checked against the common
  desktop fonts.
