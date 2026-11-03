# Clear objectives · design

Approved from the mock page (`2026-11-03-clear-objectives-mock/`, `gen.py` over the
throwaway `objectives_mock` shot test in `objectives_mock.diff`): "C, then A. Don't
implement 4. We will work on mail improvement in another session. The rest looks good."

## Why

Played as it stands, the morning after hides its steps. After waking the only objective
is "Catch the news on the holo". Mezz's mail lands silently, the doctor's mission opens
in the same minute, the package turns openable without a word on screen, and nothing
says to read the shard or where. A new mail shows nowhere on the main screen. The JOBS
list repeats under every row the walk that the card beside it already shows.

Goal: every step of the chain is an objective you can see and follow; a new mail is
impossible to miss; the JOBS list is titles only. Non-goals: the rig's mail row and a
NEW tag on a mail (mock §4, left for a session of its own on the mail); showing a
story's objectives before they arrive (they stay hidden: no spoilers).

## 1 · The chain, as the book says it

`flavor/missions.ron`, the three missions after the house call. Steps not named here are
unchanged.

**The morning after** (`morning_after`, from Castellan)

| step | when | then |
|---|---|---|
| `wake` | as today | as today |
| `news` | as today | as today (`Objective("Catch the news on the holo")`) |
| `saw` | `Ended("news")` | Mezz's mail "the news" as today, then `Objective("Read {mezz}'s mail · on your rig")` |
| `word` | `Ended("saw")`: Mezz's mail read | `Unseal("taped_package")`, `Holo(from: "seren", text: "Doctor's in by ten. Eat first.")`, `Objective("Open the package · it's in your pack")` |
| `open` | `Holds(Carries("ledger_shard"))` | `Log("`The morning after` · a ledger shard, in your pack")` |

**Doctor's in by ten** (`doctor`)

- `word` now fires on `Holds(Unsealed("taped_package"))` (was `MissionDone(morning_after)`):
  `Hold("odile")`, Seren's mail with the code, the objective "See the doctor · … · by
  ten", all as today. Its `Holo` moved to the morning after's `word`; `Unseal` too.
- `tray` gains `Told("ledger_shard")` (§2.4), after its `Tray` and `Objective`.
- The rest as today.

**The ledger** (`ledger`, from Castellan)

| step | when | then |
|---|---|---|
| `whose` | `Holds(Carries("ledger_shard"))` | `ObjectiveFit` (§2.3): already told (`Read("ledger_shard")`) "Find out whose shard this is", no hint (from the final review); hacking "Find out whose shard this is · read it at your rig"; electronics "… · open the casing at your bench"; streetwise "… · look it over in your pack"; anyone else "… · ask the doctor at {odile.place}" |
| `know` | `Holds(Read("ledger_shard"))` | `Objective("Bring it to {coz} · {coz.place} · {coz.district}")` |
| `sell` | `Talk("coz")` | the placeholder log, as today |

The ledger opens the moment the package does, beside the doctor's mission; it no longer
waits for the chair. A city with one fixer still leaves it Uncast, as today; so does a
city with no ripperdoc, since its last fit places `{odile}`.

The streetwise `Look` reading of `ledger_shard` gains `tells: true` (§2.5).

Odile's scene gains a third naming line, for whoever has read nothing and carries
neither the package nor the shard (`All([Not(Read("ledger_shard")),
Not(Carries("taped_package")), Not(Carries("ledger_shard"))])`): "He told me what
you're carrying. Black tape, three turns? That's {coz}'s." So she names Coz to every
non-reader, and `Told` (§2.4) only ever follows her saying it.

## 2 · Engine

### 2.1 `Trigger::Holds(Need)`

A step whose trigger is a need fires once that need holds (`missions::holds` with the
mission, so `Late` works), on any pass, no event needed. Validation checks the roles and
items the need names as other needs are checked today.

### 2.2 A mail step ends when its mail is read

- `credentials::Mail` gains `story: Option<(usize, usize)>`: the mission and step whose
  `Mail` effect sent it. Every other mail has `None`.
- Reading a mail (`credentials::mail_read`, which both the JOBS card and the terminal's
  `mail n` go through) that was unread and has a `story` ends that step
  (`missions::end`), as a feed's close or a scene's last beat does.
- `Effect::Mail` counts as a presentation, so `Ended(step)` may name a step that sends a
  mail.

### 2.3 `Effect::ObjectiveFit(Vec<Fit>)`

`Fit(need: Need, text: String)`. Pushes the text of the first fit whose need holds, as
`Objective` does (slots filled when it fires). Validation: at least one fit, the last
one's need is `All([])` (it always holds), as the tray's sets are checked.

### 2.4 `Effect::Told(item)`

