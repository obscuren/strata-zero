# Strata Zero

**An open-source cyberpunk roguelike.** Wake up in a megablock flat, take work from
the fixers on the corners, talk your way into buildings through their terminals and
jack into the Net to run their ICE. The city keeps its own clock: everyone in it has
a job, a bed and a bar, whether you are watching or not.

Inspired by Mike Pondsmith's *Cyberpunk 2020*. Play it in your preferred terminal, or in
the GPU-accelerated app, on macOS, Linux and Windows.
Its home on the web is [stratazero.io](https://stratazero.io).

## Download

**The app**, GPU-accelerated, in a window of its own:

- **macOS:** [strata-zero-mac-app.dmg](https://github.com/obscuren/strata-zero/releases/latest/download/strata-zero-mac-app.dmg).
  Open it and drag Strata Zero into Applications.
- **Windows:** [strata-zero-windows-app.zip](https://github.com/obscuren/strata-zero/releases/latest/download/strata-zero-windows-app.zip).
  Extract it and run `Strata Zero.exe` (if SmartScreen stops it: More info › Run anyway).
- **Linux** (x86_64): [strata-zero-linux-app.tar.gz](https://github.com/obscuren/strata-zero/releases/latest/download/strata-zero-linux-app.tar.gz).
  Extract it and run `strata-zero`, or `./install.sh` to put it in your app menu.

**In a terminal**, on macOS and Linux:

```sh
curl -fsSL https://github.com/obscuren/strata-zero/raw/main/install.sh | sh
```

on Windows, in PowerShell:

```powershell
irm https://github.com/obscuren/strata-zero/raw/main/install.ps1 | iex
```

Then run `strata`. It wants a truecolor terminal of at least 100×30 (iTerm2,
Ghostty, kitty, WezTerm, Windows Terminal). Every build is on the
[releases page](https://github.com/obscuren/strata-zero/releases).

## How to play

The manual comes with the game: `MANUAL` on the title screen or in the pause menu, or
`?` while you play for the keys and `space` from there for the manual. The same manual
is on [stratazero.io/manual](https://stratazero.io/manual), and its chapters are in
[`manual/`](manual/).

## Your saves

Your runs are kept in one folder, whichever way you play and wherever you start the
game:

- **macOS:** `~/Library/Application Support/Strata Zero/`
- **Windows:** `%APPDATA%\Strata Zero\`
- **Linux:** `~/.local/share/strata-zero/` (or `$XDG_DATA_HOME/strata-zero/`)

Before v0.2.0, `strata` in a terminal kept them in `cities/` and `runs/` folders in
whatever folder it was started from. Move those into the folder above and the game
finds them again.

## Build from source

```sh
cargo run --release -p strata-zero                               # in the terminal
cargo run --release -p strata-zero --features window -- --window  # in a window
```

[docs/development.md](docs/development.md) covers the checks, the layout of the
crates and how releases are made.

## License

Strata Zero is free software under the [GNU GPL v3](LICENSE): play it, share it,
change it, and pass the source on with whatever you build from it. The name
"Strata Zero" and its icon stay with the project, so a fork goes out under a name
of its own.

Strata Zero is an independent game, not affiliated with or endorsed by R. Talsorian
Games or CD Projekt.
