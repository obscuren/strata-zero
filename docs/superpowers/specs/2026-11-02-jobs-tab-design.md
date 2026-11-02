# The JOBS tab: the fixers on the left, the job as a route on the right

Date: 2026-11-02. Mockups: `2026-11-02-jobs-tab-mock/`. `round1_*.png` holds the tab as it is
(`now`) and concepts A (ledger), B (next steps) and C (fixers). `round2_*.png` is the one
agreed: `t` (a job in hand), `t_ready` (one ready to hand in), `v_hours`, `v_steps`,
`x_offer`, `x_fixer`, `x_closed`, `x_story` and `x_verbs`, each also at 100 × 30
(`_100x30`).
- The generator is `jobs_mock.py`. `python3 -I jobs_mock.py shots <out>` writes the
  pages side by side, `jobs_round{1,2}.html`.
- It draws over `shots/`, the frames from the throwaway `jobs_tab_now` test
  (`shots_test.diff`): seed 8, the drop open, six jobs from three fixers, one in every
  state.
- The clock is moved on to 01:20.

## Why

Jeffrey: "I feel we can seriously improve it with UX and QL." The tab today
(`round1_now.png`) has these problems:

- **The list is titles only.** The tab shows no fixer, clock, pay or state beyond a
  mark. Offered and dropped jobs are the same DIM, barely apart from the jobs in hand.
  Finished jobs stay in the list forever, with no sections.
- **The card leaves out what matters.**
  - There's no due time, distance or next step.
  - It doesn't say whether you have the door code.
  - It doesn't say why the fixer will refuse the hand-in.
  - The map shows through between its words (`put_paragraph` only backs cells that
    aren't spaces).
- **Space does nothing**, although the hints offer it.
  - There's no drop, map or mail from the deck.
  - `t` on an offer only says "take it first".
- **Standing is invisible.** Nothing tells you what a fixer thinks of you, or what more
  standing would open up.
- **The fixer's hours are hidden.** Fixers hold their corners 18:00–04:00. After that
  they're at a bar or at home. They still take a hand-in, but you have to find them.
  Nothing says when they're where.
- **The story has no brief** beyond its objective.

## What Jeffrey decided

- **C's list and B's card, together** (`round2_t`):
  - The list on the left puts the story first, then each fixer with their live job and
    what they closed.
  - The card on the right draws the selected job as a route: from you, through each
    stop, to the fixer's corner, with its window as a bar.
- **"I want all of them":**
  - Variants: the fixer's hours on the window (`v_hours`), next steps in the list with
    each fixer's closed jobs folded (`v_steps`).
  - Additions: the offer card (`x_offer`), the fixer card (`x_fixer`), the closed card
    (`x_closed`), the story card (`x_story`), space on a row (`x_verbs`).
- **His wording fixes, all in the current mock:**
  - The story card loses "no clock · the story waits for you": no clock, no line.
  - The closed plant's "reported while it stood" was too vague. Each job uses one set of
    words, the words the game uses elsewhere (§10). The hand-in is "hand it in" /
    "handed in" for every kind. A backdoor is planted, then STABLE, DEGRADING or BURNED,
    as the deck's own backdoor card says.
  - The fixer card's "2h 40m left on it" is now "2h 40m left".
  - "t track" was not clear enough. It's now:
    - a key cap with what it does ("`t` TRACK · the floor lights the way to flat 101");
    - while tracked, a lit band: "⌖ TRACKING flat 101 · 51 m · the floor lights the way",
      with a `t` stop key;
    - a cyan ⌖ on the tracked row in the list.

## Goals

- Opening the tab answers three questions at a glance: what's in hand, what to do next,
  and how long is left. The list alone says it (clock, READY, the next step); the card
  says it in full.
- Each fixer reads as a relationship: their standing, what it opens, their record, where
  and when to find them.
- Every row can be acted on from the deck: track, show on the MAP, read the job's mail,
  drop.
- One set of words for jobs throughout the game: on the cards, the objectives, the
  sidebar's JOBS pane, the log and the mails (§10).
- Both front ends, from 100 × 30 up: exact `Rgb` colours, every glyph in the window's
  fonts.

## Non-goals

- Taking or handing in a job from the deck. That still happens face to face with the
  fixer. The deck only tracks the way there.
- New job kinds, pay, standing rules or deadlines. Only the words, the hand-in hours and
  what the tab shows change.
- The rest of the deck's cards. Their `put_paragraph` gap is a separate fix (Left to the
  builder).