Records `Need::Read(item)` as if a telling reading had been shown: someone told you
whose it is. Odile names Coz in her scene to whoever had not read the shard; `Told` in the
step after her scene makes the ledger's `know` fire for them too. Harmless for those who
read it already.

### 2.5 A Look reading that tells

A `Look` reading with `tells: true` records `Need::Read` of the item when the pack shows
it: the item's row selected in GEAR & INVENTORY, its card on the right reading the Look
text. (The pack's Inspect opens the blueprint window, which for the shard has no
drawing.) Other Look readings stay as they are.

## 3 · Tracking a story step

What a tracked story step walks to, added to `track::resolve_story`:

- `Ended(step)` where that step puts out news: the flat's holo set.
- `Ended(step)` where that step sends a mail: your rig, in your flat.
- `Holds(Read(item))` when you would read the item at the rig (a `Rig` reading of it
  holds for you): your rig.
- Every other new case (`Holds` of anything else): nowhere. The card shows the objective
  with no walk and no track key, as a story step with no place does today.

The JOBS card then reads "Catch the news on the holo" with the walk to the set, "Read
Mezz's mail · on your rig" with the walk to the rig, and the hacker's "Find out whose
shard this is · read it at your rig" with the same.

## 4 · The JOBS list

- No walk line under a story or a job (`Entry::Next` goes, with what builds it).
- A tracked story's row carries `⌖` before its title, as a tracked job's row does.
- The card on the right is unchanged: a job lists both its stops; a story lists its done
  objectives ticked above the one it waits on.
- The sidebar's JOBS pane on the main screen is unchanged.

## 5 · The mail mark (C, then A)

- **Arrival.** A mail is new from the first map frame after it lands: it logs
  "`{from}` · mail · {subject}" (the sender marked), and the newest new mail puts the
  pill up. Mails already in the inbox when a city is entered announce nothing.
- **C · the pill.** On the map's top edge, right, a row in from its edge:
  `▣ MAIL` lit (dark text on magenta, bold), then ` {FROM} · {subject} ` on a deep
  magenta, `  +N` when N more are unread. It stays 6 seconds of map frames (180 frames),
  counted from the first frame the map draws with no film scene playing; cut to the map's
  width.
- **A · the status line.** Once the pill is down, while any mail is unread:
  ` :: ▣ NEW MAIL` (or `▣ N NEW MAIL`) after the credits, magenta, bold. It goes when the
  last mail is read.
- Both are drawn only on the main map screen.

## 6 · The log's newest line types out

The log stamps each line with the city's clock and types the newest out against the
frame counter. After a sleep (8 hours of city clock at a few hundred frames) the newest
line stays behind its typing cursor for minutes. An entry keeps both: the city's tick for
its clock stamp, the frame it arrived for typing out.

## 7 · Shots

- `jobs_tab` keeps its shots, now without walk lines.
- A new ignored `objectives_shots`, driven through the real chain (not faked runs), at
  120×40 and 100×30: the JOBS tab with the morning after waiting on "Open the package";
  the JOBS tab with the ledger open for a hacker; the map as Mezz's mail lands (the pill
  up, the log line); the map once the pill is down (A on the status line).

## 8 · Docs

The header comment of `flavor/missions.ron` says what `Holds`, a mail's `Ended`,
`ObjectiveFit`, `Told` and a telling Look are, in its own style.

## Calls I made

- Hints by build in the ledger's first objective, in the mock's words.
- "Read Mezz's mail · on your rig": the rig is where story mail is read until the mail
  session; "on your rig" is the fixers' own phrase ("Mail's on your rig.").
- Seren's holo line and the unseal belong to the morning after (she breaks in when Mezz's
  mail is read); her mail with the code stays the doctor's, so a city with no ripperdoc
  still plays the morning after.
- The ledger opens with the package, beside the doctor, not after the chair.
- Every mail logs its arrival, a job's mail too, though the take line already says
  "Mail's on your rig."
- The pill counts map frames, so a mail landing behind the deck or the terminal shows
  when you are back on the map.
- Story objectives that have not arrived stay hidden in the card.

## Builder rulings

From the builder check, settled without changing what was approved:

- The streetwise read happens when the pack's card shows the shard's Look text, not on
  Inspect (whose window shows the shard as a missing blueprint); the hint keeps the
  mock's "look it over in your pack".
- With no ripperdoc in the city a non-reader would be told to ask a doctor who isn't
  there: the last fit places `{odile}` ("ask the doctor at {odile.place}"), so the ledger
  stays Uncast there, as it was before.
- A player who reaches Odile's chair with neither the package nor the shard is still told
  whose it is, by her third line; so `Told` never fires on a name nobody said.
