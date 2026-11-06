//! Geometry primitives: positions, sizes, rectangles, directions and a dense grid.

use serde::{Deserialize, Serialize};
use std::ops::{Index, IndexMut};

/// A tile coordinate. Negative values are legal and simply out of bounds.
#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize,
)]
pub struct Pos {
    pub x: i32,
    pub y: i32,
}

impl Pos {
    pub const fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }
    pub fn offset(self, dx: i32, dy: i32) -> Self {
        Self::new(self.x + dx, self.y + dy)
    }
    pub fn step(self, d: Dir) -> Self {
        let (dx, dy) = d.delta();
        self.offset(dx, dy)
    }
    pub fn manhattan(self, o: Pos) -> i32 {
        (self.x - o.x).abs() + (self.y - o.y).abs()
    }
    pub fn chebyshev(self, o: Pos) -> i32 {
        (self.x - o.x).abs().max((self.y - o.y).abs())
    }
    pub fn dist2(self, o: Pos) -> i64 {
        let dx = (self.x - o.x) as i64;
        let dy = (self.y - o.y) as i64;
        dx * dx + dy * dy
    }
    pub fn neighbors4(self) -> impl Iterator<Item = Pos> {
        Dir::CARDINAL.into_iter().map(move |d| self.step(d))
    }
    pub fn neighbors8(self) -> impl Iterator<Item = Pos> {
        Dir::ALL.into_iter().map(move |d| self.step(d))
    }
}

/// Dimensions of a grid.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Size {
    pub w: u32,
    pub h: u32,
}

impl Size {
    pub fn area(self) -> usize {
        self.w as usize * self.h as usize
    }
    pub fn contains(self, p: Pos) -> bool {
        p.x >= 0 && p.y >= 0 && (p.x as u32) < self.w && (p.y as u32) < self.h
    }
    pub fn index(self, p: Pos) -> usize {
        p.y as usize * self.w as usize + p.x as usize
    }
    pub fn center(self) -> Pos {
        Pos::new(self.w as i32 / 2, self.h as i32 / 2)
    }
    pub fn rect(self) -> Rect {
        Rect::new(0, 0, self.w as i32, self.h as i32)
    }
}

/// Axis-aligned rectangle; `right()` and `bottom()` are exclusive.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

impl Rect {
    pub const fn new(x: i32, y: i32, w: i32, h: i32) -> Self {
        Self {
            x,
            y,
            w: if w < 0 { 0 } else { w },
            h: if h < 0 { 0 } else { h },
        }
    }
    pub fn right(self) -> i32 {
        self.x + self.w
    }
    pub fn bottom(self) -> i32 {
        self.y + self.h
    }
    pub fn is_empty(self) -> bool {
        self.w == 0 || self.h == 0
    }
    pub fn area(self) -> usize {
        self.w as usize * self.h as usize
    }
    pub fn contains(self, p: Pos) -> bool {
        p.x >= self.x && p.y >= self.y && p.x < self.right() && p.y < self.bottom()
    }
    pub fn intersects(self, o: Rect) -> bool {
        self.x < o.right() && o.x < self.right() && self.y < o.bottom() && o.y < self.bottom()
    }
    pub fn intersection(self, o: Rect) -> Option<Rect> {
        let x = self.x.max(o.x);
        let y = self.y.max(o.y);
        let r = self.right().min(o.right());
        let b = self.bottom().min(o.bottom());
        if r > x && b > y {
            Some(Rect::new(x, y, r - x, b - y))
        } else {
            None
        }
    }
    /// The smallest rectangle holding every one of `cells`; empty for none.
    pub fn around(cells: &[Pos]) -> Rect {
        let Some(first) = cells.first() else {
            return Rect::new(0, 0, 0, 0);
        };
        let (lo, hi) = cells.iter().fold((*first, *first), |(lo, hi), p| {
            (
                Pos::new(lo.x.min(p.x), lo.y.min(p.y)),
                Pos::new(hi.x.max(p.x), hi.y.max(p.y)),
            )
        });
        Rect::new(lo.x, lo.y, hi.x - lo.x + 1, hi.y - lo.y + 1)
    }
    pub fn clamp_to(self, bounds: Rect) -> Option<Rect> {
        self.intersection(bounds)
    }
    pub fn inset(self, n: i32) -> Rect {
        Rect::new(self.x + n, self.y + n, self.w - 2 * n, self.h - 2 * n)
    }
    /// Grow by `n` on every side.
    pub fn grow(self, n: i32) -> Rect {
        self.inset(-n)
    }
    pub fn center(self) -> Pos {
        Pos::new(self.x + self.w / 2, self.y + self.h / 2)
    }
    pub fn iter(self) -> impl Iterator<Item = Pos> {
        (self.y..self.bottom())
            .flat_map(move |y| (self.x..self.right()).map(move |x| Pos::new(x, y)))
    }
    pub fn border(self) -> impl Iterator<Item = Pos> {
        self.iter().filter(move |p| {
            p.x == self.x || p.y == self.y || p.x == self.right() - 1 || p.y == self.bottom() - 1
        })
    }
}