- The rig's mail program. The deck shows a job's mails; the rig still holds every mail.

## Anti-goals

- A dashboard of numbers. Standing shows as a meter and a word in the list. The
  number appears only on the fixer card's ladder.
- A list that grows forever. Closed jobs fold under their fixer.
- A verb that changes a job without a confirm. Drop asks first, as it does at the fixer.

## Design

Colours below are the chrome's (`ui/chrome.rs`: CYAN, MAGENTA, DIM, FAINT, FAINTER, TEXT,
RULE, PANEL_BG, BAR_BG) and the HUD's (`ui/hud.rs`: AMBER, DIM_AMBER). DONE is the deck's
done tone, `Rgb(90, 200, 140)`.

### 1. The tab

The JOBS tab draws itself, like GEAR & INVENTORY, ATTRIBUTES and the WORKBENCH. It owns
its rows, keys, card and hints. The deck only hands it the body.

- **Layout.** The body splits as today: the list is `max(width × 2/5, 28)` wide, then
  the `║` separator in RULE, then the card with a cell of margin each side.
- **Backing.** Every line the card draws is backed in PANEL_BG across the card's full
  width, spaces included. The washed map shows only below the card's last line.
- **Typewriter.** The card types on as the deck's cards do (`reveal`, from the tick the
  row or tab changed). The list never types.

### 2. The list

Top to bottom (`round2_t`, `round2_v_steps`):

