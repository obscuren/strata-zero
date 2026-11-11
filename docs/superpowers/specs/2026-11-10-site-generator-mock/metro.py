"""A large example for the site generator: Cinder Cross, an interchange on two metro
lines, six levels from the street down to a closed line under it.

Design characters, on top of the hotel's (levels.py):
  'y' track, east-west   'Y' track, north-south  ':' platform edge  'J' turnstile
  'E' escalator up       'F' escalator down      'C' train          'O' train window
  'D' train door         'V' ad screen           'i' passenger      'W' sewage
  'Q' rubble             'z' fire barrel         '-' tunnel floor
"""

import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from sitekit import GANG, OUT, PAID, PUBLIC, SECURE, STAFF, TRACK, Level  # noqa: E402

W, H = 96, 40

# Shared anchors: the same tile on every level they stand on.
KIOSK_W = [(7, 26), (8, 26)]       # west street stair: G, -1
KIOSK_S = [(34, 37), (35, 37)]     # south street stair, across the avenue: G, -1
STREET_LIFT = [(46, 8), (47, 8)]   # street lift: G, -1
PLAT_LIFT = [(59, 20), (60, 20)]   # platform lift: -1, -2, -3
FLIFT = [(67, 3), (68, 3)]         # freight lift: G yard, -1, -4 (passes the platforms)
ESTAIR = (69, 21)                  # emergency stair: G to -4
VENT = (15, 10)                    # ventilation shaft ladder: G tower to the -4 fan room
OLDSHAFT = (10, 30)                # the old line's shaft: -4 cable vault to -5


def escalator(lv, x0, y0, x1, y1, kind, zone):
    """A bank of escalators: columns alternate down and up."""
    for x in range(x0, x1 + 1):
        for y in range(y0, y1 + 1):
            lv.put(x, y, "F" if (x - x0) % 2 == 0 else "E")
            lv.kind[y][x] = kind
            lv.zone[y][x] = zone


def train_h(lv, x0, x1, y_doors, y_far, line):
    """A train standing east-west: doors on the platform side, windows on the far side."""
    for x in range(x0, x1 + 1):
        lv.put(x, y_doors, "D" if (x - x0) % 6 == 3 else "C")
        lv.put(x, y_far, "O" if (x - x0) % 3 == 1 else "C")
        for y in (y_doors, y_far):
            lv.kind[y][x] = line
            lv.zone[y][x] = TRACK
    lv.put(x0, y_far, "C")
    lv.put(x1, y_far, "C")


# ----------------------------------------------------------------------------- G

