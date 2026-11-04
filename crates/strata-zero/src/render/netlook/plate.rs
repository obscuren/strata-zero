//! A construct seen from the open Net: rings stepping in from its footprint, one per
//! step of its tier, and the plate on top that carries its name.

use citygen::{Pos, Rect};

/// Where a tile of a footprint sits among the steps.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ring {
    /// The footprint's border: the construct's wall.
    Edge,
    /// On the ring `k` tiles in; the first step is 1.
    Line(i32),
    /// Inside the last ring.
    Plate,
}

/// How many of `want` steps fit in a footprint: each keeps at least 3 by 1 inside it.
pub fn steps_in(fp: Rect, want: i32) -> i32 {
    let mut s = want.max(0);
    while s > 0 && (fp.w - 2 * (s + 1) < 3 || fp.h - 2 * (s + 1) < 1) {
        s -= 1;
    }
    s
}

/// Tiles from the footprint's nearest side; 0 on its border.
pub fn inset(fp: Rect, p: Pos) -> i32 {
    (p.x - fp.x)
        .min(p.y - fp.y)
        .min(fp.right() - 1 - p.x)
        .min(fp.bottom() - 1 - p.y)
}

/// Where `p` sits in a footprint of `steps` steps.
pub fn ring_of(fp: Rect, steps: i32, p: Pos) -> Ring {
    match inset(fp, p) {
        0 => Ring::Edge,
        i if i <= steps => Ring::Line(i),
        _ => Ring::Plate,
    }
}

/// The plate: the footprint inside its last ring.
pub fn plate(fp: Rect, steps: i32) -> Rect {
    fp.inset(steps + 1)
}

/// Lines a plate's label is cut to.
const LINES: usize = 2;

/// `label` wrapped to `w` cells in at most `lines` lines: a word too long ends in `…`,
/// and a label that runs over ends its last line in `…`.
pub fn wrap(label: &str, w: usize, lines: usize) -> Vec<String> {
    if w == 0 || lines == 0 {
        return Vec::new();
    }
    let mut out: Vec<String> = Vec::new();
    let mut cur = String::new();
    for word in label.split_whitespace() {
        let word: String = if word.chars().count() > w {
            word.chars().take(w - 1).chain(['…']).collect()
        } else {
            word.to_string()
        };
        if cur.is_empty() {
            cur = word;
        } else if cur.chars().count() + 1 + word.chars().count() <= w {
            cur.push(' ');
            cur.push_str(&word);
        } else {
            out.push(std::mem::take(&mut cur));
            cur = word;
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    if out.len() > lines {
        out.truncate(lines);
        let last = out.last_mut().expect("lines > 0");
        if last.chars().count() >= w {
            last.pop();
        }
        if !last.ends_with('…') {
            last.push('…');
        }
    }
    out
}

/// The label's cells on a plate: wrapped to its width, each line centred, the block as
/// near the plate's middle row as tiles where `blocked` is true allow (it stays in the
/// middle when no row is clear, and skips the blocked cells). Nothing on a plate
/// narrower than 4, where a name would be all `…`.
pub fn layout(plate: Rect, label: &str, blocked: impl Fn(Pos) -> bool) -> Vec<(Pos, char)> {
    if plate.w < 4 || plate.h < 1 {
        return Vec::new();
    }
    let lines = wrap(label, plate.w as usize, LINES.min(plate.h as usize));
    if lines.is_empty() {
        return Vec::new();
    }
    let n = lines.len() as i32;
    let cells = |top: i32| -> Vec<(Pos, char)> {
        lines
            .iter()
            .enumerate()
            .flat_map(|(i, line)| {
                let len = line.chars().count() as i32;
                let x0 = plate.x + (plate.w - len) / 2;
                line.chars()
                    .enumerate()
                    .filter(|(_, c)| *c != ' ')
                    .map(move |(k, c)| (Pos::new(x0 + k as i32, top + i as i32), c))
            })
            .collect()
    };
    let mid = plate.y + (plate.h - n) / 2;
    let mut tops: Vec<i32> = (plate.y..=plate.bottom() - n).collect();
    tops.sort_by_key(|t| (t - mid).abs());
    let top = tops
        .into_iter()
        .find(|t| cells(*t).iter().all(|(p, _)| !blocked(*p)))
        .unwrap_or(mid);
    cells(top)
        .into_iter()
        .filter(|(p, _)| !blocked(*p))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rings_step_in_from_the_footprint() {
        let fp = Rect::new(10, 20, 12, 8);
        let s = steps_in(fp, 3);
        assert_eq!(s, 2, "12 by 8 holds two steps and a plate");
        assert_eq!(ring_of(fp, s, Pos::new(10, 20)), Ring::Edge);
        assert_eq!(ring_of(fp, s, Pos::new(21, 24)), Ring::Edge);
        assert_eq!(ring_of(fp, s, Pos::new(11, 23)), Ring::Line(1));
        assert_eq!(ring_of(fp, s, Pos::new(13, 22)), Ring::Line(2));
        assert_eq!(ring_of(fp, s, Pos::new(14, 23)), Ring::Plate);
        assert_eq!(plate(fp, s), Rect::new(13, 23, 6, 2));
    }

    #[test]
    fn small_footprints_have_no_rings() {
        for fp in [
            Rect::new(0, 0, 3, 3),
            Rect::new(0, 0, 4, 2),
            Rect::new(0, 0, 5, 3),
        ] {
            assert_eq!(steps_in(fp, 4), 0, "{fp:?}");
            assert!(layout(plate(fp, 0), "KIROFLUX MART", |_| false).is_empty());
        }
    }

    #[test]
    fn labels_wrap_cut_and_dodge_nodes() {
        assert_eq!(wrap("VELFORM GROUP PLAZA", 7, 2), ["VELFORM", "GROUP…"]);
        assert_eq!(wrap("KIROFLUX MART", 7, 2), ["KIROFL…", "MART"]);
        assert_eq!(wrap("FLATS", 7, 2), ["FLATS"]);
        assert_eq!(wrap("ABCDEFG HIJ KLM", 7, 1), ["ABCDEF…"]);
        let plate = Rect::new(0, 0, 9, 5);
        let mid = layout(plate, "STACK 101", |_| false);
        assert!(mid.iter().all(|(p, _)| p.y == 2), "{mid:?}");
        assert_eq!(mid.first(), Some(&(Pos::new(0, 2), 'S')));
        // A node on the middle row: the label moves to a clear row nearby.
        let dodged = layout(plate, "STACK 101", |p| p == Pos::new(4, 2));
        assert!(dodged.iter().all(|(p, _)| p.y != 2), "{dodged:?}");
        assert_eq!(dodged.len(), 8);
        // Nowhere clear: it keeps the middle and leaves the node's cell alone.
        let stuck = layout(Rect::new(0, 0, 9, 1), "STACK 101", |p| p.x == 4);
        assert!(stuck.iter().all(|(p, _)| p.x != 4));
    }
}