- **STORY**: a section header (DIM label, RULE line to the pane's edge).
  - Each open mission is a row: its title, with the distance to its step on the right
    ("30 m", DIM) when the step has a place to walk to.
  - Under each open mission, a next-step line as under a live job (item 3 below): "{m} m
    · {next}".
  - Done missions follow as `✓` rows in DONE.
  - The section is missing while no mission has started.
- **FIXERS · on their corners till 04:00**: a header.
  - The tail comes from the listed fixers' hours (`World::work_hours`), when they share
    one end time.
  - On shift it says "on their corners till {end}"; off shift, "off their corners till
    {start}".
  - Narrower than 44 cells it shortens to "· till {end}" / "· off till {start}".
  - Without a shared end time, the header is just "FIXERS".
- **Each fixer who has given you any job**, in this order: fixers with a job in hand (the
  soonest due first), then those with an offer, then the rest, ties by their first
  job's id. Each fixer has:
  1. **The fixer's row.**
     - The name in capitals, bold WHITE (`Rgb(255, 255, 255)`), then " · {district}" in
       DIM when it fits.
     - On the right, the standing meter: eight cells, `▮` in CYAN for each point of
       `clamp(standing, 0, 8)`, `▯` in FAINTER for the rest.
     - It's selectable: it opens the fixer card (§7).
  2. **Each live job of theirs**, indented two:
     - A job in hand (TEXT): on the right, READY (CYAN, bold) when the hand-in would be
       taken now, then the clock left (`clock_text`, MAGENTA under `WARN_MINUTES`), then
       "¤{pay}" in DIM.
     - An offer (DIM title): "offer" in AMBER, then "¤{pay}".
     - Narrower than 44 cells, the pay of a job in hand drops first.
     - The mark column holds a cyan, bold `⌖` while that job is the track.
  3. **The next step** under each live job, indented further (DIM, not selectable):
     - In hand: "{m} m · {next}" from the walk to its current stop (`route::next_text`).
       For the hand-in leg: "{m} m · hand in to {fixer}".
     - Offered: "take it from {fixer} · {m} m".
     - Tracked: the line leads with a CYAN `⌖ `.
     - Jacked in: "jacked in". No way there: "no way from here".
  4. **Their closed jobs**, folded:
     - One row, "{n} closed", with "→ unfold" in FAINTER on the right.
     - `→` (or `l`) on it, or on the fixer's row, unfolds it. `←` (or `h`) folds it back.
     - Unfolded, each closed job is a row, newest first, in FAINT: `✓` in DONE with
       "done", `✗` in MAGENTA with "late", `–` in FAINT with "dropped".
     - The fold is kept per fixer for the session.
     - A fixer with no closed jobs has no such row.
  5. A blank row between fixers. At 100 × 30, once next steps make the list taller than
     the pane, the blank rows go; the list scrolls as the deck's lists do.
- **No story and no fixers yet:** the list shows one row, "no work yet · find a fixer"
  (DIM, not selectable), and the card is empty.
- **The cursor** is the deck's: a BAR_BG bar with the lit text and `▸`. It skips headers
  and next-step lines. It starts on the first live job, else the first selectable row.

### 3. The job card: the route

The card for a job in hand (`round2_t`, `round2_t_ready`). The route's `$` stop is the
fixer's corner in every kind.

**The title line:**
- The kind's label in capitals, MAGENTA, bold: FETCH, COPY, PLANT, DELIVER.
- " ━ " in RULE, then the title in capitals, CYAN, bold.
- A RULE `━` fill.
- " ¤{pay}" in TEXT, right-aligned.

**Under it**, in DIM: "for {fixer} · {district} · {standing word}".

| standing | word |
| --- | --- |
| ≤ REFUSE_AT | refuses you |
| < 0 | cold |
| < 2 | new |
| < 4 | known |
| < 6 | trusted |
| ≥ 6 | inner circle |

**The route.** One line per stop, two cells in.
- The glyph sits in its own column, bold. The stop's text follows two cells on. Its verb
  is right-aligned in TEXT.
- `@` (WHITE) is you: "you · {place} · by {nearest building}" (or "· in {building}"
  indoors).
- `@` sits just above the first stop not yet done. Below it, in CYAN, a `┃` line:
  "{m} m · {next}", the walk to that stop.
- Stops already done sit above `@`, each with a `✓` in DONE and the done word in FAINT.
  Stops still to come sit below the current one, joined by `┊` lines in FAINT.

Each kind's stops:

| kind | target stop | glyph | to do | done | the line under it |
| --- | --- | --- | --- | --- | --- |
| Fetch | flat {n} · {floor} · {building} | `◆` MAGENTA | take the shard | shard taken | the door (below) |
| Copy | {host} · security {s} | `◆` MAGENTA | copy its server / copy its vault | server copied / vault copied | "in the Net" |
| Plant | {host} · security {s} | `◆` MAGENTA | plant a backdoor | backdoor planted | the backdoor (below) |
| Deliver | {name} · {workplace} | `◆` MAGENTA | deliver the package | package delivered | "{profession} · {hours}" |
| hand-in | {fixer} · {district} corner | `$` AMBER | hand it in | handed in | the corner (below) |

**The lines under a stop**, in DIM unless noted:
- **The door** (Fetch), from `credentials::opens`:
  - `Code(c)`: "door code {c:04}" in DONE.
  - `Keycard`: "you hold its keycard" in DONE.
  - `Cracked`: "the lock is cracked" in DONE.
  - None: "door code not known · {how}" in MAGENTA, where `how` is the fetch's own hint
    (`how_server` or `how_card`, the way `finish_fetch` picks it).
- **The backdoor** (Plant), while the stop isn't done: "hand it in before the backdoor
  burns". Once planted: "backdoor {HEALTH}", from `Backdoor::health`, in DONE while
  STABLE, AMBER while DEGRADING, MAGENTA once LIKELY BURNED or BURNED.
- **The corner** (hand-in), from the fixer's shift and the clock:
  - On shift: "on the corner till {end}".
  - Off shift: "off the corner · back at {start}" in AMBER.
  - Off the corner, the fixer still takes the hand-in wherever you find them, and the
    walk and the track follow them there.
- **READY.** The hand-in's verb column says READY (CYAN, bold) in place of "hand it in"
  when both hold:
  - `ready` is Ok;
  - the fixer would deal with you now. That means not refused, as `jobs::ui::rows`
    decides, which includes `fixer_quiet` after a raid.
- **The reason.** When every stop before the hand-in is done but the hand-in would fail,
  the reason shows under the corner line in MAGENTA:
  - `ready`'s reason (§10);
  - or "{fixer} won't deal with you now", for a fixer who won't.

**Below the route**, after a blank line:
- the window (§4);
- a blank line, then the brief in DIM, quoted (`“` … `”`), wrapped, each later line one
  cell in;
- a blank line, then the track line (§5).

### 4. The window

**Line 1.**
- "window  " in FAINT.
- A bar across the rest of the card: `━` in FAINTER from taken to now, `╋` in WHITE
  (bold) at now, then `━` in CYAN to the due time.

**Line 2.** "taken {HH:MM}" in FAINT under the bar's start. At the right: "now {HH:MM} ·
" in DIM, "{left} left" in TEXT, " · due {HH:MM}" in DIM.

**The fixer's hours** (`round2_v_hours`) are a heads-up, not a deadline. A fixer takes
the hand-in wherever you meet them, on the corner or off it: `jobs::ui::rows` checks no
clock, and the track's hand-in target is the fixer, wherever they are. The window shows
where to look.

- **The dashes.** Every bar cell whose minute falls outside the fixer's shift, from now
  to the due time, is drawn in `┄`, DIM_AMBER.
- **The fixer leaves before the due time and isn't back by it** (`jobs::corner` gives
  `Leaves(H)`):
  - line 2 puts "{H}" under the bar where the dashes begin;
  - a third line says "off the corner from {H}" in AMBER, then " · {fixer} is back at
    {start}" in DIM. The tail drops when it would overflow.