def street():
    lv = Level("g", "G", "The plaza, the station hall, the service yard and the avenue", W, H)
    # Outside: the plaza, the alley on the east, the avenue to the south.
    lv.area(0, 0, 75, 28, "plaza", OUT, "w")
    lv.area(76, 0, 81, 28, "alley", OUT, "w")
    lv.area(0, 29, 95, 29, "pave", OUT, "w")
    lv.area(0, 30, 95, 33, "road", OUT, "_")
    lv.area(0, 34, 95, 34, "pave", OUT, "w")
    lv.many("~", [(79, 14), (12, 31), (60, 33), (3, 4)])
    lv.many("%", [(80, 3), (81, 22), (24, 2), (2, 29)])
    lv.many("Z", [(77, 6), (77, 7)])

    # The station hall: double height over the ticket hall, the drop in its middle.
    lv.room(26, 6, 64, 28, "hall", PUBLIC)
    lv.windows(26, 64, 6, 6)
    lv.windows(26, 64, 28, 28, keep={(43, 28), (44, 28), (45, 28), (46, 28)})
    lv.windows(26, 26, 6, 28, keep={(26, 17), (26, 18)})
    lv.many("+", [(43, 28), (44, 28), (45, 28), (46, 28), (26, 17), (26, 18)])
    lv.area(42, 10, 50, 17, "drop", PUBLIC, "A")
    lv.many("r", [(x, 9) for x in range(41, 52)] + [(x, 18) for x in range(41, 52)])
    lv.many("r", [(41, y) for y in range(10, 18)] + [(51, y) for y in range(10, 18)])
    escalator(lv, 36, 10, 39, 14, "hall", PUBLIC)
    lv.many("r", [(35, y) for y in range(10, 15)] + [(40, y) for y in range(10, 15)])
    for p in STREET_LIFT:
        lv.put(*p, "L")
    lv.many("t", [(28, 9), (28, 11), (28, 13)])
    lv.many("V", [(31, 26), (60, 26)])
    lv.many("h", [(32, 21), (33, 21), (55, 22), (56, 22), (55, 24), (56, 24)])
    lv.many("p", [(30, 8), (62, 8), (62, 20)])
    lv.many("*", [(34, 16), (57, 14), (46, 22)])
    lv.many("i", [(38, 17), (44, 21), (48, 24), (53, 12), (58, 18), (31, 15), (45, 26)])
    lv.many("c", [(27, 7), (63, 7), (63, 27), (40, 27)])
    lv.many("g", [(46, 25), (59, 11)])
    # Transit police booth by the west door.
    lv.room(26, 20, 34, 28, "secure", STAFF)
    lv.put(34, 23, "+")
    lv.many("t", [(28, 22), (28, 23)])
    lv.put(31, 25, "g")
    lv.many("V", [(31, 26)])
    lv.put(31, 26, ".")
    # Staff door from the hall into the yard.
    lv.put(64, 12, "K")

    # The service yard off the alley: freight lift house, the emergency stair house.
    lv.area(65, 0, 74, 24, "yard", STAFF, "R")
    lv.many("f", [(75, y) for y in range(0, 26)] + [(x, 25) for x in range(65, 76)])
    lv.put(75, 10, "G")
    lv.put(74, 11, "c")
    lv.room(65, 1, 71, 6, "service", STAFF)
    for p in FLIFT:
        lv.put(*p, "S")
    lv.put(68, 6, "K")
    lv.many("q", [(67, 12), (68, 12), (69, 12)])
    lv.many("x", [(73, 2), (73, 3)])
    lv.many("g", [(72, 16), (67, 8)])
    lv.room(66, 18, 72, 24, "service", STAFF)
    lv.put(*ESTAIR, "^")
    lv.put(66, 21, "K")

    # The ventilation tower on the plaza: fans, a grille, the shaft ladder.
    lv.room(12, 8, 18, 13, "service", STAFF)
    lv.many("a", [(13, 9), (17, 9), (13, 12), (17, 12)])
    lv.put(*VENT, "H")
    lv.put(15, 13, "v")

    # The west street stair.
    lv.room(4, 23, 12, 28, "hall", PUBLIC)
    for p in KIOSK_W:
        lv.put(*p, "^")
    lv.many("+", [(7, 28), (8, 28)])
    lv.put(11, 24, "c")
    # The south street stair, across the avenue.
    lv.room(31, 35, 38, 39, "hall", PUBLIC)
    for p in KIOSK_S:
        lv.put(*p, "^")
    lv.many("+", [(34, 35), (35, 35)])

    # Plaza: a noodle stall, an ad pillar on the station's Net, people.
    lv.many("n", [(x, 17) for x in range(3, 10)])
    lv.many("s", [(x, 18) for x in range(3, 10, 2)])
    lv.put(2, 16, "*")
    lv.put(20, 22, "V")
    lv.many("p", [(22, 4), (22, 14), (8, 3)])
    lv.many("h", [(20, 6), (21, 6), (6, 12), (7, 12)])
    lv.many("*", [(24, 10), (24, 24), (73, 27)])
    lv.many("i", [(5, 19), (8, 19), (18, 20), (15, 25), (22, 27), (40, 29), (52, 29), (70, 27)])
    lv.many("c", [(25, 1)])
    lv.many("q", [(55, 31), (56, 31), (57, 31), (14, 32), (15, 32)])
    return lv


# ---------------------------------------------------------------------------- -1

