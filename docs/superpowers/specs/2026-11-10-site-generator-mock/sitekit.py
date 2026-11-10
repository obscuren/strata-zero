"""The drawing kit the site mocks share: a level as a grid of design characters, with a
room kind (floor light) and a zone (who belongs there) per tile."""

PUBLIC, GUEST, PAID, STAFF, SECURE, TRACK, GANG, OUT = (
    "public", "guest", "paid", "staff", "secure", "track", "gang", "out",
)


class Level:
    def __init__(self, key, label, blurb, w, h):
        self.key = key
        self.label = label
        self.blurb = blurb
        self.w = w
        self.h = h
        self.g = [[" "] * w for _ in range(h)]
        self.kind = [[None] * w for _ in range(h)]
        self.zone = [[None] * w for _ in range(h)]

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
                if x in (x0, x1) or y in (y0, y1):
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

    def open(self, pts, kind=None, zone=None):
        """Floor through walls: a doorway with no door."""
        for x, y in pts:
            self.g[y][x] = "."
            if kind:
                self.kind[y][x] = kind
                self.zone[y][x] = zone

    def dump(self):
        print("====", self.label, self.blurb)
        print("   " + "".join(str(x // 10 % 10) if x % 10 == 0 else " " for x in range(self.w)))
        print("   " + "".join(str(x % 10) for x in range(self.w)))
        for y, row in enumerate(self.g):
            print(f"{y:2} " + "".join(row))