- **The fixer is off the corner now and not back by the due time** (`Corner::Off`): the
  third line says "off the corner now" in AMBER, then " · back at {start}, after it's
  due" in DIM.
- **The fixer is on the corner at the due time** (`Corner::Through`): no third line. Any
  dashes are only the stretches before.

The list's clock counts to the due time, as the take mail's "By {due}." does.

### 5. The track line

The card's last line, on every card you can track (job in hand, offer, fixer, story).

**Not tracked:** " t " as a key cap (PANEL_BG on CYAN, bold), then "  TRACK" (CYAN, bold),
then " · the floor lights the way to {where}" in DIM. It shortens to " · lights the way to
{where}" when that doesn't fit.

**Tracked** (`round2_t`):
- The whole line on BAR_BG: "⌖ TRACKING" (CYAN, bold), then "  {where} · {m} m · the
  floor lights the way" in TEXT.
- That shortens to "  {where} · {m} m" when it doesn't fit.
- At the right, the " t " cap, then " stop" in DIM.
- {m} is the track's own distance (`track::distance`). Jacked in or with no way, the
  distance gives way to the track's reason ("jacked in", "no way from here"), as the
  JOBS pane says it.

**{where}** is the current stop as the route names it, kept short:
- "flat {n}" for a fetch;
- the host for a copy or plant;
- the person for a delivery or a story step;
- "{fixer}'s corner" for a hand-in while the fixer is on shift, else "{fixer}".

### 6. The offer card

`round2_x_offer`:
- The title line as a job's.
- Under it: "on offer from {fixer} · {district} · {standing word}".
- **The route starts at the fixer:**
  - `@` you;
  - the `┃` walk to the fixer;
  - `$` "{fixer} · {district} corner" with "take the job", and under it the corner line.
- Then the job's own stops and its hand-in, in FAINT with their glyphs in FAINT. Each
  follows the one before after a blank `┊` line.
- The window: "window  " in FAINT, "{hours} hours" in TEXT, ", from when you take it" in
  DIM.
- The brief, quoted.
- A line in FAINT naming the standing that opened this kind of work: "{kind} work ·
  {fixer} offers it from standing {n} · you are at {standing}". Fetch and deliver work
  come from REFUSE_AT + 1; copy from COPY_AT; a vault copy from VAULT_AT; plant from
  PLANT_AT.
- The track line: `t` tracks the fixer (§9), so {where} is the fixer's corner.