def concourse():
    lv = Level("c1", "−1", "Ticket hall, the gate line, the paid concourse and the station office", W, H)
    # Ticket office, cash room and the staff corridor across the top.
    lv.room(28, 0, 42, 6, "warm", STAFF)
    lv.room(42, 0, 62, 6, "service", STAFF)
    lv.room(44, 0, 52, 4, "secure", SECURE)
    lv.put(48, 4, "K")
    lv.put(47, 1, "x")
    lv.put(51, 1, "c")
    lv.put(45, 3, "T")
    lv.put(42, 3, "+")
    lv.put(62, 3, "+")
    lv.many("d", [(31, 3), (35, 3)])
    lv.many("t", [(32, 3), (36, 3)])
    lv.put(40, 1, "x")
    lv.put(30, 1, "*")

    # The ticket hall (unpaid).
    lv.room(20, 6, 50, 22, "hall", PUBLIC)
    lv.many("n", [(x, 7) for x in range(31, 38)])
    lv.put(38, 6, "K")
    lv.room(20, 6, 28, 12, "warm", PUBLIC)
    lv.many("n", [(x, 8) for x in range(22, 27)])
    lv.many("l", [(21, 11), (22, 11), (23, 11)])
    lv.put(28, 10, "+")
    escalator(lv, 36, 15, 39, 19, "hall", PUBLIC)
    for p in STREET_LIFT:
        lv.put(*p, "L")
    lv.many("t", [(48, 9), (48, 11), (48, 17), (48, 19)])
    lv.many("V", [(21, 14), (33, 21)])
    lv.many("h", [(24, 18), (25, 18), (43, 20), (44, 20)])
    lv.many("i", [(30, 14), (34, 11), (42, 13), (45, 17), (27, 20), (41, 9)])
    lv.many("c", [(21, 7), (49, 7), (49, 21), (29, 21)])
    lv.many("*", [(30, 9), (44, 15)])
    lv.many("g", [(46, 12), (33, 3)])

    # The paid concourse beyond the gate line.
    lv.room(50, 6, 62, 26, "concourse", PAID)
    escalator(lv, 53, 18, 56, 22, "concourse", PAID)
    for p in PLAT_LIFT:
        lv.put(*p, "L")
    lv.many("h", [(52, 9), (52, 10), (60, 9), (60, 10)])
    lv.many("i", [(55, 12), (58, 15), (53, 24)])
    lv.put(61, 7, "c")
    lv.put(58, 12, "g")
    lv.put(51, 25, "c")
    lv.put(57, 8, "V")
    lv.put(62, 17, "K")
    # The gate line: turnstiles on the Net, a wide gate for staff.
    for y in range(8, 21):
        lv.put(50, y, "J" if y % 2 == 0 else "#")
    lv.put(50, 15, "K")

    # The station office block: freight lift lobby, security, the staff room.
    lv.room(62, 0, 74, 26, "service", STAFF)
    lv.hwall(62, 74, 7)
    lv.put(66, 7, "+")
    for p in FLIFT:
        lv.put(*p, "S")
    lv.room(62, 7, 74, 15, "secure", SECURE)
    lv.put(68, 15, "+")
    lv.many("t", [(64, 9), (65, 9), (66, 9), (67, 9)])
    lv.put(65, 10, "d")
    lv.put(70, 12, "g")
    lv.put(73, 8, "x")
    lv.put(73, 14, "c")
    lv.room(66, 19, 72, 23, "service", STAFF)
    lv.put(*ESTAIR, "^")
    lv.put(66, 21, "+")
    lv.many("l", [(73, 17), (73, 18), (73, 19), (73, 20)])
    lv.many("o", [(64, 23), (64, 24)])
    lv.put(63, 16, "c")

    # West subway from the street stair.
    lv.room(4, 13, 20, 19, "subway", PUBLIC)
    lv.room(4, 19, 12, 30, "subway", PUBLIC)
    lv.open([(x, 19) for x in range(6, 11)], "subway", PUBLIC)
    lv.open([(20, 15), (20, 16), (20, 17)], "subway", PUBLIC)
    for p in KIOSK_W:
        lv.put(*p, "^")
    lv.many("l", [(x, 14) for x in range(8, 19)])
    lv.many("x", [(9, 14), (13, 14), (17, 14)])
    lv.many("i", [(6, 16), (15, 17), (9, 23)])
    lv.put(5, 14, "c")
    lv.put(11, 29, "*")

    # South subway, under the avenue.
    lv.room(30, 22, 39, 39, "subway", PUBLIC)
    lv.open([(x, 22) for x in range(32, 38)], "subway", PUBLIC)
    for p in KIOSK_S:
        lv.put(*p, "^")
    lv.many("m", [(31, 28), (31, 29)])
    lv.many("i", [(37, 26), (33, 31)])
    lv.put(38, 33, "c")
    lv.put(36, 24, "*")
    return lv


# ---------------------------------------------------------------------------- -2

