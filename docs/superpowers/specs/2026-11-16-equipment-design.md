# Equipment: a body of parts

Status: designed with Jeff, 2026-11-16. The look is the mock beside this file,
`2026-11-16-equipment-mock/` (run `gen.py` as its docstring says), the version with the item's name
and what it gives in each tile. Follow-ups: #103 (chrome on this tab), the field scrambler worn on
the back (after this lands).

## Why

The seven slots (HEAD, EYES, TORSO, LEGS, PRIMARY, SECONDARY, DECK) were always a stand-in, and they
leave too little to customize. Jeff wants the slots to come from the body, the way Caves of Qud
does it: a set of parts, each holding what it can hold, drawn as tiles where the parts sit and joined
by lines. Done right, something that changes the body later (chrome that adds an arm) changes the
slots with no new code.

## Today

- `game::Slot` is an enum of the 7 slots; `Character.equipment: BTreeMap<Slot, ItemId>` is the truth
  and `Character.worn: BTreeMap<Slot, ItemCopy>` holds per-copy state (sockets, stickers). An item
  names the one slot it goes in (`Item.slot`). `Host::Worn(Slot)` addresses sockets and stickers on
  worn items.
- Only the PRIMARY weapon fires (`combat::your_weapon`). SECONDARY holds the Monowire shiv, which
  has no `weapon:` spec and only gives +1 REFLEX. Melee is Strike abilities from chrome and talents.
- The DECK slot is the deck (`Character::deck()`); the story reads TORSO (changing your jacket).
- Saves write the slot maps by variant name; all 3 fixtures hold Torso, Primary and Deck.
- GEAR & INVENTORY (`screens/kit.rs`) draws the slots as a 3-wide rack in the left half of the top,
  the pack in the right half, and the card with a half-size drawing across the bottom 13 rows.
- Chrome sits on 11 sites (`game::Site`) shown on ATTRIBUTES; nothing links sites to slots.

## Goals

- A body made of parts: HEAD, FACE, BACK, TORSO, L ARM, L HAND, R ARM, R HAND, WAIST, LEGS, FEET.
  Each part holds one item, or nothing.
- Weapons are held in hands. The right hand is the main hand; the other hand's weapon joins in.
- The body is data, so a later part (a third arm with its hand) is content, and the screen makes
  room for it.
- GEAR & INVENTORY draws the body as tiles joined by lines (the mock), each tile with the item's
  name and what it gives.
- Old saves open, with what they wore in the matching parts.

## Non-goals

