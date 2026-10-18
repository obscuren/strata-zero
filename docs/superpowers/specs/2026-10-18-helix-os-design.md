# HELIX/OS: a real operating system on every machine

Date: 2026-10-18. Status: built on `feat/helix-os`; approved in chat by Jeffrey ("Yes, go"): the world's
verbs become programs, every machine has a generated filesystem, your rig's
files are yours, everything you may write is writable, lore files, and nano in
the same slice.
Builds on: `2026-10-17-access-terminal-design.md` (the terminal screen, tiers,
the world commands), `2026-10-17-device-themes-design.md` (HELIX/OS, makers),
`2026-10-17-credentials-journal-design.md` (cards, codes, the journal, mail).

## Intent

Until now a terminal was a menu with a prompt. HELIX/OS is a Unix: every
machine in the city is a host with a filesystem, users and permissions, and the
session is a shell. The building is in `/dev`, the district is in `/net`, the
residents are in `/home` and `/etc/passwd`, the hack log is in `/var/log`. You
`cd`, `ls`, `cat` and `grep` your way to a door code in somebody's notes, and
you `echo release > /dev/door` because you are logged in as that somebody. Your
rig is yours: its files survive the run, your mail lands in `~/mail`, and
`nano` edits anything you may write. The old verbs still work, as programs in
`/bin`.

## Hosts

- A **host** is one machine with one filesystem: `HostId::Rig`,
  `HostId::Building(BuildingId)` (its server), `HostId::District(DistrictId)`
  (a street terminal: every street terminal in a district is the same host).
- Hosts live in `st.helix: Helix { hosts: HashMap<HostId, Host> }`, generated
  the first time they are touched (`Helix::host(st, id)`), kept for the run
  like the journal. `enter_city` resets them.
- A host has a **hostname**: the rig `zenbo`, a district terminal the
  district's name slugged (`lowtown`), a building its label slugged
  (`hollis-court`, `apartment-78`). Slug: lower case, non-alphanumerics to `-`,
  runs collapsed, trimmed.
- `Host { name: String, fs: Vfs, users: Vec<User> }`,
  `User { name, home: String, display: String }`.

## The filesystem

`helix/vfs.rs`, pure: no app state.

```rust
pub struct Vfs { nodes: Vec<Node> }            // nodes[0] is /
pub struct Node { pub name: String, pub kind: Kind, pub owner: String, pub mode: Mode }
pub enum Kind { Dir(BTreeMap<String, usize>), File(String), Dev(Dev) }
pub struct Mode { pub owner_r: bool, pub owner_w: bool, pub other_r: bool, pub other_w: bool }
pub enum Dev { Door, Lift, Cam(usize), Alarm, Plan, AuthLog, NetLog, Journal, Mail(usize), NetHost(BuildingId), Passwd }
```

- `Dev` nodes are rendered or written by the shell with the app state (see
  Programs); the vfs only stores them. `cat` on a device reads it, `echo ... >`
  writes it, `ls -l` shows them with a `c` type.
- Paths: absolute, relative to the cwd, `.`, `..`, `~` and `~/x` for the
  user's home, trailing slashes tolerated. `Vfs::resolve(cwd, home, path) ->
  Result<usize, VfsError>`; `VfsError::{NotFound, NotADirectory, IsADirectory,
  Denied, Exists, NotEmpty}` with Unix wording via `Display`:
  `No such file or directory`, `Not a directory`, `Is a directory`,
  `Permission denied`, `File exists`, `Directory not empty`.
- Permissions: `Vfs::can_read(node, user)` and `can_write(node, user)`:
  `root` may everything; the owner gets the owner bits; the pseudo-owner
  `users` grants owner bits to anyone but `guest`; otherwise the other bits.
  Creating in a directory needs write on the directory; `rm` needs write on
  the parent.
- Operations: `mkdir`, `write(path, text, append)`, `read`, `list`, `remove(path,
  recursive)`, `copy`, `rename`, `stat`. Every mutating op takes the user and
  checks.
- `ls -l` rows: `drw-r-- vex   /home/vex`, i.e. type, owner bits, other bits,
  owner padded, name; files show their size in bytes before the name.

## Users and login

- Each host's `/etc/passwd` lists its users, one a line `name:home:display`:
  `root:/root:root`, `guest:/tmp:guest`, then the residents (a building host)
  as `mara-voss:/home/mara-voss:Mara Voss`, or on the rig your handle slugged.
- The session's user comes from the tier, worked out when you log in to a host
  and again on `ssh`: ADMIN → `root`; USER by a keycard or a known code whose
  household has a head → that resident's user; USER by a code with no household
  → `user` (an entry `user:/tmp:user` exists on every building host); else
  `guest`. On the rig you are always your own user, and `root` with
  `sudo`-free simplicity: the rig's files are yours, `/etc` and `/bin` too.
- `whoami` prints the user; `who` prints who is in the building now (the old
  WHO, Unix-flavoured: `mara-voss   at home`), and on a district host the
  people on that street terminal: just you.

## The tree

Generated per host from the seed, deterministic:

```
/bin/            the programs (virtual listing: every builtin and program name)
/etc/motd        the maker's greeting, from the theme (badge, OS version, a line)
/etc/hostname
/etc/passwd
/home/<user>/    one per resident household head; `mail/` with 1–2 generated
                 mails, `notes.txt` in about a third of homes
