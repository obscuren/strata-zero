# Developing Strata Zero

How the code is laid out, how to run and check it from a checkout, and how releases are
made. To play, see the [README](../README.md).

## Layout

```
crates/citygen       world model + city generator + interiors (no terminal dependencies)
crates/game          characters, items, abilities, the build rules (no terminal dependencies)
crates/sim           the living city: people, places, paths and the world's minute
crates/strata-zero   the game: ratatui app, screens, renderer, walking and jacking in, the `gen` CLI
flavor/              names.ron (names, handles, signage glyphs) · style.ron (palettes, glyph sets) · content.ron (game content) · people.ron (professions and their lines)
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
WezTerm). Saved cities go to `./cities/*.city`.

### The window

The `window` feature builds the window front end; `--window` opens it:

```sh
cargo run --release -p strata-zero --features window -- --window
scripts/macos-app.sh   # target/smoke/macos/Strata Zero.app, universal, ad-hoc signed
```

`STRATA_FONT=<file>` picks the font, `STRATA_SCREEN_FX=off|soft|crt` the screen
effects.

`STRATA_WINDOW_SHOT=<file.png>` writes what the window shows 1.5 s in. The designs,
and what a Steam build still needs, are in
`docs/superpowers/specs/2026-10-29-window-prototype-design.md` and
`docs/superpowers/specs/2026-10-29-window-gpu-effects-design.md`.

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
and the title has NEW GAME (DEBUG) and NEW GAME · COMBAT while it is on.
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
  `scripts/macos-package.sh`, the only step that sees the secrets.

dist's notes list only the files dist builds, so once the release is out
`.github/workflows/mac-app-notes.yml` puts the DMG at the top of them with
`scripts/mac-app-notes.sh <tag>`, which can also be run by hand.

After changing `dist-workspace.toml`, run `dist generate`.

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
- clippy and the tests on macOS, Linux and Windows, for both the terminal and the
  window build;
- the Mac app, built unsigned.
