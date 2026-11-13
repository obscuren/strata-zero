# Guards on floors (#33)

Guards in a mission copy of a hotel, office or station stop standing still. They walk
their rounds, some between floors; they see you by the floor's light; they open the doors
their card opens; they chase you through doors and up stairs and lifts; they fire, and you
fire back. A guard who spots you radios it in, and the site's alarm goes off unless you
drop it first. A guard who finds the body you left jacked in pulls you out of the Net.

Jeff picked #33 as the next ticket after the site's own Net (#95) and left the design to
me, so every rule below that the tickets and the earlier specs don't already fix is my
call, marked **(call)**.

## Why

The site generator (#34) lays out guards with posts and patrols, and the site's Net (#95)
gives a netrunner camera loops, lift spoofs and door cracks to get past them. None of that
matters while the guards stand still. With guards that move, the walk through a copy gets
tense, and the Net's tools have something to beat. The site-net spec says it plainly: the
walk only gets tense once guards act (#33) and the alert lands (#36).

## Today

- Guards are `Foe`s with a `Post { building, zone, patrol }` (`foes.rs`), made by
  `venues::guard_foes` from the copy's `MissionLayer.guards` when a copy goes live, and
  dropped by `copy::finish`. They are `inert`: they never look, move or fire, and take no
  damage.
- A `GuardPost`'s patrol is its post and 1–3 more spots of rooms of its zone on its own
  level (`citygen/src/interior/venue/mission.rs`, `post_guards`). No patrol leaves its level.
- Foes see, path and fire on strata only. `foes::line_clear`, `step_toward` and `step_out`
  give up off a stratum, and so do, on the player's side: `combat::reach` (so shots say "no
  line" on a floor), `miss_impact`, `push_foe`, `chain_foes`, `blast::footprint` and
  `landing`, the throw shapes, `explore::open_at` (an engaged player can't step on a floor)
  and the aimed walk's `street_path`.
- The turn engine itself (`net::engage`, `initiative`, `pace`, `end_turn`) and the calm
  look (`foes::look_all` every frame on foot) already work on any place, and foes on a
  floor are already drawn.
- `MapRef::for_place` gives a floor as a `TileMap` (walkable, tile, light, glow), and
  `citygen::sight::sees` is the one rule for "an eye by the map's light", already used by
  floor cameras.

## Goals

1. **Fights on floors.** Everything a fight does on the street works on a floor of any
   building: foes see, step and fire; you step, aim-walk, fire, strike, push, chain and
   throw. The rule is the map's, read through `MapRef`, never `place.stratum()`.
2. **Guards walk their rounds** while you, or your body, are in the building.
3. **Some rounds cross floors** by the stairs, escalators and lifts the guard's card rides.
4. **Guards see you by the floor's light** and hunt you as a goon does on the street.
5. **They chase you through doors and across floors.**
6. **A guard who spots you radios it in**: on its first turn hunting you, the site's alarm
   goes off. Drop it before that turn and nobody hears.
7. **Shots are heard**: a gun fired on a floor wakes the guards near it.
8. **A guard who finds your body** while you're jacked in raises the alarm and drags you out
   of the Net.
9. **A downed guard leaves its zone card** on its tile.
10. Old saves keep opening, guards in them start to act.

## Non-goals

- Alert states, suspicion, noise beyond gunshots, a site locking down floor by floor, the
  guards closing in on an alarm: #36. Here a guard is calm or hunting, as a goon is.
- Melee (#37), deeper gunplay (#38), loot beyond the zone card (#39), hostiles in normal
  play (#35), turrets firing (#97), containers (#96), site objectives (#98).
- People in plain buildings stay as they are: placed at stations, never hostile. Guards live
  only in mission copies.
- The sim doesn't run guards while you're away from the building: nothing in a copy moves
  while you're outside it (site generator, call 9).

## Anti-goals

- A guard that walks through a locked door it has no card for, or through a wall.
- A guard that sees you through a wall or a shut door, or across a dark room from far off.
- A fight on a floor that plays differently from one on the street in any way the map
  doesn't explain.
- A guard, its alarm or its body showing in the plain building after the mission.
- An `Actor::Foe` index pointing at the wrong foe because the list changed under it.

## The rules

### Seeing (calls)

- A guard sees by `citygen::sight::sees` over the floor's map: a clear line, and you
  either within its dark reach, `GUARD_DARK_REACH` 3 tiles (as a camera's `DARK_REACH`), or
  standing on a lit tile no farther than its `sight` (8). So a lamp the Net turns off, or a
  dark corner, hides you past 3 tiles; a lit corridor doesn't.
- A door tile blocks sight, as it does for you and for cameras. A guard standing in a
  doorway sees both rooms (a line's ends don't block).
- Street foes keep their own rule (no light). Only a floor reads light. **(call: the street
  rule is #36's to change.)**
- Your body left on a floor while you're jacked in is seen by the same rule.

### Calm: the round

- A guard's round is its patrol: post, then each spot in order, then back the way it came,
  pausing a few seconds at each spot. **(call)**
- It walks at an amble, slower than you walk. **(call: a step every `GUARD_STEP_TICKS`,
  three times `WALK_TICKS`.)**
- It paths over the floor's map with A*, around other foes and you, through the doors its
  card opens (below). A spot it can't reach is skipped; a round with nothing reachable
  stands at its post.
- Guards walk on the clock, out of turns, while you're in the building: on one of its
  floors, or on its own Net, or with your body on one of its floors. Elsewhere they hold
  where they are. **(call)**
- A guard on a floor you haven't seen yet walks there too: its floor is drawn when needed.
- During a fight, a calm guard holds still; it looks each round, and joins if it sees you
  (`seat_foes`, as on the street).
- During a fight on a floor, every hunting guard on that floor has a seat, whether or not
  you've seen it, so a guard woken by a shot comes for the shot on turns. The turn bar shows
  a guard only once you've seen it. The fight ends, as today, once nothing on your floor
  hunts: a woken guard that reaches the shot and doesn't find you loses you after
  `LOST_TURNS`. **(call)**

### Rounds that cross floors (call)

- The mission layer gives some guards one stop on another level: about 1 guard in 3, and at
  least 1 when the copy keeps 2 or more levels joined by a link whose stops that guard's card
  opens. The stop is a spot of a room of its zone on that level.
- At the end of its round on its own level, such a guard walks to the link, takes it, walks
  to the stop, waits, and comes back the same way.
- Taking a link: the guard walks onto the link's tile on its level and is moved to the
  link's stop on the other level, at once for stairs, escalators and ladders, after a short
  wait for a lift. A lift must answer (`sitenet::lift_answers`); in an alarm only a lift
  spoofed since then answers, for guards as for you, so a guard uses stairs, or waits. Crawls and ladders that need Tech are for you, not guards.
- The stop is new data in the saved layer (`GuardPost`, `#[serde(default)]`), drawn from
  its own random stream so the devices the layer places after the guards don't move.

### Doors (call)

- A guard opens a door lock when it stands on the lock's free side, or when the lock takes a
  card (`Ways::CARD`) and the lock's zone is at most the guard's zone. Keypads that take
  only a code, Keyjack or Net don't open for it.
- A guard opens a private room's door when the room's zone is at most the guard's zone.
- Cards still open doors in an alarm, so the alarm doesn't trap guards.
- One function, `guards::opens(post, floor, lock_or_room)`, answers this, and the guard's
  A* and its link choice both ask it.

### Hunting

- A guard that sees you hunts you, as a goon does: on your turns and its own, it closes in
  and fires when you're in range with a clear line, and loses you after `LOST_TURNS` turns
  without sight, then walks back to its round.
- **Radio (call).** A guard that has seen you radios it in at the start of its next turn,
  before it acts: the site's alarm goes off (`net::raise_alarm`), with the line "guard
  radios it in · ALARM". Only seeing you counts: a guard woken by a shot or a blast doesn't
  radio until it sees you itself. A guard radios once, ever (`Post.radioed`), so one that
  follows you to another floor doesn't call again. A guard downed or stunned before that
  turn never radios. An alarm already on means no call, but the guard is marked as having
  radioed.
- **Following you across floors.** When you leave a floor by a link while a guard hunts
  you, it marks where you went (the landing). It walks there and follows: takes the
  same link if its card opens it (a lift only if it answers), and lands hunting you at the
  other end, a turn or two behind. A guard that can't follow loses you there. **(call)**
- Leaving a floor still ends the fight (`disengage`), as on the street; guards that follow
  you start a new one when they see you again.
- Guards never leave the building. Out through a door to the street, or a crawl out, and
  the guards hunting you lose you at the door and go back to their rounds. **(call)**

### Shots are heard (call)

- A gun fired on a floor, by you or a guard, wakes every calm guard on that floor within
  `HEAR_RADIUS` (12) tiles of the shooter: it hunts toward the shot. Walls don't stop it.
- A grenade's blast does the same (it already wakes foes in its footprint; on a floor the
  radius is `HEAR_RADIUS` too).

### Your body (call)

- While you're jacked in from a floor of a copy, a guard that sees your body raises the
  alarm, and you're thrown back into your body (dumped: FOCUS gone, the deck cools) with
  that guard hunting you. The line: "a guard found you · jacked out".
- The site Net's body panel already counts guards on the body's floor; it now shows the
  nearest one moving.

### Down

- A guard is no longer inert: it takes damage and goes down at 0 HP, as a goon does.
- A downed guard leaves its zone card (`keycard:{b}:zone{z}`) on its tile, picked up as
  any card on the ground. It goes in the copy's `left` cards, so the copy's end clears it
  with the rest. A guard of zone 0 (public) carries no card and drops nothing. **(call)**
- When the copy ends, its guards go, down or standing, as today.

### Fights on floors

Every place below that asks `place.stratum()` asks the place's `MapRef` instead:
`foes::line_clear`, `step_toward`, `step_out`; `combat::reach`, `miss_impact`, `push_foe`,
`chain_foes`; `blast::footprint`, `landing`; the throw shapes; `explore::open_at` and the
aimed walk. On a floor:
- an engaged step onto a lock or a grille goes through `venues::step_onto` as a calm one
  does;
- the line of sight and fire is the foes' (`foes::blocks_sight`), over the floor's tiles;
- the death line names the place: "a guard in {building}", not "on the street".
- `console foe` and `dummy` spawn on floors too.

### Saving

- `Post` gains the round's state (where in the round, the pause, a link being taken, the
  landing being followed) and whether it has radioed, each `#[serde(default)]`.
- Old saves: a foe with a `post` is turned active on restore (`after_restore` sets
  `inert = false`), so guards in a saved copy start walking.
- The fixtures in `runs/fixtures/` keep opening.
- `Actor::Foe(i)` stays an index. Nothing removes foes during an engagement; a copy's guards
  go only when the copy ends, with you outside.

## What a player sees

- Guards (`@`, foe red) walk their rounds; you watch them pass doorways and take the stairs.
- The log: "guard spots you", "guard radios it in · ALARM", "a guard found you · jacked
  out", and the usual fight lines.
- The manual: 07 (turns and fights) says fights work indoors and guards radio in; 06 (the
  city: buildings) says what guards in a mission copy do; 08 (the Net) says a guard can
  find your body.

## Left to the builder

- Module layout (a `guards` module beside `foes` is my suggestion), constant values within
  the ranges above, the exact log lines' wording around the fixed ones.
- How a floor's `MapRef` is reached from `foes` (pass a map, or build it inside).
- Shots: a guard on its round and a fight on a floor, from the DEVELOPER office copy.

## Calls made

1. Floors read light for guards' sight (3 tiles in the dark, 8 lit); the street rule
   stays as it is.
2. A guard that has seen you radios the alarm on its next turn, once ever; a guard woken
   by noise doesn't; dropping it first keeps the site quiet.
3. Gunshots wake guards within 12 tiles on the floor; walls don't muffle them.
4. Guards walk only while you or your body are in the building.
5. About 1 guard in 3 has a stop on another floor; lifts only when they answer.
6. A guard's card opens locks and private rooms of its zone and below.
7. A guard that finds your body dumps you out of the Net and raises the alarm.
8. A downed guard drops its zone card.
9. Hunting guards follow you across floors when their card lets them, but never out of
   the building.
10. In a fight on a floor, every hunting guard on that floor takes turns, seen or not.