### 7. The fixer card

`round2_x_fixer`:
- **The title line.** "FIXER" in AMBER, bold, then " ━ ", the name in CYAN, bold, the
  fill, and the standing word on the right.
- **Under it**, in DIM: "{district} corner · {start}–{end} · on it now" (or "· off it till
  {start}").
- **The route:** `@` you, the `┃` walk to the fixer, then `$` "{fixer} · {district}
  corner". On shift, its right column says "{left} left" (minutes until the shift ends,
  `clock_text`). Off shift it says "back at {start}".
- **Standing:**
  - "standing " in DIM, the number in WHITE (bold), " · {word}" in CYAN.
  - Then the ladder: a `━` bar from REFUSE_AT to 8. `┿` marks each threshold: REFUSE_AT,
    REFUSE_AT + 1, COPY_AT, PLANT_AT, VAULT_AT. Everything up to the standing is CYAN,
    the rest FAINTER.
  - Under it, labels in FAINT at the thresholds: "refuses", "fetch", "copy", "plant",
    "vault".
  - Under those, a `▲` in WHITE at the standing (clamped to the ladder's ends).
  - Then "done +1 · dropped −1 · late −2" in FAINTER, from GAIN_DONE, LOSS_DROP and
    LOSS_LATE.
  - Labels never overlap. When "refuses" and "fetch" are too close at this width,
    "refuses" moves to the left of the bar's first `┿`.
- **The record.**
  - "record  " in FAINT, then "{n} done" in DONE, " · {n} late · {n} dropped · " in DIM,
    then "¤{sum}" in TEXT and " paid" in DIM. The sum is the pay of the done jobs.
  - Then "in hand " with the title and its clock, or "on offer" with the offer's title
    and pay. The line is missing when neither exists.
  - Then "said    " with their last line in DIM, quoted. That's the flavor line for the
    latest thing that happened between you: Taken, Handed, Dropped or Late. Each is
    keyed by the job's id, as the log line used it, and "latest" is decided by
    `Job.taken` and `Job.closed` (§11).
- **The track line:** `t` tracks the fixer.

### 8. The closed card

`round2_x_closed`:
- The title line as a job's.
- Under it: "for {fixer} · {district} · done" (or "late", "dropped").
- **The route:** every stop with its mark and done word, the `┊` between them.
  - A late or dropped job ticks only the stops it reached. The rest keep their FAINT
    glyph and to-do word.
  - The hand-in reads "handed in" on a done job.
- **The outcome**, after a blank line:
  - Done: "PAID " in DONE (bold), "¤{pay}" in WHITE (bold), then " · standing +1 →
    {after}" in DIM.
  - Late: "LATE" in MAGENTA (bold), " · standing −2 → {after}".
  - Dropped: "DROPPED" in FAINT (bold), " · standing −1 → {after}".
- **What it left you.** A done Plant adds "yours   " in FAINT, then "the backdoor on
  {host} · health " in DIM, then the health label in TEXT.
  - No backdoor records its job, so this is the newest backdoor on that building in
    `backdoors.list`, burned ones included.
  - A burned one reads "BURNED · {why}" in MAGENTA.
  - With none, the line is left out.
- **The brief**, quoted.
- **No track line.** A closed job has nothing to walk to.

### 9. The story card

`round2_x_story`:
- The title line: "STORY" in MAGENTA, bold, the title in CYAN, bold, and no pay.
- Under it, in DIM: "from {who} · open" (or "· done").
- **The route:**
  - First, the mission's earlier objectives (`run.objectives`, all but the last). Each is
    a `✓` stop in DONE with its text in FAINT, joined by `┊` lines.
  - Then `@` you, and the `┃` walk to the step's place (the story track's target).
  - Then `◆` in MAGENTA with the step's objective text as the mission writes it, wrapped
    under itself. There's no verb column: the mission's own text says what to do.
  - A step with no place to walk to shows only its `◆` line.
- **A done mission:** every objective is a `✓` stop, with no `@`, no walk and no track
  line.
- **No window.**
- The track line, on an open mission.

### 10. Keys, verbs and words

**Keys on the tab:**

