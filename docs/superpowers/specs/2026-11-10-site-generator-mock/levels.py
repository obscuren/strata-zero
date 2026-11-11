"""A hand-built example of what the site generator should make: a hotel, four levels.

Design characters (rendered to the game's glyphs by gen.py):
  ' ' outside     '#' wall     '=' window   '.' floor    '+' door     'K' keycard door
  'L' guest lift  'S' service lift          '^' stair    'H' ladder   'v' vent grille
  'A' open drop   'r' railing  'c' camera   'T' turret   'g' guard    'x' container
  'X' objective   'b' bed      'd' desk     'o' table    's' stool    'n' counter
  'l' locker      'm' machine  't' terminal 'k' rack     'h' couch    'p' plant
  '*' lamp        'R' outdoor ground        'a' AC unit  'q' car      'w' pavement
  '_' road        '%' trash    '~' puddle   'f' fence    'G' gate     'P' pillar
  '$' bar         'u' pool     'j' skylight 'Z' bin
"""

W, H = 46, 28

# What each zone is: who belongs there.
PUBLIC, GUEST, STAFF, SECURE, OUT = "public", "guest", "staff", "secure", "out"


class Level:
    def __init__(self, key, label, blurb):
        self.key = key
        self.label = label
        self.blurb = blurb
        self.w = W
        self.h = H
        self.g = [[" "] * W for _ in range(H)]
        self.kind = [[None] * W for _ in range(H)]
        self.zone = [[None] * W for _ in range(H)]

    def put(self, x, y, c):
        self.g[y][x] = c

    def at(self, x, y):
        return self.g[y][x]

    def area(self, x0, y0, x1, y1, kind, zone, c=None):
        for y in range(y0, y1 + 1):
            for x in range(x0, x1 + 1):
                if c is not None:
                    self.g[y][x] = c
                self.kind[y][x] = kind
                self.zone[y][x] = zone

    def room(self, x0, y0, x1, y1, kind, zone):
        """Walls on the border (shared with neighbours), floor inside."""
        for y in range(y0, y1 + 1):
            for x in range(x0, x1 + 1):
                edge = x in (x0, x1) or y in (y0, y1)
                if edge:
                    if self.g[y][x] not in "+K=":
                        self.g[y][x] = "#"
                else:
                    self.g[y][x] = "."
                    self.kind[y][x] = kind
                    self.zone[y][x] = zone

    def hwall(self, x0, x1, y):
        for x in range(x0, x1 + 1):
            self.g[y][x] = "#"

    def vwall(self, x, y0, y1):
        for y in range(y0, y1 + 1):
            self.g[y][x] = "#"

    def windows(self, x0, x1, y0, y1, keep=()):
        """Windows on every other wall tile of a straight run of outer wall."""
        pts = [(x, y) for y in range(y0, y1 + 1) for x in range(x0, x1 + 1)]
        for i, (x, y) in enumerate(pts):
            if i % 2 == 1 and self.g[y][x] == "#" and (x, y) not in keep:
                self.g[y][x] = "="

    def many(self, c, pts):
        for x, y in pts:
            self.g[y][x] = c


# Shared anchors: the same tile on every level they stand on.
SSTAIR = (2, 3)               # service stair: B1, G, 2F, PH
SLIFT = [(4, 3), (5, 3)]      # service lift: B1, G, 2F
GLIFT = [(15, 13), (16, 13)]  # guest lift: B1, G, 2F, PH (a keycard for PH)
GRAND = [(21, 20), (22, 20)]  # the lobby's grand stair: G and 2F only
RISER = (11, 6)               # maintenance riser, a ladder behind a grille: every level
ESCAPE = (19, 10)             # fire escape: G yard, 2F balcony, PH ledge
RAMP = [(37, 3), (38, 3)]     # car ramp: G yard, B1 car park


def core(lv, slift=True):
    """The service core in the tower's north-west corner: stair and service lift."""
    lv.room(0, 0, 7, 7, "service", STAFF)
    lv.put(*SSTAIR, "^")
    if slift:
        for p in SLIFT:
            lv.put(*p, "S")


def riser(lv, room_kind, zone):
    """The riser closet: a ladder shaft with a grille onto the room north of it."""
    x, y = RISER
    lv.many("#", [(x - 1, y - 1), (x + 1, y - 1), (x - 1, y), (x + 1, y), (x - 1, y + 1), (x, y + 1), (x + 1, y + 1)])
    lv.put(x, y - 1, "v")
    lv.put(x, y, "H")
    lv.kind[y][x] = "service"
    lv.zone[y][x] = STAFF


def glift(lv):
    for p in GLIFT:
        lv.put(*p, "L")


# ---------------------------------------------------------------------------- B1