def spine():
    lv = Level("c2", "−2", "The Spine line: an island platform, a train in, tunnels both ways", W, H)
    lv.hwall(0, 95, 15)
    lv.hwall(0, 95, 28)
    lv.area(0, 16, 95, 17, "track", TRACK, "y")
    lv.area(0, 26, 95, 27, "track", TRACK, "y")
    # The tunnels past the platform ends, each track in its own tube.
    lv.hwall(0, 11, 18)
    lv.hwall(0, 11, 25)
    lv.hwall(76, 95, 18)
    lv.hwall(76, 95, 25)
    lv.vwall(11, 18, 25)
    lv.vwall(76, 18, 25)
    # The island platform.
    lv.area(12, 18, 75, 25, "platform", PAID, ".")
    lv.area(12, 18, 75, 18, "platform", PAID, ":")
    lv.area(12, 25, 75, 25, "platform", PAID, ":")
    escalator(lv, 50, 20, 53, 23, "platform", PAID)
    escalator(lv, 28, 20, 31, 23, "platform", PAID)
    escalator(lv, 38, 20, 41, 23, "platform", PAID)
    for p in PLAT_LIFT:
        lv.put(*p, "L")
    lv.many("#", [(58, 20), (61, 20), (58, 21), (59, 21), (60, 21), (61, 21)])
    for x in range(22, 64, 6):
        for y in (20, 23):
            if lv.at(x, y) == ".":
                lv.put(x, y, "P")
    lv.many("h", [(24, 21), (25, 21), (34, 22), (35, 22), (45, 21), (46, 21), (56, 22), (57, 22)])
    lv.many("i", [(23, 19), (33, 24), (36, 19), (44, 24), (48, 19), (55, 24), (62, 19), (27, 24)])
    lv.many("V", [(22, 21), (63, 22)])
    lv.many("c", [(13, 19), (62, 24), (43, 19)])
    lv.many("g", [(46, 22), (26, 19), (60, 24), (70, 22)])
    lv.many("*", [(19, 22), (55, 21)])
    # The staff end: the emergency stair, the Spine's signal box.
    lv.room(64, 19, 75, 24, "service", STAFF)
    lv.put(*ESTAIR, "^")
    lv.put(64, 21, "K")
    lv.put(73, 21, "t")
    lv.many("k", [(66, 20), (66, 23), (72, 23)])
    lv.put(74, 20, "c")
    # The cleaners' store at the west end.
    lv.room(12, 19, 19, 24, "service", STAFF)
    lv.put(19, 21, "+")
    lv.many("m", [(14, 20), (14, 21)])
    lv.put(17, 23, "x")
    # The 03:10 east, doors open.
    train_h(lv, 24, 69, 26, 27, "spine")
    return lv


# ---------------------------------------------------------------------------- -3

def keel():
    lv = Level("c3", "−3", "The Keel line under the Spine: two side platforms and the relay room", W, H)
    lv.area(44, 0, 47, 39, "track", TRACK, "Y")
    for x in (43, 48):
        lv.vwall(x, 0, 5)
        lv.vwall(x, 34, 39)
    # West and east platforms.
    lv.area(34, 6, 43, 33, "platform", PAID, ".")
    lv.area(43, 6, 43, 33, "platform", PAID, ":")
    lv.vwall(33, 5, 34)
    lv.hwall(33, 42, 5)
    lv.hwall(33, 42, 34)
    lv.area(48, 6, 57, 33, "platform", PAID, ".")
    lv.area(48, 6, 48, 33, "platform", PAID, ":")
    lv.vwall(58, 5, 34)
    lv.hwall(49, 58, 5)
    lv.hwall(49, 58, 34)
    escalator(lv, 36, 9, 39, 13, "platform", PAID)
    escalator(lv, 52, 9, 55, 13, "platform", PAID)
    lv.many("h", [(34, y) for y in (17, 18, 25, 26)] + [(57, y) for y in (17, 18, 25, 26)])
    lv.many("i", [(38, 16), (41, 22), (36, 29), (51, 19), (55, 28), (50, 31)])
    lv.many("V", [(34, 21), (57, 22)])
    lv.many("c", [(34, 6), (57, 6), (57, 33), (34, 33)])
    lv.many("g", [(52, 25), (38, 8)])
    lv.many("*", [(39, 24), (53, 16)])
    lv.many("Q", [(34, 32), (35, 33), (36, 33), (34, 31)])
    # Platform lift lobby, off the east platform.
    lv.room(58, 17, 64, 23, "concourse", PAID)
    for p in PLAT_LIFT:
        lv.put(*p, "L")
    lv.put(58, 20, "+")
    lv.put(64, 20, "K")
    # Staff passage: the emergency stair, and a corridor to a door onto the track.
    lv.room(64, 16, 75, 26, "service", STAFF)
    lv.put(*ESTAIR, "^")
    lv.many("l", [(74, 17), (74, 18), (74, 19)])
    lv.room(48, 1, 69, 4, "service", STAFF)
    lv.room(65, 4, 69, 16, "service", STAFF)
    lv.open([(66, 4), (67, 4), (68, 4), (66, 16), (67, 16), (68, 16)], "service", STAFF)
    lv.put(48, 2, "K")
    lv.put(67, 10, "c")
    # The Keel's relay room off the west platform.
    lv.room(20, 12, 33, 22, "secure", STAFF)
    lv.put(33, 17, "K")
    lv.put(22, 17, "t")
    lv.many("k", [(25, 14), (27, 14), (29, 14), (25, 20), (27, 20), (29, 20)])
    lv.put(31, 13, "c")
    # The 03:12 north, standing at the east platform.
    for y in range(0, 15):
        lv.put(46, y, "O" if y % 3 == 1 else "C")
        lv.put(47, y, "D" if y in (7, 10, 13) else "C")
        for x in (46, 47):
            lv.kind[y][x] = "keel"
            lv.zone[y][x] = TRACK
    return lv


