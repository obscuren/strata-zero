//! Items drawn by a look: the drawing an item with sockets wears on its blueprint
//! and where its sockets sit on it.

pub mod ratchet;

use crate::ui::art::{Art, SocketPlace, StickerArea};
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
            for c in &buf.content {
                assert_eq!(c.symbol().width(), 1, "{:?}", c.symbol());
                assert!(matches!(c.fg, Color::Rgb(..) | Color::Reset));
                assert!(matches!(c.bg, Color::Rgb(..) | Color::Reset));
            }
        }
    }
}
