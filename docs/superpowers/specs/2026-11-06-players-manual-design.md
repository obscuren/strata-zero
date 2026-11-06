# The player's manual · design

Issue #7 of the milestone "First public release (v0.2.0)", "A player's manual".

## Why

The README dropped its key list to make way for a manual, and the in-game help
(`screens/help.rs`) lists keys and nothing else. A new player needs more than keys: what
each screen is for, how the systems work, and what to do when stuck.

Goal: one manual that a new player reads in the game or on stratazero.io, always in step
with the build it describes.

Non-goals: a walkthrough of the story; a tutorial (the DISABLE TUTORIAL setting has no
tutorial behind it yet, that is its own work); search inside the manual; a manual for
modding or development (`docs/development.md` stays that).

Anti-goals: a manual that tells you a key the game doesn't have; three copies (game,
site, wiki) that drift apart; a page that reads like a design doc or like narrator prose.

## 1 · One source: `manual/`

Chosen: the manual is a folder of Markdown files in the game repo, `manual/`, beside
`flavor/`. Not the GitHub wiki: a wiki is a second repo that falls behind the game, looks
like GitHub instead of the game, and needs edit restrictions once the repo is public.

- **A chapter is a file** `manual/NN-slug.md`: `NN` two digits giving the order, `slug`
  lower-case letters, digits and hyphens (the web page's name). The file opens with its
  title on a `# ` line; there is no front matter.
- **`manual/README.md`** describes the format for whoever writes a chapter; it has no
  number, so neither reader shows it.
- **A branch that changes what a player sees** (a key, a screen, a rule) changes the
  manual in the same branch.

## 2 · The manual's Markdown

A small Markdown that both readers draw the same way. Blocks are separated by blank
lines:

- `# Title` once, on the first line. `## Section` and `### Subsection` below it.
- A paragraph: lines that follow each other, joined with spaces.
- A list: `- ` items, one level; a line indented two spaces continues the item above.
  Numbered steps the same with `1. `, `2. `.
- A table: a header row `| a | b |`, a rule row `|---|---|`, then rows. Two or three
  columns. In a key table the first column is the key.
- A tip: `> ` lines, drawn set apart from the text.
- Inline: `` `code` `` for keys, commands and words the game shows, `**bold**` for a
  term where it is defined. Nothing else: no links (refer to a chapter by its title), no
  images, no HTML, no italics, no fenced code.
- Any character the window's fonts draw (`every_drawn_character_has_a_glyph` scans
  `manual/` too).

Anything outside this is an error the game's tests report with its file and line, so a
chapter that one reader can't draw never lands.

## 3 · In the game: MANUAL

- **The files are compiled in** with `include_str!`, as `flavor/*.ron` is. A test checks
  that the list in code and the files in `manual/` are the same.
- **The screen is the booklet from the box.** Chosen from the mock
  (`2026-11-06-players-manual-mock/`, `python3 gen.py round1.html` draws its three
  looks; look B, "the open spread"): two pages side by side
  on the game's own dark, stapled at the spine, over the screen it opened from.
  - **The cover** comes first, alone, as the closed booklet: the STRATA / ZERO logo in
    neon, a drawing of the city's strata under it, PLAYER'S MANUAL and the version.
  - **Pages are a fixed size** (46×28 cells, text 40 wide and 22 rows), whatever the
    terminal or window, so a page number means the same page everywhere. The spread is
    94×28 with a keys line under it: it fits the minimum terminal (100×30).
  - **Page 1 is the contents:** each chapter's number, title, a dotted leader and the
    page it starts on. `↑↓` picks one, `space` (or `enter`) turns to it.
  - **Every chapter opens on a new page** under its number and title; the pages after
    carry the chapter's title as a running head. Page numbers sit in the outer bottom
    corners. A heading or a table's head never ends a page. An odd last page faces a
    NOTES page, as booklets did.
  - **Keys:** `space` opens the cover; `← →` (`h l`, `pgup pgdn`) turn a spread; `[ ]`
    go to the previous or next chapter; `c` goes back to the contents; `esc`/`q` close.
- **Where it opens:** a MANUAL row on the title (before QUIT) and in the pause menu
  (after SETTINGS), both on the cover; and from the `?` key list, whose last line says
  `space opens the manual`, on the Controls chapter.
- **How the text draws:** headings in the game's cyan, text in its text grey, `code` in
  cyan like the key list, bold as bold, list items hanging under a `·`, tables as
  aligned columns whose last column wraps, tips behind a cyan bar.

## 4 · On stratazero.io: /manual

The site (`strata-zero-web`) makes `/manual` from the same files at each release tag,
the way `scripts/updates.mjs` makes the update pages: a page per chapter in the files'
order, its contents down the side. It is built and checked there; this repo only keeps
the files and the format above. Its little Markdown needs tables, `#` and `###`
headings, numbered lists and tips for this.

## 5 · Keys in step

- The Controls chapter's "On foot" table lists the same keys as the `?` key list on foot
  (`help::WALK`, the dev keys left out): a test compares the two sets of key cells, so a
  key added to one and not the other fails.
- The key list itself is brought up to date with the key handlers where the crawl for
  this manual found them apart.

## 6 · What the manual covers

Getting started; the screen; controls; the deck; the city and the world; on foot and in
a fight; the Net; the body (chrome, ripperdocs, drones); work (fixers, jobs, tracking,
the rig and mail); time (waiting, sleeping); saving and dying. Written for the player:
second person, plain words, short paragraphs, what to press and what happens. Dev mode
is left out.

## Calls made while writing this

- The booklet remembers nothing between openings: each opening starts on its cover or
  its chapter. Remembering would put a UI field on `AppState` for little gain.
- The site shows the manual as web pages, not as the booklet: pages there scroll, and
  the booklet's page numbers are the game's own.
- Chapter order comes from the file names, not an index file, so both readers need
  nothing but the folder.