/// The eight compass directions.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Dir {
    N,
    NE,
    E,
    SE,
    S,
    SW,
    W,
    NW,
}

impl Dir {
    pub const CARDINAL: [Dir; 4] = [Dir::N, Dir::E, Dir::S, Dir::W];
    pub const ALL: [Dir; 8] = [
        Dir::N,
        Dir::NE,
        Dir::E,
        Dir::SE,
        Dir::S,
        Dir::SW,
        Dir::W,
        Dir::NW,
    ];
    pub fn delta(self) -> (i32, i32) {
        match self {
            Dir::N => (0, -1),
            Dir::NE => (1, -1),
            Dir::E => (1, 0),
            Dir::SE => (1, 1),
            Dir::S => (0, 1),
            Dir::SW => (-1, 1),
            Dir::W => (-1, 0),
            Dir::NW => (-1, -1),
        }
    }
    pub fn opposite(self) -> Dir {
        match self {
            Dir::N => Dir::S,
            Dir::NE => Dir::SW,
            Dir::E => Dir::W,
            Dir::SE => Dir::NW,
            Dir::S => Dir::N,
            Dir::SW => Dir::NE,
            Dir::W => Dir::E,
            Dir::NW => Dir::SE,
        }
    }
}

/// Dense row-major grid of `T`. Deserialization rejects data that does not match `size`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "RawGrid<T>")]
#[serde(bound(deserialize = "T: Deserialize<'de>"))]
pub struct Grid<T> {
    size: Size,
    data: Vec<T>,
}

/// The on-disk shape of a grid, before the size/data consistency check.
#[derive(Deserialize)]
struct RawGrid<T> {
    size: Size,
    data: Vec<T>,
}

impl<T> TryFrom<RawGrid<T>> for Grid<T> {
    type Error = String;

    fn try_from(raw: RawGrid<T>) -> Result<Self, Self::Error> {
        if raw.data.len() != raw.size.area() {
            return Err(format!(
                "grid of {}x{} needs {} cells, found {}",
                raw.size.w,
                raw.size.h,
                raw.size.area(),
                raw.data.len()
            ));
        }
        Ok(Grid {
            size: raw.size,
            data: raw.data,
        })
    }
}