# ---------------------------------------------------------------------------- -4

def service():
    lv = Level("s4", "−4", "Service level: fans, the substation, the duty room and Line Control", W, H)
    # The corridor along the level.
    lv.room(4, 17, 64, 22, "service", STAFF)
    # North: fan room, substation, duty room, freight bay.
    lv.room(4, 2, 22, 17, "service", STAFF)
    lv.put(*VENT, "H")
    for fx, fy in ((7, 5), (7, 11), (18, 5), (18, 11)):
        lv.many("m", [(fx, fy), (fx + 1, fy), (fx, fy + 1), (fx + 1, fy + 1)])
    lv.put(13, 17, "+")
    lv.put(20, 3, "t")
    lv.room(22, 2, 42, 17, "power", STAFF)
    for x in range(25, 40, 4):
        lv.many("a", [(x, 5), (x + 1, 5), (x, 6), (x + 1, 6), (x, 11), (x + 1, 11), (x, 12), (x + 1, 12)])
    lv.many("t", [(24, 15), (25, 15), (26, 15)])
    lv.put(32, 17, "K")
    lv.put(41, 3, "c")
    lv.room(42, 2, 58, 17, "warm", STAFF)
    lv.many("b", [(44, 4), (44, 5), (47, 4), (47, 5), (50, 4), (50, 5)])
    lv.many("l", [(57, 4), (57, 5), (57, 6), (57, 7)])
    lv.many("o", [(49, 11), (50, 11)])
    lv.many("s", [(48, 11), (51, 11)])
    lv.many("g", [(46, 12), (53, 9), (49, 14)])
    lv.put(50, 17, "+")
    lv.put(54, 3, "*")
    lv.room(58, 0, 76, 17, "service", STAFF)
    for p in FLIFT:
        lv.put(*p, "S")
    lv.many("x", [(62, 8), (63, 8), (62, 9), (72, 6), (72, 7)])
    lv.many("q", [(66, 12), (67, 12)])
    lv.put(74, 9, "T")
    lv.put(70, 14, "g")
    lv.put(60, 17, "+")
    lv.put(75, 1, "c")
    # The stair room at the corridor's east end.
    lv.room(64, 17, 76, 25, "service", STAFF)
    lv.put(*ESTAIR, "^")
    lv.put(64, 19, "K")
    lv.put(75, 24, "c")
    lv.put(74, 18, "T")
    lv.put(70, 23, "g")
    # South: the cable vault (and the old shaft), Line Control, the pump room.
    lv.room(4, 22, 22, 37, "old", STAFF)
    lv.put(13, 22, "K")
    lv.many("k", [(x, 24) for x in range(16, 21)] + [(x, 35) for x in range(16, 21)])
    lv.room(6, 27, 14, 34, "old", STAFF)
    lv.put(*OLDSHAFT, "H")
    lv.put(10, 27, "K")
    lv.put(5, 23, "c")
    lv.room(22, 22, 46, 37, "secure", SECURE)
    lv.put(34, 22, "K")
    for x in (25, 28, 31):
        lv.many("k", [(x, y) for y in range(28, 35)])
    lv.many("t", [(x, 25) for x in range(36, 44)])
    lv.many("d", [(x, 26) for x in range(37, 43, 2)])
    lv.put(34, 34, "X")
    lv.put(22, 30, "v")
    lv.put(40, 28, "i")
    lv.many("g", [(37, 31), (43, 33), (24, 35)])
    lv.put(30, 24, "T")
    lv.many("c", [(23, 23), (45, 36)])
    lv.room(46, 22, 64, 37, "service", STAFF)
    lv.put(55, 22, "+")
    lv.area(50, 29, 60, 34, "sump", STAFF, "W")
    lv.many("m", [(49, 25), (50, 25), (53, 25), (54, 25), (57, 25), (58, 25)])
    lv.many("c", [(40, 18), (8, 21)])
    lv.many("g", [(27, 19), (58, 20)])
    return lv


# ---------------------------------------------------------------------------- -5

