# The broadcast and perma death

Date: 2026-11-02. Designed with Jeffrey in chat; spec, plan and build run in one go
(his standing ask once the mockups are agreed). Mockups:
`2026-11-02-splash-perma-death-mock/` (round 1: NAME with perma death, splash concepts
A Marquee, B Cutaway, C Channel; round 2: B with the welcome as three broadcasts).

Builds on `2026-10-31-story-missions-design.md` (flavor files with text slots, the HOLO
call's typing), the character creation wizard (`screens/create/`), the title and LOAD
GAME (`screens/title.rs`, `screens/files.rs`), and
`2026-10-29-window-prototype-design.md` (what draws in both front ends).

## Why

Starting a game ends on a progress screen: the city grows in a viewport while a stage
checklist fills. It takes ~35 ms in a release build, so the screen is a formality, and
it is the player's first look at the city. Jeffrey wants that moment to be the game's
splash: STRATA ZERO, the city talking to you, and the run's last settings before you
drop in. He also wants two flags carried through the game from here on, unused for now:
**perma death** and **disable tutorial**.

## What Jeffrey decided

- **Perma death is picked at the end of character creation, on NAME**, and is **on by
  default**.
- **The generation screen goes entirely.**
- **The new screen**: STRATA ZERO and a welcome, a **DISABLE TUTORIAL** flag (off by
  default, carried through the game, not used yet) and a **warning banner with red
  borders at the bottom** about perma death: the last chance to turn it off.
- **Layout B, Cutaway**: the title's logo; the welcome on the left; the city cut open on
  the right, its strata named. With a caveat: **"make it super easy to hide the city
  graphics in case I don't like it."**
- **The welcome is something the city broadcasts**, raw and in-world ("you know in CP
  2077 you have the WEEEEELCOME NIGHT CITY… it reads real, raw"). Round 2's three
  voices: 1 STATIC 66 (pirate radio), 2 NITEWIRE (the news), 3 BLOCK 44 (your
  building's PA). **"I love that number 1 for our first game… the others are great too,
  so make sure that particular screen can be customized to accommodate all of them."**
- **The screen shows on NEW GAME and on LOAD GAME, not at launch.** It is the way into a
  game, "the part where we get to display things that make sense to the current events
  of the game".

## Calls I made (Jeffrey: check these)

- **A load keeps the runner, not the run.** A `.city` save holds the city only; the
  story and the clock are not saved, and loading keeps whatever runner is in the session
  (VEX on a fresh launch, your runner from the pause menu or after quitting to the
  title). The broadcast screen therefore offers both toggles on a load too, starting
  from the run's flags (`st.perma_death`, `st.tutorial_off`), which are the defaults on
  a fresh launch. LOAD GAME from the pause menu goes through the broadcast as well. When
  saves carry the run, a loaded game's perma death becomes a fixed status there, and its
  broadcast can read the run's events.
- **A flatline resets the flags** with the runner: FLATLINED already swaps in a fresh
  VEX, and now puts perma death back on and the tutorial back.
- **`esc` drops what was flipped on the broadcast screen**: going back to the sheet and
  jacking in again starts from the build's perma death.
- **Which broadcast plays**: each has a `when` (`NewGame` or `LoadGame`); among those
  that fit, the city's seed picks. STATIC 66 is the only `NewGame` one; NITEWIRE and
  BLOCK 44 are `LoadGame`.
- **Broadcasts live in `flavor/broadcasts.ron`**, compiled in like the other flavor
  files, with slots filled from the city (below). Place names in the mockups (Lantern
  Market, Rust Row, Block 44) become the city's own.
- **The art hides itself** when the terminal is under 120×36, where the broadcast needs
  the room.
- **The summary's `[enter] JACK IN` becomes `[enter] next`**: JACK IN now happens on
  the broadcast screen, one step later.
- **Space jacks in; while the broadcast is still typing, the first space shows it all.**
  Enter does nothing on the broadcast screen (space confirms in this game).
- **`esc` goes back**: to the sheet on a new game (the build intact), to the save list
  on a load. The built city is dropped.
- **The dev starts** (NEW GAME (DEBUG), the city setup's GENERATE, `r` in the free
  camera) build their city at once and go straight to the street, without the
  broadcast, as the generation screen used to end.
- **Neither flag is saved**: saves do not hold the run.

## The way in

- **NEW GAME**: chooser → wizard (perma death on NAME) → the sheet → `enter` → the city
  is built (`citygen::Generator::run` with `Title::fresh_params`) → the broadcast screen,
  pushed over the wizard → `space` → the street.
- **LOAD GAME** (from the title or the pause menu): the save list → `enter` loads the
  city → the broadcast screen, pushed over the list → `space` → the street, with the
  list's `loaded <path>` message.
- On `space` (once the broadcast has typed out): a new game makes the character from the
  build (`Character::from_build`, taking `st.build`) as JACK IN does today; both set
  `st.params` from the city, `st.enter_city(city)`, copy the screen's two toggles into
  the run's flags, and `Switch` to `screens::explore_screen` with the glitch.
- A city that fails to build stays on the sheet with the error as a warning, as now.

## The flags

- `game::Build` gets `perma_death: bool`, `true` in `Build::blank` (so templates and
  `from_character` start on too). Stepping back and forth in the wizard keeps it; a new
  build starts on.
- `AppState` gets the run's flags, `perma_death: bool` (`true`) and
  `tutorial_off: bool` (`false`), documented as carried through the game and not used
  yet. Only the broadcast screen's JACK IN writes them. The dev starts leave them as
  they are.
- The broadcast screen holds its own pair while it is up: from the build (and the run's
  tutorial flag) on a new game, from the run's flags on a load.

## NAME: the perma death field

Mock: round 1, section 1 (`gen.py`, `name_step`).

- The left pane gets a second field under the handle's rules: the label `perma death`
  (dim, like `handle`), a blank row, then `ON · OFF`. The pick is bold, ON in red
  `(255, 40, 40)` or OFF in cyan; the other is faint; the `·` is faint.
- Under it, wrapped to the pane: on, `One life. If {HANDLE} flatlines, the save goes too
  and the run is over.` in a light red `(255, 96, 112)` (`{HANDLE}` is the handle as
  typed, uppercased; `your runner` while it is blank); off, `A flatline sends you back
  to your last save.` dim.
- **Focus**: the step has two fields, the handle and perma death. The `▸` marks the one
  with focus; the handle's cursor `▮` shows only while it has focus. `↓` moves to perma
  death, `↑` back to the handle. On perma death, `←`, `→` and `space` flip it; letters
  and backspace do nothing there. On the handle, typing works as now. The step opens on
  the handle.
- **Footer**: on the handle `[type]name [↓]perma death [enter]next [esc]back`; on perma
  death `[←→]on/off [↑]handle [enter]next [esc]back`.
- **The sheet**: while perma death is on, `PERMA DEATH` in bold red sits right-aligned
  on the name's row inside THE SHEET. Off, nothing.
- Fits 100×30 and 120×40.

## The broadcast screen

Mock: round 2 (`gen2.py`, `splash`), frame 1 at 120×40 and the 100×30 frame.

From the top, on the solid panel background:

- **The logo**: the title's `Logo` (STRATA ZERO, magenta, flickering as on the title),
  centred, from row 2 (row 1 under 36 rows).
- **The subtitle** `h i g h   t e c h   ·   l o w   l i f e` in cyan under it, as the
  title draws it, when there are 36 rows or more; under that, it is left out.
- **The broadcast** (below), in a column 50 wide from x 4 with the art on, or centred,
  `min(76, width − 12)` wide, with the art off. With the art off the broadcast also sits
  in the middle of the rows it has.
- **The art**, when `CUTAWAY` is on and the terminal is 120×36 or more: the city cut
  open in half-block pixels, right of a 13-column gutter (the broadcast column, then the
  gutter with each stratum's name right-aligned before a `─` tick, then a `│` rule in
  the rule colour, then the art). It runs from the broadcast's first row down to the
  toggles' row, at most 56 columns wide; the whole composition (column, gutter, art) is
  centred on a wider terminal.
- **The toggles' row** (9 rows from the bottom): `[ ] DISABLE TUTORIAL` at the
  column's left (`[✓]` and the label bold when ticked) and `[space] JACK IN` at its
  right, JACK IN in bold magenta.
- **The banner** (rows h−7 to h−4, 4 columns in from each side):
  - perma death on: a double-line box (`╔═╗║╚╝`) in red `(255, 40, 40)` on a dark red
    `(28, 5, 8)`, titled ` ▲ PERMA DEATH IS ON ` in bold red; `One life. If {HANDLE}
    flatlines, the save is wiped and the run is over.` in the text colour; `This is your
    last chance to turn it off.` in bold light red, with `[p]turn it off` at the right.
  - off: a rounded box in the faintest grey, titled ` PERMA DEATH IS OFF ` dim; `A
    flatline sends you back to your last save.` dim; `Turn it on now, or play this run
    without it.` faint; `[p]turn it on` at the right.
- **The footer** (row h−2): `[space]JACK IN  [t]tutorial  [p]perma death  [esc]back` in
  the wizard's `[key]verb` style; at the right `{city} · {handle}` in the faintest grey.

Keys: `space` (show all, then jack in), `t` (tutorial), `p` (perma death), `esc` (back).

### The art, behind one switch

- The art is its own file (e.g. `ui/cutaway.rs`), drawn by one call. The screen has
  `const CUTAWAY: bool = true;` beside the layout; set to `false`, the broadcast takes
  the middle and nothing else changes. Deleting the art is that file and that call.
- It is drawn from the city's seed (each city's skyline differs a little) and does not
  move. Port `cutaway_art` from round 2's `gen.py`: pixel rows split into the Heights
  (from the top), the Skyways (30%), the Street (50%), the Undercity (70%) and the Net
  (86%); a sky running from near black to the Street's magenta haze; a back row of dim
  towers with masts and red beacons, a front row of lit ones with rim light, neon down
  their faces at street level; two skyway bridges with pods; the road's glow with car
  lights; the Undercity's pillars, pipes and amber lamps; the Net as a dim cyan lattice
  with lit nodes. The colours are in `gen.py`. Use `ui::pixels::Pixels`.
- The gutter names each band where it starts: HEIGHTS, SKYWAYS, STREET, UNDERCITY, NET.

### The broadcast, as drawn

- **The tag line**: the broadcast's tag spans, each in its colour (bold where coloured,
  the text colour otherwise), and the clock `HH:MM` right-aligned in faint grey.
- A rule of `─` across the column under it, then a blank row.
- **The shout** in bold white, wrapped; a blank row.
- **The paragraphs** in the text colour, wrapped, a blank row between them.
- **The sign-off** in the broadcast's colour, bold.
- It **types itself out** at the HOLO call's pace (`missions::call::TYPE_PER_SECOND`
  characters a second), the magenta cursor `▮` after the last character typed, as
  `ui::chrome::reveal` draws it. The first `space` before it is done shows it all.
