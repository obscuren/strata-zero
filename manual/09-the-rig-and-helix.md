# The rig and HELIX

Every terminal in the city runs HELIX, a command line. Your rig at home runs it too, logged in as you, and it is where your mail arrives.

## Opening a terminal

Stand next to a terminal and press `space`, then `i Interact: Terminal`. A street terminal logs you in as a guest on its district's host; your rig logs you in as yourself. On the Net, `r` on a terminal you hold opens a shell on its host from where you stand, and `r` in your rig's own Net opens the rig's shell.

Type a command and press `enter`. `pgup` and `pgdn` scroll back, and `esc` closes the terminal.

## Commands

| Command | What it does |
|---|---|
| `help` | the commands this host knows |
| `scan` | the hosts you can reach from here |
| `ssh` | log in to one of them |
| `exit` | log out |
| `plan` | the building's plan |
| `door release` | open the building's doors |
| `lift answer` | bring the lifts to you |
| `cam loop` | loop the building's cameras |
| `alarm off` | end the alarm, as an admin |
| `backdoor` | plant a key; it raises trace |

What a command lets you do depends on who you're logged in as: a guest, a user, or an admin. The usual tools are there too: `ls`, `cat`, `grep`, `man` and the rest.

### Hotels and offices

A hotel's or office's host stands behind the building's firewall (The Net). From a street terminal or your rig, `ssh` to it is refused while any of the firewall's locks is whole: `FIREWALL · 3 LOCKS`. Break every lock and it takes you as any other host does. Root on it gets you back in through the firewall: a Net backdoor on its server, or the server cracked. A shell on one of the building's own terminals, held from its Net, is on the host already.

There, `door release`, `lift answer`, `cam loop` and `alarm off` want root, the admin's login, and answer `ROOT ONLY · CRACK THE SERVER` until you have it: crack the building's server on its own Net, or plant a backdoor on it there. With root, `door release` opens every door lock and turnstile switch on the floors the riser reaches, `lift answer` frees every lift there and `cam loop` loops every camera there, but none of them reach across an air gap.

### Only on your rig

| Command | What it does |
|---|---|
| `mail` | your inbox |
| `journal` | what you know |
| `route` | your backdoors, hop by hop |
| `burn` | give up a backdoor |
| `run` | start a job on the rig |
| `ps` | the jobs running, the seats, and what finished since you last looked |
| `kill` | stop a job |

## Working on the rig

Stand at your rig and press `space`, then `o Open up the rig`. The desk shows your rack from the front, bay by bay. It won't open while the rig is held as evidence.

The rack is the frame, and its bays take **sleds**, small cards that slide in. Each bay takes one kind of sled, and each kind does one thing:

| Bay | What its sled gives |
|---|---|
| `CORE` | seats: how many jobs run at once, and how fast |
| `RELAY` | lines: how many backdoors you can keep |
| `LINE` | hold: how many days a planted Net backdoor lasts |
| `FIREWALL` | a stronger fight against a trace |

The stock rig is a Zenbo half-rack with a ZB-8 core (1 seat), an R-3 relay (3 lines) and a CU-1 copper line (3 days). It has no firewall. A rig with no relay holds no lines, and you can't plant. With no line sled nothing holds, so a Net backdoor won't plant either; a key from the shell still goes in, on its own clock.

| Key | What it does |
|---|---|
| `↑↓` | move between the frame, the bays and the reader |
| `space` | what fits here; in the list, fit it |
| `x` | pull the sled into your pack |
| `esc` | back, then stand up |

