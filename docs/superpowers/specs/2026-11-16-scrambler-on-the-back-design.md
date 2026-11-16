# The field scrambler on the back, and WREN

Status: agreed with Jeff, 2026-11-16 (short design, built straight after). Follows the stealth
cloak (#36, `2026-11-15-stealth-design.md`) and the body of parts
(`2026-11-16-equipment-design.md`).

## Why

The field scrambler was a pack item: carrying one granted Scramble, so it cost nothing to have.
Jeff wants it equipped, on the back. And stealth, now a whole way to play, gets a runner of its own
to start as.

## The scrambler on the back

- `field_scrambler` takes `slot: Back`. It grants Scramble only while worn, like any worn item.
  A scrambler in the pack grants nothing.
- The cell is the worn copy's: Scramble runs on it, it drains while the cloak runs, and only the
  worn scrambler recharges on the world clock; a night's sleep fills it. Spares in the pack keep
  the charge they had.
- Taking it off (or swapping it for something else on the back) while it runs switches the cloak
  off, as putting it down does today.
- The back holds one item: the Courier sling (carry +3) and the scrambler are one or the other.
- The footer's charge (`◌ field scrambler  62%`) reads the worn scrambler; with none worn, nothing
  shows. The BACK tile's gives line is its charge, `◌ 62%` (`· burned` when burned out).
- Content rule: an item with a `cell` needs a `slot` (its grant counts while worn, as every other
  grant). The "a cell's grant counts while carried" exception goes.
- Old saves: a scrambler in the pack stays there with its charge; the player puts it on. A save
  that was cloaked on a pack scrambler opens with the cloak off.
- Manual 06 (the field scrambler) and 07 (the cloak) say it's worn on the back.

## WREN, the third runner

- Template `wren`, handle **WREN**, line "every camera on the block kept a smear, never a face".
- Origin SLAB RAT, profession FIXER; BODY 5, REFLEX 7, TECH 4, COOL 8, NET 3.
- Categories GHOST, IRON, STREET; skills Stealth, Lockpick, Handguns, Streetwise.
- Kit on top of the fixer's: a field scrambler and a Whisper SMG. Worn: the scrambler on the back,
  the Whisper in the left hand beside the fixer's Ratchet (so the off hand joins in); the fixer's
  Courier sling goes to the pack.
- For that, a runner's own kit (the build's) is worn before the profession's (my call). VEX and
  KESSLER wear the same as before.
- Manual 02 lists WREN with VEX and KESSLER.

## Calls made

1. Only the worn scrambler recharges; spares keep their charge.
2. A cloak saved on a pack scrambler opens off.
3. The build's kit is worn before the profession's.
4. WREN's name, line, numbers and kit as above.