- **When it does not fit**: blank rows go first (the one under the rule, then those
  between paragraphs, then the one over the sign-off); if it still does not fit, the
  last paragraphs are left out (the sign-off stays). Every shipped broadcast fits
  100×30 with its paragraphs, filled from a real city. (Built: real names run longer
  than the mockup's, so even 120×40 often loses the rule's blank row.)

## Broadcasts: `flavor/broadcasts.ron`

One file, a list of broadcasts. Each has:

- `id`: unique.
- `when`: `NewGame` or `LoadGame`. More conditions (story steps, the clock, the run's
  record) come when saves carry the run; the enum is where they go.
- `tag`: spans of `(text, colour)`, the colour an optional `(r, g, b)`.
- `shout`, `paras` (a list), `sign` and `sign_colour`.

Slots, filled from the city being entered and the runner:

| slot | fills with |
|------|------------|
| `{city}` | the city's name |
| `{city.loud}` | the name in capitals, its last two vowels that come before a consonant held four times: HOLLOWSIDE → HOLLOOOOWSIIIIDE |
| `{you}` | the handle (the build's on a new game, the character's on a load) |
| `{home}` | the name of the building with your flat (`citygen::home`), else a Megablocks district's name, else `the block` |
| `{corp}` | a corp tower's name in the city, else `Orodyne` |
| `{market}` `{zone}` `{blocks}` `{docks}` `{core}` `{strip}` `{industry}` `{oldtown}` | the name of a district of that theme (NightMarket, CombatZone, Megablocks, Docks, CorpCore, NeonStrip, Industrial, OldTown), else of any district, else `the Street` |

