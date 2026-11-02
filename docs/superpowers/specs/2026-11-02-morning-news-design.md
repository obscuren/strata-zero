# The morning news: waking up someone, and a city that talks about it

The first slice of the morning after the house call. Castellan's morning call wakes
you; a short scene and a few unsteady seconds; then the city's news: a holo set in
your flat reads it out while you move about, interacting with the set shows the whole
broadcast, and out in the city people mutter it, holo ads cut into it and screens over
counters show it. Mezz has seen it too.

The news is built as a general thing: the story puts out a news item and every outlet
shows whatever is current. Later the city can put out its own.

Mockup: `docs/superpowers/specs/2026-11-02-morning-news-mock/` (`news_mock.rs`, the
throwaway shot test that drew the broadcast; `broadcast_120x40.png`, its picture).

## What Jeffrey decided

- **The morning after, in two slices.** This one is waking and the news; the shard,
  Seren's word and the ripperdoc (Odile Vance) come next, with their own spec.
- **Waking** (the pitch, beat 1): Castellan's call drags you awake. When it ends, a
  short bubbles scene: the stage line *Your head swims. Last night comes back in
  pieces: the red pad, the couch, her cable.* and you: *"…so that happened."* For a
  few seconds the screen itself is unsteady: colours bleed and the map drifts, then
  settles. The package is still in your pack.
- **The city noticed** (beat 2): the news of an informer dead on Marlo's street and a
  courier seen leaving with a package. The description is vague, but it could be you.
  Then Mezz, rattled, sends a mail: "Saw the news. Don't go out looking like you."
- **The news comes in two layers** ("Yes! Including the mockup"):
  - On the map: the holo set in your flat is on, and it reads the news in chat
    bubbles over the set while you keep control of the flat, a new line every few
    seconds, repeating like a news loop.
  - Interacting with the set opens the whole broadcast: the mock. `space turn it off`
    switches the set off; Mezz's mail follows.
- **"We need more ways to display news in the city. Subtle."** Of the four outlets
  offered he picked three, while the news is fresh:
  1. **Overheard**: now and then someone you walk past says a line in a small bubble,
     only close by and not often.
  2. **Holo ads cut in**: a holo ad you pass sometimes glitches out of its loop into
     the headline for a second: its glow goes HOLO magenta and a short
     `NEWSLINE ▸ …` floats over it; Look at it then and it reads the headline.
  3. **A screen indoors**: walking into a bar or noodle shop writes a quiet log line,
     "Over the counter, a screen: …".
  (Not picked: cameras gaining a Look line.)
- Draw everything through ratatui so the terminal and the window both show it
  (CLAUDE.md); `space` confirms, `esc` backs out, `enter` is not a window's key.

## Calls I made

- **The news is a book item** (`news` in `flavor/missions.ron`, §2) put out by a new
  effect `News("id")`; one item is current at a time; it stays fresh for its `fresh`
  game minutes (a day for this one) and then every outlet goes quiet.
- **Mezz's mail comes when the news step ends**: when you switch the set off from the
  broadcast, when you are out of your flat (you never watched, or you were not home
  when it came), or when the item goes stale (see the builder rulings). The set stays
  on, and Watch stays offered, as long as the news is fresh.
- **The waking chain is its own mission**, `morning_after` ("The morning after", from
  Castellan), started by `MissionDone(house_call)`. Today `house_call` is done the
  moment the morning call starts to ring (a last step marks its mission done as it
  fires), and a scene does not wait for a call, so `house_call` gets one more step
  after the call (`Ended("morning")`, a log line) and is done when the call ends; its
  "Sleep · Castellan will be in touch" objective stays up until then.
- **An objective "Catch the news on the holo"** while the news step waits, so a player
  who does not notice the set is pointed at it. The mission's last step (Mezz's mail)
  sets none: a last step's objective is never seen. Seren's word starts the next slice.
- **The still is content**, 18 by 18 pixels in rows of characters like a face, with a
  red mark character for the newsroom's bracket (§2). The camera's name is content
  (`CAM 14`).
- **SOUGHT is filled from you** (§7): your BODY gives the build, your Torso item the
  clothes, the minute you walked away from the body and which way you went. That walk
  is recorded as it happens (no history exists today).
- **The channel is named after one of the city's corps**: the first word of the name
  of the first corp building (by id) that has a name, in capitals, then `NEWSLINE`
  (`VELFORM NEWSLINE`); `CITY NEWSLINE` in a city without one.