| key | does |
| --- | --- |
| `↑` `↓` (`k` `j`) | move the cursor |
| `←` `→` (`h` `l`) | fold or unfold the selected fixer's closed jobs |
| `t` | track the selected row, or stop |
| `m` | show it on the MAP |
| space, enter | the row's verbs |
| `Q` `E`, digits, tab, esc | the deck's |

The hint reads "Q/E tabs · ↑↓ select · ←→ fold · space verbs · t track · m map · tab
close".

**What `t` tracks:**
- a job in hand: its first stop not done (`job_source`, as today);
- an offer or a fixer row: the fixer, as the MAP's fixer listing does
  (`Source::Listing(ListingId::Person(fixer))`);
- the story: its step (`story_source`);
- a closed job: nothing, silently.

Refusals ("you're there", "no way there from here") land in the strip, as today.

**What `m` does.** It switches to the MAP tab with the cursor on the row's listing: the
job (`ListingId::Job`), the story (`ListingId::Story`) or the fixer
(`ListingId::Person`). The view is centred there, as `[` `]` leave it on a listing.
Today a visit to MAP starts its cursor on the track or on you, so this placement has to
happen after that reset. A kind filter that would hide the listing is cleared.

The MAP shows only its own stratum (`atlas::map_stratum`). When the row's listing isn't
on it (a fixer seen from the Heights, say), or the row has no listing at all (a closed
job, a story step with nowhere), the tab stays on JOBS and "not on the map from here"
goes to the strip.

**Space** opens the row's verbs as a popup anchored on the row, in the deck's popup style
(`round2_x_verbs`). Each choice is its key, its name and a tail:

| row | verbs |
| --- | --- |
| job in hand | `t` Track (Stop tracking, with a `⌖` tail, while it is the track) · `m` Show on the MAP · `r` Read the mail ({n}) · `d` Drop |
| offer | `t` Track {fixer} · `m` Show on the MAP |
| fixer | `t` Track {fixer} · `m` Show on the MAP · `r` Read {fixer}'s mail ({n}) |
| story | `t` Track · `m` Show on the MAP |
| closed job | `r` Read the mail ({n}) |

- `d` Drop opens the existing ask, "Drop {title}?" with y and n (`drop_confirm`), then
  runs `jobs::ui::drop`.
- `r` shows the job's mails (or all of that fixer's job mails) as a card popup, newest
  first: "From", "Subject", then the body. They're marked read as the rig marks them
  (`credentials::mail_read`). A job's mails are the ones it sent: the take and the
  late notice (§11). Story mails never show here, even from a fixer who is also in the
  story. With no mails, the verb is disabled with " · no mail".

**The words.** Every place a job speaks uses these:

| what | words |
| --- | --- |
| an offer becomes a job | take the job |
| each kind's stop, to do / done | take the shard / shard taken · copy its server (vault) / server (vault) copied · plant a backdoor / backdoor planted · deliver the package / package delivered |
| the last stop of every job | hand it in / handed in, at "{fixer}'s corner" |
| a backdoor | planted; then STABLE, DEGRADING, LIKELY BURNED, BURNED (`Health::label`); "burns" as a verb |
| the time | the window; taken, now, left, due; "on the corner till", "off the corner from" |
| a closed job | done (paid), late, dropped |

**The objective texts follow** (`jobs/generate.rs`). They show in the sidebar's JOBS
pane and in the log.
- **Every kind's hand-in**, Plant included: "Hand it in to {fixer} · {district} corner ·
  {start}–{end}". The hours come from `World::work_hours` through `hours_text`.
  - This replaces Fetch, Copy and Deliver's "Bring it to …", which hard-codes
    "18:00–04:00".
  - It also replaces Plant's "Report to {fixer} while it stands".
- **Deliver's first objective**: "Deliver the package to {name} · {place} · {hours}"
  (was "Hand the package to …").
- **Fetch, Copy and Plant's first objectives** don't change.

**The hand-in reasons** (`Jobs::ready`) stay, except Plant's undone stop: "the backdoor is
not planted yet" (was "no backdoor stands there yet").