- A slot written in capitals fills in capitals: `{CITY}`, `{HOME}`, `{YOU}`.
- Picks (which district, which corp) come from the city's seed, so a city always reads
  the same.
- Loading checks every text's slots, that `id`s are unique, that no text is empty, and
  that there is at least one broadcast for each `when`, so the screen always has one.

The three to ship, from round 2 with the city's names:

1. **`static66`**, `NewGame`. Tag `●` and ` ON AIR  ` in red `(255, 40, 40)`, then
   `STATIC 66 · PIRATE`. Shout `{city.loud}! WAKE UP!`. Paragraphs: `Rise and grind, you
   beautiful wrecks. Static Sixty-Six, coming off a dish on the roof of {home} that we
   absolutely paid for.` / `Acid rain on the Heights, so the suits stay in and the
   Street's ours. The skyway over {industry}'s shut, somebody's car is part of it now.
   Noodles in {market}, still four. Cops in {zone}, still twenty.` / `Fresh meat in town
   tonight. Don't take work from anyone who's smiling.` Sign-off `Static Sixty-Six. Turn
   it up, lock your door.` in magenta.
2. **`nitewire`**, `LoadGame`. Tag `◆` and ` CH9 NITEWIRE  ` in cyan, `LIVE` in red.
   Shout `GOOD EVENING, {CITY}.`. Paragraphs: `Eleven dead in {zone} since sundown.
   Police are calling it a quiet night.` / `Smog advisory on the Street until further
   notice. The Heights report clear skies and record bookings.` / `After the break:
   who's buying up the Undercity one flooded block at a time, and why your landlord is
   smiling.` Sign-off `This hour of NITEWIRE is brought to you by {corp}. Breathe
   responsibly.` in a darker cyan `(0, 172, 191)`.