impl<T: Clone> Grid<T> {
    pub fn filled(size: Size, v: T) -> Self {
        Self {
            size,
            data: vec![v; size.area()],
        }
    }
    pub fn size(&self) -> Size {
        self.size
    }
    pub fn in_bounds(&self, p: Pos) -> bool {
        self.size.contains(p)
    }
    pub fn get(&self, p: Pos) -> Option<&T> {
        self.in_bounds(p).then(|| &self.data[self.size.index(p)])
    }
    pub fn get_mut(&mut self, p: Pos) -> Option<&mut T> {
        if self.in_bounds(p) {
            let i = self.size.index(p);
            Some(&mut self.data[i])
        } else {
            None
        }
    }
    /// Returns false when `p` is out of bounds.
    pub fn set(&mut self, p: Pos, v: T) -> bool {
        match self.get_mut(p) {
            Some(slot) => {
                *slot = v;
                true
            }
            None => false,
        }
    }
    pub fn iter(&self) -> impl Iterator<Item = (Pos, &T)> {
        let w = self.size.w as i32;
        self.data
            .iter()
            .enumerate()
            .map(move |(i, t)| (Pos::new(i as i32 % w, i as i32 / w), t))
    }
    pub fn fill_rect(&mut self, r: Rect, v: T) {
        for p in r.iter() {
            self.set(p, v.clone());
        }
    }
    pub fn as_slice(&self) -> &[T] {
        &self.data
    }
    pub fn as_mut_slice(&mut self) -> &mut [T] {
        &mut self.data
    }
    pub fn map<U: Clone>(&self, f: impl Fn(&T) -> U) -> Grid<U> {
        Grid {
            size: self.size,
            data: self.data.iter().map(f).collect(),
        }
    }
    pub fn count(&self, pred: impl Fn(&T) -> bool) -> usize {
        self.data.iter().filter(|t| pred(t)).count()
    }
}

impl<T: Clone> Index<Pos> for Grid<T> {
    type Output = T;
    fn index(&self, p: Pos) -> &T {
        self.get(p).expect("grid index out of bounds")
    }
}

impl<T: Clone> IndexMut<Pos> for Grid<T> {
    fn index_mut(&mut self, p: Pos) -> &mut T {
        self.get_mut(p).expect("grid index out of bounds")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rect_iter_and_border() {
        let r = Rect::new(2, 3, 3, 2);
        assert_eq!(r.area(), 6);
        assert_eq!(r.iter().count(), 6);
        assert_eq!(r.border().count(), 6);
        assert!(r.contains(Pos::new(4, 4)));
        assert!(!r.contains(Pos::new(5, 4)));
        assert_eq!(r.inset(1), Rect::new(3, 4, 1, 0));
    }

    #[test]
    fn rect_intersection_and_clamp() {
        let a = Rect::new(0, 0, 10, 10);
        let b = Rect::new(5, 5, 10, 10);
        assert_eq!(a.intersection(b), Some(Rect::new(5, 5, 5, 5)));
        assert_eq!(a.intersection(Rect::new(20, 20, 1, 1)), None);
        assert_eq!(b.clamp_to(a), Some(Rect::new(5, 5, 5, 5)));
    }

    #[test]
    fn grid_set_get_oob() {
        let mut g = Grid::filled(Size { w: 4, h: 3 }, 0u8);
        assert!(g.set(Pos::new(3, 2), 7));
        assert!(!g.set(Pos::new(4, 2), 7));
        assert_eq!(g.get(Pos::new(3, 2)), Some(&7));
        assert_eq!(g.get(Pos::new(-1, 0)), None);
        assert_eq!(g.iter().count(), 12);
        g.fill_rect(Rect::new(0, 0, 2, 2), 9);
        assert_eq!(g.count(|&v| v == 9), 4);
    }

    #[test]
    fn grid_rejects_inconsistent_serialized_data() {
        #[derive(serde::Serialize)]
        struct Fake {
            size: Size,
            data: Vec<u8>,
        }
        let bad = ron::to_string(&Fake {
            size: Size { w: 4, h: 3 },
            data: vec![0; 5],
        })
        .unwrap();
        assert!(ron::from_str::<Grid<u8>>(&bad).is_err());
        let good = ron::to_string(&Grid::filled(Size { w: 4, h: 3 }, 7u8)).unwrap();
        let g: Grid<u8> = ron::from_str(&good).unwrap();
        assert_eq!(g.count(|&v| v == 7), 12);
    }

    #[test]
    fn dir_delta_opposite() {
        assert_eq!(Dir::N.delta(), (0, -1));
        assert_eq!(Dir::SE.opposite(), Dir::NW);
        assert_eq!(Pos::new(1, 1).step(Dir::W), Pos::new(0, 1));
        assert_eq!(Pos::new(0, 0).chebyshev(Pos::new(3, -2)), 3);
    }
}