The receiver's popup verb "Hand over" stays: it's the act, and the objective names
the job.

### 11. What the cards need from the jobs

- **`Job.closed: Option<Closed>`** with `Closed { at: u64, standing: i32 }`: the game
  minute it closed, and the fixer's standing just after. It's set by `Jobs::hand_in`,
  `Jobs::drop_job` and `Jobs::minute` (late). Jobs aren't saved, so no save format
  changes.
- **`credentials::Mail.job: Option<JobId>`**: the job a mail belongs to. `take_job` and
  `jobs::tick` (late) set it; every other mail sets `None`.
- **Distances and next steps** come from the track's routing:
  - `track::resolve` and `track::locate` give the stop's target, `route::leg` the walk,
    `route::next_text` the next leg.
  - They're computed in the tab's update, never in its draw, for the rows on show and
    the selected card.
  - They're kept until you move, a job changes state or ticks, or the tab is left. The
    deck doesn't move you, so that's once per visit in practice.
  - The tracked row's distance is the track's own.
- **The fixer's shift and whether they're on it**: `World::work_hours`, read against the
  clock's minute of the day (`ui::hud::minute_at`), wrapping past midnight.

### 12. Where the code goes

- **`screens/jobs_tab.rs`** (new): the tab's state (folds, cursor, the distance cache),
  its rows, keys, popup and draw. `Deck` routes JOBS's update, keys, draw and hints to
  it, as it does for GEAR & INVENTORY.
  - It has its own entries: story missions, fixers, jobs, next-step lines, fold rows and
    headers.
  - `Deck::track_row` and the JOBS arms of `Deck::rows` and `Deck::card` move here.
  - Only JOBS used `Detail::Job` and `Detail::Mission`, so they go.
- **`jobs/card.rs`** (new): the route, the window, the track line, and the job, offer,
  fixer and closed cards, as `Vec<Line<'static>>`. The track line is drawn by the tab,
  since its band fills the full width.
- **`missions/ui.rs`**: the story's rows and card in the same language; the story's old
  `deck_rows` header pair goes.
- **`jobs/ui.rs`**: `deck_rows` and `detail` go; the JOBS pane stays.
- **`track/ui.rs`**: `detail_line` goes; the pane line stays.
- **`jobs/mod.rs`, `jobs/generate.rs`, `credentials.rs`**: §10 and §11.

### 13. Tests and shots

**Unit tests:**
- the list's order and sections;
- the fold;
- each kind's stops, words and done words;
- the door line in each `How`;
- the backdoor line in each health;
- READY and the refusal;
- the window with and without the fixer's dashes, including a shift that ends after
  midnight;
- the track line in both states and short;
- `t` and `m` on each kind of row;
- the verbs on each kind of row;
- Drop through the ask;
- mail filtered to the job;
- `Closed` set by each of the three ends;
- the objective texts.

**Layout tests at 100 × 30 and 120 × 40:**
- nothing overflows the card or the list;
- the hint fits;
- every line of the card is backed across the card's width.