def b1():
    lv = Level("b1", "B1", "Car park, laundry, security and the server room")
    lv.room(0, 0, 40, 24, "service", STAFF)
    core(lv)
    lv.put(3, 7, "+")
    # Stores over the riser.
    lv.room(7, 0, 17, 7, "service", STAFF)
    lv.put(7, 4, "+")
    lv.put(14, 7, "+")
    lv.many("l", [(9, 1), (10, 1), (13, 1), (14, 1), (15, 1)])
    lv.many("x", [(15, 5), (16, 5)])
    riser(lv, "service", STAFF)
    # Corridor under the core and stores.
    lv.area(1, 8, 16, 9, "service", STAFF)
    # Car park under the yard: pillars, cars, the ramp up, a dock.
    lv.room(17, 0, 40, 12, "park", STAFF)
    lv.many("P", [(23, 4), (29, 4), (34, 4), (23, 8), (29, 8), (34, 8)])
    lv.many("q", [(19, 2), (20, 2), (25, 2), (26, 2), (31, 6), (32, 6), (25, 10), (26, 10)])
    for p in RAMP:
        lv.put(*p, "^")
    lv.many("f", [(36, 2), (36, 4), (39, 2), (39, 4)])
    lv.many("x", [(37, 10), (38, 10)])
    lv.put(39, 10, "Z")
    lv.put(18, 11, "c")
    lv.put(27, 6, "g")
    # The guest lift comes down to a lobby off the car park.
    lv.room(10, 10, 17, 14, "lobby", PUBLIC)
    glift(lv)
    lv.put(13, 10, "+")
    lv.put(17, 11, "+")
    # Laundry and staff lockers on the west side.
    lv.room(0, 10, 10, 17, "service", STAFF)
    lv.put(5, 10, "+")
    lv.many("m", [(2, 12), (4, 12), (6, 12), (8, 12), (2, 15), (4, 15)])
    lv.room(0, 17, 10, 24, "service", STAFF)
    lv.put(5, 17, "+")
    lv.many("l", [(1, 19), (1, 20), (1, 21), (1, 22)])
    lv.put(9, 23, "x")
    lv.put(10, 20, "+")
    # Security office: monitors, the guard, the master keycard in a locker.
    lv.room(10, 14, 20, 24, "secure", SECURE)
    lv.put(13, 14, "K")
    lv.many("t", [(12, 16), (13, 16), (14, 16)])
    lv.put(13, 17, "d")
    lv.put(16, 20, "g")
    lv.put(19, 23, "x")
    lv.put(19, 15, "c")
    # Server room: racks, the site's server, a turret on the door.
    lv.room(20, 12, 40, 24, "secure", SECURE)
    lv.put(20, 19, "K")
    for x in (24, 26, 28, 30, 32):
        lv.many("k", [(x, 14), (x, 15), (x, 17), (x, 18), (x, 21), (x, 22)])
    lv.put(37, 18, "t")
    lv.put(22, 23, "T")
    lv.put(39, 13, "c")
    lv.put(35, 12, "v")
    lv.many("#", [(18, 13), (19, 13)])
    return lv


# ----------------------------------------------------------------------------- G

