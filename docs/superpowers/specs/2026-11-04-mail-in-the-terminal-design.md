# Mail in the terminal · design

Approved from the mock page (`2026-11-03-mail-at-the-rig-mock/`, `gen.py` over the
throwaway `term_mail_mock` shot test in `mail_mock.diff`): "B 100%". An earlier round of
mocks (a mail row on the rig's window, an inbox popup, a JOBS verb) got "I honestly want
a better mail IN terminal. Not a different UI".

## Why

Story mail is read only in the rig's terminal, and the terminal hides it: it boots
without a word about mail, `mail` lists the inbox oldest first with a bare `*` for
unread, and `mail 4` prints `FROM:`/`SUBJECT:` in capitals over the body. The objective
"Read Mezz's mail · on your rig" stalls there. Goal: mail at the rig reads like a mail
program, says what's new and what's been read, and never needs a number typed to read
the next new one. Non-goals: any mail UI outside the terminal; mail on district
terminals (`mail` stays rig-only); deleting, replying, sorting.

Branch `worktree-mail-at-the-rig`, stacked on `worktree-clear-objectives` (whose
`credentials::mail_read` ends a story step and whose mail mark this leaves alone).

## 1 · The boot says what waits

When the rig's terminal boots and any mail is unread, after the motd: a blank line, then
`YOU HAVE N NEW MAIL · TYPE mail`. Not on other hosts; nothing when all is read. The line
is the shell's greeting, not a line of `/etc/motd` (the file stays as it is).

## 2 · `mail` opens HELIX mail

`mail` with no argument opens a program that owns the glass, as `nano` does: the bezel,
LED and scanlines stay; every key is the program's until it closes. With no mail at all,
`mail` prints `NO MAIL` as today and opens nothing.

**The inbox.**

- Title bar (inverse, the full glass width):
  `  HELIX mail 2.2   INBOX · {n} mail · {k} new`.
- One row a mail, newest first, from the glass's third row: a new one
  `  ▸ NEW   {hh:mm}  {From}          {subject}`, a read one the same with blanks where
  `▸ NEW` stands. `From` padded to 12, cut with `…` to fit; the subject cut to the glass.
  New rows bold.
- The cursor bar (inverse) starts on the newest new mail, else the newest.
- Status row (third from the bottom): `  [ {k} new ]`, or `  [ no new mail ]`.
- Two key rows, dim, as nano's: `space Read     n Next new    ↑↓ Move` and `esc Exit`.
- Keys: ↑↓ move the bar (the list scrolls when longer than the glass); space reads the
  mail under the bar; `n` reads the newest new mail (status `[ no new mail ]` when none);
  esc closes the program, back to the prompt.

**The mail.**

- Title bar: `  HELIX mail 2.2   {i} of {n}`, with ` · NEW` when it was unread as it opened.
- Rows: `  From:     {From}`, `  Subject:  {subject}`, `  Time:     {hh:mm}`, a blank,
  then the body, each line wrapped to the glass width less two, indented two.
- Opening it reads it: `credentials::mail_read` (so a story step it ends fires, and the
  map's mark clears for it).
- Status row: `  [ {k} new left ]`, or `  [ no new mail ]`, counted after this read.
- Key rows: `n Next new     ↑↓ Other mail` and `esc Inbox`.
- Keys: ↑↓ open the mail above or below it in the inbox's order (reading it); `n` the
  newest new; PgUp/PgDn scroll a body taller than the glass; esc back to the inbox, the
  bar on this mail.

A mail that lands while the program is open shows in the inbox on its next draw (the
inbox is read from the journal each time).

## 3 · `mail N` prints, readably

`mail N` still prints mail N to the scrollback, without opening the program:

```
MAIL {i} OF {n} · NEW          (· NEW only if it was unread)
FROM     {FROM}
SUBJECT  {SUBJECT}
TIME     {hh:mm}
────────────────────────────────────────────
{body lines}
────────────────────────────────────────────
{k} NEW LEFT · mail            (or NO NEW MAIL)
```

The rule is 44 `─`. `cat ~/mail/…` prints the same (see Builder rulings).

## 4 · Time

A mail's `at` is a game minute; its time of day is `jobs::day_minute(at)` as `hh:mm`, as
the JOBS card shows taken and due times.

## 5 · Shots

A new ignored `mail_shots` at 120×40 and 100×30, through the real story (Mezz's news mail
just in, older mails read): the boot line; the inbox; the mail open (Seren's mail having
landed, `[ 1 new left ]`); `mail 4` printed. Each asserts what it's for in the buffer.

## Calls I made

- `mail` with no argument always opens the program; there is no plain listing any more
  (A's printed inbox was the other option, not chosen). `mail N` keeps the shell's way.
- No `mail new` command: `n` in the program does it.
- Space reads, esc backs out, as everywhere in the game; enter does nothing.
- ↑↓ in the mail view move to the neighbouring mail, as the mock's key row says; long
  bodies scroll with PgUp/PgDn (story mails are short; the keys cost nothing).
- Closing the program leaves nothing in the scrollback.
- The program reads mail from `st.journal.mails`, not from `~/mail/` files, so a mail
  file deleted with `rm` still shows in the program (the inbox is the rig's, the files
  are its copies).

## Builder rulings

- `{i}` is the mail's place in the journal, 1 = oldest: the same number `mail N` takes.
  The inbox lists newest first, so its top row is `{n} of {n}`; its rows show no number.
- `mail` reports `Output.open_mail`, handled as `open_nano` is: it wins and ends the line
  (`mail; echo hi` opens the program and drops the rest).
- The new-mail line shows wherever the rig's motd prints at boot today (each time the
  terminal opens on the rig, and wherever an `ssh` back to the rig prints the motd).
- Each stored body line wraps on its own at word breaks; blank lines stay. `mail N`
  prints the stored lines as they are (the scrollback wraps as it does today).
- `access::mail_open` takes §3's format, so `cat ~/mail/…` (which reads through it)
  prints the same as `mail N`. `access::mail_inbox` goes (nothing else lists the inbox).
- `[ {k} new left ]` and `n` count after the open mail's read, so `n` on the last new
  one says `[ no new mail ]`. ` · NEW` is per mail: whether it was unread as it opened.
- The bar and the open mail are pinned by journal index; a mail landing while open moves
  them down a row, not onto another mail; `{n}` grows.
- A glass narrower than the columns cuts the subject first, then `From`.
