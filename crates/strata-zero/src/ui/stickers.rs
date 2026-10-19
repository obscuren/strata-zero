//! Stickers drawn: lettering on its ground or half-block pixels, at any of the four
//! turns, wherever a copy has them stuck on its drawing.

use crate::ui::art::{Art, SocketPlace};
use crate::ui::cyberdeck::Pen;
use crate::ui::pixels::Pixels;
use game::{Content, StickerArt, Stuck, Turn};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Color;

/// A half turn: the glyph's turned form. A glyph with none turns to itself (N O S X Z
/// H I x o s z l 0 8 and the like are their own half turns).
pub const TURN180: &[(char, char)] = &[
    ('A', '∀'),
    ('B', 'ꓭ'),
    ('C', 'Ɔ'),
    ('D', 'ᗡ'),
    ('E', 'Ǝ'),
    ('F', 'Ⅎ'),
    ('G', '⅁'),
    ('J', 'ſ'),
    ('K', 'ꓘ'),
    ('L', '˥'),
    ('M', 'W'),
    ('P', 'Ԁ'),
    ('R', 'ꓤ'),
    ('T', '⊥'),
    ('U', '∩'),
    ('V', 'Λ'),
    ('W', 'M'),
    ('Y', '⅄'),
    ('a', 'ɐ'),
    ('b', 'q'),
    ('c', 'ɔ'),
    ('d', 'p'),
    ('e', 'ǝ'),
    ('f', 'ɟ'),
    ('g', 'ƃ'),
    ('h', 'ɥ'),
    ('i', 'ı'),
    ('j', 'ɾ'),
    ('k', 'ʞ'),
    ('m', 'ɯ'),
    ('n', 'u'),
    ('p', 'd'),
    ('q', 'b'),
    ('r', 'ɹ'),
    ('t', 'ʇ'),
    ('u', 'n'),
    ('v', 'ʌ'),
    ('w', 'ʍ'),
    ('y', 'ʎ'),
    ('6', '9'),
    ('9', '6'),
    ('‿', '⁀'),
    ('⁀', '‿'),
    ('(', ')'),
    (')', '('),
    ('<', '>'),
    ('>', '<'),
    ('^', 'v'),
    ('↑', '↓'),
    ('↓', '↑'),
    ('←', '→'),
    ('→', '←'),
    ('▲', '▼'),
    ('▼', '▲'),
    ('◀', '▶'),
    ('▶', '◀'),
    ('▸', '◂'),
    ('◂', '▸'),
    ('▾', '▴'),
    ('▴', '▾'),
    ('▀', '▄'),
    ('▄', '▀'),
    ('▌', '▐'),
    ('▐', '▌'),
    ('◢', '◤'),
    ('◤', '◢'),
    ('◣', '◥'),
    ('◥', '◣'),
];

/// A quarter turn clockwise, for symbols with a true one. Letters and digits have
/// none: they stand upright, stacked.
pub const TURN90: &[(char, char)] = &[
    ('‿', '('),
    ('⁀', ')'),
    ('(', '⁀'),
    (')', '‿'),
    ('<', '^'),
    ('>', 'v'),
    ('^', '>'),
    ('v', '<'),
    ('↑', '→'),
    ('→', '↓'),
    ('↓', '←'),
    ('←', '↑'),
    ('▲', '▶'),
    ('▶', '▼'),
    ('▼', '◀'),
    ('◀', '▲'),
    ('▸', '▾'),
    ('▾', '◂'),
    ('◂', '▴'),
    ('▴', '▸'),
    ('▀', '▐'),
    ('▐', '▄'),
    ('▄', '▌'),
    ('▌', '▀'),
    ('◢', '◣'),
    ('◣', '◤'),
    ('◤', '◥'),
    ('◥', '◢'),
    ('-', '|'),
    ('|', '-'),
    ('─', '│'),
    ('│', '─'),
    ('/', '\\'),
    ('\\', '/'),
];

fn turn_by(table: &[(char, char)], ch: char) -> char {
    table
        .iter()
        .find(|(from, _)| *from == ch)
        .map_or(ch, |&(_, to)| to)
}

/// A glyph turned half way round.
pub fn half(ch: char) -> char {
    turn_by(TURN180, ch)
}

/// A glyph turned `times` quarters clockwise.
pub fn quarter(ch: char, times: u8) -> char {
    (0..times % 4).fold(ch, |ch, _| turn_by(TURN90, ch))
}

fn rgb((r, g, b): (u8, u8, u8)) -> Color {
    Color::Rgb(r, g, b)
}