3. **`block_pa`**, `LoadGame`. Tag `▌` and `{HOME} ` in amber `(255, 200, 80)`, then
   `RESIDENT ANNOUNCEMENT`. Shout `ATTENTION, {HOME}.`. Paragraphs: `Water on floors 12
   to 30 is off until further notice. Do not drink from the cooling lines.` / `The lift
   is working. The lift is not safe. Residents use it at their own risk.` / `A body was
   found in the stairwell on 9. If it belongs to you, collect it from the lobby before
   06:00.` Sign-off `{city} Housing Authority. You live here now.` in amber.

Added with the morning news (`2026-11-02-morning-news-design.md`), Jeffrey's pick C of
three drawn options (a NEWSLINE late edition and a precinct scanner leak on NEW GAME
were the others):

4. **`courier`**, `LoadGame`. Static 66 the morning after the drop, the story's news as
   the pirates tell it. Tag as `static66`. Shout `{city.loud}, WHO'S THE COURIER?`.
   Paragraphs: `Somebody put a noodle cook on the pavement last night, and the badges
   want a word with whoever walked off with a package. Funny thing: the cook sold
   names. To the badges.` / `So, for the record. Static Sixty-Six never saw your face,
   your jacket or your package. Cameras see everybody. Cameras don't testify.` /
   `Courier, if you're listening: don't open it. Don't come by the dish.` Sign-off
   `Static Sixty-Six. We never saw a thing.` in magenta. A load starts the story over,
   so it plays as the city's memory of the drop, not the run's; one of three loads.

## What goes away

- `screens/generating.rs` and its tests. Its callers (NEW GAME's JACK IN, NEW GAME
  (DEBUG), the city setup's GENERATE, the free camera's `r`) build the city with
  `Generator::run` and, except NEW GAME, go straight to the street through one shared
  helper (set `st.params`, `st.enter_city`, `Switch` to `explore_screen`, glitch), or
  warn on an error.
- The free camera's and the city setup's `esc` paths that pointed at the generation
  screen keep working (they return to the title or the setup).

## Both front ends

Nothing here touches the window code. Everything draws through ratatui with exact
colours; the new symbols (`●`, `▲`, `✓`, `▮`, `◆`, `▌`, `╔`…) must pass
`every_drawn_character_has_a_glyph` (it scans `flavor/*.ron` too).

## Tests and shots

- Build: `perma_death` starts on in `blank` and templates.
- NAME: focus moves with `↓`/`↑`; `←`/`→`/`space` flip it only on its field; typing
  goes to the handle only on its field; the footer follows the focus; the sheet's tag
  shows only while on.
- Broadcasts: the shipped file loads; a bad slot, a duplicate id, an empty text and a
  missing `when` are refused; `{city.loud}` on a few names; capitals fill in capitals;
  the theme slots fall back; a city always picks the same broadcast.
- The screen: a new game's JACK IN on the sheet pushes the broadcast screen with the
  city built and the build kept; `esc` pops back with the build; `space` once shows it
  all, again enters the street with the character made and the flags copied; `t` and
  `p` flip and redraw (banner red/grey); LOAD GAME's `enter` pushes it with a
  `LoadGame` broadcast and `esc` returns to the list; enter does nothing.
- Layout: every shipped broadcast fits 100×30 and 120×40 for a few seeds, art on and off;
  the art shows at 120×40 and not at 100×30; with `CUTAWAY` off nothing overlaps.
- The dev starts reach the street without the generation screen.
- Shots (`screens/shots.rs`): NAME with perma death on and on its field turned off; the
  broadcast screen for each shipped broadcast at 120×40 and STATIC 66 at 100×30; perma
  death off with the tutorial ticked.
- The checks in CLAUDE.md, both front ends.

## Out of scope

- Using either flag (death, the tutorial).
- Saving the run, and with it a loaded game's fixed perma death and broadcasts about the
  run's events.
- A marker for a player's first game ever.