def ground():
    lv = Level("g", "G", "Lobby, bar and the service yard")
    # Outside: the avenue to the south, the alley to the east.
    lv.area(0, 25, 45, 25, "pave", OUT, "w")
    lv.area(0, 26, 45, 27, "road", OUT, "_")
    lv.area(41, 0, 45, 24, "pave", OUT, "w")
    lv.many("%", [(43, 4), (44, 21)])
    lv.many("~", [(42, 15), (13, 26)])
    # The yard: fenced off the alley, the ramp down, bins, a van, the fire escape.
    lv.area(18, 0, 40, 11, "yard", STAFF, "R")
    lv.many("f", [(x, 0) for x in range(18, 41)] + [(40, y) for y in range(0, 12)])
    lv.put(40, 6, "G")
    lv.put(39, 7, "c")
    for p in RAMP:
        lv.put(*p, "^")
    lv.many("f", [(36, 2), (36, 4), (39, 2), (39, 4)])
    lv.many("Z", [(22, 1), (23, 1)])
    lv.put(25, 2, "%")
    lv.many("q", [(29, 8), (30, 8), (31, 8)])
    lv.put(26, 6, "g")
    lv.put(*ESCAPE, "H")
    # The tower: service core and kitchen to the north.
    lv.room(0, 0, 18, 24, "service", STAFF)
    core(lv)
    lv.put(3, 7, "+")
    lv.room(7, 0, 18, 7, "service", STAFF)
    lv.put(7, 4, "+")
    lv.put(14, 7, "+")
    lv.put(18, 4, "K")
    lv.put(17, 2, "c")
    lv.many("n", [(9, 1), (13, 1), (14, 1), (15, 1), (16, 1)])
    lv.many("m", [(9, 3), (9, 4)])
    riser(lv, "service", STAFF)
    lv.windows(0, 18, 0, 0)
    lv.windows(0, 0, 0, 24, keep={(0, 3)})
    # Staff corridor, lockers, the manager's office, the lift hall.
    lv.area(1, 8, 17, 9, "service", STAFF)
    lv.room(0, 10, 9, 15, "service", STAFF)
    lv.put(5, 10, "+")
    lv.many("l", [(2, 14), (3, 14), (4, 14), (6, 14), (7, 14)])
    lv.put(8, 11, "x")
    lv.room(9, 10, 14, 15, "warm", STAFF)
    lv.put(11, 10, "+")
    lv.put(12, 12, "d")
    lv.put(13, 12, "t")
    lv.put(10, 14, "x")
    lv.put(10, 11, "*")
    lv.room(14, 10, 18, 15, "lobby", PUBLIC)
    glift(lv)
    lv.put(16, 10, "K")
    lv.put(18, 13, ".")
    lv.put(18, 14, ".")
    # The bar: its own door to the avenue, a back door, a door to the lobby.
    lv.room(0, 15, 18, 24, "bar", PUBLIC)
    lv.windows(0, 18, 24, 24, keep={(7, 24), (8, 24)})
    lv.many("+", [(7, 24), (8, 24), (18, 20)])
    lv.put(5, 15, "K")
    lv.many("$", [(x, 17) for x in range(3, 15)])
    lv.many("s", [(x, 18) for x in range(4, 15, 2)])
    for x in (3, 7, 11, 15):
        lv.put(x, 21, "o")
        lv.put(x + 1, 21, "s")
    lv.put(1, 22, "*")
    lv.put(16, 23, "*")
    # Lobby: double height, the drop above it, the grand stair to the mezzanine.
    lv.room(18, 12, 40, 24, "lobby", PUBLIC)
    lv.put(18, 13, ".")
    lv.put(18, 14, ".")
    lv.windows(18, 40, 24, 24, keep={(28, 24), (29, 24), (30, 24)})
    lv.windows(40, 40, 12, 24, keep={(40, 18)})
    lv.many("+", [(28, 24), (29, 24), (30, 24), (40, 18)])
    for p in GRAND:
        lv.put(*p, "^")
    lv.many("n", [(x, 15) for x in range(32, 38)])
    lv.put(35, 14, "t")
    lv.put(33, 13, "g")
    lv.many("h", [(26, 19), (27, 19), (33, 20), (34, 20)])
    lv.many("p", [(30, 18), (25, 22), (38, 22)])
    lv.many("c", [(19, 16), (39, 23)])
    lv.many("*", [(22, 16), (30, 13), (36, 21)])
    return lv


# ---------------------------------------------------------------------------- 2F

def second():
    lv = Level("f2", "2F", "Rooms, the balcony and the mezzanine round the drop")
    lv.room(0, 0, 18, 24, "warm", GUEST)
    core(lv)
    lv.put(3, 7, "+")
    lv.windows(0, 18, 0, 0)
    lv.windows(0, 0, 0, 24, keep={(0, 3)})
    # Room 205 over the kitchen, the riser behind its wardrobe.
    lv.room(7, 0, 18, 7, "warm", GUEST)
    lv.put(14, 7, "+")
    lv.many("b", [(16, 2), (16, 3)])
    lv.put(9, 2, "d")
    lv.put(9, 5, "x")
    riser(lv, "warm", GUEST)
    # Corridor: across under the core, then down the tower.
    lv.area(1, 8, 17, 9, "corr", GUEST)
    lv.area(8, 10, 9, 23, "corr", GUEST)
    # West rooms 201-203.
    for i, (y0, y1) in enumerate(((10, 15), (15, 20), (20, 24))):
        lv.room(0, y0, 7, y1, "warm", GUEST)
        lv.put(7, y0 + 2, "+")
        lv.put(2, y0 + 2, "b")
        lv.put(2, y0 + 3, "b") if y1 - y0 > 4 else None
        lv.put(5, y1 - 1, "d")
    lv.put(4, 22, "x")
    lv.vwall(7, 10, 24)
    lv.many("+", [(7, 12), (7, 17), (7, 22)])
    # The lift hall opens onto the mezzanine.
    lv.room(10, 10, 18, 15, "lobby", PUBLIC)
    glift(lv)
    lv.put(10, 12, "+")
    lv.put(18, 13, ".")
    lv.put(18, 14, ".")
    lv.put(11, 11, "c")
    # The corner suite.
    lv.room(10, 15, 18, 24, "warm", GUEST)
    lv.put(10, 19, "+")
    lv.many("b", [(15, 21), (16, 21), (15, 22), (16, 22)])
    lv.many("h", [(12, 17), (13, 17)])
    lv.put(17, 17, "x")
    lv.put(17, 23, "*")
    lv.windows(0, 18, 24, 24)
    # The balcony over the yard, the fire escape at its corner.
    lv.put(18, 8, "+")
    lv.area(19, 7, 22, 11, "yard", OUT, "R")
    lv.many("r", [(19, 7), (20, 7), (21, 7), (22, 7), (22, 8), (22, 9), (22, 10), (22, 11), (19, 11), (20, 11), (21, 11)])
    lv.put(*ESCAPE, "H")
    # The mezzanine round the drop over the lobby.
    lv.room(18, 12, 40, 24, "lobby", PUBLIC)
    lv.put(18, 13, ".")
    lv.put(18, 14, ".")
    lv.windows(18, 40, 24, 24)
    lv.area(26, 15, 34, 21, "drop", PUBLIC, "A")
    lv.many("r", [(x, 14) for x in range(25, 36)] + [(x, 22) for x in range(25, 36)])
    lv.many("r", [(25, y) for y in range(15, 22)] + [(35, y) for y in range(15, 22)])
    for p in GRAND:
        lv.put(*p, "^")
    # A meeting room off the east side of the mezzanine.
    lv.room(36, 12, 40, 24, "lobby", PUBLIC)
    lv.put(36, 17, "+")
    lv.many("o", [(38, 15), (38, 16), (38, 18), (38, 19)])
    lv.windows(40, 40, 12, 24)
    lv.put(30, 13, "c")
    lv.put(31, 23, "g")
    lv.many("p", [(19, 23), (24, 13)])
    lv.put(20, 16, "*")
    return lv