**The shots.** The throwaway `jobs_tab_now` test becomes the real shot `jobs_tab` in
`screens/shots.rs`. It's the same city and six jobs, with the clock moved on to 01:20,
so the shots match the mock's situations at both sizes: `jobs_tab_{t, t_ready, offer,
fixer, closed, story, verbs}_{w}x{h}`. The old `track_jobs_tab` shot is folded into it.

**Both front ends** are checked as CLAUDE.md says: fmt; clippy with and without the
window; tests with and without the window; `cargo check` for Linux and Windows.
`every_drawn_character_has_a_glyph` covers the new glyphs: `▮ ▯ ┿ ┊ ┃ ╋ ┄ ◆ ◇ ▲ ⌖ $`.

## Calls I made

- **The tab draws itself** (§12), like the other rich tabs. The deck's shared list and
  `put_paragraph` can't back whole lines or draw a band.
- **The standing word at REFUSE_AT and below is "refuses you"**, not "burned". "Burned" is
  the backdoor's word (§10). The ladder's mark there was already "refuses".
- **The ladder is truthful.** Fetch and deliver work start at REFUSE_AT + 1 (−2), where
  `Jobs::allowed` starts them, not at 0 as round 1 drew it. "refuses" moves left of the
  bar when the two labels would collide.
- **The hand-in text is one line for every kind** (§10). It reads the fixer's real
  shift. Plant's old "while it stands" goes into the card's backdoor line and the
  refusal.
- **"Deliver the package"** is the delivery's verb on the card and in its objective. The
  receiver's popup keeps "Hand over".
- **Closed jobs fold by default**, one "{n} closed" row per fixer. The fold is per fixer
  and per session.
- **`t` on an offer tracks the fixer** instead of refusing, since taking it means
  walking there. The same goes for a fixer's row.
- **Read the mail shows a job's own mails only** (`Mail.job`), so the story's mails stay
  the rig's. It marks them read, as the rig does. No job waits on a mail being read.
- **The story's stop is the mission's own objective text**, not split into a place and a
  verb as the mock shows "Marlo · Old Kowloon Row Station / deliver the package". The
  story content writes free text ("Get home · keep the package"), and splitting it would
  guess.
- **The fixer's hours come from the sim**, not a constant, in the objectives and in the
  take mail's where line (`{start}`). Every fixer has 18:00–04:00 today, so nothing a
  player sees changes yet.
- **The off-corner stretch is advice** (§4 and the builder check), so "hand in by"
  appears nowhere.

Made while building, from the real shots (`jobs_tab_*` in `screens/shots.rs`):
- **The story's distance** moves onto its next line, "24 m · Marlo", instead of sitting
  on the row's right as well. Its target is named the way the story names the person
  ("Marlo", the role), not by the cast person's own name.
- **Copy and plant stops name the host** the way the brief and the objective do
  (`bar-oromed`), not by the building's display name.
- **The window's second line puts the due time first.** The label for when the fixer
  leaves goes under the bar only when it fits beside the due time. The third line
  says it anyway.
- **The offer's standing line** shortens to "{kind} work from standing {n} · you are at
  {s}" when the full line doesn't fit.
- **A tracked fixer** shows its `⌖` after their name on the fixer row ("MEZZ ⌖").
- **Distances are cached per visit** (§11), not per frame. The legs are path searches.

## The builder check

A fresh reader planned from this document and asked three things. I settled each as my
call, as written above.

1. **Is the fixer's shift a rule for the hand-in?** No.
   - A fixer hands out and takes work wherever you meet them. `jobs::ui::rows` checks no
     clock; off shift they're at a bar, which `a_fixer_met_indoors_offers_too` tests.
     The track follows them.
   - Making it a rule would change `ready`, the clocks and the take mail. Jobs taken off
     shift could never be handed in. All of that goes against the non-goals.
   - So §4 shows the off-corner stretch as a heads-up ("off the corner from 04:00").
     The mock's "hand in by 04:00" (`round2_v_hours`) doesn't ship. It came from my
     reading the hours as a deadline, which the code doesn't do.
2. **`m` on a listing that's on another stratum.** The strip says "not on the map from
   here"; the MAP doesn't open on other strata (§10). A kind filter that hides the
   listing is cleared.
3. **The story card beyond its step.**
   - Earlier objectives show as `✓` stops.
   - A done mission is all `✓`, with no track.
   - A next-step line goes under the story row, as `v_steps` draws it (§2, §9).

It also flagged points a builder would settle, and I settled them:
- the closed Plant's backdoor is the newest on that building, burned ones included (§8);
- READY waits on a fixer who won't deal with you, as after a raid (§3);
- the take mail's "I hold the corner on {district} from 18:00." reads the start time
  from the sim through a new `{start}` slot, like the objectives (§10).

## Left to the builder

- Each card's exact line order when the card is taller than the pane at 100 × 30. The
  brief wraps last and may be cut with `…`; the route and the track line never are.
- How the tab's popup asks the deck to switch to MAP (a field on `AppState` the deck reads
  on resume, or a `ScreenCmd`). It must survive MAP's cursor reset.
- The same backing fix for the deck's other cards (`put_paragraph` backing only cells that
  aren't spaces) is a separate change, not this one.
