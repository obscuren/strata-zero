# Coz's book: the end of act one

Issues #4 (sell the ledger to Coz) and #5 (the story stalls if the shard leaves your
pack), the last pieces of the first public slice.

You know whose shard you carry: Coz, a street fixer. You find him on his corner at
night. He pulls you off the street, knows his tape, sees the seal popped, and when you
name Castellan he tells you what the last page of his book holds: three of his runners
on Castellan's money, one job, and the money came home without them. He pays you for
bringing it back, offers you work, and asks you the question Odile asked: where are
they. Walking off, the first holo ad you pass tears magenta with Seren's line: he's gone
quiet, work for Coz, she'll find you. The same words land as a mail. The JOBS tab shows
every mission of the act ticked and one open, **The three**, with nothing to track.
That open mission is where the story stands until the next version picks it up.

Before any of that, the story stops losing its items: whatever it gave you stays in your
pack until it takes it back.

Mockup: `docs/superpowers/specs/2026-11-06-coz-ledger-mock/` (`gen.py` builds the page
from the shots of the throwaway `coz_mock` test in `coz_mock.diff`, run with
`MOCK_KEEP=1`). Its pictures are drawn through the real screens: seed 8, a netrunner,
21:40 on Siren Avenue.

## What Jeff decided

- The pitch "Coz's book" with ending **A**: Seren through a holo ad, then her mail. B
  (Castellan's call) and C (Static 66 on an ad) were drawn and not taken.
- Coz's voice: a street guy, slangy, swearing, rattled. Jeff rewrote the first half of
  the conversation; the lines in §4 are his, with a slang pass he left as drawn,
  including the replacement for "Nobody's seen them since": *"The money came home. They
  didn't."*
- #5 as pitched: the story keeps what it gave you, the Drop rows greyed with one line.
- The pay (¤800), Coz offering work, the hook back to Odile's question, **The three**
  open afterwards: as pitched and drawn, unchanged.

## Goals

- Every build can go from knowing whose shard it is to the end of Coz's scene, and the
  objectives and the JOBS tab then show the act finished with The three as the next
  thing (#4's done-when).
- No step of the chain can be left without a next step by dropping a story item (#5's
  done-when), with a test that tries to drop the shard between opening the package and
  reading it.
- Each new engine piece is general and driven from `flavor/missions.ron`, as the scenes,
  needs, readings and trays before it.

## Non-goals

- Coz walking you into a doorway. It is a stage line; no doorway mark and no walking of
  a sim person in a scene (my call, flagged in the mock).
- Castellan answering for the three, Seren's lesson, the next act. The three waits.
- Shops, a stash, selling items. None exist; when they come, the keep rule applies to
  them too (§1).
- Giving every build a way into a Fetch job (#6).

## Anti-goals

- A "to be continued" card, a narrator line, or anything that steps outside the world to
  say the story stops. Seren says it, in the world.
- A dead objective: The three's objective must read as something that will happen to you
  ("Seren will find you"), never as a task the player can try and fail to do.
- A silent refusal: every refused drop says why, once, in the same voice.

## 1. The story keeps what it gave you (#5)

An item the story gave you stays in your pack until the story takes it back.

- **Which items.** `Book::gives` (every item a `Give` names, plus what it opens; today
  the taped package and the ledger shard), **while a live mission still names it**: a
  mission that is `Waiting` or `Open` whose steps name the item in a trigger's need or an
  effect (`Give`, `Take`, `Unseal`, `Told`, `Carries`, `Read`, `Unsealed`, a fit's need).
  Once every mission that names it is done or uncast, it is an ordinary item. So in a
  city with one fixer, where the ledger is uncast, the shard can be dropped once the
  morning after is done. Worked out from the book and the runs; no flag on items and
  nothing saved.
- **What is refused.** Dropping it, one or all, as a stack or a copy, from any path:
  `actions::perform` refuses `Drop` and `DropCopy` of a kept item with the line *"You're
  not leaving that lying around."*, so keys, menus and anything later all meet the same
  rule. The ways the story itself moves the item stay open: `Open` on the unsealed
  package, `Take`, and the readings.
- **What the pack shows.** The verbs popups (`item_popup` and `copy_popup` in
  `screens/deck.rs`, which `kit.rs` reuses; no other Drop row exists outside tests) grey
  the Drop rows and show the line as the popup's body, as the jacked-in rule already
  does.
- **The dev console** keeps its own `drop` (dev only, not a player path).
- No other path removes a story item from the pack today (no shops, stash, giving,
  death loss; `Use`, `Throw`, `Build` and the job hand-ins can't take these items). The
  builder confirms this; if one can, it gets the same refusal.

## 2. Finding Coz

- The ledger's `know` objective gains his hours: `Bring it to {coz} · {coz.place} ·
  {coz.district} · {coz.hours}`, which reads *"Bring it to Coz · the corner · Siren
  Avenue · 18:00–04:00"*. A fixer works a corner from 18:00 to 04:00; by day there is
  nobody there.
- Tracking already resolves `Talk("coz")` to Coz. Nothing new.

## 3. The sale

- Coz is a fixer, so space on him offers the fixer rows (Talk, Work). **Talk** sends
  `Talked`, the ledger's `sell` step listens, and the scene plays. His job rows stay as
  they are.
- You can catch him anywhere, by day too: tracking follows him, and *"Not here, choom.
  Not out in the open."* fits any street. The objective names his corner and hours
  because that is where he can be counted on.
- `sell`: `Stay("coz")` and `Scene("coz")`. `Stay` keeps the role's person where they
  are now (on the street, or inside a building) until released, so he neither walks off
  mid-scene nor jumps to his corner. (`Hold` keeps a person at work and would move him
  there, which the doctor needs and Coz must not get.)
- `paid` (on `Ended("sell")`): `Release("coz")`, `Take` the shard, `Pay(800)`,
  `Standing(role: "coz", by: 1)`, and the log line *"`Coz` · ¤800 · for bringing it
  home"*. It is the ledger's last step, so the ledger is done.

## 4. Coz's scene

Look `Bubbles`, title "The ledger", the doctor's pace. Beats in order:

| beat | who | line |
|---|---|---|
| Say | coz | Not here, choom. Not out in the open. |
| Stage | | He pulls you into a doorway. |
| Say | coz | Black tape, three turns. You're holdin' somethin' of mine, choomie. |
| Stage | | He turns it under the light. |
| Say | coz | Seal's been popped. Somebody's been in my book. Fuck. |
| Say, need `Skill("hacking")` | coz | You got sum skills, huh. You been in it too, ain't you. |
| Say, need `All([Build("fixer"), Not(Skill("hacking"))])` | coz | You're in the trade, I can smell it on you. You checked it, ey? |
| Say | coz | So who sent you? |
| Say | you | {castellan}. |
| Stage | | A long breath. |
| Say | coz | Course it was him. |
| Say | coz | Three years of my work in there, choom. Who I sent, where, who paid. |
| Say | coz | Fuck! How'd it all go so sideways? |
| Say | coz | Last page is his. Three of mine, on his scrip. One job, in an' out. |
| Say | coz | We were s'posed to walk out clean. Damn it. |
| Say, hold 4.0 | coz | The money came home. They didn't. |
| Stage | | He peels off a roll, and doesn't look at you while he counts. |
| Say | coz | Eight hun'. For bringin' it home, an' for not shoppin' it round first. |
| Say | coz | You want work, you come to me. Not {mezz}. Me. |
| Say, hold 4.0 | coz | An' when {castellan} sits you in a chair, an' he will, you ask him where my people are. Then you come tell me. |

The scene is the same for every build apart from the two need lines; builds that only
learned the owner from Odile hear it all. "Scrip" is street slang for money; the game
writes money as ¤.

## 5. The act ends: Seren through an ad (ending A)

A new mission, **`the_three`**, title "The three", from Coz, after the ledger in the
book:

1. `ask`, on `Holds(All([Done("ledger"), Done("doctor")]))`: the act ends once both
   are done, in either order. A netrunner can read the shard and sell it before seeing
   Odile; then The three waits until her chair, and Seren's line comes on the way out of
   the clinic.
   - `Objective("Ask {castellan} where they are · Seren will find you")`
   - `Ad(from: "seren", text: "He's gone quiet. Work for {coz}. I'll find you. —S")`
2. `quiet`, on `Ended("ask")` (the ad line shown, §5.1):
   - `Mail(from: "seren", subject: "quiet", body: ["He's gone quiet. When he goes quiet,
     so do we.", "Work for {coz}. Keep your head down. I'll find you.", "— S"])`
3. `later`, on `Later`: the story stands here (§5.2). It never fires in this version.

### 5.1 The ad line

A new effect, `Ad(from, text)`: the role's line, torn through a holo ad on the street.

- It waits until **you have walked off**: 6 tiles (Chebyshev) from where you stood when
  the step fired, or onto another place (out of the clinic, off a floor). Fixers keep to
  ad-thick corners, so without this it would go up while Coz's last bubble fades.
- Then it waits for a holo ad on the street in sight within the news cut's reach, under
  the same conditions the news cut checks before it (on the street, walking, asides
  free to show: one shared function in `news.rs` for both), and shows at once: the ad
  glows magenta as a news cut does, and an aside goes up over it, labelled as Seren's
  holo line is (the channel name cut short, `▚▞▚`, her initial) in her colour, the text
  filled. It stays up for the aside's own time (`asides::lasts`).
- `Ad` is a presentation (`Effect::presents`): its showing marks the step `Ended`, as a
  feed's close or a mail's first read does.
- If no ad has shown 30 game minutes after the step fired, it ends without showing, so
  the mail still lands (indoors, a quiet district, standing still).
- It is independent of the news: the glow and the aside work with or without a news
  item out. The explore screen's magenta glow takes either cut.
- The waiting line is story state, saved with the run (`#[serde(default)]`); it names
  its step, so `runs::ids::clear_lost` drops it with a lost step.

### 5.2 Where the story stands

A new trigger, `Later`, that never fires: "the story continues here in a later version".
The three stays open with its one objective and nothing to track (tracking resolves
`Later` to nothing). Saves name the step `later` by id, so a later version can give
that step a real trigger and a reopened run carries on from it.

## 6. The JOBS tab after

- The STORY list shows The three open, above every mission of the act ticked.
- Coz is already under FIXERS: standing beside a fixer rolls him an offer, and a fixer
  with a job in the list is listed. The +1 standing is his "you come to me"; the tab
  needs no change.

## 7. New engine pieces

All parsed from `flavor/missions.ron` and documented in its header comment.

- `Need::Build(id)`: the build (`character.profession`) you chose.
- `Need::Done(mission)`: the mission is done.
- `Effect::Pay(n)`: adds `n` credits. Silent; the content logs what it means.
- `Effect::Standing { role, by }`: adds `by` to your standing with the role's person (a
  fixer), through `jobs.fixer_mut`. A role nobody plays does nothing.
- `Effect::Stay(role)`: keeps the role's person where they are now until `Release`
  (a new sim call beside `keep_at_work`).
- `Effect::Ad { from, text }`: §5.1.
- `Trigger::Later`: §5.2.
- Checks: `Standing` and `Stay` name roles the mission needs cast (as `Hold` does);
  `Ad`'s `from` must be in the cast (as `Holo`'s is; Seren is a visitor); `Done` names a
  mission in the book (`validate`); `Build` names a build in `content.ron`, checked by a
  test that loads the content, as skill and item ids are (`validate` never sees the
  content).

## 8. Saves

- New state: the waiting ad line in `Missions` (`#[serde(default)]`; the glow after it
  shows is `#[serde(skip)]`, as asides are), with a line in `runs::ids::clear_lost`.
  A held-in-place person (`Stay`) is the sim's state, saved as `keep_at_work`'s is.
- New mission `the_three`: a run saved before it existed opens with it `Waiting`
  (`runs::ids` already puts a fresh run in place of a missing mission). A run saved at
  the ledger's `sell` step opens there and goes on to `paid`.
- A dev run saved after the old placeholder `sell` fired opens with the ledger done and
  the shard never taken; with the live keep rule the shard is then free to drop, and
  The three opens once the doctor is done. Saves are not released yet, so there is no
  reader for it.
- `runs/fixtures/` keep opening unchanged.

## 9. Tests

- #5: dropping the shard (one, all, as a copy) between opening the package and reading
  it is refused with the line, the shard stays, and the ledger still points at reading
  it. The same for the package before it is opened. The popups grey the Drop rows. An
  item the story never gave drops as before; a story item no live mission names drops.
- The chain, for a netrunner and for a build without hacking: know → talk to Coz → the
  scene (the need lines said or not as the build says) → paid (shard gone, ¤800 more,
  standing +1, Coz released) → the ledger done → The three open with its objective →
  the ad waits while you stand by Coz, shows once you have walked off by an ad → the
  mail lands → the step waits on `Later`.
- Sold before the doctor: the ledger done, The three still waiting; after Odile's chair
  it opens.
- `Stay` keeps Coz where he stood through the scene (not moved to his corner);
  `Release` lets him go.
- The ad line times out after 30 game minutes with no ad shown, and the mail lands.
- A save taken with the ad waiting reopens with it waiting; a save at The three's
  `later` reopens there.
- A content test rejects a `Build` that names no build; `validate` rejects a `Done`
  that names no mission.
- Shots: an ignored `coz_shots` test (as `doctor_shots`) draws the corner, the scene's
  opening and its hook, ending A, the JOBS tab after, and the greyed Drop row, at 120×40
  and 100×30.

## Calls I made

- **The keep rule lives in `actions::perform`** so no path can bypass it, and the popups
  only reflect it. Kept is `Book::gives` while a live mission names the item, worked
  out, never saved.
- **No doorway walk.** The doorway is a stage line (non-goals).
- **The Fixer-build line needs `Build("fixer")`**, a new need, because no skill marks
  the build; the netrunner line keys off `hacking`, as the shard's rig reading does.
- **Pay and standing are two effects**, separate from the log line, so other missions
  can pay without a fixer and raise standing without pay.
- **Coz stays where you caught him** (`Stay`), on his corner or not, and is released
  when the scene ends. `Hold` would move him to his corner.
- **The act ends when the ledger and the doctor are both done** (`Need::Done`), so the
  ending never comes before Odile's question.
- **The ad waits for you to walk off** (6 tiles or another place), then for an ad in
  sight, up to 30 game minutes, then ends unseen so the mail always comes.
- **The three is from Coz**: he asked the question last.
- **`Later` is an explicit trigger** rather than an unreachable time, so the content says
  plainly where the story stands and a later version has a step to pick up.
- **No JOBS tab change**: Coz is listed by the offer standing beside him rolls.

## Known limits

- In a city with one fixer (Mezz), Coz is uncast and the ledger and The three never
  open, as today: act one ends after the doctor. The home city of a new game has more
  than one fixer.
- The ad needs a street with a holo ad within reach; fixers keep to Neon Strip and Night
  Market corners, where ads are thick.
