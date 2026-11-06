# Saving and dying

Each runner is one run, and each run is one save. The game keeps it up to date for you, and you can save whenever you like.

## When the game saves

- When you jack in at the start of a new run.
- When you wake from sleep.
- When a step of the story is done.
- Every five minutes of play, though not in the middle of a fight.
- When you press `s` on the map, or choose `SAVE GAME` in the pause menu.
- Before `LOAD GAME`, `QUIT TO TITLE` and `QUIT GAME`, and when you close the app's window.

The bottom line shows `:: SAVED` for a moment, and the pause menu says how long ago the last save was. If a save fails, the game says why.

> In a terminal, quit with `QUIT GAME` from the pause menu. Closing the terminal window doesn't save first.

## Carrying on

- `CONTINUE` on the title opens your most recent run.
- `LOAD GAME` lists every run that's still going, newest first: the runner, their build and level, the city, the time and where the story stands. Pick one with `↑↓` and press `space` to carry on.

A run comes back exactly as you left it: where you stood, what you carried, the city's hacked doors and looped cameras, the story where it was.

## Where the saves are

The terminal game and the app keep their saves in the same folder:

- Mac: `~/Library/Application Support/Strata Zero`
- Windows: `%APPDATA%\Strata Zero`
- Linux: `~/.local/share/strata-zero`

Each run is a file in its `runs` folder.

## Dying

When your HP runs out, you flatline. The screen says how you died.

- **With perma death on**, that's the end. The run is over for good and can't be opened again. Its runner goes into the `HALL OF FAME`, and any key takes you back to the title.
- **With perma death off**, `space` takes you back to your last save, and `esc` to the title.

Perma death is set when you make a runner, and stays as it was set for the whole run.

## The hall of fame

`HALL OF FAME` on the title remembers every runner who died with perma death on: who they were, how far they got, how long they lasted, what they left behind, and what killed them.