/// A picture's pixels turned clockwise: each quarter, the left column becomes the top
/// row, read right to left.
fn grid(rows: &[String], turn: Turn) -> Vec<Vec<char>> {
    let mut g: Vec<Vec<char>> = rows.iter().map(|r| r.chars().collect()).collect();
    for _ in 0..turn.degrees() / 90 {
        let (h, w) = (g.len(), g.first().map_or(0, Vec::len));
        g = (0..w)
            .map(|r| (0..h).map(|c| g[h - 1 - c][r]).collect())
            .collect();
    }
    g
}

/// The lettering as it reads at a turn, top to bottom: level and half turned, one
/// row; a quarter either way, one char a row.
fn letters(text: &str, turn: Turn) -> Vec<char> {
    match turn {
        Turn::Level => text.chars().collect(),
        Turn::Half => text.chars().rev().map(half).collect(),
        Turn::Quarter => text.chars().map(|c| quarter(c, 1)).collect(),
        Turn::ThreeQuarter => text.chars().rev().map(|c| quarter(c, 3)).collect(),
    }
}

fn stood_up(turn: Turn) -> bool {
    matches!(turn, Turn::Quarter | Turn::ThreeQuarter)
}

/// A stuck sticker with its art, ready to draw.
#[derive(Clone, Debug, PartialEq)]
pub struct Placed {
    pub art: StickerArt,
    pub x: u16,
    pub y: u16,
    pub turn: Turn,
}

/// The stickers a copy has stuck, with their art; one the content does not know as a
/// sticker is left out.
pub fn placed(c: &Content, stuck: &[Stuck]) -> Vec<Placed> {
    stuck
        .iter()
        .filter_map(|s| {
            let item = c.item(&s.sticker)?;
            let art = item.sticker.clone()?;
            Some(Placed {
                art,
                x: s.x,
                y: s.y,
                turn: s.turn,
            })
        })
        .collect()
}

/// A sticker's width and height in cells at a turn. Stood up, lettering is a column
/// three wide, one cell of text a row.
pub fn size(art: &StickerArt, turn: Turn) -> (u16, u16) {
    match art {
        StickerArt::Lettered { text, .. } => {
            let n = text.chars().count() as u16;
            if stood_up(turn) { (3, n) } else { (n, 1) }
        }
        StickerArt::Picture { rows, .. } => {
            let g = grid(rows, turn);
            let (w, h) = (g.first().map_or(0, Vec::len), g.len());
            (w as u16, h.div_ceil(2) as u16)
        }
    }
}

/// The cells a sticker writes at a turn, from its top-left: every cell of lettering,
/// and the cells of a picture with a pixel in them.
pub fn footprint(art: &StickerArt, turn: Turn) -> Vec<(u16, u16)> {
    let (w, h) = size(art, turn);
    match art {
        StickerArt::Lettered { .. } => (0..h).flat_map(|y| (0..w).map(move |x| (x, y))).collect(),
        StickerArt::Picture { rows, .. } => {
            let g = grid(rows, turn);
            let set = |x: usize, y: usize| g.get(y).is_some_and(|r| r[x] != '.');
            (0..h)
                .flat_map(|y| (0..w).map(move |x| (x, y)))
                .filter(|&(x, y)| {
                    let (x, y) = (usize::from(x), 2 * usize::from(y));
                    set(x, y) || set(x, y + 1)
                })
                .collect()
        }
    }
}

/// Draws a sticker with its top-left at (x, y) from the pen's origin.
pub fn draw_at(art: &StickerArt, turn: Turn, x: u16, y: u16, pen: &mut Pen) {
    match art {
        StickerArt::Lettered {
            text,
            ink,
            ground,
            bold,
        } => {
            let (ink, ground) = (rgb(*ink), Some(rgb(*ground)));
            let chars = letters(text, turn);
            if stood_up(turn) {
                for (dy, ch) in (0..).zip(chars) {
                    pen.put(x, y + dy, &format!(" {ch} "), ink, ground, *bold);
                }
            } else {
                let row: String = chars.into_iter().collect();
                pen.put(x, y, &row, ink, ground, *bold);
            }
        }
        StickerArt::Picture { rows, colours } => {
            let g = grid(rows, turn);
            let (w, h) = size(art, turn);
            let mut canvas = Pixels::new(w, h);
            for (py, row) in (0..).zip(&g) {
                for (px, ch) in (0..).zip(row) {
                    if let Some(c) = colours.get(ch) {
                        canvas.px(px, py, Some(rgb(*c)));
                    }
                }
            }
            canvas.fold(pen, i32::from(x), i32::from(y));
        }
    }
}

/// A sticker on its own blueprint: level, at its size, centred; no sockets, and no
/// stickers on a sticker.
#[derive(Clone, Debug, PartialEq)]
pub struct Loose {
    pub art: StickerArt,
}

impl Art for Loose {
    fn size(&self) -> (u16, u16) {
        size(&self.art, Turn::Level)
    }