def oldline():
    lv = Level("s5", "−5", "The old line, closed before the city was built over it; a gang lives here", W, H)
    lv.hwall(0, 83, 22)
    lv.area(0, 20, 83, 21, "track", TRACK, "y")
    lv.hwall(0, 11, 19)
    lv.hwall(66, 83, 19)
    # The old platform.
    lv.area(12, 13, 65, 19, "old", GANG, ".")
    lv.area(12, 19, 65, 19, "old", GANG, ":")
    lv.hwall(11, 66, 12)
    lv.vwall(11, 12, 19)
    lv.vwall(66, 12, 19)
    # The old ticket hall behind it, bricked up from above: the gang's den.
    lv.room(24, 4, 46, 12, "old", GANG)
    lv.put(34, 12, "+")
    lv.many("x", [(26, 6), (27, 6), (44, 6), (44, 7)])
    lv.many("o", [(34, 7), (35, 7)])
    lv.many("s", [(33, 7), (36, 7)])
    lv.many("g", [(35, 9), (28, 8)])
    lv.put(30, 5, "*")
    lv.put(25, 10, "m")
    # The squat on the platform.
    lv.many("b", [(14, 14), (14, 15), (18, 14), (18, 15), (55, 14), (55, 15), (60, 14), (60, 15)])
    lv.many("z", [(22, 16), (48, 15), (38, 17)])
    lv.many("g", [(21, 15), (40, 15), (52, 17), (62, 16)])
    lv.many("x", [(29, 14), (30, 14), (64, 13)])
    lv.many("*", [(16, 13), (44, 13), (58, 13)])
    lv.many("~", [(33, 18), (47, 18), (26, 17)])
    lv.many("Q", [(1, 20), (2, 20), (1, 21), (2, 21), (3, 21), (4, 20)])
    # The old shaft, and a cut through the tunnel wall onto the track.
    lv.room(4, 26, 16, 34, "old", STAFF)
    lv.put(*OLDSHAFT, "H")
    lv.room(11, 22, 14, 26, "old", STAFF)
    lv.open([(12, 22), (13, 22), (12, 26), (13, 26)], "old", STAFF)
    lv.many("Q", [(14, 32), (15, 32), (15, 33)])
    # East: a rusted gate, then the sewer that runs under the whole Undercity.
    lv.many("G", [(82, 20), (82, 21)])
    lv.vwall(83, 0, 39)
    lv.open([(83, 20), (83, 21)], "track", TRACK)
    lv.area(84, 0, 95, 39, "sewer", OUT, ".")
    lv.area(88, 0, 91, 39, "sewer", OUT, "W")
    lv.many("~", [(85, 9), (93, 27)])
    lv.put(86, 30, "%")
    return lv


LEVELS = [street(), concourse(), spine(), keel(), service(), oldline()]

TITLE = "Cinder Cross"
TAB = "METRO"
SIZE = "large"
LEDE = (
    "A metro interchange where the Spine line crosses the Keel: six levels from the plaza "
    "down to a line closed before the city was built over it. Five locks stand between the "
    "street and the ledger, and the trains keep running while you work."
)
INPUTS = [
    ("TYPE", "metro station · interchange"),
    ("SIZE", "large · 6 levels"),
    ("OBJECTIVE", "the Spine's ledger in Line Control"),
    ("SECURITY", "320"),
    ("TIME", "03:00"),
    ("SEED", "8c03"),
]

