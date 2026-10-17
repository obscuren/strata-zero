# The Access Terminal: design

Date: 2026-10-17. Status: built on `feat/access-terminal` (one screen, two
sessions; tiers from what you hold; the first mail is a neighbour's code and a
pickup). HELP lists every verb and tags the ones above your tier.
Builds on: `2026-10-17-credentials-journal-design.md` (cards, codes, the
journal), `2026-10-17-keypads-design.md` (the device kit, `ScreenCmd::Replace`),
`2026-10-15-constructs-and-programs-design.md` (hacks the Net state records),
`2026-10-17-megablock-home-design.md` (the rig).
Riff notes: `docs/ideas/2026-10-17-terminals-access-apartment.md` §2, §5.

## Intent

The second way into a terminal. Beside "jack in" there is now "access
terminal": a typed, MUTHUR-style session on the machine itself, in a beige CRT
over the map. It is slow and shallow where the Net is fast and deep: it sees
the district's buildings, tells you who is inside and what the ground floor
looks like, and, with the right standing, unlocks doors, answers lifts and
loops cameras, all through the same hack records the Net uses. Your own rig
runs the same shell at home, with your mail and your journal on the big
screen, and the first mail is the game's first job.

## Goals

### Reaching it
- Beside a street terminal, Space offers `jack in` and `access terminal`.
  Beside your rig, `jack in` and `access rig`. Both replace the popup with the
  terminal screen (`ScreenCmd::Replace`), like the keypad.

### The CRT
- A 72 by 24 overlay centred on the screen, built from the device kit: the
  bezel with a badge ("MUNI-OS" for a district terminal, "HOME RIG" for the
  rig) and an LED (amber idle, green while a reply types, red on a denial), a
  recessed near-black screen with phosphor text, a scanline tint on alternate
  rows (the text a shade darker there).
- Inside: a scrollback of the session (the last rows that fit), the prompt on
  the bottom row with a block cursor, the input line from the dev console
  widget (history with the arrows, Tab completion of verbs).
- **Boot**: the first time a terminal is powered on in a session it types
  three lines before the prompt: the OS line ("MUNI-OS 4.1 · LOWTOWN SUBNET"
  or "HOME RIG · STRATA/OS"), "MEMORY CHECK OK", "READY". A terminal is
  keyed by its tile (the rig by `home`); `Journal::booted: BTreeSet<(Place, Pos)>`.
- Replies **type out** at `TYPE_CPS` 60 characters a second; a key press
  finishes the reply at once. The LED is green while typing.
- The prompt names the session and the tier: `LOWTOWN NET >` before a
  connect, `HOLLIS COURT [USER] >` after, `RIG [HOME] >` on the rig.
