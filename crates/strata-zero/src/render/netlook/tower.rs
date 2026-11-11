//! The open tower (look D): a site's Net in section. The level you're on is open at its
//! full plan, tile for tile where it stands in the construct's grid; every other kept level
//! is folded to three rows (its back, the wall with its doors, its front) under a slab, out
//! from it both ways, the roof on top. The street runs across under G with the earth below
//! it; the riser runs down through the levels on it, cut `╳` where an air gap leaves one
//! off; the firewall stands round the riser's levels; each level's label is on its left.
//! Spec `docs/superpowers/specs/2026-11-11-site-net-design.md`, "The look: the open tower";
//! Rulings 21, 22 and 23.

use super::inside::{LATTICE, mask, watch_tint};
use super::{
    AMBER, BLACK_RED, DASHED, ICE_BLUE, NetLookView, ROOM, TERMINAL, VOID, WHITE, YOU, ice_colour,
    put,
};
use crate::render::Rgb;
use crate::render::map::ConstructMap;
use crate::render::overlay::SIGNAL;
use crate::render::style::StylePack;
use crate::render::tileset::{DOUBLE, LIGHT};
use crate::render::viewport::{screen_to_tile, tile_to_screen};
use citygen::{Construct, Feature, FloorPlan, NetDevice, NodeKind, Pos, Rect as TileRect, Terrain};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};

/// Words beside the tower.
const TEXT: Rgb = Rgb(205, 212, 222);
const DIM: Rgb = Rgb(138, 147, 163);
const FAINT: Rgb = Rgb(74, 82, 94);
/// The earth under the street.
const EARTH: Rgb = Rgb(58, 47, 38);
const TURRET: Rgb = Rgb(255, 122, 69);
/// How far a level out of reach is pulled into the void.
const DIMMED: f32 = 0.45;
/// The column each level's label and role end on, left of the firewall.
const LABEL_END: i32 = -4;
/// The column of the mark on the open level's label.
const MARK_X: i32 = -3;
/// The firewall's left side; its right stands as far out on the other side.
const FIREWALL_X: i32 = -2;
/// The widest word right of the tower: "out of reach".
const RIGHT_W: i32 = 12;

/// What the open tower draws besides the construct: the level you're on, what you hold and
/// where you jacked in, the firewall's locks, what you can reach, and what the site's Net
/// works out from its plan (each level's fold, the lifts through each slab, the
/// turnstiles).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SiteView {
    /// The level you're on, open at its full plan.
    pub open: i32,
    /// Grid tiles of the nodes you hold or have hacked.
    pub held: Vec<Pos>,
    /// The node you jacked in at, in the grid.
    pub jacked: Option<Pos>,
    /// The cord: from `jacked` to you, on the open level (Ruling 23).
    pub cord: Vec<Pos>,
    /// Each firewall lock, in lock order: broken.
    pub broken: Vec<bool>,
    /// The levels you can reach from the open one.
    pub reach: Vec<i32>,
    /// Each kept level and the floor row it folds at (Ruling 22).
    pub folds: Vec<(i32, i32)>,
    /// The lifts through the slab under each level: the level and the shaft's x.
    pub lifts: Vec<(i32, i32)>,
    /// Each turnstile's tile in the grid, and whether its line's switch is held.
    pub gates: Vec<(Pos, bool)>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stacked {
    Roof,
    Open {
        level: i32,
    },
    /// Row 0 (its back), 1 (the wall with its doors) or 2 (its front).
    Fold {
        level: i32,
        row: u8,
    },
    /// The slab under `above`; under G it is the street line.
    Slab {
        above: i32,
    },
}

/// The tower's rows by the grid row each is drawn on: the open level's own rows, and out
/// from them a slab and three folded rows for each kept level, the roof on top. Top down,
/// one row each; empty when `open` is no level of `k`.
pub fn stack(k: &Construct, open: i32) -> Vec<(i32, Stacked)> {
    let Some(here) = k.net_level(open) else {
        return Vec::new();
    };
    let area = here.area;
    let mut rows = Vec::new();
    // Up, nearest first: each level above folded over its slab, then the roof.
    let mut y = area.y;
    for l in (k.levels.iter().rev()).filter(|l| l.level > open) {
        y -= 1;
        rows.push((y, Stacked::Slab { above: l.level }));
        for row in [2, 1, 0] {
            y -= 1;
            rows.push((
                y,
                Stacked::Fold {
                    level: l.level,
                    row,
                },
            ));
        }
    }
    rows.push((y - 1, Stacked::Roof));
    rows.extend((area.y..area.bottom()).map(|y| (y, Stacked::Open { level: open })));
    // Down, nearest first: the open level's slab, then each level below over its own.
    let mut y = area.bottom();
    rows.push((y, Stacked::Slab { above: open }));
    for l in (k.levels.iter()).filter(|l| l.level < open) {
        for row in 0..3 {
            y += 1;
            rows.push((
                y,
                Stacked::Fold {
                    level: l.level,
                    row,
                },
            ));
        }
        y += 1;
        rows.push((y, Stacked::Slab { above: l.level }));
    }
    rows.sort_by_key(|(y, _)| *y);
    rows
}

/// What the camera keeps on screen round level `open` of site construct `k`: the tower top
/// to bottom, its labels on the left and the words on its right.
pub fn bounds(k: &Construct, open: i32) -> TileRect {
    let rows = stack(k, open);
    let (Some(first), Some(last)) = (rows.first(), rows.last()) else {
        return TileRect::new(0, 0, k.size.w as i32, k.size.h as i32);
    };
    let words = (k.levels.iter())
        .map(|l| l.name.chars().count().max(FloorPlan::label(l.level).len()))
        .chain(["street".len(), "99 held".len()])
        .max()
        .unwrap_or(0) as i32;
    let x0 = LABEL_END + 1 - words;
    let x1 = k.size.w as i32 + 2 + RIGHT_W - 1;
    TileRect::new(x0, first.0, x1 - x0 + 1, last.0 - first.0 + 1)
}