- The chrome view and the key between meat and chrome (#103).
- Anything that adds a part (third arm): the model and the screen allow it, nothing adds one yet.
- The field scrambler as a worn piece on the back: its own change, right after this one.
- Melee weapons. The shiv stays a hand item that gives REFLEX; strikes stay abilities.
- Two-handed weapons.
- Choosing the main hand. It's the right hand.

## Anti-goals

- Pictures in the tiles. At this size they don't read (Jeff turned the mock with icons down); the
  picture lives in the card.
- A layout that only works at one size. Jeff plays bigger than 100×30, but 100×30 must still work.
- Special-casing slot names in code. Parts come from the body's data; code asks for a part by what
  it is (the main hand, the waist), not by a variant per slot.

## The body

- A part has a kind and a place:

  | Part | Kind | Holds |
  |---|---|---|
  | HEAD | head | hats, helmets |
  | FACE | face | shades, masks |
  | BACK | back | worn on the back |
  | TORSO | torso | jackets, vests |
  | L ARM, R ARM | arm | nothing yet |
  | L HAND, R HAND | hand | weapons, the shiv |
  | WAIST | waist | the deck |
  | LEGS | legs | trousers |
  | FEET | feet | boots |

- An item says which kind of part it fits (`slot:` in `flavor/content.ron` names a kind). A hand
  item fits any hand. Arms take nothing yet; they're on the body because the hands hang off them and
  because chrome sits in them (#103).
- The default body is defined once, in content, as the list of parts with their kind, side and the
  part each hangs off (the graph's lines). Every runner has it. Parts that chrome adds later come
  from the chrome; the body isn't saved, it's rebuilt from content and chrome (my call).
- What's worn is keyed by part (`equipment`, `worn`), as by slot today.
- For #103, the chrome sites belong to parts: NEURAL, EARS and JAW the head; EYES the face; SPINE
  and BACK the back; ORGANS and SKIN the torso; L ARM and R ARM the arms; LEGS the legs. This spec
  records it; #103 draws it.

### Old slots to parts

| Slot | Part |
|---|---|
| HEAD | HEAD |
| EYES | FACE |
| TORSO | TORSO |
| LEGS | LEGS |
| PRIMARY | R HAND |
| SECONDARY | L HAND |
| DECK | WAIST |

The same mapping reads old saves and rewrites `content.ron`'s `slot:` values.

## Hands and fighting

- The weapon you fire is the main hand's (R HAND). If the main hand holds no gun, the first other
  hand with one fires instead (my call), so a runner who only has a gun in the left hand can shoot.
- **The off hand joins in** (Jeff's pick): after you fire, each other hand holding a gun (with a
  `weapon:` spec) gets a 1 in 4 chance to fire too, at the same target, for no AP, if the target is
  in that gun's range. Its volley rolls with its own chance, damage and rounds. The log says it:
  `off hand · Scrap nine`. A talent that raises the 1 in 4 comes later, on the talent board.
- The ambush (#36) is the main hand's shot only: sure hit, ×2. A follow-up from the off hand rolls
  as normal (my call).
- The follow-up comes after a shot only, not after a strike, throw or ability.
- Today's kits put a gun in the right hand and the shiv in the left, so a solo's shiv keeps giving
  REFLEX and never fires.

## Content

- Every item's `slot:` takes the new kind: PRIMARY and SECONDARY weapons become hand, EYES face,
  DECK waist. A `weapon:` spec needs kind Weapon and slot hand.
- The rule that every slot appears on some item covers the kinds that take items (all but arm).
- New items for the new parts, small and plain, from the perks that exist (my call):
  - **Tread boots** (feet): +1 armour.
  - **Courier sling** (back): carry +3.
  They show up where the other clothes come from (kits, finds, the workbench as the builder sees
  fit). No drawings needed yet; they show MISSING BLUEPRINT like other undrawn items.
- Kits: worn as today, first fitting free part wins, hands in order main then off.
- Wearing a hand item from the pack (a pack row's Wear, or a popup on the map) goes to the first
  empty hand, main first; with every hand full it replaces the main hand's item, which goes to the
  pack with the usual "swapped" line (my call). To put it in a particular hand, use that hand's tile
  (`space`, then the item). So `Equip` learns an optional target part.
- The back holds one item, so once the field scrambler moves there it competes with the Courier
  sling: carry or cloak. That's the point of a slot (my call); the sling can move if it plays badly.

## Saving

- `equipment` and `worn` keep their place on `Character`, keyed by part. The save writes the part's
  name.
- Old saves name slots (`Primary`, `Deck`, `Torso`, ...). They're read through the mapping above,
  in the style of `net::EchoRead` (`docs/saving.md`), so the 3 fixtures open with the Ratchet in the
  right hand, the deck on the waist and the jacket on the torso. Never regenerate a fixture.
- `missions` keeps reading the torso (now the TORSO part) for changing your jacket.
- `docs/saving.md` gets a line for the new keys.

## The screen

The look is the mock. GEAR & INVENTORY becomes two halves:

- **Left, full height: the body.**
  - A header row: `▌MEAT▐` on the left (#103 adds `CHROME` and its key beside it), a rule under it.
  - The parts as tiles, 11 wide and 5 tall (TORSO 12 by 7), placed where they sit on a body and
    joined by box-drawing lines: HEAD with FACE beside it, a line down to TORSO; L HAND, L ARM,
    TORSO, R ARM, R HAND in a row; under TORSO a line down to WAIST, branching left to BACK; WAIST
    down to LEGS, FEET beside it. Lines meet tile borders with `┬ ┴ ├ ┤`, clear of the part's name
    in the top border.
  - The positions are a table keyed by part, with free places for parts added later: a third arm
    and its hand go under R ARM and R HAND, on a branch off the line under TORSO (the mock shows it).
  - In a tile: the part's name in the top border; the item's name on two lines (wrapped at a space,
    a word too long for the line cut with `…`); under it, dim, what it gives:
    - a gun: damage × rounds (`4×3`) and a pip per socket (`■` fitted, `□` empty);
    - a hand also starts its line with `●` for the main hand, `○` for an off hand;
    - the deck: its program slots and sockets, as its tile shows today (`■■■■■ □□`);
    - clothes and the rest: the first perk, short (`+5 HP`, `+4 kg`, `COOL +1`, `+1 armour`).
  - An empty part shows a dim `·`. Arms always do, for now.
  - The cursor's tile is lit: cyan border, the name bold on the selection colour, as the rack's
    selected tile is today.
- **Right: the pack, then the card.**
  - The pack on top with its filter row, as today, about 10 rows.
  - The weight and value rule (`═ 68.1 kg · ¤ 108359 ═`).
  - The card: the item's name, `kind · part · kg · ¤`, its mods, for a gun its damage and range and
    the off hand's chance, then the half-size drawing under the text when 8 rows are left for it.
    Otherwise the drawing waits for Inspect (`space inspect`).
- **Keys.** The arrows move the cursor from tile to tile along the body (each tile's neighbour in
  each direction is part of the layout table). Right from the right-most tile in a row crosses to the
  pack; left from the pack comes back. `space` on a tile opens the slot popup as today: wear what fits
  that part, take off, inspect. The hint reads `←↑↓→ body, then over to the pack · [ ] filter · space
  act · Q/E tabs · tab close`.
- **Sizes.** The body takes the width it needs, not half (my call): the full body needs 61 columns
  and 27 rows plus a column of margin, and the pack and card take the rest, never under 50 columns.
  So the full body shows when the tab's body is at least 113 columns wide and 27 rows tall; 120×40
  has 118 by 27 (the mock, separator at column 63). Below that the body goes compact (my call, not
  drawn yet) in the left half: no arm tiles (the hands join the torso), tiles 4 tall with the name
  on one line, TORSO 6 tall, no rule under the header; 100×30 gives it 48 by 22. The card's drawing
  waits for Inspect there. Bigger windows give the extra width to the pack and card, and centre the
  body vertically in its rows.

## The manual

- 05 the deck: the GEAR & INVENTORY section (the parts, the tiles, the keys) replaces the slot list
  and "Only your PRIMARY weapon fires".
- 07 turns and fights: "Your gun" becomes the main hand, with the off hand joining in.
- 08 the Net and 11 fixers, jobs and the story: "your DECK slot" becomes the deck on your waist.
- 02 your runner: the solo's kit line, if it names slots.

## Shots and tests

- `gear_and_inventory` shots at 100×30 and 120×40 (the body, the cursor on the right hand), and a
  shot with a part added by hand in a test, to show the free places hold a third arm.
- The fixtures open with the mapping. The off hand's follow-up rolls 1 in 4, skips a target out of
  its range, gives no AP cost and no ambush bonus.

## Left to the builder

- The names of the types (`Part`, `PartKind`, the body list) and where the body is defined in
  content.
- How a part is named in saves, as long as old slot names read through the mapping.
- The exact compact layout at 100×30 within the rules above, and the cursor's neighbour table.
- Where the boots and the sling are found.

## Calls made

1. The body isn't saved; it's rebuilt from content and installed chrome.
2. With no gun in the main hand, the first other hand with a gun fires.
3. The off hand's follow-up gets no ambush bonus and only follows a shot.
4. Arms take no items yet.
5. Two new items: Tread boots (+1 armour) and a Courier sling (carry +3).
6. The full body takes the width it needs (61 columns + margin) when the pack and card keep at
   least 50; below 113 columns the body goes compact in the left half: no arm tiles, one-line
   names, the drawing on Inspect.
7. The chrome sites belong to parts as listed, for #103.
8. A hand item worn from the pack goes to the first empty hand, main first, else replaces the main
   hand's; a hand's tile picks a particular hand.
9. The Courier sling and, later, the field scrambler share the back: one or the other.
