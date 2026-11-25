# Fixers, jobs and the story

Money comes from work, and work comes from fixers. Alongside it runs the story, which finds you whether you go looking or not.

## Fixers

Fixers stand on their street corners from six in the evening until four in the morning, marked amber on the map. The deck's MAP lists them under fixers (`$`). Stand next to one and press `space`:

- `t Talk` passes the time.
- `w Work` hears out a job. The offer says what it pays, how long you have and what kind of job it is: `y` takes it, `n` turns it down.
- `h Hand in` finishes a job you've done. If you can't hand it in yet, the line says why. A finished Fetch pays a stock fuse along with the credits, a spare for your Keyjack.
- `d Drop` gives a job back. It asks first.

## Kinds of job

| Job | What you do |
|---|---|
| Fetch | take a paydata shard from someone's flat |
| Deliver | carry a sealed package to someone, and hand it over |
| Copy | copy a server or a data vault in the Net |
| Plant | leave a backdoor on a server; it must still stand when you hand in |
| Site | get into a guarded hotel or office, do one thing there, and get out |

Copy and Plant are Net work, and a fixer only offers them (vaults too) to a runner with a deck on their waist; a deck left in your pack doesn't count. Fetch and Deliver are for every build. A job already offered or taken stays yours. Until one is on your waist, the fixer's card says so in amber under its standing ladder: `copy, plant and vault: needs a deck on your waist`.

Every job ends the same way: back to the fixer's corner, while they're on it, to hand it in. When you take a job its brief arrives as mail on your rig, with its deadline.

Miss the deadline and the job fails. The fixer hears of it.

## Site jobs

A fixer who trusts you offers site jobs: a hotel or office across the city, its doors flagged while the job is on, with guards, cameras, turrets and locks between the way in and what you came for. The job's spot is marked `◎` on its floor; a Take's is the container itself. A site job is small from a fixer who trusts you, medium from the inner circle, and large once they've known you a while; the bigger it is, the more floors, locks and guards, the longer the window (12, 18 or 24 hours) and the better the pay.

| Asks you to | What you do |
|---|---|
| Take | open the container on the spot (a key card, the Keyjack, the Net or force) and take what's in it |
| Copy | copy the data vault on the spot, in the Net; only offered with a deck on your waist |
| Wipe | wipe the data vault: run Copy on it in the Net, or stand at the rack and force it with Body, which sets off the alarm |
| Plant | press the tap the fixer gives you onto the spot: `space`, then `i Plant the tap`, and stand still while the bar fills |
| Reach | step onto the spot |

Every site job has three steps: the deed, then **getting out** of the building, then the hand-in. You're out when you leave the building on foot with what the job wants carried: a Take's item, or a Copy's vault dump. The deck's JOBS tab says `now get out of` once the deed is done.

A site job can fail as well as run late:

- When the building's alarm goes off on a Copy or a Wipe, its vault purges itself 5 minutes later. The JOBS pane counts it down. An uncopied Copy fails; a Wipe is done for you, at half the pay.
- A Take fails if its item is lost: out of your pack and lying nowhere. Dropping it on the floor isn't losing it, but you can't get out without it.

When a site job ends, whether handed in, dropped or failed, the building goes back to itself once you're out.

## Standing

Each fixer keeps count of you. A job done raises your standing, a job dropped lowers it, a job left late lowers it twice as far. As it rises, a fixer goes from new to known, trusted, and inner circle, and the better work opens up: copying first, then planting and site jobs, then vaults. Let it sink far enough and they won't deal with you at all. After a raid on your rig, the fixers keep quiet for a day.

Standing also pays in hardware for your rig, once each, across the whole run, straight into your pack with the hand-in. A fixer who comes to know you pays a tier-two sled. The first fixer to trust you pays a Civitas cabinet, the second a Lotus tower, and the first to take you into the inner circle a Halcyon server frame. The first Plant you finish pays the `sniff` program and the first Copy pays `keepalive`. The hand-in line says what came. See the rig chapter for what they do.

## Your jobs on screen

The JOBS pane (`F2`) shows the job you've taken, the fixer, the time left (in magenta when it's under an hour) and the next thing to do. The deck's JOBS tab has the whole route of a job, from where you are to the target, and back to the fixer, with the time you have on a bar.

## Tracking

Track one thing at a time: a job's next step, a fixer, a place on the map. Press `t` on a job in the JOBS tab or on a place on the MAP.

- The way there lights up on the ground in cyan, pulsing every few seconds. Where you're going ripples in magenta.
- When the target is off the edge of the screen, a tag on that edge points to it.
- Indoors, the way runs through the door, the stairs and the lifts to the right floor, keeping to the doors, lifts and crawls that open for you; the close map marks that floor with `⌖`.
- The JOBS pane says how far it is and what comes next, like `lift to 6F`.

Tracking shows you the way; it doesn't walk it for you. A job's track moves on to its next step by itself.

## The story

The story reaches you in four ways:

- **Calls.** A ringing card says who is calling. `space` answers, `esc` lets it ring. Unanswered, or if a fight starts, it goes to voicemail in your mail. On the call, `space` moves to the next line and hangs up at the end; `esc` hangs up early.
- **The HOLO feed.** News and messages that show over the map. `space` or `esc` closes them.
- **Scenes.** Now and then the game takes over to show you something. `space` moves it on, `esc` skips it.
- **Mail**, on your rig.

What the story wants next is always at the top of the JOBS pane, and at the top of the deck's JOBS tab, where `✓` marks what you've done and `◆` what's next. You can track it like any job.