/// Cells written by grid tile: those off the view are dropped.
struct Paint<'b> {
    area: Rect,
    origin: Pos,
    buf: &'b mut Buffer,
}

impl Paint<'_> {
    fn put(&mut self, p: Pos, ch: char, fg: Rgb, bg: Rgb, bold: bool) {
        if let Some((sx, sy)) = tile_to_screen(self.area, self.origin, p) {
            put(self.buf, sx, sy, ch, fg, bg, bold);
        }
    }

    fn text(&mut self, at: Pos, s: &str, fg: Rgb, bg: Rgb, bold: bool) {
        for (i, ch) in s.chars().enumerate() {
            self.put(at.offset(i as i32, 0), ch, fg, bg, bold);
        }
    }

    /// `s` ending on `end`.
    fn text_to(&mut self, end: Pos, s: &str, fg: Rgb, bg: Rgb, bold: bool) {
        let n = s.chars().count() as i32;
        self.text(end.offset(1 - n, 0), s, fg, bg, bold);
    }

    /// The grid columns on screen.
    fn cols(&self) -> std::ops::Range<i32> {
        self.origin.x..self.origin.x + self.area.width as i32
    }

    fn shows_row(&self, y: i32) -> bool {
        (self.origin.y..self.origin.y + self.area.height as i32).contains(&y)
    }
}

/// What a frame of the tower works out once: its rows, where each level stands in them,
/// and what the open level's tiles are looked up by.
struct Tower<'a> {
    k: &'a Construct,
    view: &'a NetLookView,
    site: &'a SiteView,
    rows: Vec<(i32, Stacked)>,
    /// Each kept level's first and last row in the tower.
    span: HashMap<i32, (i32, i32)>,
    /// The widest level's width.
    w: i32,
    /// The street's row: the slab under G.
    street: Option<i32>,
    /// The riser's column: G's hub's.
    rx: i32,
    /// The rows the riser runs down, top and bottom.
    riser: Option<(i32, i32)>,
    held: HashSet<Pos>,
    gates: HashMap<Pos, bool>,
    /// The open level's nodes and traces, and the cord over it.
    nodes: HashMap<Pos, &'a NetDevice>,
    traces: HashSet<Pos>,
    cord: HashSet<Pos>,
}

