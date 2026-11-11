"""The hotel mock's data for gen.py: The Pale Orchid, medium, four levels (levels.py)."""

import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from levels import LEVELS  # noqa: E402,F401

TITLE = "The Pale Orchid"
TAB = "HOTEL"
SIZE = "medium"
LEDE = (
    "One hotel the site generator could make for a job: four levels, a way in for every build, "
    "one Net linking its cameras, doors, turrets and safe."
)
INPUTS = [
    ("TYPE", "hotel"),
    ("SIZE", "medium · 4 levels"),
    ("OBJECTIVE", "the shard in the penthouse safe"),
    ("SECURITY", "180"),
    ("SEED", "5a17"),
]

ROUTES = [
    dict(key="solo", name="SOLO", colour="#ff5a3c",
         line="Loud. Up the fire escape and in at the balcony, through the bodyguard, out across the terrace.",
         crawl=False, steps=[
             ("g", 44, 8, None),
             ("g", 40, 6, "Force the yard gate. Its lock is on the site's Net, so the alert starts climbing."),
             ("g", 19, 10, "Put the guard on his smoke break down, or go round him, to the fire escape."),
             ("f2", 19, 10, None),
             ("f2", 18, 8, "Off at the 2F balcony. Force the balcony door."),
             ("f2", 2, 3, "Down the corridor, past the camera, to the service stair."),
             ("ph", 2, 3, None),
             ("ph", 6, 17, "Up into the penthouse. The bodyguard in the lounge comes at you."),
             ("ph", 13, 16, "The foyer turret opens up. Force the office door."),
             ("ph", 16, 23, "Crack the safe by force: two turns, and loud."),
             ("ph", 18, 14, "Out through the terrace door."),
             ("ph", 19, 10, "The terrace turret wakes. Run the ledge to the fire escape."),
             ("f2", 19, 10, None),
             ("g", 19, 10, None),
             ("g", 40, 6, "Down into the yard and through the gate before the lockdown seals it."),
             ("g", 44, 6, None),
         ]),
    dict(key="net", name="NETRUNNER", colour="#3cf0ff",
         line="Never touches a lock. The gate camera is the way into the site's Net, and every door, the lift, the turret and the safe are nodes on it.",
         crawl=False, steps=[
             ("g", 44, 9, None),
             ("g", 42, 7, "Jack the gate camera from the alley. From it, the site's Net: loop the yard and kitchen cameras, open the gate."),
             ("g", 18, 4, "Through the yard behind the guard's back. The staff door opens over the Net."),
             ("g", 16, 10, "So does the card lock on the lift hall."),
             ("g", 15, 13, "Call the guest lift and send it to PH without a card."),
             ("ph", 15, 13, None),
             ("ph", 15, 14, "The foyer turret is a node: shut it off, or turn it on the bodyguard."),
             ("ph", 13, 16, "Open the office door over the Net."),
             ("ph", 16, 23, "The safe's lock is a node too. Open it, take the shard."),
             ("ph", 16, 13, "Back into the lift, down to the car park."),
             ("b1", 16, 13, None),
             ("b1", 37, 3, "Across the car park and up the ramp, the loops still holding."),
             ("g", 37, 3, None),
             ("g", 40, 6, "Out of the gate."),
             ("g", 44, 6, None),
         ]),
    dict(key="tech", name="TECHIE", colour="#ffc23c",
         line="Keyjack and crawlspace: in through the bar, up the maintenance riser, out through the car park.",
         crawl=True, steps=[
             ("g", 8, 25, None),
             ("g", 8, 22, "In through the bar's street door. It's public, so nobody looks twice."),
             ("g", 5, 15, "Keyjack the bar's back door into the staff lockers."),
             ("g", 11, 5, "Through the kitchen. Unscrew the riser grille behind the cold store."),
             ("g", 11, 6, "Climb the riser: three floors of ladder in the dark."),
             ("f2", 11, 6, None),
             ("ph", 11, 6, None),
             ("ph", 11, 4, "Out of the grille behind the penthouse bedroom's wardrobe."),
             ("ph", 13, 11, "The foyer turret watches the lift, not the bedroom door."),
             ("ph", 13, 16, "Keyjack the office door."),
             ("ph", 16, 23, "Keyjack the safe: the hardest lock in the building."),
             ("ph", 11, 6, "Back down the riser, all the way to the basement."),
             ("f2", 11, 6, None),
             ("g", 11, 6, None),
             ("b1", 11, 6, None),
             ("b1", 37, 3, "Out through the stores and the car park, up the ramp."),
             ("g", 37, 3, None),
             ("g", 40, 6, "The gate opens from the inside."),
             ("g", 44, 6, None),
         ]),
    dict(key="fixer", name="FIXER", colour="#d26bff",
         line="Talk. The concierge, the night manager and his master card do the work.",
         crawl=False, steps=[
             ("g", 29, 25, None),
             ("g", 34, 16, "In at the front. Tell the concierge there's a package for the penthouse."),
             ("g", 16, 10, "He buzzes you through to the night manager."),
             ("g", 11, 12, "Talk, or pay: the master card from his safe, and the code he shouldn't sell."),
             ("g", 15, 13, "The card sends the guest lift to PH."),
             ("ph", 15, 13, None),
             ("ph", 15, 14, "You're expected, so the turret stays quiet."),
             ("ph", 9, 14, "Talk the bodyguard out into the hall."),
             ("ph", 13, 16, "The card opens the office."),
             ("ph", 16, 23, "The code opens the safe."),
             ("ph", 16, 13, "Down in the lift."),
             ("g", 16, 13, None),
             ("g", 29, 25, "Out of the front door, with a tip for the concierge."),
         ]),
]

# Top of the building first.
ORDER = ["ph", "f2", "g", "b1"]

LINKS = [
    ("⇅", "#dcebf5", "Service stair", {"ph", "f2", "g", "b1"}, ""),
    ("≡", "#ff9a3c", "Service lift", {"f2", "g", "b1"}, "staff card"),
    ("≡", "#dcebf5", "Guest lift", {"ph", "f2", "g", "b1"}, "card for PH"),
    ("╫", "#dcebf5", "Maintenance riser", {"ph", "f2", "g", "b1"}, "behind grilles"),
    ("⇅", "#dcebf5", "Grand stair", {"f2", "g"}, "G to 2F only"),
    ("╫", "#dcebf5", "Fire escape", {"ph", "f2", "g"}, "outside"),
    ("⇅", "#dcebf5", "Car ramp", {"g", "b1"}, "yard to car park"),
]


ZONES = [
    ("public", "#2f7fd8", "public"),
    ("guest", "#9a6bff", "guests"),
    ("staff", "#e0a22e", "staff"),
    ("secure", "#ff3c46", "secure"),
]

TRAITS = [
    ("Ways in", "Front door, the bar's street door, the yard gate and staff door, the car ramp, the fire escape, the riser, and the Net from the gate camera."),
    ("Loops", "The Solo goes in at the balcony and up the service stair, and comes out over the terrace. The lockers have a back door into security. The lift hall opens onto both the lobby and the staff corridor."),
    ("Layers", "Public lobby and bar, guest floors, staff areas, then security, the server room and the penthouse. Turn on Zones to see them."),
    ("Height", "Four levels and seven links between them. One stair joins only G and 2F, one lift needs a card for PH, one ladder hides behind grilles."),
    ("Inside and out", "The yard, the 2F balcony, the roof ledge and the terrace, the ramp down to the car park."),
    ("Set pieces", "The double-height lobby under its drop, the neon bar, the server room, the roof terrace with its pool."),
]