# ---------------------------------------------------------------------------- PH

def penthouse():
    lv = Level("ph", "PH", "The penthouse in the tower; the roof of the lobby is its terrace")
    lv.room(0, 0, 18, 24, "warm", SECURE)
    core(lv, slift=False)
    lv.put(3, 7, "+")
    lv.many("n", [(4, 1), (5, 1)])
    lv.windows(0, 18, 0, 0)
    lv.windows(0, 0, 0, 24, keep={(0, 3)})
    # Bedroom, the riser's grille behind the wardrobe.
    lv.room(7, 0, 18, 10, "warm", SECURE)
    lv.put(7, 8, "+")
    lv.put(13, 10, "+")
    lv.many("b", [(15, 2), (16, 2), (15, 3), (16, 3)])
    lv.put(9, 2, "x")
    lv.put(16, 8, "*")
    riser(lv, "warm", SECURE)
    # A hall under the core, the lounge down the west side with the bodyguard.
    lv.area(1, 8, 6, 9, "warm", SECURE)
    lv.room(0, 10, 10, 24, "warm", SECURE)
    lv.put(5, 10, "+")
    lv.many("h", [(3, 13), (4, 13), (5, 13)])
    lv.put(4, 15, "o")
    lv.put(2, 21, "*")
    lv.put(6, 17, "g")
    lv.put(10, 13, "+")
    # Foyer at the lift, a turret watching the doors.
    lv.room(10, 10, 18, 16, "lobby", SECURE)
    glift(lv)
    lv.put(17, 11, "T")
    lv.put(11, 15, "c")
    lv.put(18, 14, "K")
    lv.put(13, 16, "K")
    # The office and its safe.
    lv.room(10, 16, 18, 24, "secure", SECURE)
    lv.put(14, 19, "d")
    lv.put(15, 19, "t")
    lv.put(16, 23, "X")
    lv.put(11, 23, "c")
    lv.put(17, 17, "*")
    lv.windows(0, 18, 24, 24)
    # Terrace on the lobby roof, a ledge up from the fire escape.
    lv.area(19, 7, 21, 11, "roof", OUT, "R")
    lv.area(19, 12, 40, 24, "roof", OUT, "R")
    lv.many("r", [(19, 7), (20, 7), (21, 7)] + [(22, y) for y in range(7, 12)])
    lv.many("r", [(x, 12) for x in range(22, 41)] + [(40, y) for y in range(12, 25)] + [(x, 24) for x in range(19, 41)])
    lv.area(26, 15, 34, 21, "roof", OUT, "j")
    lv.area(27, 22, 33, 23, "roof", OUT, "u")
    lv.many("a", [(37, 14), (38, 14), (37, 15), (38, 15)])
    lv.many("p", [(23, 14), (23, 22), (38, 22)])
    lv.put(*ESCAPE, "H")
    lv.put(37, 19, "T")
    lv.put(21, 13, "c")
    return lv


LEVELS = [b1(), ground(), second(), penthouse()]

if __name__ == "__main__":
    for lv in LEVELS:
        print("====", lv.label, lv.blurb)
        print("   " + "".join(str(x % 10) for x in range(W)))
        for y, row in enumerate(lv.g):
            print(f"{y:2} " + "".join(row))