    fn draw(&self, (ox, oy): (u16, u16), clip: Rect, buf: &mut Buffer) {
        let mut pen = Pen {
            clip: clip.intersection(buf.area),
            buf,
            ox,
            oy,
        };
        draw_at(&self.art, Turn::Level, 0, 0, &mut pen);
    }

    fn sockets(&self) -> Vec<SocketPlace> {
        Vec::new()
    }
}

/// Draws stuck stickers in their order, so each newer one lies over the older, on a
/// drawing `w` by `h` from the pen's origin: nothing of them lands past its edges,
/// though the pen may reach further (a deck's cables do).
pub fn draw(stickers: &[Placed], (w, h): (u16, u16), pen: &mut Pen) {
    let mut on = Pen {
        clip: pen.clip.intersection(Rect::new(pen.ox, pen.oy, w, h)),
        buf: &mut *pen.buf,
        ox: pen.ox,
        oy: pen.oy,
    };
    for s in stickers {
        draw_at(&s.art, s.turn, s.x, s.y, &mut on);
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use unicode_width::UnicodeWidthChar;

    pub fn lettered(text: &str) -> StickerArt {
        StickerArt::Lettered {
            text: text.into(),
            ink: (1, 2, 3),
            ground: (4, 5, 6),
            bold: true,
        }
    }

    fn picture(rows: &[&str]) -> StickerArt {
        StickerArt::Picture {
            rows: rows.iter().map(|r| r.to_string()).collect(),
            colours: [('a', (9, 9, 9)), ('b', (7, 7, 7))].into_iter().collect(),
        }
    }

    const UNDER: Color = Color::Rgb(40, 41, 42);

    /// The sticker drawn alone on a buffer of `UNDER`, its top-left at the origin.
    fn drawn(art: &StickerArt, turn: Turn) -> Buffer {
        let (w, h) = size(art, turn);
        let area = Rect::new(0, 0, w, h);
        let mut buf = Buffer::empty(area);
        for cell in &mut buf.content {
            cell.set_bg(UNDER);
        }
        let mut pen = Pen {
            buf: &mut buf,
            ox: 0,
            oy: 0,
            clip: area,
        };
        draw_at(art, turn, 0, 0, &mut pen);
        buf
    }

    fn glyphs(art: &StickerArt, turn: Turn) -> Vec<String> {
        let buf = drawn(art, turn);
        (0..buf.area.height)
            .map(|y| (0..buf.area.width).map(|x| buf[(x, y)].symbol()).collect())
            .collect()
    }

    fn content() -> Content {
        Content::load().unwrap()
    }

    #[test]
    fn lettering_turns_as_stacked_letters() {
        let nf = lettered(" NO FUTURE ");
        assert_eq!(glyphs(&nf, Turn::Level), [" NO FUTURE "]);
        assert_eq!(glyphs(&nf, Turn::Half), [" Ǝꓤ∩⊥∩Ⅎ ON "]);
        let up = glyphs(&nf, Turn::Quarter);
        assert_eq!(up.len(), 11);
        assert_eq!((up[0].as_str(), up[1].as_str()), ("   ", " N "));
        assert_eq!((up[3].as_str(), up[9].as_str()), ("   ", " E "));
        let down = glyphs(&nf, Turn::ThreeQuarter);
        assert_eq!((down[1].as_str(), down[9].as_str()), (" E ", " N "));
        let face = lettered(" x‿x ");
        assert_eq!(glyphs(&face, Turn::Half), [" x⁀x "]);
        assert_eq!(glyphs(&face, Turn::Quarter)[2], " ( ");
        assert_eq!(glyphs(&face, Turn::ThreeQuarter)[2], " ) ");
        assert_eq!(glyphs(&lettered(" JACK IN ▸ "), Turn::Quarter)[9], " ▾ ");
        assert_eq!(size(&nf, Turn::Level), (11, 1));
        assert_eq!(size(&nf, Turn::Quarter), (3, 11));
        assert_eq!(footprint(&nf, Turn::Quarter).len(), 33);
        // Every cell is the ground with the ink on it, bold.
        let buf = drawn(&nf, Turn::Quarter);
        for cell in &buf.content {
            assert_eq!(
                (cell.fg, cell.bg),
                (Color::Rgb(1, 2, 3), Color::Rgb(4, 5, 6))
            );
            assert!(cell.modifier.contains(ratatui::style::Modifier::BOLD));
        }
    }

    #[test]
    fn a_turn_table_entry_turns_back() {
        assert_eq!(half('R'), 'ꓤ');
        assert_eq!(half('N'), 'N');
        assert_eq!(half('‿'), '⁀');
        assert_eq!(quarter('‿', 1), '(');
        assert_eq!(quarter('‿', 4), '‿');
        assert_eq!(quarter('N', 1), 'N');
        for &(from, _) in TURN90 {
            assert_eq!(quarter(from, 4), from, "{from}");
        }
    }

    #[test]
    fn every_turned_glyph_is_one_cell_wide() {
        for &(a, b) in TURN180.iter().chain(TURN90) {
            for ch in [a, b] {
                assert_eq!(ch.width(), Some(1), "{ch}");
            }
        }
        for item in &content().items {
            let Some(art) = &item.sticker else { continue };
            for turn in Turn::ALL {
                for cell in &drawn(art, turn).content {
                    assert_eq!(
                        unicode_width::UnicodeWidthStr::width(cell.symbol()),
                        1,
                        "{} {turn:?} {:?}",
                        item.id,
                        cell.symbol()
                    );
                    assert!(
                        matches!(cell.fg, Color::Rgb(..) | Color::Reset),
                        "{}",
                        item.id
                    );
                    assert!(matches!(cell.bg, Color::Rgb(..)), "{}", item.id);
                }
            }
        }
    }

    #[test]
    fn pictures_turn_true() {
        for item in &content().items {
            let Some(art @ StickerArt::Picture { rows, .. }) = &item.sticker else {
                continue;
            };
            let (w, h) = (rows[0].chars().count() as u16, rows.len() as u16);
            assert_eq!(size(art, Turn::Level), (w, h.div_ceil(2)), "{}", item.id);
            assert_eq!(size(art, Turn::Quarter), (h, w.div_ceil(2)), "{}", item.id);
            assert_eq!(size(art, Turn::Half), size(art, Turn::Level), "{}", item.id);
            // Turned all the way round, it draws as it did.
            let level = drawn(art, Turn::Level);
            let mut back = art.clone();
            let StickerArt::Picture { rows: turned, .. } = &mut back else {
                unreachable!()
            };
            for _ in 0..4 {
                *turned = turn_rows(turned);
            }
            assert_eq!(drawn(&back, Turn::Level), level, "{}", item.id);
        }
        // Equal halves fold to a blank cell on the colour; an empty half shows what is under.
        let tall = drawn(&picture(&["a", "a"]), Turn::Level);
        let cell = &tall[(0, 0)];
        assert_eq!((cell.symbol(), cell.bg), (" ", Color::Rgb(9, 9, 9)));
        let top = drawn(&picture(&["a"]), Turn::Level);
        let cell = &top[(0, 0)];
        assert_eq!(
            (cell.symbol(), cell.fg, cell.bg),
            ("▀", Color::Rgb(9, 9, 9), UNDER)
        );
        // A quarter turn clockwise: the left column becomes the top row, read right to
        // left, so `a` over nothing turns to nothing beside `a`, and the top row to the
        // right-hand column.
        let turned = drawn(&picture(&["ab", ".."]), Turn::Quarter);
        assert_eq!(turned[(0, 0)].bg, UNDER, "nothing written");
        let cell = &turned[(1, 0)];
        assert_eq!(
            (cell.symbol(), cell.fg, cell.bg),
            ("▀", Color::Rgb(9, 9, 9), Color::Rgb(7, 7, 7))
        );
        assert_eq!(footprint(&picture(&["a.", ".."]), Turn::Half), [(1, 0)]);
    }

    /// The rows turned a quarter clockwise, as text, for the round trip above.
    fn turn_rows(rows: &[String]) -> Vec<String> {
        let g: Vec<Vec<char>> = rows.iter().map(|r| r.chars().collect()).collect();
        let (h, w) = (g.len(), g[0].len());
        (0..w)
            .map(|r| (0..h).map(|c| g[h - 1 - c][r]).collect())
            .collect()
    }

    #[test]
    fn stickers_draw_in_order() {
        let area = Rect::new(0, 0, 6, 1);
        let mut buf = Buffer::empty(area);
        let mut pen = Pen {
            buf: &mut buf,
            ox: 0,
            oy: 0,
            clip: area,
        };
        let at = |text: &str, x| Placed {
            art: lettered(text),
            x,
            y: 0,
            turn: Turn::Level,
        };
        draw(&[at("aaaa", 0), at("bb", 2)], (6, 1), &mut pen);
        let row: String = (0..6).map(|x| buf[(x, 0)].symbol()).collect();
        assert_eq!(row, "aabb  ");
    }

    #[test]
    fn placed_resolves_known_stickers_only() {
        let c = content();
        let stuck = |id: &str| Stuck {
            sticker: game::ItemId::new(id),
            x: 1,
            y: 2,
            turn: Turn::Half,
        };
        let out = placed(&c, &[stuck("sticker_neko"), stuck("crack"), stuck("nope")]);
        assert_eq!(out.len(), 1);
        assert_eq!((out[0].x, out[0].y, out[0].turn), (1, 2, Turn::Half));
        assert!(matches!(out[0].art, StickerArt::Picture { .. }));
    }
}
