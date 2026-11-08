# Level up and the lock-on

Two animations played round a tile of the map, picked from the four looks in
`2026-11-08-level-up-mock/` (drawn through the real street screen; `gen.py` builds the
page, `levelup_mock.diff` holds the throwaway frames).

## Why

Act one now pays XP and ends on level 3, and the only sign of a level was a log line.
A level should be felt where you stand. The same mock produced a lock-on that suits
Deadeye, the mark that makes every round find its foe; today Deadeye flashes a
12-tick pulse on the foe and the foe keeps its `⌖`.

## What Jeff picked

- **Upload (mock B) plays when you gain a level.** A beam of data falls on you from the
  top of the view, `0`s and `1`s running down it; as it lands, a scanline runs out
  along your row and your cell flashes cyan; the beam drains from the top, and
  `▲ LEVEL n` types out beside you.
- **Lock-on (mock C) plays when Deadeye marks a foe.** Four brackets close in on the
  foe from about the edges of the view and lock round it, white as they bite, then
  magenta; a readout types out beside them.

## How it behaves

- Both are new effects beside the others in `game::Fx` (`Upload`, `Lock`), drawn by
  the effects overlay over the map, so they show in the terminal and the window alike
  and play on the Net as on the street.
- Timing follows the mock: 18 frames at 15 a second (two game ticks a frame, the app
  runs at 30), then the last frame holds for 8 more so the words can be read. About
  1.7 s in all.
- **Upload** plays on your tile. A level gained while the map isn't the top screen (in
  a scene, the pack, the deck, the console) waits, and plays the first frame the map
  is on top again, on wherever you stand then. Several levels at once play once, with
  the last level's number. The log lines stay as they are.
- **Lock** plays on the marked foe's tile. Its readout: `MARKED` in bold magenta, then
  the foe's name and the mark's bonus in amber (`goon · +20% to hit`). Deadeye's
  content swaps its `Mark` pulse for `Lock`; the foe's own `⌖` and pulse carry on
  after it, as now.
- Words that would run off the right of the view go on the left of the tile instead.

## Not in this

- No sound, no screen shake, no change to the window's effects.
- Other marks (the Net's slip marks, netscan) keep the `Mark` pulse.
- The mock's other looks (Shockwave, Circuit) are kept on the mock page for later.

## Calls made while writing this

- The 8-frame hold at the end, so a readout is on screen long enough to read.
- The readout flips to the left near the right edge.
- The pending level is UI: not saved (a save in the instant before the map is back
  loses an animation, nothing else).