# (level, x, y, note): consecutive steps on one level are joined by the shortest walk;
# a step on another level is a ride, a climb or a train.
ROUTES = [
    dict(key="solo", name="SOLO", colour="#ff5a3c",
         line="Loud, and straight down: over the turnstiles, down the emergency stair, through the "
              "blast door. Out on foot along the Spine tunnel, with a train due.",
         crawl=False, steps=[
             ("g", 44, 29, None),
             ("g", 44, 28, "In at the front doors at three in the morning. The hall is nearly empty."),
             ("g", 37, 12, "Down the escalators."),
             ("c1", 37, 17, None),
             ("c1", 50, 14, "Vault the turnstiles. The gate line is on the station's Net: the alert starts climbing."),
             ("c1", 62, 17, "The transit cop by the lifts comes at you. Force the staff door."),
             ("c1", 69, 21, "Into the emergency stair: four floors down, past both platforms."),
             ("s4", 69, 21, None),
             ("s4", 64, 19, "The stair room's turret wakes. Force its door onto the corridor."),
             ("s4", 50, 18, "Three guards come out of the duty room."),
             ("s4", 34, 22, "Blow Line Control's blast door: the loudest thing you'll do tonight."),
             ("s4", 30, 25, "The turret over the door opens up, and so do the guards inside."),
             ("s4", 34, 33, "Pull the ledger from the core."),
             ("s4", 69, 21, "Back to the emergency stair before the lockdown seals it."),
             ("c2", 69, 21, None),
             ("c2", 64, 21, "Up two floors, out through the Spine's signal box."),
             ("c2", 75, 25, "Along the platform edge to its east end."),
             ("c2", 95, 26, "Down onto the track and run the tunnel east. The next train is due in forty turns."),
         ]),
    dict(key="net", name="NETRUNNER", colour="#3cf0ff",
         line="The ad pillar on the plaza is the way into the station's Net. Every gate, lift, "
              "door and turret below is a node on it, and so are the trains: it leaves on one.",
         crawl=False, steps=[
             ("g", 18, 24, None),
             ("g", 19, 22, "Jack the ad pillar on the plaza. Behind it is the station's Net."),
             ("g", 26, 17, "Loop the hall's cameras and walk in at the west door."),
             ("g", 46, 8, "Take the street lift down."),
             ("c1", 46, 8, None),
             ("c1", 50, 14, "Open a turnstile over the Net. No alert."),
             ("c1", 62, 17, "The staff door opens the same way."),
             ("c1", 68, 15, "Through security, where the guard is watching monitors you've looped."),
             ("c1", 67, 3, "Send the freight lift to −4 without a card."),
             ("s4", 67, 3, None),
             ("s4", 72, 9, "The freight bay's turret is a node: shut it down."),
             ("s4", 60, 17, "Out into the corridor."),
             ("s4", 34, 22, "Line Control's blast door is a node too. Open it."),
             ("s4", 30, 25, "Turn the door's turret on the guards inside."),
             ("s4", 34, 33, "The core is the deepest node, behind its own ICE. Copy the ledger."),
             ("s4", 38, 26, "From the consoles, hold the Keel train at its platform with the doors open."),
             ("s4", 69, 21, "Up the emergency stair. Its doors are nodes too."),
             ("c3", 69, 21, None),
             ("c3", 58, 20, "Through the staff passage and the lift lobby onto the Keel's east platform."),
             ("c3", 47, 10, "Board the held train."),
             ("c3", 47, 0, "Let it go. Out north towards Northgate, the doors shut behind you.", "ride"),
         ]),
    dict(key="tech", name="TECHIE", colour="#ffc23c",
         line="Shafts and ducts: down the ventilation tower, through a cable duct into Line "
              "Control, out through the old line and the sewer.",
         crawl=True, steps=[
             ("g", 15, 16, None),
             ("g", 15, 13, "Unscrew the grille on the plaza's ventilation tower."),
             ("g", 15, 10, "Climb down the shaft: four floors of ladder, with the fans turning below."),
             ("s4", 15, 10, None),
             ("s4", 20, 4, "Stop the fans at their panel."),
             ("s4", 13, 17, "Out into the service corridor behind the patrol."),
             ("s4", 13, 22, "Keyjack the cable vault."),
             ("s4", 22, 30, "Crawl the cable duct into Line Control."),
             ("s4", 23, 30, "Out through a grille behind the racks. The turret watches the door, not the racks."),
             ("s4", 34, 33, "Keyjack the core's housing and pull the drive."),
             ("s4", 22, 30, "Back through the duct."),
             ("s4", 10, 27, "Keyjack the rusted door on the old shaft."),
             ("s4", 10, 30, "Down the old shaft."),
             ("s5", 10, 30, None),
             ("s5", 12, 22, "Through a cut in the old tunnel's wall."),
             ("s5", 80, 21, "East along the old track, below the gang's squat."),
             ("s5", 82, 21, "Keyjack the rusted gate from the inside."),
             ("s5", 86, 26, "Out into the sewer that runs under the whole Undercity."),
         ]),
    dict(key="fixer", name="FIXER", colour="#d26bff",
         line="Talk. The night foreman owes you, and his crew's vest and lift card get you as far "
              "as the core.",
         crawl=False, steps=[
             ("g", 36, 34, None),
             ("g", 34, 35, "Down the south street stair, across the avenue from the hall."),
             ("g", 34, 37, None),
             ("c1", 34, 37, None),
             ("c1", 35, 8, "At the ticket office window, ask for the night foreman by name."),
             ("c1", 50, 15, "He walks you through the staff gate."),
             ("c1", 72, 18, "A hi-vis vest and the crew's lift card, out of his locker."),
             ("c1", 67, 3, "The card calls the freight lift to −4."),
             ("s4", 67, 3, None),
             ("s4", 50, 18, "The duty room's guards see a vest, not a face."),
             ("s4", 34, 22, "The controller lets the night crew in to check the relays."),
             ("s4", 34, 33, "Copy the ledger while he makes the tea."),
             ("s4", 67, 3, "Back up in the freight lift."),
             ("c1", 67, 3, None),
             ("c1", 50, 15, "Out through the staff gate with a nod to the foreman."),
             ("c1", 34, 37, "Back up the south stair."),
             ("g", 34, 37, None),
             ("g", 36, 34, "Gone before the 03:10 leaves."),
         ]),
]

