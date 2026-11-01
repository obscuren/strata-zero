# Wait and sleep: time that passes, and a mission that waits on it

The house call ends with "Sleep · Castellan will be in touch". This spec gives the
player a way to sleep through the night in their own bed, a window for choosing when
to wake that later also serves waiting anywhere, and the story step that fires on
elapsed time whether the player sleeps through it or stays up: Castellan's morning
call.

Mockups: `docs/superpowers/specs/2026-11-01-wait-mock/` (`gen.py`; round 1 drew A, a
list of choices, B, a timeline, and C, a big clock, each choosing and asleep).

## What Jeffrey decided

- **An interface for waiting**, opened by interacting with your bed; "it will later
  also work as 'Waiting' from anywhere in the game".
- **B, the timeline** (round 1).
- **The playback, as written** ("what you wrote is perfect. That's it. Time truly
  passes"):
  - Space beside the bed opens its window (`x Look`, `i Interact: Sleep`); Sleep
    opens the timeline.
  - The strip: the next 24 hours, two cells an hour, night blue, day amber, past grey,
    the sleep teal, markers for now and the wake time; `h`/`l` an hour, `H`/`L` ten
    minutes; the length and what you wake to under it; `enter` sleeps, `esc` goes
    back. At night the wake time starts on 07:00.
  - Asleep: the window stays, titled Asleep, the strip filling; the flat dark, z's
    over you, the HUD clock racing; `esc` wakes you.
  - The window works for Wait as well; only the bed opens it in this build.
  - A night's sleep refills HP and FOCUS, a nap a share; waiting heals nothing.
  - A call, a knock, someone walking in or a fight wakes you early.
  - The city lives through the night in a few seconds; nothing jumps.
- **An elapsed-time trigger moves the mission on**, "both normal time and sleep time
  should trigger it. In fact sleeping is nothing more than advancing time more
  quickly."

## Calls I made

- **The morning beat is a call from Castellan** eight hours after Mezz's "lay low"
  mail: the elapsed time Jeffrey asked for, not a time of day. A house call over by
  23:00 has the phone wake you before the default 07:00; a later one rings soon after
  you wake; staying up, it rings at the same minute. Its three lines (§6) are mine,
  written to lead into the next part (the ripperdoc on Castellan's tab, then Seren's
  word) and to read right whether you slept or not; they are data and easy to
  rewrite.
- **The night runs at four game minutes a frame at most**: 8h in about 4 seconds at
  30 frames a second, longer when the city is busy (§3).
- **A night counts from 6 hours**: six hours or more refills HP and FOCUS; less gives
  that share (three hours, half).
- **The default wake time**: in the lamps' hours (18:00–05:59) the next 07:00; by day,
  8 hours on. Wait starts an hour on. The wake time can be set from 10 minutes to 20
  hours ahead, in ten-minute steps.
- **What wakes you**: the wake time; `esc`; a call coming up; a scene queued or
  playing; a HOLO feed queued; a fight; losing HP; a visitor arriving in your flat.
  A mail does not (it lands, as it would awake).
- **A dev console `wait`** opens the Wait window anywhere, so the Wait path can be
  tried before the game gives it a key.
- **Sleeping is not saved**: a save taken asleep loads awake, at the time it was
  saved.

## Builder rulings

A fresh builder read this spec against the code. Its three questions, settled:

- **The call can come after the default wake.** The game's start time includes the
  whole session before it (creation counts), and the drop and the house call take game
  time, so Mezz's mail lands anywhere from about 22:10 to past midnight. Ruling: keep
  elapsed time (Jeffrey's rule), make it eight hours (`After(minutes: 480)`), and word
  the first line so it fits a player woken by it, woken earlier, or never asleep.
- **A last step's objective is never seen.** A mission's last step marks it done, so
  an objective it sets shows ticked or not at all. Ruling: no `word` step; the call
  is `house_call`'s last step, the mission finishes when it ends, and the next part
  opens its own mission. Until the call, "Sleep · Castellan will be in touch" stays
  up through the night (today it goes two minutes after the scene).
- **Paths at four minutes a frame.** Awake the world turns up to 4 path requests
  into trips a frame, which is 360 a game minute. Ruling: asleep each minute gets the
  same 360, so the city's trips start when they would awake; a frame runs whole
  minutes until four are done or its time budget (16 ms) is spent, at least one, so
  a busy hour takes longer to sleep through and the game never hangs.

## Goals

- Sleep in your own bed, choosing when to wake on a timeline.
- Time passing asleep is the same time as awake, only faster: the city, the Net, jobs
  and the story all run through it, minute by minute.
- A mission step that waits on elapsed minutes fires at the same game minute whether
  you were awake or asleep, and a step that needs you (a call, a scene) wakes you.
- The window serves Wait too, so a later key or verb opens it anywhere.

## Non-goals

- A key or verb for waiting anywhere (later; the console's `wait` stands in).
- Other beds (hotels, a friend's couch), sleeping rough, being robbed asleep.
- Hunger, fatigue or any need that sleep answers besides HP and FOCUS.
- The next part of the quest past Castellan's call (the next spec).

## Anti-goals

- Skipping time: no reseed, no jump of the world's clock. If the city would not have
  done it awake in those minutes, it does not happen asleep.
- A night that hangs the game: a frame runs a bounded number of minutes.
- A call that rings out to voicemail while you sleep through it.

## 1. The bed's window

`interact::gather` offers the bed when you are in your own flat (`st.home`, its
`rect`) and a bed tile of it (`Feature::Furniture(Furniture::Bed)`) is within reach,
as other things are. One subject for the bed, at the bed tile nearest you: header
`Bed` with the tile's mark, and one row, `Choice::on('i', "Interact", "Sleep", CYAN,
Pick::Sleep)`. Look is added by `window` as for every subject.

`Pick::Sleep` replaces the popup with the Rest window for Sleep (§2).

## 2. The Rest window (choosing)

A screen pushed over the map (`ScreenCmd::Replace` from the popup), so the stack washes
what is under it. It draws one window in the game's window style (`┍━ ▤ Sleep ━┑`, light
sides, `┕━┙`, `CYAN` frame, the title `TEAL` bold, the glyph `▤` for Sleep and `☾` for
Wait), 57 columns wide and 12 rows high, centred across the map
(`explore::layout(area, false).map`) with its bottom row on the map's last row, on
`PANEL_BG`. Rows from the top border (row 0):

- 2: hour labels, `FAINT`, every two hours at the hour's first cell (`20  22  00 …`).
- 3: the strip, 48 cells, two an hour, `█` in:
  - `#23262d` for the hours on the strip before now;
  - the sleep (now to the wake time) in `SEL_FG` (`Rgb(0, 165, 184)`);
  - otherwise night (the lamps' hours, 18:00–05:59) `#17213a`, day `#3d3318`.
  The strip starts on the hour two hours before now and runs 24 hours; a cell is in
  the sleep when any of its half hour is.
- 4: `▴` under now (`TEXT`, bold) and under the wake time (`SEL_FG`, bold).
- 5: `now 22:01` under the first, `wake 07:00` under the second (when the two labels
  would touch, the wake label moves right of the now label's end, one space between).
- 7: what it comes to, `DIM`: `8h 59m asleep · you wake rested` (Sleep, 6 hours or
  more), `2h 0m asleep · a nap, some HP and FOCUS back` (Sleep, less),
  `1h 0m waiting` (Wait).
- 9: the keys, `FAINT`: `h l an hour · H L ten minutes · enter sleep · esc` (`enter
  wait` for Wait).

Keys: `h`/`←` an hour earlier, `l`/`→` an hour later, `H` and `L` ten minutes, all
clamped to 10 minutes to 20 hours ahead; `enter` starts it (§3) and pops the window;
`esc` pops it and nothing happens. The wake time is held as a game minute and shown
rounded to ten minutes (the default is already on a ten).

The window fits 100×30 (the map is 59 wide there) and 120×40.

## 3. Asleep (and waiting)

`AppState` gains `rest: Option<Rest>` (`crate::rest`): the kind (Sleep or Wait), the
game minute it started and the wake minute, and what it needs to tell waking (your HP
then, the visitors in your flat then). It is not saved.

**Time passes.** The explore screen's frame step runs the city's clock (§7). While
`st.rest` is set it runs up to `rest::MINUTES_PER_FRAME` (4) minutes a frame instead of
the frame's ticks, one minute at a time, each minute the full step: the world (with a
path budget of 360 requests, what a minute awake allows: 4 a frame for 90 frames),
the bots, the Net's minute, backdoors, jobs, the story (`missions::tick`: conditions,
scenes, calls). After each minute it checks what wakes you (§4) and stops there. A
frame stops early, after a whole minute, once `rest::FRAME_BUDGET` (16 ms) has passed
since it began; it always runs at least one. No minute is skipped and none is run
twice.

**Keys.** While resting, `esc` wakes you and every other key does nothing; the dev
console's `:` still opens over it.

**Drawing.** The explore frame as usual, HUD and side panel bright (the clock races
in the HUD), with:
- the flat dark, as `Lights(0.4)` draws it (or darker, when the story's lights are
  lower) — Sleep only; waiting keeps the lights;
- `z` (`SEL_FG`) one cell up and right of your `@`, `Z` (`CYAN`, bold) two up and two
  right, where those cells are on the map — Sleep only;
- the Rest window from §2 in the same place over the map, title `Asleep` (or
  `Waiting`), the strip with the part slept in `CYAN` up to the time now and the rest
  of the sleep in `SEL_FG`, the now marker and label on the time now (`03:12`, in
  `CYAN`), row 7 `5h 11m slept · 3h 48m to go` (`5h 11m waited · …`), row 9
  `esc wakes you`.

## 4. Waking

Checked after every minute run asleep, in this order; the first that holds wakes you:

1. A fight (`st.net.engaged`).
2. Your HP below what it was when you lay down.
3. A call up (`st.missions.call`).
4. A scene playing or queued (`st.missions.scene`, `st.missions.scenes`).
5. A HOLO feed queued (`st.missions.feeds`).
6. A visitor in your flat who was not there when you lay down.
7. The wake minute reached.

`esc` wakes you at the minute it is.

On waking `st.rest` is cleared, then, for Sleep only, HP and FOCUS each rise by
`ceil(max × min(1, minutes slept / 360))`, capped at their max. One log line says what
happened: `You wake at 07:00, rested.` (a night), `You wake at 03:12.` (a nap or
`esc`), `A call wakes you at 06:47.`, `A fight wakes you.`, `Someone in the flat wakes
you.`, and for Wait `You wait until 07:00.` / `A call ends your wait at 06:47.`. A
night's line adds nothing for the healing; the HUD shows it.

## 5. Wait

The same window and the same running of time with the kind Wait: title `☾ Wait`, no
dark flat, no z's, no healing, the default an hour on, the lines in §2–§4 for Wait.
The dev console's `wait` opens it anywhere you stand (dev mode); `wait` is added to the
console's word list and help.

## 6. The story: elapsed time

`Trigger::After { minutes }` already counts game minutes since the step before
(`run.since`, from `st.net.now`, which is the world's tick). Asleep the world's tick
runs on, so it fires asleep at the same game minute it would awake; no new trigger is
needed. What this spec adds is that time can pass that fast, that a call or scene it
starts wakes you, and the step itself.

`house_call` gains one step after `lay_low`, its last:

```ron
Step(id: "morning", when: After(minutes: 480), then: [
    Call(from: "castellan", lines: [
        "Morning, {you}. Eight hours. That's all anyone gets from me.",
        "There's a doctor who owes me. She'll put something in you worth having, on my tab.",
        "Seren will send you the where. Don't keep her waiting.",
    ]),
]),
```

Castellan has no face; the call shows the silhouette, as any faceless caller's does.
Answered, ignored or sent to voicemail, the call ends the step and the house call is
done. Until then the mission is open and "Sleep · Castellan will be in touch" shows
through the night.

## 7. Code

- `crate::rest` (new, `rest.rs`): `Rest`, `Kind`, `MINUTES_PER_FRAME`, `FRAME_BUDGET`,
  the default wake minute, the clamp, `start(st, kind, wake)`, `frame(st)` (the asleep
  frame: the minutes, the checks, waking), `wake(st, why)`, `key(st, k)`, and the
  drawing of the window (shared by the choosing screen and asleep) and of the z's.
- `crate::screens::rest` (new): the choosing screen (`Screen`), opened by `Pick::Sleep`
  and the console's `wait`.
- The city's clock step leaves the explore frame for a function the frame and `rest`
  both call (`clock.rs`): given a number of world ticks (or none, for the free camera)
  and a path budget, it advances the world and paths, the bots, the Net's minute,
  backdoors, jobs and the story, as the frame does now (the frame passes its 4). What depends on the explore screen's own state
  (walks, aims, the camera, `set_blocker`) stays in the frame.
- `interact.rs`: the bed's subject; `popup.rs`: `Pick::Sleep`.
- `missions/visitors.rs` (the flat's lights): the dark flat while asleep.
- `flavor/missions.ron`: the step of §6.
- `screens/console.rs`: `wait`.

## 8. Testing

- The bed: in your flat beside the bed, Space offers `Bed` with `Interact: Sleep`;
  beside the rig no bed; a bed outside your flat offers nothing.
- The window: the default wake time at 22:01 is 07:00, at 13:00 is 21:00; `h`, `l`,
  `H`, `L` move it and clamp at 10 minutes and 20 hours; the strip's cells (past,
  sleep, night, day) at 22:01 to 07:00; `esc` pops with no rest; `enter` starts one.
- Time passes: asleep from 22:01 to 07:00 the world's tick advances by exactly the
  minutes run, 4 a frame with time to spare and at least 1 with none, and the clock
  reads 07:00 on waking; the jobs' clocks and the Net's minute moved the same.
- The story: a step `After(minutes: 30)` fires at the same game minute awake (frames
  at the normal rate) and asleep; a call it rings wakes you with the call up and
  ringing, not on voicemail.
- Waking: each cause in §4 wakes you with its line; `esc` mid-night; a scene queued
  wakes you before it starts.
- Healing: 6 hours from 10/45 HP refills; 3 hours from 10/45 ends at 33 (10 +
  `ceil(45 × 0.5)`); Wait heals nothing.
- The house call: after the house call and Mezz's mail, Castellan's call comes up 480
  game minutes after the mail, staying awake; asleep across that minute, it wakes you
  (`A call wakes you at hh:mm.`) at the same minute, ringing; when it ends the house
  call is done; until then its objective shows.
- Drawing: the choosing window and the asleep frame at 120×40 and 100×30 (the window
  inside the map; the z's, the dark flat); every new glyph passes
  `every_drawn_character_has_a_glyph`; shots `rest_shots` (choosing, asleep) for both
  front ends.
- The console's `wait` opens the Wait window; waiting an hour moves the clock an hour
  and heals nothing.
