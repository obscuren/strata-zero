# Keypads on flat doors: design

Date: 2026-10-17. Status: approved in chat by Jeffrey (moulded-buttons mockup,
a right code is learned, three wrong codes lock the pad and nothing more).
Builds on: `2026-10-17-credentials-journal-design.md` (door codes, the journal,
`credentials::opens`) and `2026-10-15-interiors-design.md` (locked flats).
Riff notes this came out of: `docs/ideas/2026-10-17-terminals-access-apartment.md`
(§4 Hardware: screens and keypads).

## Intent

The first piece of diegetic hardware. A locked flat's door has a keypad on it.
Stand beside the door, press Space, pick "interact", and a small beige device
appears over the map: an LCD with four digit slots, twelve raised keys, a
status LED. Key the flat's code and the door is yours from then on. The pad's
worn keys give away which digits the code uses, so a patient player can work a
door without the Net; three wrong tries lock the pad for half an hour.

The device is drawn from reusable parts (bezel, LCD, keycaps) so the
terminal's CRT frame in the Access Terminal slice is built from the same kit.

## Goals

- **Reaching it.** On a floor, with a locked `Flat` door among the player's
  four neighbours and the player outside that room, Space offers one row per
  such door: "interact · keypad, Mara Voss's flat 1204" (or "flat 1204" when
  nobody is named). Firing it replaces the popup with the keypad overlay.
  Bumping the door still says "locked · ...", unchanged. Your own door and
  hotel rooms have no keypad.
- **The overlay.** Drawn over the washed map, anchored just right of the
  door's tile and clamped to the map area; never full screen.
- **The device look**, the moulded mockup:

```
   ▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄
  ▐ SECURLOK           ● ▌
  ▐ ┌───────────────────┐ ▌
  ▐ │ [4][4][_][_]      │ ▌
  ▐ │ flat 1204         │ ▌
  ▐ └───────────────────┘ ▌
  ▐   ┌───┐ ┌───┐ ┌───┐   ▌
  ▐   │ 1 │ │ 2 │ │ 3 │   ▌
  ▐   └───┘ └───┘ └───┘   ▌
  ▐   ┌───┐ ┌───┐ ┌───┐   ▌
  ▐   │ 4 │ │ 5 │ │ 6 │   ▌
  ▐   └───┘ └───┘ └───┘   ▌
  ▐   ┌───┐ ┌───┐ ┌───┐   ▌
  ▐   │ 7 │ │ 8 │ │ 9 │   ▌
  ▐   └───┘ └───┘ └───┘   ▌
  ▐   ┌───┐ ┌───┐ ┌───┐   ▌
  ▐   │ X │ │ 0 │ │ V │   ▌
  ▐   └───┘ └───┘ └───┘   ▌
   ▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀
```

  25 columns by 19 rows. Bezel: a light beige top edge (`▄`), mid beige
  sides (`▐` `▌`), a dark beige bottom edge (`▀`), the inside filled with
  the mid beige; a badge "SECURLOK" top left; an LED top right, amber idle,
  green on OPEN, red on DENIED and while LOCKED. LCD: phosphor green text on
  near-black, four slots `[4][4][_][_]`, the flat's label beneath. Keycaps:
  box-drawn, label in dark text on the beige; the code's distinct digits are
  **worn**, drawn in a dimmer ink, always. X in dull red, V in dull green.
- **Keys.** `0`-`9` type into the next empty slot, four at most. `X` or
  backspace clears the slots. `V` or enter verifies. `Esc` or `q` steps away
  (the overlay pops). With empty slots and the flat's code already in the
  journal, the LCD shows that code dimly beneath the slots as "journal 4471"
  and `V` keys it in.