# Top of the site first.
ORDER = ["g", "c1", "c2", "c3", "s4", "s5"]

# The links between levels: which floors each one serves.
LINKS = [
    ("⇅", "#dcebf5", "Street stairs", {"g", "c1"}, "the west and south kiosks"),
    ("↓", "#dcebf5", "Hall escalators", {"g", "c1"}, "into the ticket hall"),
    ("≡", "#dcebf5", "Street lift", {"g", "c1"}, ""),
    ("↓", "#dcebf5", "Platform escalators", {"c1", "c2"}, "behind the gate line"),
    ("≡", "#dcebf5", "Platform lift", {"c1", "c2", "c3"}, "behind the gate line"),
    ("↓", "#dcebf5", "Interchange escalators", {"c2", "c3"}, "Spine to Keel"),
    ("≡", "#ff9a3c", "Freight lift", {"g", "c1", "s4"}, "staff card; passes the platforms"),
    ("⇅", "#ff9a3c", "Emergency stair", {"g", "c1", "c2", "c3", "s4"}, "card doors top and bottom"),
    ("╫", "#dcebf5", "Ventilation shaft", {"g", "s4"}, "a grille, then a long ladder"),
    ("╫", "#dcebf5", "Old shaft", {"s4", "s5"}, "behind a rusted door"),
    ("╪", "#8c7d5f", "Spine tunnels", {"c2"}, "east and west, onto the city's rail"),
    ("╫", "#8c7d5f", "Keel tunnels", {"c3"}, "north and south"),
    ("≈", "#469646", "The sewer", {"s5"}, "through the old line's gate"),
]

ZONES = [
    ("public", "#2f7fd8", "public"),
    ("paid", "#1fbfa0", "paid, past the gates"),
    ("staff", "#e0a22e", "staff"),
    ("secure", "#ff3c46", "secure"),
    ("track", "#c7cdd6", "track"),
    ("gang", "#7bd14a", "gang"),
]

TRAITS = [
    ("Size", "Six levels and about three times the hotel's floor. The plaza, three passenger levels, "
             "a service level and the old line under it all."),
    ("Ways in", "Two street stairs, the hall's front and west doors, the street lift, the yard's freight "
                "lift, the ventilation tower, four tunnels, the sewer through the old line, and the "
                "Net from the ad pillar."),
    ("Depth", "Five locks from the street to the ledger: the gate line, a staff door, the service "
              "level's doors, Line Control's blast door and the core's own ICE."),
    ("Danger that moves", "Trains on a timetable, the duty room's guards as reinforcements, a gang "
                          "under the station that answers to nobody."),
    ("Height", "Thirteen links. The freight lift and the ventilation shaft skip floors, the emergency "
               "stair only opens at both ends, and the escalators come out a few steps from where "
               "they started."),
    ("Set pieces", "The hall over its drop, the gate line, the island platform with a train in, the "
                   "substation, Line Control, the old platform."),
]

# What the station sets up for the metro system.
SYSTEM = [
    ("Lines", "A line is a stretch of the city's Undercity rail. The game already lays one across "
              "every city (verticals.rs); stations stand on it, and a platform's tunnel mouths are "
              "the site's exits onto it."),
    ("Trains", "Trains run on a timetable. The track is walkable between trains; a train at the "
               "platform is cover, and a way out."),
    ("Riding", "Riding is travel between stations: a ride starts and ends on a platform, and a "
               "station's platforms are where a job can start or finish."),
    ("Kinds of station", "A stop (one platform, one stair), an interchange like this one, a terminus "
                         "with sidings and a depot. One type in sites.ron, sized by line count."),
    ("Line Control", "One per line: the node that holds trains, reroutes them and opens the tunnels' "
                     "doors. Hacking it changes the network for a while, not just this station."),
    ("Old lines", "Closed lines under the new ones: squats, gangs and a way in from the sewer. A kind "
                  "of level any deep site can have."),
]

# The network around the station, for the line card.
SPINE = ["Tallow Docks", "Kiln Street", "Cinder Cross", "Vesper Market", "Bastion Yards"]
KEEL = ["Northgate", "Cinder Cross", "Sumpside"]

if __name__ == "__main__":
    for lv in LEVELS:
        lv.dump()
