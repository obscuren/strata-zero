# The manual's files

The player's manual is written once, here, and read in two places: the game's MANUAL
booklet (title menu, pause menu, or `space` on the `?` key list) and the manual on
stratazero.io. Both read these files as they are.

## Chapters

- A chapter is a file named `NN-slug.md`: `NN` is two digits giving its place in the
  manual, `slug` is lower-case words joined by hyphens and becomes its web page's name.
- The file opens with its title on a `# ` line. There is no front matter.
- This README has no number, so neither reader shows it.
- A new chapter goes into `FILES` in `crates/strata-zero/src/manual/mod.rs` too; a test
  checks that list against this folder.
- A branch that changes what a player sees (a key, a screen, a rule) changes the
  chapter that describes it in the same branch.

## The Markdown

Blocks are separated by blank lines:

- `# Title` once, on the first line; `## Section` and `### Subsection` below it.
- A paragraph: lines that follow each other, joined with spaces.
- A list: `- ` items, one level. A line indented two spaces continues the item above.
  Numbered steps the same with `1. `, `2. `.
- A table: a head row `| a | b |`, a rule row `|---|---|`, then rows. Two or three
  columns; in a key table the first column is the key.
- A tip: `> ` lines, drawn set apart from the text.
- Inside them: `code` for keys, commands and words the game shows, and `**bold**` for
  a term where it is defined.

Nothing else: no links (name the chapter instead), no images, no HTML, no italics, no
fenced or indented code. Any character the game's window can draw is fine; a test
checks that its fonts have every one.

## How it reads

Write for the player, in the second person: what to press, what happens, what to look
out for. Short paragraphs, plain words, the game's own names in capitals as the game
shows them (`NEW GAME`, the deck's `JOBS` tab). Leave dev mode out.

## Keys in step

The Controls chapter's "On foot" table lists the same keys as the `?` key list on foot
(`WALK` in `crates/strata-zero/src/screens/help.rs`). A test compares the two, so a key
added to one goes into the other.