The rows under the bays, `READER 1` and on, are the reader's slots. On a small screen the rows the desk has no room for come into view as you move down to them, with `↓` where there are more. `space` on one lists what the reader takes from your pack: a credchip, a locked or black credchip, a rig program shard, paydata or a vault dump, a story shard. A filled row shows its glyph and what `space` there does in your rig's Net (Your rig's Net, in The Net); `x` pulls it back to your pack. The slots come with the rack.

Fitting or pulling takes ten minutes. A swap puts the old sled in your pack. Pull a core, or swap to one with fewer seats, and the newest jobs over the new count are killed; the log says which. Lowering your lines burns nothing, but you can't plant until you're under.

### Racks

| Rack | Bays | Reader slots |
|---|---|---|
| Zenbo half-rack | core, relay, line | 1 |
| Civitas cabinet | core, relay, line, firewall | 3 |
| Lotus tower | core, core, relay, line | 2 |
| Halcyon server frame | core, core, relay, line, firewall | 4 |

Racks are items: spare ones ride in your pack and weigh nothing. `space` on FRAME shows the racks in your pack. Change racks and your sleds move to the new bays of their kind; the old rack goes to your pack, with any sleds left over. Change racks with fewer reader slots while the reader is full and the extras go to your pack. Your terminal wears its rack's look from then on, but the rig is still called `zenbo`.

### Firewalls

Each tier does what the one below does.

- Sandbag: the trace's last leg takes twice as long.
- Tripwire: your phone rings when a trace starts, wherever you are.
- Killswitch: when the trace reaches your door, the rig wipes itself. Every backdoor burns, but nothing is seized, and the fixers don't go quiet.

### Where parts come from

- Fixers. The first time a fixer comes to know you, they pay a tier-two sled with the hand-in, once each: a Tinman 2X core, a Switchback relay, a Longwire, a Sandbag. The first fixer to trust you hands over a Civitas cabinet, the second a Lotus tower, and the first to take you into the inner circle a Halcyon server frame. The rack goes in your pack with the hand-in, and a mail from the fixer follows it.
- The workbench. Tier-two sleds and the rest are built from parts with the `electronics` skill. The top tier eats a data vault's dump, so a netrunner's haul becomes hardware.

## Jobs

Your rig can work while you're out or asleep. `run <job> [target]` starts a job in a free seat, `ps` lists them and `kill <n>` stops one. Each says `rig only` on any other host. Jobs run on game time, faster on a fast core, and catch up when you come back to the city.

| Job | What it does |
|---|---|
| `sift` | digs through a paydata shard or a vault dump from your pack: 6 hours (10 for a vault). It finds the door codes of 2 flats (4 for a vault) and, one time in three, a stash left on a flat's floor, marked on your MAP. The shard comes back sifted, still worth the same, and can't be sifted again. A sifted shard still hands in wherever its shard does and still builds at the workbench in its place; an unsifted one is used first |
| `rent` | sells the spare cycles: ¤6 an hour, paid as it goes |
| `sniff <host>` | listens on a Net backdoor you hold and learns a flat's door code every 3 hours |
| `keepalive <host>` | holds a Net backdoor's life still while it runs |

`sniff` and `keepalive` take a host as `route` lists it, by name, prefix or number. A key backdoor won't hold that long. `sift` and `rent` start installed; the other two come on shards, which a job pays: the first Plant gives `sniff`, the first Copy gives `keepalive`. Carry one to your rig and the next time you sit down it says `INSTALLED sniff FROM SHARD`. `ls /opt/jobs` lists what is installed.

A job takes one seat. With every seat busy, `kill` one or fit a bigger core. A seized rig kills every job.

### Noise

`sniff` and `keepalive` make noise on the building they work on. At 18 points the mail warns you once. At 36 the admins patch the hole: the backdoor burns and the jobs on it end. Noise fades while nothing runs there. `sift` and `rent` make none.

A job doesn't use its backdoor, so it never springs a honeypot, but a line that dies of age under a job burns there and then.

### The morning mail

At 06:00 each day with something to say, a mail arrives from `cron`: what each job found, grouped by job, with the day's warnings and ends. The finds are already in your journal and on your map. When you sit down at the rig, the boot says how many jobs ran while you were out; type `ps`. Under its table `ps` lists what the jobs found and how they ended since you last looked, each line once; the morning mail still carries the night.

## Mail

When you sit down at your rig it tells you how many new mails you have. Type `mail` to open the inbox.

| Key | What it does |
|---|---|
| `↑↓` | move between mails |
| `space` | read the mail |
| `n` | the next new one |
| `esc` | back, then out |

Fixers mail you when you take a job, and the story often does too. A call you let ring ends up here as voicemail. New mail shows on the map as it arrives, and the top band says how many are waiting.

If a trace ever follows you home, your rig is held as evidence for a day, and it says so.