- **The set is new furniture**, `Furniture::Holo` ("holo set"), on the flat's layout
  two tiles in front of the couch, off the line from the door to the couch. Off, it is a
  static glyph; on, it cycles the holo-ad glyphs in HOLO magenta. It throws no light (a
  city's light is fixed when it is generated).
- **Bubbles that do not take control are a new piece, "asides"** (§5): the set's lines,
  the overheard lines and the holo ads' headlines are all asides. They use the scene
  bubble's placement and frame, are smaller, never dim the frame, and show only while
  you walk (not in a scene, asleep, in a fight, or with the console open).
- **The overheard lines are the item's**, said in order, by a person you can see within
  6 tiles on the street who is not a story role; one at most every 12 game minutes,
  then a 1 in 4 chance each game minute someone is close. Labelled `overheard`.
- **Holo ads**: a visible holo ad within 10 tiles on the street cuts in at most every 8
  game minutes, then a 1 in 3 chance each game minute; the cut lasts 3 seconds. Look at
  that ad during the cut: *"The ad stutters. For a second it's a headline: COURIER
  SOUGHT IN KOWLOON ROW."*
- **Screens**: entering a building whose kind is a bar, a club or a noodle bar logs
  the line once per building per news item: a bar's or club's floor (a closed building's
  inside), or a noodle bar's footprint on the street (a walk-in stall).
- **The daze is a general effect**, `Daze(seconds)`, so later beats (a blow, a drug)
  can use it.
- **A dev console `story news <id>`** puts out a news item with no mission, the way
  `story scene` plays a scene.
- **"Odile" leaves the random names** now (the ripperdoc is Odile Vance next slice):
  replaced in `flavor/names.ron`'s `people` by a new name the list doesn't have,
  keeping the list's length so a seed makes the same city.

## Builder rulings

A fresh builder read this spec against the code. Its three questions, settled:

- **The daze moves the whole map picture.** "The screen itself is unsteady": the pass
  runs after everything the map area draws of the world, your `@`, the visitors, the
  set's glyph and a cut-in's cell included, and before what floats over it (asides,
  the call's card, the Look box, the help). So the `@` never sits over a wall the rows
  slid under it. The HUD and side panel stay still.
- **The news step always ends.** Besides `space` in the broadcast, it ends the first
  frame you are out of your flat after the item was put out, fresh or not, and when the
  item goes stale (a player who sleeps the day away in the flat gets Mezz's mail then).
  So `morning_after` always finishes and the next slice can start from it.
- **"The package is still in your pack" is narration**, not a check: nothing in this
  slice looks at the package. If a player sold it, the next slice deals with that.

## Goals

- The morning reads as one beat: the call, the dizzy scene, the screen settling, the
  set already talking.
- A player learns the city knows about the drop without a single blocking screen they
  did not choose.
- The news is one thing with several outlets, each of which works for any news item.

## Non-goals

- Police, wanted levels or anyone reacting to the SOUGHT description.
- The city making its own news; more news items than this one.
- The shard, Seren's word, the ripperdoc (next slice).
- Light from the set; sound.

## Anti-goals

- The news nagging: outlets that fire every few steps, bubbles over the player, a
  modal anywhere but the set when you choose to watch.
- Writing the outlets for this item: no "Marlo", "courier" or "package" in Rust.

## 1. The story

`flavor/missions.ron`:

- `house_call` gets a last step after the morning call:
  `Step(id: "up", when: Ended("morning"), then: [Log("`House call` · morning")])`.
- A new mission after it:

```ron
Mission(id: "morning_after", title: "The morning after", from: "castellan", steps: [
    Step(id: "wake", when: MissionDone(mission: "house_call", minutes: 0), then: [
        Scene("waking"),
    ]),
    Step(id: "news", when: Ended("wake"), then: [
        Daze(4.0),
        News("marlo"),
        Objective("Catch the news on the holo"),
    ]),
    Step(id: "saw", when: Ended("news"), then: [
        Mail(from: "mezz", subject: "the news", body: [
            "Saw the news.",
            "Don't go out looking like you.",
        ]),
    ]),
]),
```

- The scene:

```ron
"waking": Scene(title: "The morning after", look: Bubbles,
    pace: Pace(type: 45, walk: 0.3, hold: Some(2.5)), beats: [
    Stage("Your head swims. Last night comes back in pieces: the red pad, the couch, her cable."),
    Wait(1.5),
    Say(who: "you", text: "…so that happened."),
]),
```

`MissionDone` fires when `house_call`'s last step fires, now when the call has ended,
so the scene never starts under a ringing call. A player who is not at home when the
call comes gets the scene where they are and the news step ends at once (they are out
of their flat), so Mezz's mail comes; the set is on when they get home.

## 2. A news item

`Book` gains `news: BTreeMap<String, News>` (`#[serde(default)]`). One item, written
for this story:

```ron
news: {
    "marlo": News(
        about: "marlo",
        headline: "Courier sought in {marlo.district}",
        report: [
            "A noodle cook was found dead late last night outside his stall at {marlo.place}. He was {marlo}, 41. Sources at the precinct say he was a paid informer.",
            "Officers want to speak to a courier a street camera caught leaving soon after, carrying a package. Anyone who knows them is asked to call it in.",
        ],
        reads: [
            "This is Newsline. A cook found dead outside his stall in {marlo.district}.",
            "Sources say {marlo} sold names to the precinct.",
            "Police want a word with a courier seen leaving with a package.",
            "Rain by noon. Lift nine is out again.",
        ],
        crawl: [
            "{marlo}, 41, named as police informer",
            "Courier sought in {marlo.district}",
            "Lift 9 out of service",
            "Rain by noon",
            "Arcology rents up 4%",
        ],
        overheard: [
            "That noodle cook in {marlo.district}. Informer, they're saying.",
            "Courier, they said. Jacket like that one.",
            "Whoever did the cook did the block a favour.",
            "Cameras saw a courier. Cameras see everybody.",
            "Sold names, my aunt says. Had it coming.",
        ],
        sought: "a courier",
        carrying: "a taped package",
        camera: "CAM 14",
        still: [
            "155555511111116661",
            "144444411011011211",
            "323232321111111211",
            "244444421101111211",
            "245555421111111211",
            "2222222211rr11rr11",
            "3333333322r3034r33",
            "333333332230004443",
            "300300332230066443",
            "300000332230304433",
            "331113332230303333",
            "2222222222r1113r32",
            "2222222222rr22rr22",
            "111111111311111111",
            "111111111311111111",
            "111111111111111111",
            "111111111311111111",
            "111111111311111111",
        ],
        close: "turn it off",
        fresh: 1440,
    ),
},
```

- `about`: the role whose body the camera saw. The SOUGHT time and heading come from
  you walking away from that body (§7); the caption's district is that role's
  `{role.district}`.
- Every text takes the usual `{role}` slots, filled when the item is put out. The
  roles they name count as roles the mission needs (it is `Uncast` without them).
- `still`: 18 rows of 18 characters, `0`–`6` the camera's seven greens dark to
  light, `r` the bracket's red (§4). The rows above are the mock's still (its pixel
  grid with the bracket marked). Its grain and scanlines are drawn, not stored.
- `fresh`: game minutes the item stays current.
- The book refuses: an unknown news id in `News`, an `about` role not in the cast, an
  empty `headline`, `report`, `reads` or `overheard`, a still that is not 18 by 18 or
  has another character, `fresh` of 0.
- `Effect::News(id)` presents (like `Feed`, `Call`, `Scene`), so `Ended("news")` is
  valid; the step ends as in "Calls I made".
- `Effect::Daze(seconds)` (§8). Neither has text of its own.

Putting out an item (the effect, or `story news <id>`) fills its texts, works out the
channel, records the mission and step (none from the console) and the game minute,
switches the flat's set on and makes it current, replacing any earlier one.

## 3. The holo set

- `citygen::Furniture::Holo`, appended after `Chair` (saved cities keep their bytes),
  label `"holo set"`. Blocks like all furniture.
- The flat's `LAYOUT` (`citygen/src/interior/layouts/home.rs`) gets it as `V` on row 4,
  column 16: two tiles in front of the couch, off the door's column (17).
- Glyph `◫` in the wall colour, as other furniture. While it is on (§2), the explore
  screen draws over its cell the holo-ad glyphs in turn (`pick(&g.holo, tick / 8)`), in
  `MAGENTA`.
- Look: head `Holo set`; `flavor/looks.ron` `furniture` gets `"holo set"` lines in
  house voice (end with a full stop, no digits): "A cheap holo set, the picture a
  hand's width off true." and "Older than the flat. It still gets every channel."
- Legend: `('◫', "holo set")`.
- Space beside it in your own flat offers the subject `Holo set` with `Interact: Watch`
  (`i`) while a news item is fresh; `Pick::Watch` pushes the broadcast (§4) and switches
  the set on. Not fresh: no Watch, the set is furniture.

## 4. The broadcast

A screen over the map, drawn as the mock (`news_mock.rs`), from the current item:

- The box: 90 columns at most and 4 fewer than the screen, as tall as its text needs
  (17 rows at least) and 2 rows fewer than the screen at most, centred;
  `face::bracket_frame`.
- Header row: `HOLO` bold MAGENTA, `▸ news` DIM, `· <CHANNEL>` FAINT; right, `●` red
  and `LIVE` bold red, then the game clock DIM where the feed has it.
- Left, where the feed's face goes: the still, 18 columns by 9 rows of `▀` (top pixel
  as foreground, bottom as background), the greens and red of the mock's
  `news_colour`, odd pixel rows at 80 %; a light grain (the mock's 5 % up, 5 % down,
  seeded so it holds still). Under it: `●` red, the camera bold DIM, `· HH:MM` FAINT
  (the minute you left, §7; left out if unknown); the next row, the district FAINT.
- Right, from column 22: the report's paragraphs wrapped to the width less 26, a blank
  row between; a blank row; the SOUGHT card (§7): two rows on a dark red band
  (`(26, 8, 14)`), a red `▌`, `SOUGHT` bold red, the two lines in TEXT from 8 columns
  in.
- `space` bold MAGENTA and the item's `close` DIM, three rows from the bottom.
- The crawl on the row above the bottom edge: a `(40, 0, 32)` band, ` NEWSLINE ` bold
  on MAGENTA at its left, then the crawl lines in capitals joined by `   ▸ `, scrolling
  left at 10 columns a second (frame ticks) and wrapping round.
- Text cut with `…` where the box runs out, as the feed does.
- Keys: `space` switches the set off, ends the news step if it has not ended, and
  closes; `esc` closes with the set left on; nothing else.
- At 100×30 the whole box fits (the mock's 100×30 render).

## 5. Asides: bubbles while you move

`missions::asides` (new):

- An aside: who speaks (a tile on a place, or a person, followed as they walk), a
  label and its colour, the text, and the clock ticks it starts and ends. At most two
  at once; a new one past that replaces the oldest.
- Drawn by the explore screen over the map after the map and before the HOLO call's
  card, only in walk mode with no scene, no rest, no fight and no console. A bubble 28
  columns at most, placed with `bubbles::place` around the speaker's cell avoiding your
  `@` and the other people and visitors drawn there; the scene bubble's rounded frame
  with the label in the top edge, the tail; the text typed at the call's
  `TYPE_PER_SECOND`. A speaker off the map's screen area: not drawn (it still runs
  out its time). No dimming, no keys.
- Its time: 2.5 seconds plus 1 second for every 20 characters, in clock ticks
  (`call::TICKS_PER_SECOND`), so it pauses under a menu.
- The set: while the current item's set is on and you are in your flat (in walk mode,
  the set's tile on your place), its `reads` in turn, round and round, each an aside
  on the set's tile labelled with the channel in MAGENTA, 1.5 seconds apart.

## 6. The city's outlets

While an item is fresh, each in walk mode with no scene, rest or fight:

- **Overheard**: on the street, the people drawn within 6 tiles (Chebyshev) of you and
  visible, not cast in a story role, not dead. At most one every 12 game minutes; once
  that has passed, each game minute with someone there, a 1 in 4 chance: the next of
  the item's `overheard` lines (in order, round again) as an aside on that person,
  labelled `overheard` in DIM. The person: the nearest, ties to the lowest id.
- **Holo ads cut in**: on the street, the `HoloAd` tiles visible within 10 tiles. At
  most one cut every 8 game minutes; once passed, each game minute with one in sight,
  a 1 in 3 chance: the nearest ad cuts in for 3 seconds: its cell drawn in MAGENTA,
  and an aside on it labelled `NEWSLINE` in MAGENTA saying `▸ ` and the headline in
  capitals. Look at that tile while the cut lasts: the body line is *"The ad stutters.
  For a second it's a headline: <HEADLINE>."*
- **Screens**: the building you are in, each frame: a floor's building, or on the
  street the tile's building. When it changes to one whose kind is `Bar`, `Club` or
  `NoodleBar` and that building has not had this item: `st.say("Over the counter, a
  screen: <HEADLINE>.")` with the headline in capitals.
- Rolls use the game's RNG for the story (or a seed of the city seed and the minute),
  so tests can pin them.

## 7. SOUGHT and the walk away

- Every `Body` remembers, once, when you walked away from it: the first time you are
  more than 12 tiles from it on its place after having been within 6 tiles of it (in
  sight or not), the clock's `HH:MM` and the eight-point heading from the body to you
  (`north`, `north-east` … ; north is up the map).
- Line 1: `<sought> · <build> build · <clothes>`: build from BODY
  (`character.attribute(Body)`): 0–3 `slight`, 4–6 `medium`, 7–10 `heavy`; clothes,
  the Torso item's name in lower case, or `dark clothes` with nothing on.
- Line 2: `carrying <carrying> · <HH:MM>, heading <heading>`; not recorded (the
  console, a body you never went near): `carrying <carrying> · late last night`.
- Filled when the broadcast is drawn, so it is what you wear now.

## 8. The daze

`Effect::Daze(seconds)` starts a daze of that many seconds in frame ticks. While it
runs, after the map area's world is drawn (your `@`, the visitors, the set and a
cut-in included) and before anything floating over it (asides, the call's card, the
Look box): each map row slides
sideways by up to 2 cells on a slow wave (the rows out of step), and the colours bleed
into a magenta and a cyan fringe (as the face's glitch does); both fade to nothing over
the daze, eased out. The map only: the HUD, the side panel and bubbles stay still.
Nothing else is held up: you can move under it.

## 9. Code

- `missions/content.rs`: `News`, `Book.news`, `Effect::News`, `Effect::Daze`, the
  checks.
- `missions/news.rs` (new): the current item (`NewsRun`), putting one out, the frame's
  news tick (the set's reads, the outlets, the walk away, ending the step), the channel,
  SOUGHT's lines, the screens' log.
- `missions/asides.rs` (new): asides, their timing and drawing.
- `missions/broadcast.rs` (new): the broadcast screen.
- `missions/daze.rs` (new): the daze, drawn over the map.
- `missions/effects.rs`: the two effects. `missions/mod.rs`: state on `Missions`
  (reset with the city, not saved), `Body`'s walk away.
- `citygen`: `Furniture::Holo` (`tile.rs`), the layout; `render/tileset.rs`: glyph and
  legend; `flavor/looks.ron`; `look.rs`: the cut-in's line.
- `interact.rs` and `screens/popup.rs`: the set's subject and `Pick::Watch`.
- `screens/explore.rs`: the news tick in the update with the other per-frame ticks; the
  daze, the set's glyph, the cut-in's cell and the asides in the draw.
- `screens/console.rs`: `story news <id>` (and its completion).
- `flavor/missions.ron`: §1, §2. `flavor/names.ron`: "Odile" out.

## 10. Testing

- The book: the shipped book loads; each refusal in §2 with a small book.
- The story: after the house call's morning call is answered through, `house_call` is
  done and the waking scene plays, then the daze and the news are on, the objective
  says to catch it; switching the set off from the broadcast sends Mezz's mail and
  `morning_after` is done; leaving the flat instead sends it too; a player out of the
  flat when the news comes gets the mail at once; staying in the flat until the item is
  stale sends it then.
- The news item: its texts filled (no `{` left), the channel from the city's corp, the
  headline in capitals where it shows.
- The set: on the home layout at row 4, column 16; its glyph; Look; Watch offered only
  while fresh and only in your flat; Watch pushes the broadcast.
- The broadcast: drawn at 120×40 and 100×30 inside the screen; header, still, caption,
  report, SOUGHT, key row and crawl where §4 puts them; the crawl moves with ticks;
  `space` switches off and ends the step, `esc` doesn't, `enter` does nothing.
- SOUGHT: the build words at BODY 3, 4, 6, 7; the Torso item's name and `dark
  clothes`; the walk away recorded once, its time and heading (north up); the
  fallback line.
- Asides: placement off your `@`; the time each lasts and that it pauses with the
  clock; at most two; none in a scene, asleep, in a fight.
- The set's reads loop in order in the flat and stop when it is off or you leave.
- Overheard: never inside 12 minutes of the last; never from a story role or the dead;
  only within 6 tiles and visible; the lines in order.
- Holo ads: the cut's cell in MAGENTA and its aside; Look reads the headline during the
  cut and the ad's own line after.
- Screens: entering a bar's floor and stepping into a noodle bar's footprint each log
  once per item; a shop doesn't.
- The daze: rows shifted and colours split early, nothing changed after it ends; the
  HUD untouched.
- Outlets quiet once the item is stale (`fresh` minutes on).
- `every_drawn_character_has_a_glyph` with the new glyphs; ignored shots `news_shots`:
  the set reading in the flat, the broadcast (120×40, 100×30), an overheard bubble, a
  holo ad cutting in, the daze half way.
- The console: `story news marlo` puts it out; completion offers `news`.
