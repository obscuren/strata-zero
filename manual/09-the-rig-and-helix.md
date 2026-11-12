# The rig and HELIX

Every terminal in the city runs HELIX, a command line. Your rig at home runs it too, logged in as you, and it is where your mail arrives.

## Opening a terminal

Stand next to a terminal and press `space`, then `i Interact: Terminal`. A street terminal logs you in as a guest on its district's host; your rig logs you in as yourself. On the Net, `r` on a terminal you hold opens a shell on its host from where you stand.

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