- **Verdicts.**
  - *Right:* the LCD reads `OPEN`, the LED goes green, the journal learns the
    code with `Source::Keypad` (journal line "keyed in at the door, day 1
    22:14"), the log says "`Mara Voss`'s flat 1204 · the keypad reads OPEN",
    and the overlay pops after a beat (`OPEN_TICKS` 24) or on any key. From
    then on bumping the door says "you key in 4471", as for a copied code.
  - *Wrong:* the LCD reads `DENIED` in red for a beat (`DENIED_TICKS` 18),
    the LED red, the log says "the keypad buzzes", the slots clear, and the
    flat's wrong count rises.
  - *Third wrong in a row:* the pad locks: LCD `LOCKED 30m` counting down in
    game minutes (`LOCKOUT_MINUTES` 30 from the third miss), LED red, digits
    and V refused with a short red flash, Esc still leaves. The Space row
    reads "interact · keypad, locked 27m" while locked and still opens the
    pad (to see the countdown). The count resets on a right code or when the
    lockout ends.
- **State** lives in the journal, session-only like everything else:
  `Journal::keypads: BTreeMap<(BuildingId, i32, u16), KeypadState { wrong: u8,
  locked_until: Option<u64> }>` in game minutes.
- **No other consequence** of a lockout: no heat, no security. Jeffrey's call.

## Non-goals

- The terminal frame and the Access Terminal.
- Keypads on anything but `Flat` doors (no corp doors, no hotel rooms).
- A card slot or two-factor doors.
- Brute-force protection beyond the lockout.
- Sound. "The keypad buzzes" is a log line.

## Architecture

- `ui/device.rs` (new): the kit. `bezel(buf, area, badge, led: Led)` draws the
  moulded frame and fills it; `lcd(buf, area, lines: &[(String, Style)])` draws
  the recessed screen; `keycap(buf, x, y, label, ink: Style)` draws one 5×3
  key. `Led { Amber, Green, Red }`. Colours as constants there: `BEIGE_LIGHT`,
  `BEIGE`, `BEIGE_DARK`, `INK`, `INK_WORN`, `PHOSPHOR`, `LCD_BG`.
- `screens/keypad.rs` (new): `Keypad` implements `Screen`. Holds the flat
  (`building, level, number`), its label, the anchor, the typed digits, and a
  `Verdict` with its tick (`Idle`, `Open(tick)`, `Denied(tick)`). Reads the
  lockout from the journal each frame. `Keypad::open(st, building, level,
  number, anchor) -> Keypad`.
- `credentials.rs`: `Source::Keypad`; `KeypadState`; `Journal::keypad(&self,
  b, level, number) -> KeypadState` (default when absent),
  `Journal::keypad_locked_until(&self, ..., now) -> Option<u64>` (None once
  passed); `try_code(st, b, level, number, typed: u16) -> Verdict` with
  `pub enum Verdict { Open, Denied { left: u8 }, Locked { until: u64 } }`: it
  checks the lockout, compares with `door_code`, learns on a hit (resetting
  the count), counts a miss and locks on the third, and writes the log lines.
- `screens/popup.rs`: `Pick::Keypad { building, level, number, anchor }`;
  firing it returns `ScreenCmd::Replace { to: Box::new(Keypad::open(..)), fx: None }`.
- `app.rs`: `ScreenCmd::Replace { to, fx }` pops the top screen and pushes
  `to` in one step (never pops the last screen: with one screen it pushes).
- `screens/explore.rs`: `interact` adds the keypad rows; the anchor is the
  door tile's screen position plus two columns, from `last_map` and the
  camera origin, or `None` (centred) when the door is off screen.

## Testing

- `device`: the bezel fills its area with beige and puts the badge and LED
  where the mockup has them; the LED cell's colour follows `Led`; a keycap
  is 5×3 with the label centred.
- `credentials`: `try_code` on the right code learns it with `Source::Keypad`
  and resets a prior count; a wrong code counts down `left` 2, 1, then locks
  with `until = now + 30`; while locked any code returns `Locked`; after the
  lockout passes the count is fresh; the journal line for a keypad code says
  "keyed in at the door".
- `keypad` screen: digits fill the slots and a fifth is ignored; X clears;
  backspace clears; V on the right code sets `Open` and the next key pops; V
  on a wrong code shows DENIED and clears; the third miss shows LOCKED with
  the countdown and refuses digits; Esc pops; V on empty slots with the code
  in the journal opens; the draw at 120×40 contains SECURLOK, the slots and
  the flat label and no stray backticks; worn keys are drawn in `INK_WORN`.
- `explore`: Space beside a locked flat door offers the row and beside your
  own door does not; firing it yields `ScreenCmd::Replace` with a keypad;
  after a right code the door opens on bump with "you key in".
- `app`: `Replace` leaves the stack the same height with the new top.
