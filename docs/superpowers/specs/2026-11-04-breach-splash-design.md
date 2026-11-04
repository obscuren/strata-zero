# The breach splash

Date: 2026-11-04. Designed with Jeffrey in chat; spec, plan and build run in one go (his
standing ask once the mockups are agreed). Mockups: `2026-11-04-breach-splash-mock/`
(`round1.html`; the animation is `splash.js`, concept **D · Key**).

Builds on the title (`screens/title.rs`, `ui/logo.rs`), the frame loop
(`app::Runner::frame`) and `2026-10-29-window-prototype-design.md` (what draws in both
front ends).

## Why

Jeffrey loves how Cyberpunk 2077 opens with a "breach" and wants the game to start with
something like it: a splash before the title. Four animated concepts were drawn
(A Hijack, B ICE wall, C Drill, D Key). He picked **D, Key**, alone, for the splash.

## What Jeffrey decided

- **The splash is D, Key**: a memory dump races past a read head and slows; the scanner
  fills in a key a byte at a time; it locks on `53 54 52 41 54 41 20 5A 45 52 4F`,
  which is STRATA ZERO in hex; the dump dissolves; each byte flies up and becomes its
  letter of the logo.
- It plays **at launch, before the title**, and ends with STRATA ZERO lit **exactly where
  the title draws it**, so the title's subtitle and menu type in under it with no cut.
- **Any key skips it.**
- **Save all four animations.** They are committed in the mock folder. A (Hijack) is kept
  for hacking bodies and non-public terminals later; C (Drill) and B (ICE wall) are kept
  as they are. None of them is built now.

## What it looks like

Everything below is in ticks from the splash opening, at the game's 30 a second, and
follows `splash.js` (`KEYDUMP`), which is the reference for anything this leaves out.

**Layout**, on a terminal of `w × h` (100 × 30 and up):

- The dump is 79 columns wide, centred: `x0 = (w − 79) / 2`. A row is an address
  (`0x00A1F3C0`, cyan at 55 %), two spaces, 16 bytes in two groups of eight (byte `j` at
  `x0 + 12 + 3j`, one more column from the ninth on), then `│`, 16 ASCII characters (`.`
  for anything unprintable) and `│`.
- Header on row 0: `ICEPICK 2.1  ·  memory read  ·  jack port 7`, faint.
- Dump rows from row 2 to row `h − 4`. The read head is row `key_y = h / 2 + 4`.
- The scanner's status line on row `h − 2`.
- The logo's spot is the title's: `x = (w − 66) / 2`, `y = h / 6`.

**Timeline**

| ticks | what happens |
|------:|--------------|
| 0–70 | The dump scrolls past the read head, fast at first and slowing to a stop: the top row is `end − 560·(1 − t/70)³`. Bytes dim as the speed rises; about one in forty flashes magenta (a near miss). The read head is a dark cyan band with `▶` at its left. The status line reads `SCAN <address>  <16-cell bar> <n>%   KEY` and then the 11 key bytes, filled in from tick 6 to tick 66 a byte at a time (magenta), the rest `??`. |
| 70 | **KEY FOUND.** The key row stops on the read head: two ticks of a bright flash, then a dark magenta band; its 11 key bytes magenta and bold, `STRATA ZERO` in its ASCII column white and bold, `◀ KEY` after the row. The status line becomes `KEY FOUND  <address>+03   53 54 52 41 54 41 20 5A 45 52 4F`. |
| 84–104 | Everything but the key's bytes and its ASCII letters dissolves: each cell at its own tick, showing a glitch character (`effects::GLITCH_CHARS`, dim cyan) for its last two ticks. |
| 92–146 | **The bytes fly.** Every cell of the logo comes out of its letter's byte (the space has none). Each starts at `92 + 2.2·letter + 0–8`, flies 14–24 ticks on an ease-out with an upward arc of 2–7 rows, drawn as one of its byte's hex digits (magenta, a little white, bold) with a dim `·` one tick behind. It lands white for two ticks, then takes the title logo's own colour. |
| 124–138 | The key's bytes and letters dissolve the same way. |
| 150 | **Hand over** to the title (see below). The whole splash is five seconds. |

**Colours** are the game's: cyan `(0,229,255)`, magenta `(255,0,200)`, the title's
greys. Cells the splash leaves empty draw nothing, as the title's do, so the
background is the same before and after the hand-over.

## How it behaves

- `screens::initial` opens the splash; the splash switches to `Title::new(st, st.tick)`.
- **The hand-over** at tick 150 is a `Switch` with no transition. The logo the splash has
  drawn is cell for cell the one the title draws on that tick (same characters, same
  flicker and scanline colour), so only the title's own text appears (subtitle, menu,
  key hints).
- **Any key** switches to the title at once, through the glitch transition. The key does
  nothing else: it does not reach the title.
- **The title opened from the splash ignores keys for its first 10 ticks** (a third of a
  second), so a player mashing Esc or holding Enter to skip doesn't quit the game or
  start one by accident.
- Under 100 × 30 the frame loop already draws its resize notice instead of any screen;
  the splash runs on behind it and hands over as usual.
- The dump's bytes come from a seed taken at launch (the clock), so the dump differs
  launch to launch; the key row, its offset in the row and everything else are fixed.
  Tests use a fixed seed.

## Calls made

- **No Settings row.** Settings are not saved between launches and can't be reached from
  the title, so an on/off switch would do nothing; any key already skips. An off switch
  can come with saved settings.
- **Skipping uses the glitch transition**; the natural end uses none (it has to be
  seamless).
- **The hand-over at tick 150**, after the last byte has landed (146 at the latest). In
  the mock the title's text started typing at 134 while bytes were still flying; in the
  game the title starts when the splash ends.
- **The logo's colour is shared**, not copied: `ui/logo.rs` exposes the colour of a
  letter's row at a tick, used by `Logo` and by the splash; `screens/title.rs` exposes
  where the title puts its logo, used by `Title` and by the splash.
- **The window's live shot** (`STRATA_WINDOW_SHOT`, 1.5 s in) now catches the splash
  mid-scan. That still proves the app draws; its comment says so. The window's
  `window_shots` test draws the title itself, not `initial`.
- **The key guard** above (10 ticks), on both ways into the title from the splash: a key
  that arrives just as the splash ends naturally was most likely meant to skip it.
- **CI's app jobs** draw `shots::splash` with their other shots, so the Windows and Linux
  screenshots include it.
- Only characters the game already draws (`▶`, `◀`, `·`, `│`, `█`, `░`, the glitch set,
  ASCII), so `every_drawn_character_has_a_glyph` needs nothing new.

## Tests

- The splash is what `initial` opens; with no keys it hands over to the title by tick
  150 and not before 147.
- Any key hands over at once.
- At tick 75 the read head shows the key bytes and `STRATA ZERO`, and the status line
  says `KEY FOUND`.
- The hand-over is seamless: on the last tick the splash draws, every logo cell matches
  the title's logo on that tick (character and colour), at 120 × 40 and at 100 × 30.
- The splash draws without panicking at 100 × 30, 120 × 40 and 300 × 80, at every tick
  from 0 to 150.
- Shots (`shots::splash`): the scan, the key found, the dissolve and the flight, at
  120 × 40 and 100 × 30.

## Not in this

- A, B and C as screens (kept as mocks).
- Sound, a Settings row, saved settings.