impl<'a> Tower<'a> {
    fn new(k: &'a Construct, view: &'a NetLookView, site: &'a SiteView) -> Option<Tower<'a>> {
        let here = k.net_level(site.open)?;
        let rows = stack(k, site.open);
        let mut span: HashMap<i32, (i32, i32)> = HashMap::new();
        let mut street = None;
        for &(y, s) in &rows {
            match s {
                Stacked::Open { level } | Stacked::Fold { level, .. } => {
                    let e = span.entry(level).or_insert((y, y));
                    e.1 = y;
                }
                Stacked::Slab { above: 0 } => street = Some(y),
                _ => {}
            }
        }
        let rx = k.net_level(0).unwrap_or(here).hub.x;
        let nodes = (k.devices.iter())
            .filter(|d| here.area.contains(d.at))
            .map(|d| (d.at, d))
            .collect();
        let mut t = Tower {
            k,
            view,
            site,
            rows,
            span,
            w: k.size.w as i32,
            street,
            rx,
            riser: None,
            held: site.held.iter().copied().collect(),
            gates: site.gates.iter().copied().collect(),
            nodes,
            traces: here.traces.iter().copied().collect(),
            cord: site.cord.iter().copied().collect(),
        };
        t.riser = t.riser_rows();
        Some(t)
    }

    /// A kept level on the riser: every one but the gapped level.
    fn on_riser(&self, level: i32) -> bool {
        self.k.net_level(level).is_some_and(|l| !l.gapped)
    }

    fn gapped(&self, level: i32) -> bool {
        self.k.net_level(level).is_some_and(|l| l.gapped)
    }

    fn reaches(&self, level: i32) -> bool {
        self.site.reach.contains(&level)
    }

    /// The next kept level under `level`.
    fn below(&self, level: i32) -> Option<i32> {
        (self.k.levels.iter().map(|l| l.level))
            .filter(|l| *l < level)
            .max()
    }

    /// The floor row `level` folds at.
    fn fold(&self, level: i32) -> i32 {
        (self.site.folds.iter().find(|(l, _)| *l == level))
            .map(|(_, f)| *f)
            .or_else(|| self.k.net_level(level).map(|l| l.area.h / 2))
            .unwrap_or(0)
    }

    /// A floor row as a folded level's row: before its fold 0, on it 1, after it 2.
    fn fold_of(&self, level: i32, floor_y: i32) -> u8 {
        match floor_y.cmp(&self.fold(level)) {
            Ordering::Less => 0,
            Ordering::Equal => 1,
            Ordering::Greater => 2,
        }
    }

    /// The rows the riser runs down: from the roof (with the top level on it) or the first
    /// row of the highest level on it, to the hub of the lowest (folded) or the top of it
    /// (open).
    fn riser_rows(&self) -> Option<(i32, i32)> {
        let on: Vec<i32> = (self.k.levels.iter())
            .filter(|l| !l.gapped)
            .map(|l| l.level)
            .collect();
        let (hi, lo) = (*on.first()?, *on.last()?);
        let top = match self.k.levels.first() {
            Some(l) if l.level == hi => self.rows.first()?.0,
            _ => self.span.get(&hi)?.0,
        };
        let first = self.span.get(&lo)?.0;
        let bottom = if lo == self.site.open {
            first - 1
        } else {
            let l = self.k.net_level(lo)?;
            first + i32::from(self.fold_of(lo, l.hub.y - l.area.y))
        };
        Some((top, bottom))
    }

    fn riser_on(&self, y: i32) -> bool {
        self.riser
            .is_some_and(|(top, bottom)| (top..=bottom).contains(&y))
    }

    /// A node's glyph, colour, glow and weight: held green, the one you jacked in at
    /// glowing, a firewall lock `▮` whole and `▯` broken; pulled into the void out of reach.
    fn node(&self, d: &NetDevice, dim: bool) -> (char, Rgb, Option<Rgb>, bool) {
        let (glyph, fg, bg, bold) = match d.node.firewall_lock() {
            Some(i) => lock_look(
                self.site
                    .broken
                    .get(usize::from(i))
                    .copied()
                    .unwrap_or(false),
            ),
            None if self.held.contains(&d.at) => (
                node_glyph(d.kind),
                SIGNAL,
                Some(VOID.mix(SIGNAL, 0.28)),
                true,
            ),
            None if self.site.jacked == Some(d.at) => (
                node_glyph(d.kind),
                WHITE,
                Some(VOID.mix(TERMINAL, 0.45)),
                true,
            ),
            None => {
                let (fg, bold) = match d.kind {
                    NodeKind::Terminal => (TERMINAL, false),
                    NodeKind::Server | NodeKind::Hub => (WHITE, true),
                    NodeKind::DataVault | NodeKind::Container => (AMBER, true),
                    NodeKind::Turret => (TURRET, false),
                    _ => (ICE_BLUE, false),
                };
                (node_glyph(d.kind), fg, None, bold)
            }
        };
        match dim {
            true => (
                glyph,
                VOID.mix(fg, DIMMED),
                bg.map(|b| VOID.mix(b, DIMMED)),
                bold,
            ),
            false => (glyph, fg, bg, bold),
        }
    }

    /// The void round the tower: the lattice past it, nothing on its label side, the earth
    /// under the street.
    fn ground(&self, p: &mut Paint) {
        let (top, bottom) = match (self.rows.first(), self.rows.last()) {
            (Some(a), Some(b)) => (a.0, b.0),
            _ => return,
        };
        for sy in p.area.y..p.area.bottom() {
            for sx in p.area.x..p.area.right() {
                let q = screen_to_tile(p.area, p.origin, (sx, sy));
                let earth =
                    self.street.is_some_and(|s| q.y > s) && (MARK_X..=self.w + 2).contains(&q.x);
                let (ch, fg) = if earth {
                    ('░', VOID.mix(EARTH, 0.55))
                } else if (top..=bottom).contains(&q.y) && q.x <= self.w + 1 {
                    (' ', VOID)
                } else if q.x.rem_euclid(4) == 0 && q.y.rem_euclid(2) == 0 {
                    ('·', LATTICE)
                } else {
                    (' ', LATTICE)
                };
                put(p.buf, sx, sy, ch, fg, VOID, false);
            }
        }
    }

    /// The street across the map under G, named at each end.
    fn street_line(&self, p: &mut Paint) {
        let Some(s) = self.street else {
            return;
        };
        for x in p.cols() {
            p.put(Pos::new(x, s), '▔', TEXT.scale(0.35), VOID, false);
        }
        p.text_to(Pos::new(LABEL_END, s - 1), "street", FAINT, VOID, false);
        p.text(Pos::new(self.w + 3, s - 1), "street", FAINT, VOID, false);
    }

    /// A row of the open level: its outer wall doubled, its inner walls light, its nodes,
    /// ICE and turnstiles, the traces to its hub, the cord and the watch.
    fn open_row(&self, p: &mut Paint, level: i32, y: i32) {
        let Some(l) = self.k.net_level(level) else {
            return;
        };
        let area = l.area;
        let floor = match l.gapped {
            true => VOID.mix(BLACK_RED, 0.06),
            false => VOID.mix(ROOM, 0.10),
        };
        let outer = if l.gapped { BLACK_RED } else { ROOM.scale(0.8) };
        let wall_bg = VOID.mix(ROOM, 0.05);
        let ring = |q: Pos| {
            area.contains(q)
                && (q.x == area.x
                    || q.y == area.y
                    || q.x == area.right() - 1
                    || q.y == area.bottom() - 1)
        };
        let wall = |q: Pos| {
            area.contains(q) && (self.k.tiles.get(q)).is_some_and(|t| t.terrain == Terrain::NetWall)
        };
        let faces_open = |q: Pos| q.neighbors8().any(|n| area.contains(n) && !wall(n));
        for x in p.cols().filter(|x| (area.x..area.right()).contains(x)) {
            let q = Pos::new(x, y);
            let Some(tile) = self.k.tiles.get(q) else {
                continue;
            };
            let mut bg = floor;
            if let Some(grade) = self.view.watch.get(&q) {
                let (tint, f) = watch_tint(*grade);
                bg = bg.mix(tint, f);
            }
            if let Some(Feature::Ice(grade)) = tile.feature {
                let glow = VOID.mix(watch_tint(grade).0, 0.35);
                p.put(q, '◈', ice_colour(grade, self.view.tick), glow, true);
            } else if let Some(d) = self.nodes.get(&q) {
                let (glyph, fg, glow, bold) = self.node(d, false);
                let under = if ring(q) { VOID.mix(outer, 0.12) } else { bg };
                p.put(q, glyph, fg, glow.unwrap_or(under), bold);
            } else if ring(q) {
                let m = mask(q, ring);
                p.put(q, DOUBLE[m], outer, VOID.mix(outer, 0.12), l.gapped);
            } else if wall(q) && faces_open(q) {
                let m = line_mask(q, |n| wall(n) && (ring(n) || faces_open(n)));
                p.put(q, LIGHT[m], ROOM.scale(0.6), wall_bg, false);
            } else if wall(q) {
                p.put(q, ' ', ROOM, wall_bg, false);
            } else if let Some(open) = self.gates.get(&q) {
                let c = if *open { SIGNAL } else { ICE_BLUE };
                p.put(q, '┄', c, VOID.mix(c, 0.12), false);
            } else if self.cord.contains(&q)
                && self.site.cord.first() != Some(&q)
                && self.site.cord.last() != Some(&q)
            {
                let m = mask(q, |n| self.cord.contains(&n));
                p.put(q, DASHED[m], YOU, bg, true);
            } else if self.traces.contains(&q) {
                let m = line_mask(q, |n| {
                    self.traces.contains(&n) || self.nodes.contains_key(&n)
                });
                p.put(q, LIGHT[m], bg.mix(SIGNAL, 0.38), bg, false);
            } else {
                p.put(q, ' ', WHITE, bg, false);
            }
        }
    }

    /// A row of a folded level (Ruling 22): its back rooms' walls, the wall with its doors
    /// and turnstiles, or its front's walls; the riser through it; its nodes and ICE by
    /// their floor row against its fold, a collision shifting right (left once it meets the
    /// level's last column). Dimmed out of reach.
    fn fold_row(&self, p: &mut Paint, level: i32, row: u8, y: i32) {
        let Some(l) = self.k.net_level(level) else {
            return;
        };
        let area = l.area;
        let dim = !self.reaches(level);
        let d = |c: Rgb| if dim { VOID.mix(c, DIMMED) } else { c };
        let bg = d(match l.gapped {
            true => VOID.mix(BLACK_RED, 0.07),
            false => VOID.mix(ROOM, 0.10),
        });
        let line = ROOM.scale(if dim { 0.25 } else { 0.5 });
        let fold = self.fold(level);
        let wall = |x: i32, fy: i32| {
            (self.k.tiles.get(Pos::new(area.x + x, area.y + fy)))
                .is_none_or(|t| t.terrain == Terrain::NetWall)
        };
        let solid = |x: i32, rows: std::ops::Range<i32>| {
            !rows.is_empty() && rows.into_iter().all(|fy| wall(x, fy))
        };
        for x in p.cols().filter(|x| (0..area.w).contains(&(x - area.x))) {
            let fx = x - area.x;
            let (ch, fg) = match row {
                0 if solid(fx, 1..fold) => ('│', line),
                1 => match self.gates.get(&Pos::new(x, area.y + fold)) {
                    Some(open) => ('┄', d(if *open { SIGNAL } else { ICE_BLUE })),
                    None if wall(fx, fold) => ('─', line),
                    None => (' ', line),
                },
                2 if solid(fx, fold + 1..area.h - 1) => ('│', line),
                _ => (' ', line),
            };
            p.put(Pos::new(x, y), ch, fg, bg, false);
        }
        if self.on_riser(level) && self.riser_on(y) {
            p.put(Pos::new(self.rx, y), '║', d(WHITE.scale(0.7)), bg, true);
        }
        let mut ours: Vec<&NetDevice> = (self.k.devices.iter())
            .filter(|n| n.node.level == level)
            .collect();
        ours.sort_by_key(|n| (n.at.y, n.at.x));
        let mut taken: HashSet<i32> = HashSet::new();
        let mut place = |at: Pos| {
            if self.fold_of(level, at.y - area.y) != row {
                return None;
            }
            let x = free_column(&mut taken, at.x, area.x..area.right())?;
            Some(Pos::new(x, y))
        };
        for n in ours {
            if let Some(q) = place(n.at) {
                let (glyph, fg, glow, bold) = self.node(n, dim);
                p.put(q, glyph, fg, glow.unwrap_or(bg), bold);
            }
        }
        let pieces = (self.k.guards.iter()).filter(|g| g.standing && area.contains(g.pos));
        for g in pieces {
            if let Some(q) = place(g.pos) {
                let c = d(ice_colour(g.grade, self.view.tick));
                p.put(q, '◈', c, VOID.mix(c, 0.35), true);
            }
        }
    }

    /// The slab under `above`: the riser through it between two levels on it, and where the
    /// lift passes floors the copy drops, its shafts and the floors named.
    fn slab(&self, p: &mut Paint, above: i32, y: i32) {
        let Some(l) = self.k.net_level(above) else {
            return;
        };
        let colour = match l.gapped {
            true => BLACK_RED.scale(0.45),
            false => ROOM.scale(if self.reaches(above) { 0.45 } else { 0.22 }),
        };
        for x in -1..=self.w {
            p.put(Pos::new(x, y), '━', colour, VOID, false);
        }
        if self
            .below(above)
            .is_some_and(|b| self.on_riser(above) && self.on_riser(b))
        {
            p.put(Pos::new(self.rx, y), '║', WHITE.scale(0.7), VOID, true);
        }
        let Some((lo, hi)) = l.passed else {
            return;
        };
        let mut shafts: Vec<i32> = (self.site.lifts.iter())
            .filter(|(level, _)| *level == above)
            .map(|(_, x)| *x)
            .collect();
        for &x in &shafts {
            p.put(Pos::new(x, y), '║', ICE_BLUE.scale(0.5), VOID, false);
        }
        shafts.push(self.rx);
        let words = match lo == hi {
            true => format!(" {} passed ", FloorPlan::label(lo)),
            false => format!(" {}–{} passed ", FloorPlan::label(lo), FloorPlan::label(hi)),
        };
        let n = words.chars().count() as i32;
        let free = |x0: i32| !shafts.iter().any(|s| (x0..x0 + n).contains(s));
        let x0 = (1..=(self.w - n).max(1)).find(|x| free(*x)).unwrap_or(1);
        p.text(Pos::new(x0, y), &words, FAINT, VOID, false);
    }

    /// The firewall round the riser's levels, two columns out: its top edge with its lock
    /// count, its sides broken beside a gapped level, closed under the lowest level on it
    /// when that one is under the street (else the street closes it); with G folded, its
    /// locks on its sides beside G (Ruling 22).
    fn firewall(&self, p: &mut Paint) {
        let on: Vec<i32> = (self.k.levels.iter())
            .filter(|l| !l.gapped)
            .map(|l| l.level)
            .collect();
        let (Some(hi), Some(lo)) = (on.first(), on.last()) else {
            return;
        };
        let (Some(&(first, _)), Some(&(_, last))) = (self.span.get(hi), self.span.get(lo)) else {
            return;
        };
        let top = first - 1;
        let (bottom, closed) = match self.street {
            Some(s) if *lo >= 0 => (s - 1, false),
            _ => (last + 1, true),
        };
        let gap = (self.k.levels.iter())
            .find(|l| l.gapped)
            .and_then(|l| self.span.get(&l.level))
            .copied();
        let broken = |y: i32| gap.is_some_and(|(a, b)| (a..=b).contains(&y));
        let (x0, x1) = (FIREWALL_X, self.w + 1);
        let edge = |q: Pos| {
            let across = (x0..=x1).contains(&q.x) && (q.y == top || (closed && q.y == bottom));
            let side = (q.x == x0 || q.x == x1) && (top..=bottom).contains(&q.y) && !broken(q.y);
            across || side
        };
        let glow = VOID.mix(BLACK_RED, 0.12);
        for y in top..=bottom {
            let xs: Vec<i32> = match y == top || (closed && y == bottom) {
                true => (x0..=x1).collect(),
                false => vec![x0, x1],
            };
            for x in xs {
                let q = Pos::new(x, y);
                if edge(q) {
                    p.put(q, DOUBLE[mask(q, edge)], BLACK_RED, glow, true);
                }
            }
        }
        let n = self.site.broken.len();
        let words = format!(" FIREWALL · {n} {} ", if n == 1 { "lock" } else { "locks" });
        let len = words.chars().count() as i32;
        // Off the riser's head: four past it as look D has them, else wherever they fit
        // inside the corners clear of it, nearest the middle; the head drawn after them.
        let centred = (x0 + x1 + 1 - len) / 2;
        let starts = x0 + 1..=x1 - len;
        let clear = |x: &i32| starts.contains(x) && !(*x..x + len).contains(&self.rx);
        let mut near_middle: Vec<i32> = starts.clone().collect();
        near_middle.sort_by_key(|x| (x - centred).abs());
        let at = (std::iter::once(self.rx + 4).chain(near_middle))
            .find(clear)
            .unwrap_or(centred);
        p.text(Pos::new(at, top), &words, BLACK_RED, glow, false);
        if self.riser.is_some_and(|(a, _)| a <= top) {
            p.put(Pos::new(self.rx, top), '╦', BLACK_RED, glow, true);
        }
        // G folded: its locks on the firewall's sides beside it.
        if self.site.open != 0
            && let Some(&(g0, _)) = self.span.get(&0)
        {
            let spots = [(x0, g0 + 1), (x1, g0 + 1), (x0, g0), (x1, g0)];
            for (i, broken) in self.site.broken.iter().enumerate() {
                if let Some(&(x, y)) = spots.get(i) {
                    let (glyph, fg, bg, bold) = lock_look(*broken);
                    p.put(Pos::new(x, y), glyph, fg, bg.unwrap_or(glow), bold);
                }
            }
        }
    }

    /// `╳` on the riser where an air gap cuts a level off it: on each slab between a level on
    /// the riser and the gapped level.
    fn cuts(&self, p: &mut Paint) {
        for &(y, s) in &self.rows {
            let Stacked::Slab { above } = s else {
                continue;
            };
            let Some(below) = self.below(above) else {
                continue;
            };
            let cut = (self.on_riser(above) && self.gapped(below))
                || (self.gapped(above) && self.on_riser(below));
            if cut {
                let glow = VOID.mix(BLACK_RED, 0.2);
                p.put(Pos::new(self.rx, y), '╳', BLACK_RED, glow, true);
            }
        }
    }

    /// Left of each level its label and role, the open one marked with its node count and
    /// how many of them you hold; right of a level out of reach, "out of reach", or for the
    /// gapped level seen from the riser what it is and how in.
    fn labels(&self, p: &mut Paint) {
        let open = self.site.open;
        for l in &self.k.levels {
            let Some(&(y0, _)) = self.span.get(&l.level) else {
                continue;
            };
            let out = !self.reaches(l.level);
            let (label, role) = if out { (FAINT, FAINT) } else { (WHITE, DIM) };
            let at = |dy: i32| Pos::new(LABEL_END, y0 + dy);
            p.text_to(at(0), &FloorPlan::label(l.level), label, VOID, true);
            p.text_to(at(1), &l.name, role, VOID, false);
            if l.level == open {
                p.put(Pos::new(MARK_X, y0), '▸', ROOM, VOID, true);
                let ours: Vec<&NetDevice> = (self.k.devices.iter())
                    .filter(|d| d.node.level == open && d.kind != NodeKind::Hub)
                    .collect();
                let held = ours.iter().filter(|d| self.held.contains(&d.at)).count();
                p.text_to(at(3), &format!("{} ▪", ours.len()), ICE_BLUE, VOID, false);
                p.text_to(at(4), &format!("{held} held"), SIGNAL, VOID, false);
            }
            let right = |dy: i32| Pos::new(self.w + 2, y0 + dy);
            if l.gapped {
                p.text(right(0), "AIR GAP", BLACK_RED, VOID, true);
                if l.level != open {
                    p.text(right(1), "own Net", DIM, VOID, false);
                    p.text(right(2), "jack in", DIM, VOID, false);
                }
            } else if out {
                p.text(right(0), "out of reach", FAINT, VOID, false);
            }
        }
    }
}

/// A line's mask at `q` over the tiles `on` takes, two lines side by side kept apart (a
/// wall two thick, two traces abreast): each tile leans the way its longer straight run
/// goes, and a step between two tiles that both lean across it doesn't join.
fn line_mask(q: Pos, on: impl Fn(Pos) -> bool) -> usize {
    let run =
        |p: Pos, dx: i32, dy: i32| (1..=3).take_while(|i| on(p.offset(dx * i, dy * i))).count();
    let lean = |p: Pos| {
        let across = run(p, 1, 0) + run(p, -1, 0);
        let down = run(p, 0, 1) + run(p, 0, -1);
        across.cmp(&down)
    };
    let here = lean(q);
    mask(q, |n| {
        // A step down is crossed by tiles leaning across, a step across by those leaning down.
        let crossed = if n.x == q.x {
            Ordering::Greater
        } else {
            Ordering::Less
        };
        on(n) && !(here == crossed && lean(n) == crossed)
    })
}

/// The column a piece at column `x` of a folded row takes: its own when free, else the
/// next free one right of it, else left of it, all inside `cols` (the level's width);
/// none when the row is full.
fn free_column(taken: &mut HashSet<i32>, x: i32, cols: std::ops::Range<i32>) -> Option<i32> {
    if cols.is_empty() {
        return None;
    }
    let x = x.clamp(cols.start, cols.end - 1);
    let right = x..cols.end;
    let left = (cols.start..x).rev();
    right.chain(left).find(|c| taken.insert(*c))
}

/// A node's glyph on the tower: doors `▯`, terminals `◇`, the server `◆`, the vault `⌗`,
/// hubs `⬡`, the rest `▪`.
fn node_glyph(kind: NodeKind) -> char {
    match kind {
        NodeKind::DoorLock => '▯',
        NodeKind::Terminal => '◇',
        NodeKind::Server => '◆',
        NodeKind::DataVault => '⌗',
        NodeKind::Hub => '⬡',
        _ => '▪',
    }
}

/// A firewall lock: `▮` red whole, `▯` green broken.
fn lock_look(broken: bool) -> (char, Rgb, Option<Rgb>, bool) {
    match broken {
        true => ('▯', SIGNAL, Some(VOID.mix(SIGNAL, 0.28)), true),
        false => ('▮', BLACK_RED, Some(VOID.mix(BLACK_RED, 0.3)), true),
    }
}

/// The open tower over the whole map area: built from the construct and `site` alone, so it
/// costs a frame no more than the tiles it shows.
pub(super) fn draw(
    m: &ConstructMap,
    view: &NetLookView,
    site: &SiteView,
    origin: Pos,
    area: Rect,
    buf: &mut Buffer,
    style: &StylePack,
) {
    let Some(t) = Tower::new(m.construct, view, site) else {
        return super::inside::draw(m, view, origin, area, buf, style);
    };
    let mut p = Paint { area, origin, buf };
    t.ground(&mut p);
    t.street_line(&mut p);
    for &(y, s) in &t.rows {
        if !p.shows_row(y) {
            continue;
        }
        match s {
            Stacked::Open { level } => t.open_row(&mut p, level, y),
            Stacked::Fold { level, row } => t.fold_row(&mut p, level, row, y),
            Stacked::Slab { above } => t.slab(&mut p, above, y),
            Stacked::Roof => {}
        }
    }
    t.firewall(&mut p);
    t.cuts(&mut p);
    t.labels(&mut p);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::AppState;
    use crate::render::map::MapRef;
    use crate::render::netlook::NetLook;
    use crate::render::viewport::Overlay;
    use crate::screens::explore::MapScreen;
    use crate::screens::testutil::buffer_text;
    use crate::sitenet::testutil::{office_copy, on_hub};
    use citygen::venue::VenuePack;
    use citygen::{BuildingId, BuildingKind, Place, SiteNode};

    /// A terminal tall enough to show the whole tower round a real level: a site's floors
    /// are drawn tile for tile, 36 rows here, more than the map shows at 120x40.
    const TALL: (u16, u16) = (120, 64);

    /// The construct of the office copy at `security`, you on level `level`'s hub.
    fn site_construct_at(security: u8, level: i32) -> (AppState, Construct) {
        let (mut st, b) = office_copy(security);
        on_hub(&mut st, b, level);
        let k = st.constructs.site(b).expect("the site's construct").clone();
        (st, k)
    }

    /// The building of the site's Net you're on.
    fn b_of(st: &AppState) -> BuildingId {
        match st.player.as_ref().map(|p| p.place) {
            Some(Place::SiteNet { building, .. }) => building,
            other => panic!("not on a site's Net: {other:?}"),
        }
    }

    /// The map screen on a `size` terminal, you on level `level`'s hub of the office copy at
    /// `security`: its first frame learns the map's size, then the camera lands on you as
    /// it does when you land on a level.
    fn drawn_at(security: u8, level: i32, size: (u16, u16)) -> (AppState, Buffer) {
        let (mut st, b) = office_copy(security);
        on_hub(&mut st, b, level);
        let mut map = MapScreen::walking(&mut st);
        crate::screens::shots::draw_stack(&st, &[&map], size);
        let you = st.player.as_ref().expect("you").pos;
        map.jump_to(&st, you);
        let buf = crate::screens::shots::draw_stack(&st, &[&map], size);
        (st, buf)
    }

    fn screen_text(buf: &Buffer) -> String {
        buffer_text(buf)
    }

    /// The text of the map's area alone, left of the panel.
    fn map_text(buf: &Buffer) -> String {
        let map = crate::screens::explore::layout(buf.area, false).map;
        let mut s = String::new();
        for y in map.y..map.bottom() {
            for x in map.x..map.right() {
                s.push_str(buf[(x, y)].symbol());
            }
            s.push('\n');
        }
        s
    }

    #[test]
    fn the_tower_opens_your_level_and_folds_the_rest() {
        let (_, k) = site_construct_at(190, 0);
        let rows = stack(&k, 0);
        let l = k.net_level(0).unwrap().area;
        for y in l.y..l.bottom() {
            assert!(rows.contains(&(y, Stacked::Open { level: 0 })));
        }
        for lv in k.levels.iter().filter(|x| x.level != 0) {
            for r in 0..3 {
                let fold = Stacked::Fold {
                    level: lv.level,
                    row: r,
                };
                assert_eq!(rows.iter().filter(|(_, s)| *s == fold).count(), 1);
            }
        }
        // Top down: the roof, each level above folded under its slab, L open, the street, B1.
        let above = (rows.iter())
            .take_while(|(_, s)| *s != Stacked::Open { level: 0 })
            .count();
        let kept_above = k.levels.iter().filter(|x| x.level > 0).count();
        assert_eq!(above, 1 + 4 * kept_above);
        assert_eq!(rows[0].1, Stacked::Roof);
        let at = |y: i32| rows.iter().find(|(r, _)| *r == y).map(|r| r.1);
        assert_eq!(at(l.bottom()), Some(Stacked::Slab { above: 0 }));
        assert_eq!(
            at(l.bottom() + 1),
            Some(Stacked::Fold { level: -1, row: 0 })
        );
        // One row each, top down.
        assert!(rows.windows(2).all(|w| w[1].0 == w[0].0 + 1));
    }

    #[test]
    fn the_tower_draws_its_labels_firewall_street_and_gap() {
        let (st, buf) = drawn_at(190, 0, TALL);
        let text = screen_text(&buf);
        assert!(text.contains("FIREWALL · 3 locks"), "{text}");
        assert!(text.contains("street") && text.contains('▔') && text.contains('░'));
        assert!(text.contains("AIR GAP") && text.contains('╳'));
        let k = st.constructs.site(b_of(&st)).unwrap();
        let nodes = (k.devices.iter())
            .filter(|d| d.node.level == 0 && d.kind != NodeKind::Hub)
            .count();
        assert!(text.contains(&format!("{nodes} ▪")) && text.contains("0 held"));
        let map = map_text(&buf);
        assert!(
            map.lines().any(|l| l.contains('▸')),
            "the open level is marked"
        );
        let t = VenuePack::builtin().for_kind(BuildingKind::Office).unwrap();
        let role = &t.levels.ground.name;
        assert!(
            map.lines()
                .any(|l| l.trim_start().starts_with(role.as_str())),
            "its role under its label"
        );
        let label = FloorPlan::label(0);
        let label_row = map.lines().position(|l| l.trim_start().starts_with(&label));
        let role_row = map
            .lines()
            .position(|l| l.trim_start().starts_with(role.as_str()));
        assert_eq!(
            role_row,
            label_row.map(|r| r + 1),
            "the role under the label"
        );
    }

    #[test]
    fn a_level_across_the_gap_is_out_of_reach() {
        let (st, buf) = drawn_at(190, -1, TALL); // you on B1, the gapped level
        let k = st.constructs.site(b_of(&st)).unwrap();
        assert!(k.net_level(-1).unwrap().gapped);
        let map = map_text(&buf);
        assert!(map.contains("out of reach"), "{map}");
        assert!(map.contains("AIR GAP"), "your own level across the gap");
    }

    #[test]
    fn the_slab_names_the_floors_the_lift_passes() {
        let (_, k) = site_construct_at(190, 0);
        let l = (k.levels.iter())
            .find(|l| l.passed.is_some())
            .expect("a kept level the lift passes floors under");
        let (a, z) = l.passed.unwrap();
        let (_, buf) = drawn_at(190, l.level, TALL);
        let want = if a == z {
            format!("{} passed", FloorPlan::label(a))
        } else {
            format!("{}–{} passed", FloorPlan::label(a), FloorPlan::label(z))
        };
        assert!(map_text(&buf).contains(&want), "{want}");
    }

    /// The tower of the site you're on painted straight from the look over its bounds, with
    /// `site`; and the tile at the area's top left.
    fn paint(st: &AppState, site: SiteView) -> (Buffer, Pos) {
        let b = b_of(st);
        let k = st.constructs.site(b).unwrap();
        let city = st.city.as_ref().unwrap();
        let place = Place::SiteNet {
            building: b,
            level: site.open,
        };
        let map = MapRef::for_place(city, &st.interiors, &st.constructs, place, 0).unwrap();
        let r = bounds(k, site.open);
        let area = Rect::new(0, 0, r.w as u16, r.h as u16);
        let mut buf = Buffer::empty(area);
        let view = NetLookView {
            site: Some(site),
            ..NetLookView::default()
        };
        let origin = Pos::new(r.x, r.y);
        NetLook {
            view: &view,
            keep: None,
        }
        .draw(&map, origin, area, 0, &mut buf, &st.style);
        (buf, origin)
    }

    fn cell(buf: &Buffer, origin: Pos, p: Pos) -> &ratatui::buffer::Cell {
        &buf[((p.x - origin.x) as u16, (p.y - origin.y) as u16)]
    }

    #[test]
    fn the_tower_greens_what_you_hold_and_cords_you_to_your_port() {
        let (st, k) = site_construct_at(190, 0);
        let b = b_of(&st);
        let l = k.net_level(0).unwrap();
        let mut site = crate::sitenet::site_view(&st, b, 0);
        let node = (k.devices.iter())
            .find(|d| d.node.level == 0 && d.kind != NodeKind::Hub)
            .expect("a node on L");
        site.held = vec![node.at];
        site.broken = vec![true, false, false];
        // A cord along four bare floor tiles of L; every turnstile's switch held.
        let bare = |p: Pos| {
            l.area.contains(p)
                && k.tiles[p].terrain == Terrain::NetFloor
                && k.tiles[p].feature.is_none()
                && !l.traces.contains(&p)
                && !site.gates.iter().any(|g| g.0 == p)
        };
        let start = (l.area.iter())
            .find(|p| (0..4).all(|i| bare(p.offset(i, 0))))
            .expect("four bare tiles in a row");
        site.cord = (0..4).map(|i| start.offset(i, 0)).collect();
        let gate = (site.gates.iter())
            .find(|g| l.area.contains(g.0))
            .expect("a turnstile on L")
            .0;
        site.gates.iter_mut().for_each(|g| g.1 = true);
        let (buf, o) = paint(&st, site);
        let held = cell(&buf, o, node.at);
        assert_eq!(held.symbol(), node_glyph(node.kind).to_string());
        assert_eq!(held.fg, SIGNAL.to_color());
        assert_eq!(held.bg, VOID.mix(SIGNAL, 0.28).to_color());
        let lock = |i: u8| cell(&buf, o, k.device(SiteNode::firewall(b, i)).unwrap().at);
        assert_eq!((lock(0).symbol(), lock(0).fg), ("▯", SIGNAL.to_color()));
        assert_eq!((lock(1).symbol(), lock(1).fg), ("▮", BLACK_RED.to_color()));
        for i in 1..3 {
            let c = cell(&buf, o, start.offset(i, 0));
            assert_eq!((c.symbol(), c.fg), ("┄", YOU.to_color()), "the cord");
        }
        let g = cell(&buf, o, gate);
        assert_eq!(
            (g.symbol(), g.fg),
            ("┄", SIGNAL.to_color()),
            "an open turnstile"
        );
    }

    #[test]
    fn the_tower_follows_you_at_100x30() {
        let (_, buf) = drawn_at(190, 0, (100, 30));
        let text = screen_text(&buf);
        assert!(map_text(&buf).contains('@'), "you are on the map");
        assert!(text.contains("FIREWALL ▮▮▮ 0/3"), "the panel's firewall");
        assert!(text.contains("LEVELS"));
    }

    /// Construct `k` (the site's own, or one changed for a test) drawn as the tower with
    /// `site` over its bounds; and the tile at the area's top left.
    fn paint_k(st: &AppState, k: &Construct, site: SiteView) -> (Buffer, Pos) {
        let city = st.city.as_ref().unwrap();
        let m = ConstructMap {
            city,
            construct: k,
            level: Some(site.open),
        };
        let r = bounds(k, site.open);
        let area = Rect::new(0, 0, r.w as u16, r.h as u16);
        let mut buf = Buffer::empty(area);
        let view = NetLookView::default();
        let origin = Pos::new(r.x, r.y);
        draw(&m, &view, &site, origin, area, &mut buf, &st.style);
        (buf, origin)
    }

    /// Row `y` of `buf`, as text.
    fn row_text(buf: &Buffer, y: u16) -> String {
        (0..buf.area.width).map(|x| buf[(x, y)].symbol()).collect()
    }

    /// What the tower draws a node or a piece of ICE with.
    fn a_piece(symbol: &str) -> bool {
        ["▯", "◇", "◆", "⌗", "⬡", "▪", "▮", "◈"].contains(&symbol)
    }

    #[test]
    fn the_firewalls_words_keep_off_the_risers_head() {
        let (st, k) = site_construct_at(190, 0);
        let site = crate::sitenet::site_view(&st, b_of(&st), 0);
        // As laid, and with the riser moved under the middle of the firewall's top edge,
        // where its words stood centred.
        let mut middle = k.clone();
        let mid = (FIREWALL_X + k.size.w as i32 + 1) / 2;
        (middle.levels.iter_mut())
            .find(|l| l.level == 0)
            .unwrap()
            .hub
            .x = mid;
        for k in [&k, &middle] {
            let (buf, o) = paint_k(&st, k, site.clone());
            let rx = k.net_level(0).unwrap().hub.x;
            let row = (0..buf.area.height)
                .find(|y| row_text(&buf, *y).contains("FIREWALL · 3 locks"))
                .expect("the firewall's words, whole");
            let head = cell(&buf, o, Pos::new(rx, o.y + i32::from(row)));
            assert_eq!(head.symbol(), "╦", "{}", row_text(&buf, row));
        }
    }

    #[test]
    fn a_folded_levels_nodes_stay_inside_its_width() {
        let (st, mut k) = site_construct_at(190, 0);
        let site = crate::sitenet::site_view(&st, b_of(&st), 0);
        // Three nodes of a level above crowded onto its last column, on its fold's row.
        let l = (k.levels.iter()).find(|l| l.level > 0).unwrap().clone();
        let fold = (site.folds.iter())
            .find(|(lv, _)| *lv == l.level)
            .unwrap()
            .1;
        let last = Pos::new(l.area.right() - 1, l.area.y + fold);
        let mut crowd = (k.devices.iter_mut()).filter(|d| d.node.level == l.level);
        for d in crowd.by_ref().take(3) {
            d.at = last;
        }
        drop(crowd);
        let on_row = |p: Pos| p.y - l.area.y == fold;
        let pieces = (k.devices.iter())
            .filter(|d| d.node.level == l.level && on_row(d.at))
            .count()
            + (k.guards.iter())
                .filter(|g| g.standing && l.area.contains(g.pos) && on_row(g.pos))
                .count();
        assert!(pieces >= 3);
        let (buf, o) = paint_k(&st, &k, site);
        let (y, _) = (stack(&k, 0).into_iter())
            .find(|(_, s)| {
                *s == Stacked::Fold {
                    level: l.level,
                    row: 1,
                }
            })
            .unwrap();
        let drawn = |x: i32| a_piece(cell(&buf, o, Pos::new(x, y)).symbol());
        assert!(drawn(last.x), "one on its last column");
        let inside = (l.area.x..l.area.right()).filter(|x| drawn(*x)).count();
        assert_eq!(inside, pieces, "every piece of the row inside the level");
        // Nothing past it, up to the firewall's side.
        for x in l.area.right()..=k.size.w as i32 {
            assert!(!drawn(x), "a piece at {x}, past the level's {}", l.area.w);
        }
    }
}
