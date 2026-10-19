//! Items drawn by a look: the drawing an item with sockets wears on its blueprint
//! and where its sockets sit on it.

pub mod ratchet;

use crate::ui::art::{Art, PartPlace, SocketPlace, StickerArea};
use crate::ui::cyberdeck::Pen;
use crate::ui::hardware::draw_fitted;
use crate::ui::stickers::{self, Placed};
use game::{HardwareLook, ItemLook};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
#[cfg(test)]
use ratatui::style::Color;

/// An item drawn by its look, with the pieces fitted in its sockets.
#[derive(Clone, Debug, PartialEq)]
pub struct ItemArt {
    pub look: ItemLook,
    /// One per socket the item has: the look of the piece fitted there, if any.
    pub fitted: Vec<Option<HardwareLook>>,
    /// The stickers on the copy drawn, oldest first.
    pub stickers: Vec<Placed>,
}

/// The drawing's size for a look.
fn size(look: ItemLook) -> (u16, u16) {
    match look {
        ItemLook::Ratchet => (ratchet::W, ratchet::H),
    }
}

/// Where each of a look's sockets sits: x, y, width and height in cells, in the
/// order `ItemLook::sockets` names them.
fn rects(look: ItemLook) -> &'static [(i32, i32, u16, u16)] {
    match look {
        ItemLook::Ratchet => &ratchet::PLACES,
    }
}

/// Where stickers may lie on a look.
fn area(look: ItemLook) -> StickerArea {
    match look {
        ItemLook::Ratchet => StickerArea::new(
            ratchet::W,
            ratchet::H,
            &ratchet::AREA_KEEP,
            &ratchet::AREA_DROP,
        ),
    }
}

/// The first `count` sockets of a look and where they sit.
fn places(look: ItemLook, count: usize) -> Vec<SocketPlace> {
    look.sockets()
        .iter()
        .zip(rects(look))
        .enumerate()
        .take(count)
        .map(|(index, (&(name, kind), &(x, y, w, h)))| SocketPlace {
            index,
            x,
            y,
            w,
            h,
            name,
            kind,
        })
        .collect()
}

/// A look's parts and where they sit; only some looks are built from parts.
fn part_places(look: ItemLook) -> Vec<PartPlace> {
    match look {
        ItemLook::Ratchet => ratchet::PARTS
            .iter()
            .enumerate()
            .map(
                |(index, &(item, (x, y, w, h), apart, balloon, leader))| PartPlace {
                    index,
                    item,
                    x,
                    y,
                    w,
                    h,
                    apart,
                    balloon,
                    leader,
                },
            )
            .collect(),
    }
}

impl Art for ItemArt {
    fn size(&self) -> (u16, u16) {
        size(self.look)
    }

    fn draw(&self, (ox, oy): (u16, u16), clip: Rect, buf: &mut Buffer) {
        let mut pen = Pen {
            clip: clip.intersection(buf.area),
            buf,
            ox,
            oy,
        };
        match self.look {
            ItemLook::Ratchet => ratchet::draw(&mut pen),
        }
        for (look, place) in self.fitted.iter().zip(self.sockets()) {
            if let Some(look) = look {
                draw_fitted(*look, &place, &mut pen);
            }
        }
        stickers::draw(&self.stickers, size(self.look), &mut pen);
    }

    fn sockets(&self) -> Vec<SocketPlace> {
        places(self.look, self.fitted.len())
    }

    fn parts(&self) -> Vec<PartPlace> {
        part_places(self.look)
    }

    fn draw_part(&self, part: usize, (ox, oy): (u16, u16), clip: Rect, buf: &mut Buffer) {
        let mut pen = Pen {
            clip: clip.intersection(buf.area),
            buf,
            ox,
            oy,
        };
        match self.look {
            ItemLook::Ratchet => ratchet::draw_part(&mut pen, part),
        }
    }

