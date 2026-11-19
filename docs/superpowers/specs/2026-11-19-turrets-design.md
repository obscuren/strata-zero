# Turrets (#97)

A turret is a mounted gun in a job's copy of a site. It sleeps until it or a camera sees
you, then turns to follow you and fires in the turns engine. It carries armour you can
shoot off and a magazine that runs dry. A netrunner who holds its node can blind it from
the Net; one with the Turncoat talent can take it over, and then it fights on their side.

Mock: `2026-11-19-turrets-mock/` (`gen.py` writes `mock.html`). Jeff picked look A, the
arrowhead, and a turret that turns to follow you rather than one with a fixed field. The
mock's "off" and "blind" frames are one state now (blind), and its "turned" frame is a
turret taken over.

## Why

Sites (#34, #95, #96) have guards, cameras, locks and containers, but nothing stands in
place and shoots. A turret is the guarded room's hard point: you go round it, shoot it
down, stun it, or deal with it from the Net. The Net way is prep, as a netrunner works the
meat (`netrunner-meat-control`): crack it before you walk in, and it's dark, or with the
talent it's yours.

## Goals

- Turrets wake, turn, fire and fall in the same turns engine guards use.
- A turret's state shows at a glance: which way it faces, whether it's asleep, awake,
  blind, yours or wrecked.
- Any netrunner can blind a held turret; taking one over is learned, a talent in the ICE
  tree.
- Placement by site type and security stays as `flavor/sites.ron` already has it: none at
  low security, 0–1 at mid, 2–4 at high, in the secure (and an office's executive) zones.

## Non-goals

- Turrets in normal play, outside a job's copy (#35 is hostiles in normal play).
- Turrets on the street or on the city's Net.
- Picking a turret up, salvaging its gun or looting it (#39 is loot and drops).
- A new kind of weapon for the player.
- Using a turret's verbs from your body: they're Net verbs, used jacked in, as a camera's
  are.

## Anti-goals

- A turret that's only a no-go zone. Getting behind it must be a race you can win.
- A turret you hold that fights on its own while you're elsewhere: it joins a fight, it
  never starts one.

## What exists

- `citygen` places a mission layer's `DeviceKind::Turret` on a roomy room's open floor in
  the security's turret zones (`interior/venue/mission.rs`), draws it as
  `Feature::Turret` (it blocks walking) and gives it a `NodeKind::Turret` node on the
  site's Net. Cracking that node today only logs "turret cracked".
- Guards are `Foe`s with a `Post` (`foes.rs`, `guards/`), taken into `st.foes` when a copy
  is taken (`venues/copy.rs::take`).
- Floor cameras see a cone 8 deep, 90° wide (`cameras::in_cone`), by the floor's light
  (`cameras::sees`: near, or lit); a held camera offers verbs on single keys while you're
  jacked in (`holds::verbs`, `holds::Verb`).
- You carry armour: `game::turn_chance(arm)` (10% a point, at most 60%) turns a round
  that would hit (`combat::fire_at_you`).
- Bots (the drone) already take seats on your side of the turn order (`bots::seats`), but
  only follow you; a turret taken over is the first ally that shoots.
- Talents set flags read in code (`ride.rs` reads `data_surfer`); `game::flag_words`
  gives a flag its words on the TALENTS tab.

## Approaches

- **Field: turns to follow (picked by Jeff)** against a fixed field. A fixed field is
  simpler, but once you know it the turret is only a no-go zone.
- **A foe that never moves (my call)** against a device of its own with its own turn in
  the engine. As a `Foe` it gets the turn order, the turn column, stuns, blasts, EMPs,
  marks, the footer's lines and saving for free; what's new is a field of its own on the
  foe. A device of its own would mean another `Actor` and copying all of that.
- **The Net: blind for anyone, take over by talent (picked by Jeff)** against three verbs
  (off, a 3-turn blind, turn) used from your body on the turret's floor. Verbs from the
  body would have needed new wiring past `net::jack_in_refusal` and a mid-fight menu;
  Net verbs with an hour's hold make the play the prep, and the talent makes taking a gun
  over something you grow into.

## The turret

A turret is a `Foe` that never moves, with a new `turret: Option<Turret>` field
(`#[serde(default)]`). Name "turret", `chrome: true`, `post: None`, the gun below.

| | |
|---|---|
| HP | 40 |
| ARM | 4 at mid security, 6 at high (the band of `security`, as containers' grades are) |
| Gun | a mounted gun item in `flavor/content.ron`, not sold or dropped: damage 5, rounds 3, range 8, `skill` handguns, reflex as a goon's |
| Magazine | 30 rounds |
| Facing | one of the eight `Dir`s |

`Turret` holds: its node (`SiteNode`), its facing, ARM now and at most, rounds left,
whether it's awake, and what the Net has done to it (`Hacked::{None, Blind, Yours}`).

It's taken into `st.foes` with the guards when a copy is taken, one per turret device in
the layer, on the device's tile, facing the door of its room nearest to it (it watches the
way in). Its tile keeps `Feature::Turret`, so nothing walks through it, standing or
wrecked.

### Its field

The camera's cone, generalised to all eight facings: a tile is in it when its depth along
the facing is 1..=8 and its distance off the facing line is no more than its depth
(`cameras::in_cone`'s sum, which already gives a 90° quarter on a diagonal). It sees a
tile in its field with a clear line, near or lit, by the camera's rule.

### Asleep, awake and hunting

Three states, so a glimpse or an alarm doesn't hold a floor on turns (my call):

- **Asleep** (`FoeMode::Idle`, `Turret.awake` false): it neither turns nor fires.
- **Awake** (`FoeMode::Idle`, `Turret.awake` true): on the clock, not on turns. It turns one
  eighth every `GUARD_STEP_TICKS` toward the last tile it was told of, and once it faces
  that, sweeps on round clockwise. It takes no seat in a fight and doesn't keep the floor
  on turns.
- **Hunting** (`FoeMode::Hunting`): it has seen you in its own field. It takes a seat as a
  hunting guard does, and holds the floor on turns as one does.

What moves it on:

- Asleep → awake: a camera on its level sees you (it's told your tile), or the site goes
  on alarm (no tile; it sweeps).
- Asleep or awake → hunting: its own field sees you.
- Hunting → awake: it loses you as a guard does (`LOST_TURNS` turns out of its field).
- Awake → asleep: after `LOST_TURNS` game minutes awake with no sighting, unless the site
  is on alarm.

A turret doesn't radio the alarm itself. Its shots are heard (`guards::hear`), and the
guards who come radio it in.

These states are a hostile turret's; a blind one or one that's yours has neither (below).

### Its turn

Its pools are 1 move and 1 AP; the move is a turn of one eighth.

1. If its target is in its field, it fires (the AP).
2. Otherwise it turns one eighth toward the target, by the shorter way round, and fires
   if that puts the target in its field.

A hostile turret's target is you. Firing is `combat::fire_at_you`'s volley: your armour
turns rounds as today. Each round costs a round from the magazine. With the magazine
empty it still turns and watches, and its turn reads "clicks dry". The turn column's
"theirs" line reads `turns › fires`, `turns`, `fires` or `clicks dry`.

### Shooting it

A round that hits it rolls its armour: `game::turn_chance(arm)`. A turned round sparks off
and chips one point off its ARM; a round that gets through takes its HP. So the plate goes
first, then the gun. Blasts and strikes hurt it as they hurt any foe; it's chrome, so an
EMP hurts it in full; a stun stops its turn.

At 0 HP it's wrecked: down, drawn dark grey, never acts again. It drops nothing.

### The Net

Cracking or spoofing its node holds it (`holds::holdable` is true for `Turret`), an hour
(`HACK_MINUTES`) in `st.net.holds` as any hold. Its verbs are Net verbs, on single keys
while you're jacked in, as a camera's are:

| Verb | Key (my call) | Who | What it does |
|---|---|---|---|
| blind | `o` (as loop) | anyone | Dark for the hold: never wakes, never turns, never fires, takes no seat. |
| take over | `i` (as hijack) | Turncoat | Yours for the hold: on your side in a fight on its floor (below). |

- Either verb can be used again while the hold lasts, to swap between them.
- Without Turncoat the take-over key is shown and refused: "takes Turncoat".
- An alarm doesn't end either: the hold lasts its hour.
- A turret that's woken or hunting goes dark at once when blinded, and turns from
  hunting you to yours at once when taken over; a fight it sat in goes on without it, or
  with it on your side.
- When the hold ends the turret is hostile again: hunting if you're in its field and a
  fight is on, else awake if the site is on alarm, else asleep. `Turret.hacked` goes back
  to `None` there, read off `st.net.holds` each turn so a save restores it with the hold.

**A turret that's yours** (my calls, agreed with Jeff):

- Outside a fight it does nothing: it never starts one, so a turret you hold can't gun a
  patrol down while you're elsewhere.
- In a fight on its floor, whoever started it, it takes a seat on your side. Its target is
  the nearest standing guard on its level it has a clear line to. It turns toward that
  guard and fires when the guard is in its field and in range 8, as it would at you, and
  its rounds cost its magazine.
- It never fires on you, and nothing it does wakes or alerts the site on its own; its
  shots are heard as any shots are.

**A guard and a turret that's yours** (my calls): a hunting guard that can see such a
turret in reach fires on it when it can't see you, or when the turret is nearer than you.
A guard hit by it hunts toward it (as `volley` already sends a foe toward who shot it) and
fires when in reach. No new saved state: the target is picked each turn.

Firing on a foe from a foe is new: the plan generalises the volley so a foe can fire at
another foe (the turret at a guard, a guard at the turret), with the target's own armour
(only a turret has any) and `wound_foe`.

### Turncoat

A new talent in the ICE tree, in the empty slot under Black ICE (tier 4, third column of
`flavor/content.ron`'s ICE tiers), requiring Black ICE. The Deep end, under it, keeps
requiring Black ICE.

```ron
Talent(id: "turncoat", name: "Turncoat", ranks: 1, requires: Some("black_ice"), skill: Some("hacking"),
       effects: [[Flag("turncoat")]],
       desc: "Every gun has a firmware update. You wrote it."),
```

`flag_words("turncoat")`: "a held turret takes your side: it fires on the guards in a
fight on its floor". It gets a 6x6 icon in `flavor/talent_art.ron`, drawn in the talent
art script as every icon is.

## How it looks

- **On the map:** look A from the mock. An arrowhead pointing its facing: `▲ ◥ ► ◢ ▼ ◣ ◄ ◤`
  (all in Fira Code), bold, on a steel body `#2b2f36`. The glyph's colour by state:

  | State | Colour |
  |---|---|
  | asleep | `#8a4a4e` |
  | awake or hunting | `#ff3c46` |
  | blind | `#5b6b7d`, not bold |
  | yours | `#5fd38a` |
  | wrecked | `#3a3f47`, not bold |

  A turret it's dark round shows as you remember it, as other foes do.
- **Its field:** drawn as the cameras' cones are (`cameras::ConeOverlay`):
  - awake or hunting: red (`SEEING_BG`);
  - yours: green (`TAPPED_BG`);
  - asleep, blind or wrecked: not drawn.
- **The turn column:** its chip's figure reads `ARM·HP` (`4·40`), in the chip's 5 cells.
  A turret that's yours sits on your side of the column, as the drone does.
- **The look line:** `Turret · facing SE · ARM 4 · HP 40 · 22 rounds`, with its state
  where it's not awake: `Turret · asleep`, `Turret · blind · 42 min`,
  `Turret · yours · 12 min`, `Turret · wrecked`.
- **Messages:** `turret wakes`, `turret turns SE › fires · 5 · turned · miss · hp 31`,
  `turret clicks dry`, `you fire · … · plate 6 → 4`, `turret wrecked`, `turret blind`,
  `turret's yours`, `turret fires on guard · …`, `turret's back on its own net` (the hold
  ended).

## Saving

- `Foe.turret` is `#[serde(default)]`: a foe saved before is no turret.
- A copy taken before this branch has no turret foes; its turrets stay props (a
  `Feature::Turret` that blocks and does nothing). The save fixtures keep opening.
- `Turret.hacked` is saved with the foe; the hold itself is `st.net.holds`'s, saved as
  holds are.

## The manual

Chapter 06 (sites) says what a turret is and how to get past it; chapter 07 (fights) says
how it turns, fires, runs dry and loses its plate, and that a turret that's yours fights
beside you; chapter 08 (the Net) gives blind and take over, and that take over needs
Turncoat. The talents list, if a chapter has one, gains Turncoat.

## What's left to the builder

- The gun's item id and name.
- How the volley is generalised to foe-on-foe fire.
- How a turret that's yours sits in the turn order beside the bots' seats.
- Turncoat's icon.
- A shot of a turret fight in `screens/shots.rs`.