/root/
/tmp/            writable by all
/var/log/auth.log   Dev::AuthLog: the hacks on this building, newest first
/var/log/net.log    Dev::NetLog: the trace: heat, alert
/var/plan/ground    Dev::Plan: reading it loads the ground floor into your map
/dev/door   Dev::Door     cat: "locks: engaged" | "locks: released (until HH:MM)" | "lockdown"
/dev/lift   Dev::Lift     cat: "lifts: normal" | "lifts: answering" ; write "answer"
/dev/cam0.. Dev::Cam(i)   cat: "cam0: live" | "cam0: looped (until HH:MM)" ; write "loop"
/dev/alarm  Dev::Alarm    cat: "alarm: quiet" | "alarm: RINGING" ; write "off" (root)
/net/<host>      district hosts only: Dev::NetHost(b), cat: the scan row
```

- Resident homes: `mode rw-r--`? No: homes are `rw----` for the owner, so a
  guest cannot read a resident's notes; `/etc`, `/bin`, `/var/log` are
  `rw-r--` owned by root; `/tmp` is `rw-rw-`; `/dev/*` are owned by `users`
  with `rw-r--`, except `/dev/alarm` owned by root; `/var/plan` is `r--r--`.
- `notes.txt` content, a few lines in the resident's voice, with their door
  code in a third of the homes that have one ("door is 4471, don't tell the
  super") and a neighbour's in a few; that is how a shell session finds codes.
  Reading a code line learns it (`credentials::Journal::learn` with
  `Source::Shell`), the log says "a code in `Mara Voss`'s notes".
- Generated mail (`/home/<r>/mail/001.txt`): short notes between residents
  and the building management: rent, a party, a broken lift. One in ten
  mentions the lobby's lost keycard. Flavour, no mechanics.
- The rig (`HostId::Rig`): `/home/<you>/mail/NNN-<subject>.txt` as
  `Dev::Mail(i)` over `journal.mails` (reading marks read and ticks the favour),
  `/home/<you>/journal.txt` as `Dev::Journal` (the journal rendered, read-only),
  `/home/<you>/notes.txt` empty and yours. `/etc/motd` from the Zenbo. No
  `/dev`, no `/net`; `scan`, `ssh` and the building programs on the rig act on
  your own megablock as before, through `/net/<home-block>` present on the rig
  as its one net entry.
- **Lore**: `flavor/content.ron` gets `lore: [Lore(path, theme: Option<String>,
  kind: Option<String>, text: String)]`. On generation, every lore entry whose
  theme (the device theme name) and kind (the building kind label) match the
  host is written at `path`; `{resident}` in the path expands to the first
  resident's user. Ship four: a Halcyon security memo (`megacorp`,
  `/etc/security-policy.txt`), a Civitas notice (`municipal`,
  `/etc/notice.txt`), a Lotus party flyer (`neon`, `/home/{resident}/flyer.txt`),
  a zine page (`street`, `/tmp/zine.txt`).

## The shell

`helix/shell.rs`: `Shell { host: HostId, origin: HostId, user: String, cwd: String, history: Vec<String> }`.

- **Line syntax**: words split on whitespace, single and double quotes group,
  `;` separates commands, `|` pipes the output lines of one into the next as
  stdin, `>` and `>>` at the end write the output to a file (creating it,
  owner the user, mode `rw-r--`). Unknown command:
  `helix: frob: command not found`. Lower case throughout; the shell does not
  upper-case what you type.
- **Builtins** (`run(st, shell, line) -> Output { lines, status }`):
  `ls [-l] [-a] [path]`, `cd [path]`, `pwd`, `cat file..`, `echo words`,
  `touch`, `mkdir [-p]`, `rm [-r]`, `cp src dst`, `mv src dst`, `grep [-i] pat
  [file..]` (stdin when no file), `head [-n N]`, `tail [-n N]`, `wc`, `man
  cmd`, `help`, `whoami`, `who`, `uname [-a]` ("HELIX/OS 4.1 zenbo"), `hostname`,
  `history`, `clear` (an Output flag the screen honours), `exit`.
- **Programs** (`helix/bin.rs`, the world): `scan` (= `ls -l /net` with the
  rows), `ssh <host>` (`connect` alias: by hostname, a prefix, or the scan
  number; moves the shell to that host, re-logs the user), `exit` on a remote
  host returns to the origin, at the origin leaves the terminal; `mail [n]`,
  `journal`, `door release`, `lift answer`, `cam loop`, `alarm off`, `plan`:
  thin wrappers that write or read the `/dev` and `/var` devices, with the
  period voice kept for their replies ("DOOR LOCKS RELEASED · 60 MIN").
  Permission failures through a program or a device read the same:
  `door: /dev/door: Permission denied` plus the theme's denial line on the
  screen as today.
- **Device semantics** (reads and writes with the app state): as the table
  above; writes land the same hacks the old verbs did (`Op::Spoof` on the door
  lock and lift control, `Op::Blind` on cameras, `alert_until = now`), through
  the functions that `access.rs` keeps.
- **Tab completion**: the first word completes against `/bin`; later words
  against entries of the directory the partial path names.
- **Prompt**: `user@host:cwd$ ` with `~` for the home and `#` for root, e.g.
  `guest@lowtown:/$ `, `mara-voss@hollis-court:~$ `, `root@hollis-court:/var/log# `.

## nano

`helix/nano.rs`: `Nano { path, lines: Vec<String>, row, col, top, modified, readonly, cut: Vec<String>, mode: Mode, status: String }` with `Mode::{Edit, Exit, Save, Search, Help}`.

- Opens on `nano <file>`: an existing file's text, or a new empty buffer; a
  file you cannot write opens with `[ File 'x' is unwritable ]`, a path in a
  directory you cannot write likewise; a directory: `[ 'x' is a directory ]`
  and nothing opens.
- Keys: printable characters insert, Enter splits, Backspace joins or
  deletes, Delete, arrows, Home, End, PgUp, PgDn; `^K` cuts the current line
  into the cut buffer (consecutive cuts accumulate), `^U` pastes the buffer
  above the cursor, `^W` prompts for a search and jumps to the next match
  (wrapping), `^G` shows a help page (any key leaves), `^O` writes out (a
  filename prompt prefilled, Enter saves) and reports `[ Wrote N lines ]`,
  `^X` exits, asking `Save modified buffer? (Y/N)` when modified.
- The screen: the CRT's glass as nano uses it: title row `  HELIX nano 2.2
  File: notes.txt  Modified`, the text with the cursor as a reverse cell, a
  status row with the last message, two shortcut rows `^G Help  ^O Write Out
  ^W Where Is  ^K Cut  ^U Paste  ^X Exit`. Long lines are shown clipped with
  `$` at the edge (no soft wrap). Text in the theme's colours.
- Saving writes through the vfs with the shell's user; a device file refuses
  with its error.

## The terminal screen

- `Terminal` holds a `Shell` and `Option<Nano>`; keys go to nano when it is
  open. The scrollback shows `prompt + the line as typed`, then the output.
  `clear` empties the scrollback. Boot stays as the theme's lines, then
  `/etc/motd`.
- `access::Session` goes away; `Pick::Terminal` carries the origin `HostId`
  and the key; the Space rows are unchanged in wording.
- `access.rs` keeps `Tier`, `tier_on`, `ice_grade`, `district_of_terminal`,
  `building_label` and the world actions (`release_doors`, `answer_lifts`,
  `loop_cams`, `alarm_off`, `load_plan`, `log_lines`, `who_lines`, `scan_row`)
  that the devices and programs call; its verb table and `run` are deleted.
- Replies land on the glass at once (Jeffrey, 2026-10-18: no typing effect on
  output); only the one-time boot types itself. The LED, PgUp/PgDn scrolling
  without markers, wrapping and the denial glitch stay as they are.

## Non-goals

- Networking beyond `ssh`: no scp, no ports, no remote execution of files.
- Shell scripting: no variables, loops, `&&`, background jobs, globbing.
- Planting backdoors or scripts that do anything; that is the backdoor slice.
- Binary files, file sizes beyond their text, timestamps, chmod and chown.
- A pager; long output scrolls with PgUp as today.
- vi.

## Testing

- vfs: resolve (`..`, `~`, trailing slash), every error's wording, each
  permission rule including `users` and `root`, each operation, `ls -l` rows.
- generation: a building host has `/etc/passwd` with its residents, homes with
  mail, some notes with a code; the same seed twice gives the same tree; the
  rig has your mail as devices and `journal.txt`; lore lands where its rule
  matches and nowhere else.
- shell: quoting, `;`, pipes with `grep`, `>` and `>>` (and `Permission denied`
  into `/etc` as guest), every builtin, `command not found`, `man`, tab
  completion of commands and paths.
- programs and devices: `cat /dev/door` states, `echo release > /dev/door` as
  guest denied and as a resident lands the spoof the Net reads; `cat
  /var/log/auth.log` shows it; `ls /net` lists the district; `ssh` by name,
  prefix and number, `exit` returns; `cat /var/plan/ground` loads the floor;
  reading a code line in notes learns it; `mail` and `cat ~/mail/001-*.txt`
  mark read and tick the favour.
- nano: open, type, Enter, Backspace, arrows, cut and paste, search, write
  out and the `[ Wrote N lines ]`, exit with the save prompt, read-only
  refusal; a screen test of the glass.
- terminal: the prompt per user and host, boot then motd, `clear`, nano
  taking the keys and giving them back, the old Space rows still opening a
  terminal; the whole flow `ls /net`, `ssh hollis`, `cat /home/*/notes.txt`
  denied as guest, `whoami`.