    fn sticker_area(&self) -> Option<StickerArea> {
        Some(area(self.look))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use unicode_width::UnicodeWidthStr;

    const LOOKS: [ItemLook; 1] = [ItemLook::Ratchet];
    const PIECES: [HardwareLook; 2] = [HardwareLook::Hushcan, HardwareLook::Redeye];

    #[test]
    fn every_socket_of_a_look_has_a_rect() {
        for look in LOOKS {
            assert_eq!(rects(look).len(), look.sockets().len(), "{look:?}");
        }
    }

    #[test]
    fn the_ratchet_takes_stickers_on_its_body_only() {
        let art = ItemArt {
            look: ItemLook::Ratchet,
            fitted: vec![None, None],
            stickers: Vec::new(),
        };
        let area = art.sticker_area().expect("the gun takes stickers");
        let on = |x: i32, y: i32| area.contains(x, y);
        assert!(
            on(30, 5) && on(40, 12) && on(3, 6),
            "receiver, grip, butt pad"
        );
        assert!(!on(50, 5), "the ejection port");
        assert!(!on(62, 6), "the barrel");
        assert!(!on(10, 4), "the wire stock");
        for place in art.sockets() {
            for y in place.y..place.y + i32::from(place.h) {
                for x in place.x..place.x + i32::from(place.w) {
                    assert!(!on(x, y), "{} at ({x},{y})", place.name);
                }
            }
        }
    }

    #[test]
    fn the_gun_draws_its_stickers() {
        let sticker = crate::ui::stickers::Placed {
            art: crate::ui::stickers::tests::lettered(" HI "),
            x: 26,
            y: 5,
            turn: game::Turn::Level,
        };
        let art = ItemArt {
            look: ItemLook::Ratchet,
            fitted: vec![None, None],
            stickers: vec![sticker],
        };
        let (w, h) = art.size();
        let area = Rect::new(0, 0, w, h);
        let mut buf = Buffer::empty(area);
        art.draw((0, 0), area, &mut buf);
        let row: String = (26..30).map(|x| buf[(x, 5)].symbol()).collect();
        assert_eq!(row, " HI ");
        assert_eq!(buf[(27, 5)].bg, Color::Rgb(4, 5, 6));
    }

    #[test]
    fn the_gun_clips_its_stickers_to_the_drawing() {
        let sticker = crate::ui::stickers::Placed {
            art: crate::ui::stickers::tests::lettered(" HELLO "),
            x: 85,
            y: 5,
            turn: game::Turn::Level,
        };
        let art = ItemArt {
            look: ItemLook::Ratchet,
            fitted: vec![None, None],
            stickers: vec![sticker],
        };
        let (w, h) = art.size();
        let area = Rect::new(0, 0, w + 10, h);
        let mut buf = Buffer::empty(area);
        art.draw((0, 0), area, &mut buf);
        assert_eq!(buf[(86, 5)].symbol(), "H");
        for x in w..w + 10 {
            assert_eq!(buf[(x, 5)].symbol(), " ", "({x},5)");
            assert_eq!(buf[(x, 5)].bg, Color::Reset, "({x},5)");
        }
    }

    #[test]
    fn the_gun_draws_single_width_glyphs_in_exact_colours() {
        for fitted in [
            vec![None, None],
            PIECES.iter().map(|&p| Some(p)).collect::<Vec<_>>(),
        ] {
            let art = ItemArt {
                look: ItemLook::Ratchet,
                fitted,
                stickers: Vec::new(),
            };
            let (w, h) = art.size();
            let area = Rect::new(0, 0, w, h);
            let mut buf = Buffer::empty(area);
            art.draw((0, 0), area, &mut buf);
            let mut drawings = vec![buf];
            for part in 0..art.parts().len() {
                let mut buf = Buffer::empty(area);
                art.draw_part(part, (0, 0), area, &mut buf);
                drawings.push(buf);
            }
            for buf in drawings {
                for c in &buf.content {
                    assert_eq!(c.symbol().width(), 1, "{:?}", c.symbol());
                    assert!(matches!(c.fg, Color::Rgb(..) | Color::Reset));
                    assert!(matches!(c.bg, Color::Rgb(..) | Color::Reset));
                }
            }
        }
    }

    /// An FNV-1a hash of every cell's symbol, colours and modifier, the same on every
    /// Rust release. The drawings use only `Rgb` and `Reset`.
    fn hash_of(buf: &Buffer) -> u64 {
        fn colour(c: Color) -> [u8; 4] {
            match c {
                Color::Rgb(r, g, b) => [1, r, g, b],
                _ => [0; 4],
            }
        }
        let mut h: u64 = 0xcbf2_9ce4_8422_2325;
        let mut eat = |bytes: &[u8]| {
            for &b in bytes {
                h ^= u64::from(b);
                h = h.wrapping_mul(0x0100_0000_01b3);
            }
        };
        for c in &buf.content {
            eat(c.symbol().as_bytes());
            eat(&[0]);
            eat(&colour(c.fg));
            eat(&colour(c.bg));
            eat(&c.modifier.bits().to_le_bytes());
        }
        h
    }

    /// The bare gun, nothing fitted and no stickers, cell for cell as it was drawn
    /// before it was cut into parts.
    #[test]
    fn the_whole_gun_is_unchanged() {
        let art = ItemArt {
            look: ItemLook::Ratchet,
            fitted: vec![None, None],
            stickers: Vec::new(),
        };
        let area = Rect::new(0, 0, 88, 15);
        let mut buf = Buffer::empty(area);
        art.draw((0, 0), area, &mut buf);
        assert_eq!(hash_of(&buf), 3885222701974628240);
    }

    fn blank_gun() -> (ItemArt, Rect, Buffer) {
        let art = ItemArt {
            look: ItemLook::Ratchet,
            fitted: vec![None, None],
            stickers: Vec::new(),
        };
        let (w, h) = art.size();
        let area = Rect::new(0, 0, w, h);
        (art, area, Buffer::empty(area))
    }

    /// The cells a drawing wrote, with their positions.
    fn written(buf: &Buffer) -> Vec<((u16, u16), &ratatui::buffer::Cell)> {
        let empty = ratatui::buffer::Cell::EMPTY;
        let a = buf.area;
        (a.top()..a.bottom())
            .flat_map(|y| (a.left()..a.right()).map(move |x| (x, y)))
            .map(|(x, y)| ((x, y), &buf[(x, y)]))
            .filter(|(_, c)| **c != empty)
            .collect()
    }

    #[test]
    fn parts_drawn_alone_add_up_to_the_whole_gun() {
        let (art, area, mut whole) = blank_gun();
        art.draw((0, 0), area, &mut whole);
        let mut owners = std::collections::HashMap::new();
        for part in 0..art.parts().len() {
            let mut buf = Buffer::empty(area);
            art.draw_part(part, (0, 0), area, &mut buf);
            for (at, cell) in written(&buf) {
                assert_eq!(&whole[at], cell, "part {part} at {at:?}");
                assert_eq!(owners.insert(at, part), None, "{at:?} written twice");
            }
        }
        for (at, _) in written(&whole) {
            assert!(owners.contains_key(&at), "{at:?} belongs to no part");
        }
    }

    #[test]
    fn a_part_place_is_its_drawing_s_bounding_box() {
        let (art, area, _) = blank_gun();
        for place in art.parts() {
            let mut buf = Buffer::empty(area);
            art.draw_part(place.index, (0, 0), area, &mut buf);
            let cells = written(&buf);
            let min = |f: fn(&(u16, u16)) -> u16| cells.iter().map(|(at, _)| f(at)).min().unwrap();
            let max = |f: fn(&(u16, u16)) -> u16| cells.iter().map(|(at, _)| f(at)).max().unwrap();
            let (x0, y0) = (min(|p| p.0), min(|p| p.1));
            let (x1, y1) = (max(|p| p.0), max(|p| p.1));
            assert_eq!(
                (place.x, place.y, place.w, place.h),
                (i32::from(x0), i32::from(y0), x1 - x0 + 1, y1 - y0 + 1),
                "{}",
                place.item
            );
        }
    }

    #[test]
    fn every_part_is_an_input_of_the_recipe_for_its_item() {
        let content = game::Content::load().unwrap();
        for item in content.items.iter().filter(|i| i.look.is_some()) {
            let art = ItemArt {
                look: item.look.unwrap(),
                fitted: Vec::new(),
                stickers: Vec::new(),
            };
            let parts = art.parts();
            if parts.is_empty() {
                continue;
            }
            let recipe = content
                .recipes
                .iter()
                .find(|r| r.output.0 == item.id)
                .unwrap_or_else(|| panic!("no recipe makes {}", item.id));
            for part in parts {
                assert!(
                    recipe.inputs.iter().any(|(id, _)| id.0 == part.item),
                    "{} is not an input of {}",
                    part.item,
                    recipe.id
                );
            }
        }
    }
}