- `Esc`, or `EXIT`, leaves. `HELP` lists the verbs this session allows at
  your tier, one a line with a short gloss. An unknown verb answers
  `?SYNTAX ERROR`; a verb above your tier answers
  `ACCESS DENIED · CLEARANCE: GUEST · REQUIRED: USER` and a second line with
  what it takes ("a resident's card or code" for USER, "a backdoor on the
  server" for ADMIN).

### Tiers
- `Tier { Guest, User, Admin }`, worked out per building, never typed:
  USER when the pack holds one of its keycards or the journal knows one of
  its door codes; ADMIN when the Net state has a backdoor on it. The rig is
  USER on your own megablock (HOME on the prompt). Tiers are ordered.

### District terminal commands
- `SCAN`: the buildings of the terminal's district (the subnet of the
  terminal's Net node), one a line: `1  HOLLIS COURT      apartment  sec 120
  ICE white  USER`. ICE grade from security: none below 160, white to 199,
  grey to 229, black from 230. Buildings without a construct (open kinds)
  are left out.
- `CONNECT 3` or `CONNECT HOLLIS` (a case-blind prefix of the name): picks
  the building; the prompt changes; `CONNECT` alone says which one is
  connected; `DISCONNECT` goes back to the district.
- GUEST, connected: `WHO` (everyone inside now: name, profession, role as
  "at work", "at home", "a guest", "visiting"); `PLAN` (the ground floor
  painted into sight memory as remembered tiles, "GROUND FLOOR PLAN LOADED"),
  `LOG` (the building's hacks recorded by the Net state, newest first, in the
  period voice: `02:14  DOOR LOCK  SPOOFED  (expires 03:14)`, `03:02  TRACE
  ALERT` when alert, "NO ENTRIES" when empty).
- USER: `DOOR UNLOCK` (a Spoof landed on the building's door lock node:
  locked doors open for an hour, as a Net spoof does), `LIFT` (a Spoof on
  the lift control: lifts answer during an alert), `CAM LOOP` (a Blind on
  every camera node of the building for the hour). Each answers in the
  period voice and the log says what you did ("`Hollis Court`'s locked doors
  open for an hour").
- ADMIN: `ALARM OFF` (ends the building's alert: `alert_until = now`; heat
  stays). Answers `ALARM CLEARED`.
- Every command that needs a connection answers `NOT CONNECTED · SCAN then
  CONNECT` without one.

### The rig
- Session `Rig`: the building is your megablock, tier HOME (USER). Verbs:
  `MAIL` (the inbox: `1  * DEZ  a favour` with `*` for unread), `MAIL 1`
  (the message, marking it read), `JOURNAL` (the journal's groups and lines,
  the backticks stripped), `SCAN` (your megablock alone), and the connected
  set (`WHO`, `PLAN`, `LOG`, `DOOR UNLOCK`, `LIFT`, `CAM LOOP`) on it.
  No `CONNECT`.

### The first mail
- At new game (`credentials::move_in`), with a home: the fixer is the city's
  person with profession `fixer` nearest the megablock's door, else nobody
  and no mail. A neighbour flat is picked by hash among the megablock's
  lived-in flats. `Journal::mails` gets one `Mail { from, subject, body,
  read: false, at }`:

  > from DEZ · subject: a favour
  > Welcome to the block, choom. 1207 on 12 belongs to a guy who owes me.
  > Door code 4471. There's a shard in there with my name on it. Bring it.
  > Dez.

  The journal learns the code with `Source::Mail` ("in a mail from `Dez`").
  A `paydata` shard is dropped on a floor tile of that flat (the floor is
  generated for it). A quest `first_favour` ("A favour for Dez", giver the
  fixer, location your block) is added to the character's quest log with two
  objectives: "Read the mail on your rig", "Pick up the shard in flat 1207".
  Reading the mail ticks the first (`st.story("▸ a favour · the mail is
  read")` via the story log); taking the shard (any Take of `paydata` on
  that flat's place and tile) ticks the second and the log says "the shard
  is yours · Dez will want it".

## Non-goals
- Terminal backdoors, suspicion meters, lockouts, logs that someone reads.
- Passwords and `LOGIN`. Tiers come from what you hold.
- Floor plans above the ground floor; camera feeds; guard rosters beyond
  `WHO`; lights; turrets.
- Selling the shard; the fixer answering. That is spec 3 of the Net line.
- Terminals inside buildings other than the rig.

## Architecture
- `ui/device.rs`: `crt(buf, area, badge, led)` draws the bezel and a screen
  rect inside it, returning the screen rect; `scanlines(buf, area)` darkens
  alternate rows.
- `access.rs` (new, crate root): the sessions. `Tier`, `tier_on(st, b) ->
  Tier`, `Session { District { subnet, district, connected: Option<BuildingId> },
  Rig { building } }`, `Reply { lines: Vec<String>, denied: bool }`,
  `run(st, &mut Session, line) -> Reply`, `verbs(session, tier) -> Vec<&str>`
  for HELP and completion, `boot_lines(session)`, `prompt(st, session)`,
  plus the command functions (`scan`, `who`, `plan`, `log`, `door_unlock`,
  `lift`, `cam_loop`, `alarm_off`, `mail`, `journal`).
- `screens/terminal.rs` (new): `Terminal` screen: the CRT, the scrollback,
  the typing reply, the console widget, `Terminal::open(st, session, key:
  (Place, Pos))`.
- `credentials.rs`: `Source::Mail`; `Mail`; `Journal::mails`, `booted`;
  `first_mail(st)` called by `move_in`; `shard_taken(st, place, pos)` hook
  called from `actions::perform` after a Take.
- `screens/popup.rs`: `Pick::Terminal(Session, (Place, Pos))`.
- `screens/explore.rs`: the two Space rows.
- `flavor/content.ron`: the `first_favour` quest.

## Testing
- `crt` draws badge, LED and a dark screen; scanlines darken alternate rows.
- `tier_on`: guest, user by card, user by code, admin by backdoor, home.
- `run`: SCAN lists the district's constructs with grades and tiers; CONNECT
  by number and by name prefix, unknown name refused; WHO lists the people
  inside with roles; PLAN paints the ground floor as remembered; LOG shows a
  landed hack and NO ENTRIES; DOOR UNLOCK at GUEST is denied with the
  wording, at USER lands a Spoof the Net reads as `unlocked`; LIFT and CAM
  LOOP likewise; ALARM OFF needs ADMIN and ends an alert; NOT CONNECTED;
  ?SYNTAX ERROR; HELP lists by tier; EXIT.
- Rig: MAIL lists and reads, marking read and ticking the quest; JOURNAL
  prints entries; SCAN shows the megablock; WHO works without CONNECT.
- First mail: the journal has the neighbour's code with source Mail, the
  shard lies in that flat, the quest is in the log; taking the shard ticks
  the second objective.
- Terminal screen: boots once per terminal, types a reply and a key finishes
  it, the prompt shows the tier, Esc leaves, a session walks SCAN, CONNECT,
  WHO on screen.
- Explore: the Space rows at a street terminal and at the rig, each firing a
  `Replace` with the terminal.
