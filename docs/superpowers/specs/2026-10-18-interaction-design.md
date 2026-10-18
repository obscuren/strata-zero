# Interaction: the Space window, its verbs, and look: design

Date: 2026-10-18. Status: built on `feat/interaction` (plan
`docs/superpowers/plans/2026-10-18-interaction.md`). Rulings made while building:
items and keypads share amber in rows; the program window's header is always
blank; the Floors panel and the lift window share the 16 free letters (tall
towers leave the lowest floors to the arrows); jacking out drops an armed run
("run aborted"); names may carry digits in look text. Added mid-build at
Jeffrey's request: entering a building, a floor or home logs "Entering
`name`" / "Entering `11F`" / "Entering `home` · flat N", and the map's toast
lights marked names as the LOG pane does. Brainstormed with Jeffrey; the
mockups below are the ones he approved ("Yep, that's it", then "Great" for
look, with "the window should also be slightly larger"). Calls Claude made
without asking are marked **(Claude's call)**.
Builds on: `2026-10-17-keypads-design.md` (the keypad row),
`2026-10-17-access-terminal-design.md` (the terminal row),
`2026-10-17-credentials-journal-design.md` (flat labels, who carries a card).
Riff notes this came out of: `docs/ideas/2026-10-17-look-cursor.md` (look
mode; its "What already exists" section is still accurate) and, for later,
`docs/ideas/2026-10-17-lit-sight.md`.

## Intent

Space while walking opens a small window of things to do. It works, but it
reads like a dump: the title is always HERE (RUN in a construct), every
thing within reach lands in one list, the rows are worded differently each
time ("access rig", "jack in", "interact · keypad, flat 1204", "talk to Mara
Voss"), and picking "look" or "talk to" opens a card that closes back into
the list instead of the game.

The redesign, modelled on Caves of Qud's interaction menus:

- The window is about **one thing at a time**, and its header names that
  thing. Near nothing, there is no header.
- Verbs are **one fixed word with one fixed letter**, the same everywhere.
  When a verb needs an object, it is written `Verb: Thing` with the thing in
  its kind's colour.
- With several things in reach, Space first asks **which way**, and a
  direction key opens that thing's window.
- **Look** becomes a real mode: a free cursor on the map and an info window
  that describes whatever is under it, never covering the cursor, worded in
  the game's voice.
- **Cancelling always returns to the game.** No Esc ever lands the player in
  an in-between menu.
- **Space acts, Enter confirms.** On the map, Space is how the player
  starts something. Enter only confirms a choice already on screen.

Jeffrey asked for it in three passes, built in this order: how the window
looks, then the verbs, then look and interact.

## Goals

- One window style for every verb popup: the Space window, VIEW, ABILITIES,
  FLOORS, the construct's program list, and the deck screen's item, slot,
  category and skill popups.
- The header tells you what the window is about.
- Every row in the Space window follows one wording pattern with fixed
  letters.
- A look mode that describes anything in sight, from people to stairs to a
  lamp across the street, and only what the player's eyes could tell them.
- Esc, from any window, card or mode in this design, returns to the game.
- Enter never starts an action from the map; Space does.

## Non-goals

- A trade system. Trade gets its verb and letter now; the row appears once
  trading exists.
- Mouse support (hover to look, click to act).
- Lit sight, light occlusion, day and night (`docs/ideas/2026-10-17-lit-sight.md`).
  Look reads the visibility grid as it is today and picks up lit sight for
  free when that lands.
- What NPCs can see of the player (stealth).
- Rebindable keys.
- Changing the keypad and terminal devices themselves; they still replace
  the window when opened.
- The contents of VIEW, ABILITIES, FLOORS and the deck popups beyond the new
  frame, header and capitalised verbs.
- The dev inspector (`inspector_lines`, the dev Inspect sidebar, dev Explore
  mode on `c`). It stays as it is.

## Anti-goals

These would make it a failure even if it technically works:

- The look window covering the tile under the cursor.
- Look telling the player more than their eyes do: people or items on a
  remembered tile, anything at all on an unseen one.
- Numbers or stats in look text (wealth, decay, HP, footprint). Counts (`×2`)
  and floor names are the only numbers.
- An Esc that lands in a list instead of the game.
- The same verb worded or lettered two ways in two places.

## Pass 1: how the window looks

The existing `Popup` (`crates/strata-zero/src/screens/popup.rs`) keeps its
frame, solid panel, reveal animation, selection bar and pulse. What changes:

- **Width.** The Space window is 40 columns wide, growing to the existing
  `MAX_W` 60 for long rows and clamping to the screen as today. (`MIN_W` is
  24 today.) **(Claude's call)** The 40-column minimum applies to every
  `Popup`, not only the Space window.
- **Cards anywhere close the whole popup.** **(Claude's call)** A card shown
  from a popup row (the deck screen's inspect cards, Talk) closes straight
  back to whatever was under the popup: the game, or the deck screen. It
  never returns to the rows.
- **Header.** The top border carries ` <glyph> <Name> `: the glyph the map
  draws for that thing, and the name, both in the thing's map colour (a
  person's profession tint, a terminal's colour). It names the window's
  **single subject**. When the window has no subject, the top border is
  blank. **(Claude's call)** The same rule covers a window whose rows aim at
  several targets, such as the construct's program list: no header. Menus
  that are not about a thing keep a title naming the list: View, Abilities,
  Floors. Headers are written as names, not upper-cased, so the deck popups
  read "Stim patch" rather than "STIM PATCH".
- **Subtitle.** An optional dim line under the header, before a blank row.
  It is used for people: "door muscle · on the door", from the person card's
  profession and doing.
- **Rows.** `▸ k  Verb` or `▸ k  Verb: Thing`, verbs capitalised. The key
  stays magenta, the selected row keeps its bar. The thing after the colon is
  coloured by kind: **(Claude's call)** cyan for terminals, amber for keypads,
  the item's own colour for items, and the plain text colour with bold for
  places such as floors and strata. The builder may adjust the palette as
  long as every kind keeps one colour everywhere.

The approved mockups, with colours described in words:

```
Next to nothing: no header, look is the only row.
┍━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┑
│                                       │
│  ▸ x  Look                            │
│                                       │
┕━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┙

Next to a person: name in their tint, subtitle dim.
┍━ ☺ Mara Chen ━━━━━━━━━━━━━━━━━━━━━━━━┑
│  door muscle · on the door            │
│                                       │
│  ▸ x  Look                            │
│    t  Talk                            │
┕━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┙

A street access terminal: "Terminal" in cyan.
┍━ ▣ Access terminal ━━━━━━━━━━━━━━━━━━┑
│                                       │
│  ▸ x  Look                            │
│    i  Interact: Terminal              │
│    j  Jack in                         │
┕━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┙

The rig at home: header is the machine, the coloured word its kind.
┍━ ▣ Access rig ━━━━━━━━━━━━━━━━━━━━━━━┑
│                                       │
│  ▸ x  Look                            │
│    i  Interact: Terminal              │
│    j  Jack in                         │
┕━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┙

A locked flat door: "Keypad" in amber.
┍━ + Mara Voss's flat 1204 ━━━━━━━━━━━━┑
│                                       │
│  ▸ x  Look                            │
│    i  Interact: Keypad                │
┕━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┙

Standing on the stairs: floors bold.
┍━ ≡ Stairwell ━━━━━━━━━━━━━━━━━━━━━━━━┑
│                                       │
│  ▸ x  Look                            │
│    >  Go up: Level 3                  │
│    <  Go down: Level 1                │
┕━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┙
```

The glyphs above are placeholders; the header uses whatever the tileset
draws. The approved mockups showed `l` for Look; it is `x` here for the
reason given under Pass 3.

## Pass 2: the verbs

### The verb table

| Verb | Key | Shown when | Does |
|---|---|---|---|
| Look | `x` | always, first row | enters look mode, cursor on the subject (on the player when there is no subject) |
| Talk | `t` | the subject is a person | shows the talk card in place of the window |
| Trade | `r` | the subject trades; nobody does yet | reserved, never shown in this work |
| Interact: Terminal | `i` | a district access terminal or the home rig in reach | opens the terminal screen, as "access terminal" / "access rig" do today |
| Interact: Keypad | `i` | the subject is a locked flat door beside you | opens the keypad, as "interact · keypad" does today |
| Jack in / Jack out | `j` | as today | jack in as today; jack out through the confirm (see Jacking out) |
| Take: Item ×n | `g` | items on the subject tile, one stack | takes the stack |
| Take all, then Take: Item ×n per stack | `g`, then free letters | items, several stacks | as today's take rows and `t` take all |
| Go up: Place / Go down: Place | `>` / `<` | a connector, or stairs inside | as today's "go up to …" and "up to Level 3" rows |
| Ride: Floor | free letters | standing on a lift | as today's lift rows; the current floor stays disabled |

Rules:

- **Order** is the table's order. Look is always first and starts selected.
- **Free letters** for take and ride rows come from a pool that excludes
  every verb letter above and the window's own keys (`h j k l q`).
  `WORLD_LETTERS` in `screens/explore.rs` becomes that pool.
- The keypad row's lock state stays visible as today: "Interact: Keypad ·
  locked 4m".
- A disabled row stays visible and greyed, as `Choice::disabled` does today.

### Talk

Talk replaces the window with the talk card in the same frame, with the
person's name as the header: profession · doing (dim), their line in quotes
(cyan italic), and their relation line (dim) when they have one. This is
today's `card_lines` content. **Any close key on the card returns to the
game**, not to the rows. **(Claude's call)** The line also lands in the
message log as `Mara Chen: "Broth's been going since Tuesday."`, so it can
be read again.

### What counts as a subject

On the city strata and inside buildings:

- **Your own tile ("here"):** item stacks, a connector (stair, ladder,
  manhole, lift between strata), indoor stairs, a lift bank. Together they
  form one subject. Its header is the most important thing on the tile
  (connector or stairs or lift first, then the top item stack); its rows are
  all of the tile's verbs.
- **A neighbour tile:** a person (4-neighbours, as today), a locked flat
  door with its keypad (4-neighbours, as today), an access terminal or the
  home rig (8-neighbours, as `Player::terminal_in_reach` reaches today).

**Jack out** is offered on the "here" subject while jacked in.

### Which way

Space then does one of three things:

- **No subject:** the headerless window with only Look.
- **One subject:** its window opens at once.
- **Several subjects:** a direction pick. Each candidate tile gets a dimmer
  version of the look cursor's background block, and the status strip reads
  `which way?  ↑ Mara Chen   → Access terminal   · here: 2 items`. A
  direction key (arrows, `h j k l`, and `y u b n` for diagonals) **opens that
  direction's window at once**. Space or `.` opens "here". A direction with
  no subject does nothing. Esc returns to the game.

From a window opened this way, Esc returns to the game, not to the pick.
**(Claude's call)** The clock keeps running during the pick, as it does in
look mode: both are layers on walking, where the world runs. (A window on
top freezes the world, as every popup does today.)

### Space acts, Enter confirms

Jeffrey: Enter should no longer interact with things. It can confirm, but
starting an action is what Space is for.

- **Removed from walking:** Enter as the jack shortcut. Today Enter jacks in
  at a terminal in reach, warns with the refusal when it can't, and jacks out
  when jacked in (`handle_walk_inner` in `screens/explore.rs`). From now on,
  jacking in goes through the Space window's `j` row, and jacking out has its
  own key, `J` (see Jacking out below).
- **Kept, because they confirm a choice already on screen:**
  - firing the selected row in any window;
  - dismissing a card;
  - confirming a program's target while targeting (`handle_targeting`, which
    takes Space too);
  - **(Claude's call)** walking the construct aim set with `g`, which Space
    already does as well;
  - every menu and form screen (title, configure, pause, files, deck,
    keypad, terminal), where Enter is the confirm key.
- **Look and "which way":** Space opens the window. Enter does nothing
  there.
- The help screen's walk list (`screens/help.rs`, the "enter · jack in at an
  adjacent terminal / jack out" row) and the README's controls section (the
  "press `enter`" jack lines and the "`enter` jack shortcut") are updated to
  match, with `J` listed as "jack out (asks first)".

### Jacking out

Jacking out is the escape from ICE (`2026-10-15-ice-that-hunts-design.md`,
`2026-10-17-engagement-design.md`). In a construct, Space is taken first by
staying put and by rolling an armed run, so the Space window alone could
trap the player mid-fight. Jeffrey chose a key of its own, with a confirm.

- **`J` while jacked in** (the Net or a construct) opens the confirm. It
  works whenever Enter's jack out works today: on the player's turn, or at
  any time outside a fight. On the ICE's turn it warns "not your turn", as
  every key but the free ones does. It never touches an armed run, the aim
  or the move half; cancelling leaves all of them as they were.
- **The confirm** is a `Popup` in the new style:

```
┍━ Jack out? ━━━━━━━━━━━━━━━━━━━━━━━━━━┑
│                                       │
│  ▸ y  Jack out                        │
│    n  Stay in                         │
┕━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┙
```

  **(Claude's call)** A confirm carries its question as the header, the one
  exception to "the header is a name". Jack out starts selected, so `J`
  then Enter or Space leaves at once. `y` or `J` again confirms. `n` or Esc
  closes it and the player is back in the game. Confirming does what Enter
  does today: `net::jack_out`, "jacked out", the glitch transition.
- **(Claude's call)** The Space window's Jack out row on the "here" subject
  goes through the same confirm, so leaving the Net always asks.
- The confirm is where the jack-out penalty will be shown once it is
  designed (see Follow-ups).

### In a construct

The construct's program list stays one window and has no direction pick,
because its targets include yourself and pieces of ICE at range
(`net::targets_near`). Its rows take the pattern with the program as the
verb: `Crack: Lock · 64% · 3 turns · noise 4`, where today's row reads
`crack · lock · 64% · …`. It has no header unless every row aims at one
target. The RUN title goes away. Look is its first row, as everywhere.

## Pass 3: look

### The mode

- **Keys in.** `x` while walking, or Look in any window. **(Claude's call)**
  The approved sketches said `l`, but `l` is east among the walk keys
  (`dir_for`), so look takes `x` (examine, as in Crawl). The Look row uses
  `x` too, so one key means look everywhere.
- **A layer on Walk**, as `docs/ideas/2026-10-17-look-cursor.md` §1
  suggests (a `looking` field in the style of `targeting`, not a third
  `Mode`). The clock keeps running; no key moves the player. Look costs
  nothing, so in a construct it is allowed on any turn, including the ICE's.
- **The cursor** starts on the player, or on the window's subject when
  entered from a window. The tile under it gets a coloured background block
  with its glyph kept readable, pulsing gently with the existing `pulse`.
  **(Claude's call)** The colour is the builder's pick; it must read on every
  stratum's palette and differ from the dimmer "which way" block.
- **Moving.** The walk keys move it, including diagonals; Shift moves it 8
  tiles, as dev Explore does. **(Claude's call)** It is clamped to the map
  viewport and the camera stays put, so the player keeps their bearings.
- **Tab** jumps to the next thing in view, nearest first: people,
  terminals, doors, connectors and stairs, item stacks, lamps, neon, signs
  and holo ads. **Shift+Tab** goes back. Only `Visible` tiles count.
- **Space** on a thing that would be a Space-window subject from where the
  player stands opens that subject's window. Elsewhere it does nothing.
- **Esc or `x`** puts the cursor away; the player is back in the game.
- While looking, the status strip reads
  `look · arrows move · tab next · esc back`.
- It works everywhere the player walks: the strata, interiors, the Net and
  constructs, all with the same visibility rule. In the Net and constructs,
  nodes and ICE get a header and second line from their existing labels.
  Prose bodies for them are optional.
- In dev mode the Inspect sidebar follows the look cursor, which
  `focus_pos` already arranges once the cursor is the focus.

### The info window

- **48 columns wide** (Jeffrey: slightly larger than the 40-column sketches),
  in the same frame as the Space window, with a blank padding row under the
  second line and above the hint line.
- **Placement.** In a top corner of the map area, on the horizontal half
  away from the cursor: cursor in the left half, window top right, and the
  other way round. It switches sides when the cursor crosses the middle.
  When the map area is narrower than twice the window, it uses the top and
  bottom halves instead. It must never cover the cursor's tile.

```
┌─────────────────────────────────────────────────────────────┐
│ · · · · · · ·                    ┍━ ☺ Mara Chen ━━━━━━━━━━┑ │
│ · · @ · [☺] ·                    │  door muscle · on the… │ │
│ · · · · · · ·                    ┕━━━━━━━━━━━━━━━━━━━━━━━┙ │
└─────────────────────────────────────────────────────────────┘
                [ ] = the coloured block
```

### How it's written

1. **Header:** the glyph in its map colour, then the name the Space window
   would use.
2. **Second line:** dim, lowercase, saying what the thing is and its state.
   Parts are joined with ` · `, with no full stop. This matches the HUD's
   "Night Market · Street".
3. **Body:** one or two sentences in the voice of the item descriptions in
   `flavor/content.ron`: concrete, a little dry, present tense. It never says
   "You see…". A kind with nothing worth saying has no body.
4. **No numbers or stats**, apart from counts and floor names.
5. **Only what the player's eyes can tell them.**
   - `Visible`: everything.
   - `Remembered`: the header, the second line "as you last saw it", no
     body, no people, no items.
   - `Unseen`: no header, the one line "You can't see there."
6. **Several things on one tile:** the header shows the most important
   one: a person, then a fixture with verbs (terminal, door, connector,
   stairs, lift), then the top item stack, then other features (lamp, neon,
   sign, holo ad, stall, vent, trash), then the ground or building. The rest
   are named on a dim `also here: …` line at the bottom.
7. **Hint:** a dim `space · interact` line at the bottom, only when Space
   would open a window for it from where the player stands.

The approved examples:

```
┍━ ☺ Mara Chen ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┑
│  door muscle · on the door                    │
│                                               │
│  A keycard swings on a lanyard at their belt. │
┕━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┙

┍━ ▣ Access terminal ━━━━━━━━━━━━━━━━━━━━━━━━━━┑
│  public · Halvorsen district                  │
│                                               │
│  A municipal box bolted to the wall, its      │
│  screen scratched to fog.                     │
│                                               │
│  space · interact                             │
┕━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┙

┍━ ≡ Stairwell ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┑
│  up to Level 3 · down to Level 1              │
│                                               │
│  Concrete steps, a handrail rubbed bright.    │
┕━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┙

┍━ + Mara Voss's flat 1204 ━━━━━━━━━━━━━━━━━━━━┑
│  front door · locked, keypad                  │
│                                               │
│  Steel door, a peephole, a keypad under a     │
│  cracked cover.                               │
┕━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┙

┍━ ! Stim patch ×2 ━━━━━━━━━━━━━━━━━━━━━━━━━━━━┑
│  consumable · on the ground                   │
│                                               │
│  Twelve minutes of clarity, then the bill.    │
│                                               │
│  also here: Dead phone                        │
┕━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┙
```

| Under the cursor | Header | Second line | Body |
|---|---|---|---|
| Plain ground | Sidewalk | Night Market · Street | none |
| A building from outside | Club Halcyon | club · Night Market | "Bass through the wall, and a queue that doesn't move." |
| A street lamp | Street lamp | none | "Sodium light and a hum you stop hearing." |
| A remembered tile | Sidewalk | as you last saw it | none, no people or items |
| Out of sight | none | none | "You can't see there." |
| Your own tile, nothing else on it | You | Sidewalk · Night Market | none |

**(Claude's call)** On the player's own tile the player does not count as a
person for rule 6. The header is the most important *other* thing there, so
Look from the Stairwell window shows the Stairwell card. "You" only appears
when nothing else is on the tile.

### Where the words come from

- **People:** header is the card's name. Second line is profession · doing
  from `World::card`. The body holds only what can be seen: the keycard line
  when `credentials::carries` is non-empty, nothing otherwise. Their spoken
  line and relation belong to Talk, not Look. People use they/them in any
  generated text.
- **Items:** header is name and count. Second line is kind · on the
  ground. The body is the item's existing `desc`.
- **Flat doors:** header is `credentials::flat_label`, capitalised when it
  starts with "flat". Second line is front door · locked, keypad (or
  unlocked, or the keypad's lock time).
- **Terminals and the rig:** header "Access terminal" or "Access rig".
  Second line: public · the district's name, or your rig · its device name.
- **Stairs, lifts, connectors:** header "Stairwell", "Lift", "Ladder",
  "Manhole", "Stairs" and so on. The second line lists where it leads.
- **Buildings seen from outside** (their wall, window and mass tiles on a
  stratum): header is the building's name, as `World::place_name` gives it.
  Second line is the kind's label · the district.
- **Ground, features, furniture, building kinds:** the header is the
  kind's name. **The body lines are new writing:** a few lines per terrain,
  feature, furniture and building kind, in a new flavour file next to
  `flavor/content.ron` (**Claude's call:** `flavor/looks.ron`), picked by a
  hash of the tile so the same tile always reads the same and two lamps side
  by side usually don't.

## Left to the builder

- Struct and function names, module layout, where the subject gathering and
  the look text live (a `look_line`/`look_card` beside `inspector_lines` in
  `ui/panel.rs` is the obvious split).
- The exact colours, within the rules above.
- How many flavour lines per kind, and writing them in the house voice.
- Whether the "which way" prompt also draws the direction arrows on the map.
- Whether a multi-tile sign or neon strip is one Tab stop or several.
- What Space does in look mode inside a construct. The natural answer is
  the program list for that target when it is in reach.
- Small code gaps: `World::place_name` is private and returns "the
  apartment block" for unnamed buildings, and `focus_pos` needs a small
  change to follow the look cursor while in Walk.
- Test coverage. Existing tests that assert old row text ("talk to …",
  "look", HERE) are updated to the new wording.

## Order of work

1. **The window:** width, header rule, subtitle, capitalised `Verb: Thing`
   rows and kind colours, applied to every `Popup`.
2. **The verbs:** the verb table and its letters, subjects, the "which way"
   pick, Talk's card returning to the game, the construct's program rows,
   Enter no longer jacking, `J` and its confirm, and the help and README
   updated.
3. **Look:** the mode, cursor, Tab, the info window and its placement,
   visibility gating, the wording and the new flavour file.

## Follow-ups

- **A penalty for jacking out** (Jeffrey, 2026-10-18). Pulling the plug
  should cost something; what it costs is a follow-up design, not this one.
  The `J` confirm is where the cost will be shown.
