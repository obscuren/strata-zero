# Scenes: the film and the bubbles, and the house call

Date: 2026-11-01. Designed with Jeffrey in chat; spec, plan and build run in one go at
his ask ("Spec -> plan -> impl"). Mockups: `2026-11-01-house-call-mock/` (round 1:
A film, B bubbles, C script; A then got B's stage line in its top bar).

Builds on `2026-10-31-story-missions-design.md` (the mission engine, its triggers,
effects, cast and the HOLO), `2026-10-31-tracking-and-map-design.md` (the objective a
track follows), `2026-10-17-keypads-design.md` (the keypad), `2026-10-17-megablock-home-design.md`
(the flat), `2026-10-29-window-prototype-design.md` (what draws in both front ends).

## Why

The drop ends with "get home" and a mail. The story that follows needs the game to
take the camera for a moment: you key your code at your own door, the pad buzzes red,
the door opens from inside, and two strangers are waiting in your dark flat. A HOLO
feed can't show that; it has to happen on the map, with the people in it moving and
talking. That is a **scene**: a scripted stretch where the game acts for you and the
lines play out, then hands control back.

Scenes come in two looks on one engine. **Film** (concept A) takes the screen: black
bars, the map framed in the middle, subtitles below, what happens dim in the top bar.
**Bubbles** (concept B) stays in the game: the HUD dims, lines float beside whoever
speaks. The house call is a film; Seren's word on her way out is bubbles; later
missions pick either per scene.

## What Jeffrey decided

- **A for the house call** ("A is really good for these… It's beautiful").
- **B built too, ready to use** ("B will also come in handy when we need to run dialog
  without the film effect. So build both, make sure that both are done and ready and
  can be integrated").
- **B's stage line in A**: "The lock clicks from inside…" reads in the film's top bar.
- **The stage line at the bottom, italic** (after playing it): "The 'you walk in' needs
  to be italic and i think also on the bottom. theres so much to see its easy to miss".
  The film's stage line moved under the map, over the subtitle; both looks draw it
  italic, and the window leans its glyphs to show it.
- **Timing SHOULD NOT be hardcoded.** Every wait, walk speed, typing speed and line
  dwell is a value in the scene's data. A line moves on after its data-set time, or
  when you press `space`; both work now.
- **The story** (chat, 2026-11-01): the drop was a set-up to get you out of your flat;
  Marlo's death was part of it; Mezz (the fixer, renamed from Dez) was played too and
  can be trusted, and later becomes your friend; the package holds a fixer's ledger,
  nothing important. Waiting in your flat: **Castellan**, a man the game's larger arc
  turns on, and **Seren**, a netrunner, a woman who is cool, beautiful and very good.
  The larger arc points to a level below the street no map shows: Strata Zero.
- **The cinematic acts for you**: the keypad opens, your code is typed, your "WTF…?",
  the door slides open, "Come in, <name>", you walk in, Castellan sits there, the talk
  moves on, Seren comes out of the room with the rig.
- **Dez becomes Mezz** (Dez sat one letter from another game's fixer who betrays you).

## Calls I made (Jeffrey: check these)

- **Scope.** This spec is the scene engine, both looks, visitors, and the house call
  as content. The rest of the chain (the ripperdoc on Castellan's advance, the ledger's
  fixer, Seren's lesson, the job, payday, hints that fit your build) is the next spec.
- **The house call's script** is mine, from the pitch you liked (§12). Castellan talks
  quietly and precisely; Seren is dry; you speak four short lines.
- **Castellan leaves at the end of the film; Seren stays a beat**, and her two lines on
  the way out are the bubbles scene, so B is used in the shipped story from day one.
- **`esc` skips a scene and loses nothing**: every line not yet shown goes to the log,
  everyone ends where the scene would have left them.
- **`space` always means "next"**: it finishes a typing line, then moves on; on a walk,
  a keypad or a wait it jumps that beat to its end.
- **The flat is dark** while they wait: a `Lights` effect dims your flat until Seren
  turns them back on as she leaves.
- **Mezz's "lay low" mail still comes**, two minutes after the house call: Mezz doesn't
  know what happened. The drop now ends at your door.
- **No scene starts in a fight or a run**, like the HOLO call; a fight that starts
  mid-scene skips it. A call that rings during a scene waits; one up when a scene
  starts goes to voicemail.
- **Visitors** (Castellan, Seren) are authored people the story puts on the map, not
  people of the sim. They draw as `@` in their tint, look reads them, you can't walk
  through them. Talking to them is the next spec's.
- **The bars don't animate in**; they are there on the first frame (an animation would
  need its own timing in data; not worth it yet).

After the builder check (settled here):

- **Visitors are never cast and never missing.** `Mission::roles()` (what `begin`
  needs cast) leaves out visitor roles and `you`, so a mission `from: "castellan"` is
  never `Uncast`. `cast::cast` skips visitors, and its roll seeds by the role's id
  (`hash64(seed, <the id's bytes folded with hash64>)`) instead of its index in the
  map, so adding or renaming a role never re-casts another. This re-casts Marlo once in
  every city (the rename moves him in the map anyway); tests and shots that assumed the
  old Marlo are updated, the rule isn't bent to keep him.
- **Tracking follows `HomeDoor`.** `track::resolve_story` gets a `HomeDoor` arm: a spot
  on your flat's door, radius 0 (after the final review: the door is where the trigger
  holds, and a radius of 1 read "there" on the walkway's corners, where it doesn't). The
  track test that looks for `Where::Home` in the shipped book looks for `HomeDoor`.
- **The dev console keeps its priority.** It takes keys before the scene and draws over
  the film, so `story skip` can be typed while a scene runs.
- **The rename is everywhere in the game**: `flavor/content.ron`'s package text,
  `credentials::fixer_mail`'s fallback name, and every test that says "Dez" say
  "Mezz". Old specs and plans keep their history.
- **`ui::dim::wash_rect(buf, rect)`** beside `wash`, for the film's and the bubbles'
  partial washes.
- **A scripted pad holds its verdict.** `show_verdict` keeps OPEN or DENIED on the
  display for as long as the scene shows the pad; the flash's glitch runs its own
  frames as the real pad's does.
- **A line spoken `as` someone else hides the speaker**: its label and its marker are
  `DIM`, not the speaker's tint (the voice behind the door isn't Castellan yet).

After the final review: **getting home early.** A player who walked home during Mezz's
call stood in the flat with "Get home" and nothing happened (and leaving played the
arrival backwards). So:

- **`Where::HomeDoor` holds when you are home**: in the flat's rect, or outside it within
  one tile (manhattan) of its door, the door tile included.
- **`Arrive` takes `you`**: the player is put on the mark at once (place, tile, sight; an
  aimed walk or a scene walk of yours dropped), the same cut as a mission effect or a
  scene beat.
- **`Arrive` onto a held tile** (the player or another visitor on it) takes the free
  walkable tile beside it nearest the one arriving, as `Beside` picks it; and the film
  opens with `Arrive(role: "you", at: HomeOutside)`, so it always starts on the walkway
  at your door.

## Words

- **Scene**: a scripted stretch of beats played on the map; one look, one pace.
- **Beat**: one thing a scene does: a line, a stage line, a walk, a wait, a keypad, a
  door, the camera, someone arriving or leaving, the lights.
- **Look**: `Film` (A) or `Bubbles` (B).
- **Pace**: a scene's timing, all in data: typing speed, walk speed, line dwell.
- **Mark**: a named spot a beat points at (`HomeDoor`, `HomeOutside`, `Beside(You)`…).
- **Visitor**: a role the story places on the map itself (`pick: Visitor`), not cast
  from the city's people.
- **Actor**: you or a visitor, the ones a scene can move and point at.

## Goals

- `Scene`, `Beat`, `Pace`, `Mark`, `Look` in `missions/content.rs`, scenes in
  `flavor/missions.ron` under `scenes`, validated at load like everything else there.
- Effects `Scene(id)`, `Arrive(role, at)`, `Leave(role)`, `Lights(level)`; trigger
  place `Where::HomeDoor`; pick `Visitor`; role fields `tint` and `look`; the slot
  `{you}`. `Ended(step)` also ends on a scene.
- The scene runtime (`missions/scene.rs`): start, keys, tick, skip, end; never stalls.
- Film and bubbles drawing (`missions/film.rs`, `missions/bubbles.rs`) through ratatui
  only, the same in the terminal and the window, at 100×30 and up.
- Visitors on the map: drawn, looked at, blocking (`missions/visitors.rs`).
- A scripted keypad: the keypad screen drawn by a scene, typing and showing a verdict
  without touching the journal or the lockout.
- Content: Dez → Mezz; Castellan and Seren; the drop ends at your door; `house_call`
  (film) and `seren_aside` (bubbles); the mail after.
- Dev console: `story scene <id>` plays a scene; `story` lists scenes too.
- Shots of the film and the bubbles (several beats, 120×40 and 100×30).

## Non-goals

- Talking to visitors, choices in a scene, branching lines.
- Saving a scene or story progress (session only, as before).
- Bars that slide in, fades, a moving camera pan (beyond holding a mark).
- Door state outside a scene (doors stay as they are: a scene's open door is drawn by
  the scene and gone when it ends).
- The rest of Castellan's chain (next spec).

## Anti-goals

- A number of ticks, seconds or characters per second written in Rust for a scene's
  timing. The only timing constants in code are the existing ones of other systems
  (the keypad's own flash, the face's glitch).
- A scene that can stall: every beat ends on its own (its data-set time, or arrival),
  every scene ends on `esc`, a mark that can't be found never blocks.
- A scene, beat or mark that names a mission or role in Rust.
- Anything that writes to stdout or reads keys outside crossterm `KeyEvent`s.

## 1. Content

`flavor/missions.ron` gains a `scenes` map beside `missions`, keyed by scene id (lower
snake case):

```ron
scenes: {
    "house_call": Scene(
        title: "House call",
        look: Film,
        pace: Pace(type: 45, walk: 0.3, hold: Some(2.5)),
        beats: [ /* §12 */ ],
    ),
},
```

- `title`: shown in the film's top bar and the bubbles' footer, upper case. No slots.
- `look`: `Film` or `Bubbles`.
- `next` (optional): the scene that plays straight after this one, for the same step;
  the step ends with the last. The house call's film names Seren's aside, so the dev
  console's `story scene house_call` plays both, as the game does.
- `pace` (all required, all data):
  - `type`: characters per second a line types at (> 0).
  - `walk`: seconds per tile a walk takes (> 0).
  - `hold`: seconds a fully typed line stays before the scene moves on by itself;
    `None` waits for `space`. A beat can set its own.
- Seconds are `f32` in data and become clock ticks (`round(s × 30)`, at least 1).

### Beats

| Beat | Does | Ends |
|---|---|---|
| `Say(who: "role" \| "you", text: "..", as: Some("A voice"), at: Some(<Mark>), hold: Some(s))` | a line: types at `pace.type`, shown as a subtitle (film) or a bubble (bubbles); logged as it starts | after typing plus `hold` (the beat's, else the pace's); with no hold at all, on `space` |
| `Stage("..")` | the stage line, italic: under the film's map, in the bubbles' corner; replaces the last one | at once |
| `Walk(who, to: <Mark>, pace: Some(s), wait: true)` | the actor walks a path to the mark, one tile per `pace` (the beat's, else `pace.walk`) | on arrival; with `wait: false` at once, the walk going on beside later beats |
| `Wait(s)` | nothing for `s` seconds | after `s` |
| `Keypad(door: <Mark>, code: Journal \| Code(n), result: Denied \| Open, keys: s, hold: s)` | the door's keypad over the map: the code typed a digit every `keys` seconds, then the verdict, held `hold` seconds | after the last digit plus `hold` |
| `Door(at: <Mark>, open: bool)` | the door drawn open (or closed again) for the rest of the scene | at once |
| `Camera(<Mark>)` | the camera holds that tile; `Camera(You)` follows you again | at once |
| `Arrive(role \| "you", at: <Mark>)` | the visitor appears on the mark (or moves there); `you` are put there, a cut (your place, tile and sight, any walk of yours dropped); a tile another actor holds gives the free tile beside it nearest the one arriving (as `Beside`) | at once |
| `Leave(role)` | the visitor is gone | at once |
| `Lights(level)` | your flat's light level, `1.0` normal, `0.0` black (§8) | at once |

- `who` is a role id or `"you"` (the player). `as` replaces the label shown and logged
  (`"A voice"`); `at` anchors the line to a mark instead of the speaker (the voice from
  behind the door). `wait` defaults to `true`; `as`, `at`, `hold` and a walk's `pace`
  default to `None`.
- `space` ends the beat in progress early: a typing line shows whole (a second `space`
  moves on), a walk arrives, a keypad shows its verdict and closes, a wait is over.
- Text takes the cast's slots plus `{you}`, the player's handle.

### Marks

`enum Mark` resolves, when a beat runs, to a `(Place, Pos)` or nothing:

| Mark | Is |
|---|---|
| `You` | the player's tile |
| `Role("seren")` | the visitor's tile |
| `HomeDoor` | your flat's front door |
| `HomeOutside` | the walkway tile just outside it |
| `HomeInside(n)` | `n` tiles in from the door, straight in (`n ≥ 1`), the last walkable one if the room ends sooner |
| `HomeRig` | the walkable tile beside your rig nearest the flat's door |
| `HomeBed` | the tile beside the bed (`home.bed_side`) |
| `HomeFurniture(Couch)` | the flat's tile of that furniture nearest its door (ties: top, then left) |
| `Beside(<Mark>)` | the free walkable tile next to the mark nearest the actor who walks there (or the player, for other beats); "free" means no actor on it |

All `Home*` marks are on your flat's floor (`st.home`); without a home they resolve to
nothing. A beat whose mark resolves to nothing: `Walk` and `Arrive` do nothing (dev
mode warns `scene · <mark> not found`), `Say`'s `at` falls back to the speaker, `Door`
and `Camera` do nothing, a `Keypad` shows no pad (its time still passes).

### Roles, effects, triggers

- `Role` gains `tint: Option<(u8, u8, u8)>` (a speaker's colour, a visitor's `@`) and
  `look: Option<String>` (what look mode says of a visitor). Both default `None`.
- `Pick::Visitor`: never cast from the sim. A visitor role takes only the slots
  `{role}` and `{role.title}` (validation refuses `.place`, `.district`, `.hours`).
- Effects: `Scene("id")` plays the scene (the step's presentation, like a feed: its
  end, or the end of the last scene its `next` leads to, marks the step `Ended`); `Arrive(role | "you", at: <Mark>)`, `Leave(role)` and
  `Lights(level)` work as the beats do, outside a scene (an `Arrive` of `you` is the
  same cut, and the map follows you there).
- `Where::HomeDoor`: you are home: on your flat's floor, inside the flat's rect, or
  outside it within one tile (manhattan) of its door, the door tile included.
- Validation, beside the old: every scene an effect names exists; every role a beat or
  effect names exists; `Walk`, `Arrive`, `Leave` and `Role(..)` marks name visitors (or
  `you` for `Walk` and `Arrive`); no role is called `you`; `type`, `walk`, holds, keys, waits and
  paces are finite and > 0; `level` in 0..=1; `HomeInside(0)` refused; text slots
  checked with `{you}` allowed; `Ended(step)` may name a step that plays a scene; a
  scene has at least one beat; a scene's `next` exists and never leads back to it.

## 2. The runtime (`missions/scene.rs`)

`Missions` gains `scene: Option<SceneRun>`, `scenes: VecDeque<(usize, usize, String)>`
(mission, step, scene id; waiting their turn), `visitors: Vec<Visitor>` and `lights:
f32` (1.0).

- `SceneRun { mission, step, id, beat: usize, began: u64 (clock tick the beat began),
  stage: Option<String>, line: Option<Line>, walks: Vec<WalkRun>, keypad:
  Option<PadRun>, doors: Vec<(Place, Pos)>, camera: Option<(Place, Pos)> }`.
- `Line { label, colour, text, anchor: Option<(Place, Pos)>, typed_at: u64, done_at:
  Option<u64> }`; `WalkRun { who, path: Vec<Pos>, place, next: u64, step_ticks }`.
- **Start.** `Effect::Scene` queues; a queued scene starts when none runs, no fight is
  on (`st.net.engaged` is `None`) and the player is not in a construct. Starting drops
  an aimed walk in flight, sends an up HOLO call to voicemail, and runs beats from the
  first.
- **Tick** (`missions::tick`, every explore update in walk mode, clock ticks): walks in
  progress step when their `next` tick comes (the player's FOV updated after each of
  their steps); then the current beat checks whether it has ended (table in §1); when it
  has, the next beat starts at once, so instant beats chain in one tick; after the last
  beat the scene ends.
- **Keys.** While a scene runs, the explore update gives every key to the scene before
  anything else and swallows it: `space` and `enter` are "next", `esc` is "skip", the
  rest do nothing. The world still ticks.
- **Walks.** The path is A* over the place's walkable tiles, four ways, the start and
  goal allowed to be unwalkable (a visitor on a couch), tiles under other actors
  blocked; no path: the actor stands on the goal at once. A walk to another place (you,
  off the home floor) puts the actor there at once.
- **Skip.** Every remaining beat applies its end state at once: lines and the line in
  progress are logged in full, walks finish (actors on their goals), arrivals and
  leaves happen, lights set; then the scene ends.
- **End.** The scene's doors and camera are dropped; its `next` is queued first, for
  the same step, or else the step is marked `Ended`; the next queued scene may start,
  queued feeds and calls go on as before.
- **A fight mid-scene** (`st.net.engaged` becomes `Some`) skips the scene.
- **Logging.** `` `Castellan` · “Sit down…” `` as each line starts (the label as
  written, `as` if set, your handle for `you`); stage lines are not logged.

## 3. The camera

`MapScreen::update` follows the player as now unless a scene holds a mark: then the
camera targets that tile (`Camera::target`, centred, eased as usual). `Camera(You)`
ends the hold.

## 4. Visitors (`missions/visitors.rs`)

- `Visitor { role, place: Place, pos: Pos }`, in `missions.visitors`, in the order they
  arrived. `Arrive` of a visitor already there moves them.
- **Drawn** after people and before the player: `@` in the role's tint (DIM without
  one), bold, only on `Visible` tiles of the player's place.
- **Look** reads a visitor like a person: the name, the title, the role's `look`.
- **Blocking**: a walk step into a visitor doesn't move you; the log says
  `` `Castellan` is in the way ``. Visitors are never in the sim, so jobs, Talk and the
  inspector don't see them.

## 5. Film (`missions/film.rs`)

Drawn instead of the explore frame while a film scene runs:

- The whole frame black (`Rgb(0,0,0)`).
- The map at its usual size (`layout(..).map`), moved to the middle of the frame
  (`x = (W − w) / 2`) and up to the frame's row 3 (`film::map_rect`), drawn as usual
  (map, people, visitors, bodies, the player), then the scene's overlays: lights,
  doors, the speaker's marker.
- **Title**: the scene's title upper case, magenta at 60 %, bold, on the frame's second
  row at the map's left edge; a blank row under it, then the map.
- **Stage line** under the map, after a blank row (the map's bottom + 1): dim (`DIM`),
  italic, centred in the frame, cut with `…` at the frame's width less two.
- **Subtitle** under the stage line's row: the label in the speaker's colour, bold, two
  spaces, the typed text in white; wrapped to the map's width less the label,
  continuation lines indented under the text; up to three lines (more are cut with
  `…`), the block centred in the frame, starting on the map's bottom + 2, whether or
  not a stage line shows. While a line types, a `▌` in the speaker's colour follows
  it.
- **Hint**: `space next · esc skip` in `FAINT`, last row, right-aligned to the map's
  right edge.
- **Marker**: `▾` in the speaker's colour, bold, on the cell above the line's anchor
  when that cell is on the map and no actor stands on it (the voice behind the door
  never covers you on the walkway).
- What a scene writes over map cells (the marker, an open door, the bubbles' stage line,
  tail and text) sets its whole style: nothing of the cell under it (bold, dim) shows.
- **Keypad**: when a `Keypad` beat shows its pad, the map washes (`ui::dim::wash` on
  the map's rect) and the pad draws over the frame as the real one does.
- No HUD, no side panel, no footer, no toasts, no call card.

## 6. Bubbles (`missions/bubbles.rs`)

Drawn after the explore frame while a bubbles scene runs:

- The top band and the side panel washed (`ui::dim::wash` on their rects); the footer's
  four rows cleared to `PANEL_BG`: the first reads `cinematic · <title>` in magenta at
  80 %, bold; the last `space next   esc skip` in `DIM`.
- The stage line at the map's top-left (`map.x + 1, map.y + 1`): ` ▸ <stage> ` in
  `DIM` italic on `PANEL_BG`, cut at the map's width.
- **The bubble**: width `min(34, map.w − 2)`, the text wrapped to width − 4, height
  lines + 2; a rounded box (`╭ ─ ╮ │ ╰ ╯`) in the speaker's colour on `PANEL_BG`, the
  label bold in the top border (`╭─ CASTELLAN ───╮`), the text in `TEXT`, a `▌` in the
  speaker's colour after it while it types.
- **Placement** around the anchor's screen cell `(sx, sy)`: above (bottom border on
  `sy − 2`, a `▾` tail on `(sx, sy − 1)`) or else below (top border on `sy + 2`, a `▴`
  tail on `(sx, sy + 1)`), horizontally `x = sx − w / 2` clamped into the map; a spot
  is taken if it fits inside the map and neither it nor its tail covers the player's
  cell or an actor's. With no anchor on screen or no spot, the bubble sits in the map's bottom-left
  corner with no tail.
- **Keypad**: as in film, the map washes and the pad draws over it, then the bubble.

## 7. The scripted keypad (`screens/keypad.rs`)

- `Keypad::scripted(st, building, level, number) -> Keypad`: the pad of that flat with
  nothing typed.
- `Keypad::show_digit(&mut self, d: u8)` adds a digit to the display (at most four);
  `Keypad::show_verdict(&mut self, open: bool, tick: u64)` flashes OPEN or DENIED as the
  real pad does. Neither calls `credentials`: no journal entry, no lockout, no log.
- The scene keeps the pad in `PadRun { pad, digits: Vec<u8>, next: u64, step_ticks,
  hold_ticks, verdict_at: Option<u64> }` and draws it with `Screen::draw`.
- `code: Journal` types the code the journal knows for that flat (the home code
  `credentials::move_in` records); none known types `0000`.

## 8. Doors and lights

- An open door (scene `doors`) draws as `·` in amber `(255, 200, 80)` on amber at 25 %,
  and the three cells across and three deep on its inside (the side away from
  `HomeOutside`) take an amber background at 12 %, 9 %, 6 % by depth: the walkway's
  light falling in.
- `missions.lights` below 1.0 scales the colours (fg and bg) of every cell whose tile
  lies in your flat's rect, its walls included, by the level; visitors and the player
  are drawn after and stay bright. `Lights(1.0)` (or a new city) puts it back.

## 9. Keys, ticks, fights

- The scene's key handling goes before `missions::call::key` in the per-key loop.
- Timing runs on the clock tick, so a scene pauses with the world (it can't be paused
  from inside one; `esc` skips).
- Feeds queued during a scene push after it ends; calls queue behind it.

## 10. Dev console

- `story` lists scenes (`scene house_call · film · 27 beats`) after the missions.
- `story scene <id>` plays the scene now, from wherever you are (marks that need the
  home resolve against your home; you may be put on its floor).
- `story skip` skips the running scene.

## 11. What changes in the drop

- Role `dez` becomes `mezz` (name "Mezz"); the feed and call text use `{mezz}`.
- `the_drop`'s last step: `Step(id: "home", when: At(HomeDoor), then: [Log("`The
  drop` · home")])`. The "lay low" mail moves to the house call.

## 12. The house call (content)

Cast (beside `mezz` and `marlo`):

```ron
"castellan": Role(name: "Castellan", title: "stranger", pick: Visitor, face: None,
    tint: Some((255, 200, 80)),
    look: Some("A tall man in a long coat, sitting like he owns the place.")),
"seren": Role(name: "Seren", title: "netrunner", pick: Visitor, face: None,
    tint: Some((255, 0, 200)),
    look: Some("Striking. A black netsuit, your rig's cable still at her neck.")),
```

Mission:

```ron
Mission(id: "house_call", title: "House call", from: "castellan", steps: [
    Step(id: "door", when: MissionDone(mission: "the_drop", minutes: 0), then: [
        Scene("house_call"),
    ]),
    Step(id: "rest", when: Ended("door"), then: [
        Objective("Sleep · {castellan} will be in touch"),
    ]),
    Step(id: "lay_low", when: After(minutes: 2), then: [
        Mail(from: "mezz", subject: "lay low", body: [
            "You made it home. Good.",
            "Keep the package somewhere they won't look, and don't open it.",
            "Lay low. I'll call.",
        ]),
        Log("`House call` · `{castellan}` will be in touch"),
    ]),
]),
```

Scenes:

```ron
"house_call": Scene(title: "House call", look: Film, next: Some("seren_aside"),
    pace: Pace(type: 45, walk: 0.3, hold: Some(2.5)), beats: [
    Arrive(role: "you", at: HomeOutside),
    Lights(0.45),
    Arrive(role: "castellan", at: HomeFurniture(Couch)),
    Arrive(role: "seren", at: HomeRig),
    Stage("You key your code. The pad buzzes red."),
    Keypad(door: HomeDoor, code: Journal, result: Denied, keys: 0.22, hold: 0.8),
    Say(who: "you", text: "WTF…?"),
    Stage("The lock clicks from inside. The door slides open on a dark flat."),
    Door(at: HomeDoor, open: true),
    Wait(0.6),
    Say(who: "castellan", as: Some("A voice"), at: Some(HomeDoor), text: "Come in, {you}."),
    Stage("You step in."),
    Walk(who: "you", to: HomeInside(2)),
    Camera(HomeInside(2)),
    Stage("A man sits on your couch, waiting."),
    Say(who: "castellan", text: "Sit down. It's your couch, but sit anyway."),
    Say(who: "you", text: "Who the hell are you?"),
    Say(who: "castellan", text: "Everyone in this city has a file. Yours is three days old and empty."),
    Say(who: "castellan", text: "That makes you the most interesting person on this block."),
    Stage("From the bedroom, a woman, the rig's cable still at her neck."),
    Walk(who: "seren", to: Beside(You)),
    Say(who: "seren", text: "Your fixer's line leaks. I was on it the whole walk home. Sorry about the static."),
    Say(who: "you", text: "{mezz} set me up."),
    Say(who: "castellan", text: "{mezz} was told the job was clean, and believed it. That's why it worked."),
    Say(who: "you", text: "And {marlo}?"),
    Say(who: "castellan", hold: Some(4.0), text: "{marlo} sold names to the badges. Somebody was always going to put him on the pavement. I chose the night, and where you'd be standing."),
    Say(who: "castellan", text: "You walked. You kept your mouth shut. You came home. You didn't open it."),
    Say(who: "castellan", text: "Keep the package. There's a shard in it, a fixer's ledger. Old news, but its owner will pay to have it back."),
    Say(who: "seren", text: "Your deck's a toaster. I fixed what I could."),
    Say(who: "castellan", text: "Sleep. I'll be in touch."),
    Stage("He lets himself out."),
    Walk(who: "castellan", to: HomeOutside),
    Leave("castellan"),
    Door(at: HomeDoor, open: false),
]),
"seren_aside": Scene(title: "House call", look: Bubbles,
    pace: Pace(type: 50, walk: 0.3, hold: Some(2.5)), beats: [
    Stage("Seren unplugs and makes for the door."),
    Walk(who: "seren", to: HomeInside(1)),
    Say(who: "seren", text: "Your lock's yours again. Same code."),
    Say(who: "seren", text: "{mezz} didn't sell you. Remember that when he calls."),
    Stage("She flicks your lights on on her way out."),
    Lights(1.0),
    Walk(who: "seren", to: HomeOutside),
    Leave("seren"),
]),
```

## 13. Testing

- **Content**: the shipped book validates; each new validation error has a test
  (unknown scene, visitor slot `.place`, a role named `you`, `type: 0`, `HomeInside(0)`,
  `Walk` of a non-visitor, `level: 1.5`).
- **Marks**: on the shot city (seed 8, medium), every `Home*` mark resolves, the couch
  is in the flat, `HomeOutside` is walkable and outside the rect, `HomeInside(2)` is
  two in, `Beside(You)` is free and next to you.
- **Runtime**, on a scene built in the test: a line with `hold: Some(1.0)` moves on
  after typing plus 30 ticks; with no hold it waits for `space`; `space` finishes
  typing first; a walk takes `len × pace` ticks and moves the player's FOV; `space`
  ends a walk at its goal; a keypad types its digits on its ticks and leaves the journal
  and the lockout untouched; `esc` ends the scene with everyone on their goals, every
  line logged once, the step `Ended`; a scene queued in a fight waits; a fight mid-scene
  skips it; keys other than space, enter and esc do nothing (the player doesn't move).
- **The story**: finishing the drop at your door starts `house_call` with Castellan on
  the couch and Seren at the rig; skipping both scenes leaves no visitors, the lights at
  1.0, the objective "Sleep · Castellan will be in touch", and two minutes on the mail
  from Mezz.
- **Drawing**: film at 120×40 and 100×30 has no HUD row, the title, the stage line, the
  subtitle and the hint where §5 puts them; bubbles' bubble avoids the player's cell
  and lies inside the map; every new glyph (`▾ ▴ ╭ ╮ ╰ ╯ ▌`) passes
  `every_drawn_character_has_a_glyph`.
- **Shots** (`screens/shots.rs`, ignored, `SHOT_DIR`): `scene_film_keypad`,
  `scene_film_door`, `scene_film_couch`, `scene_film_seren` at 120×40 and
  `scene_film_couch_100x30`; `scene_bubbles_seren` and `scene_bubbles_seren_100x30`.

## 14. Adding a beat (the recipe)

1. A variant in `Beat` (content.rs) with its fields; serde does the RON.
2. Its checks in `Book::validate` (roles, marks, numbers).
3. In `scene.rs`: `start` (what happens when it begins), `ended` (when it is over),
   `finish` (its end state, for `space` and skip).
4. If it draws: a branch in `film.rs` and `bubbles.rs` (or the shared overlay).
5. A test in `scene.rs`'s tests and a line in §1's table.
