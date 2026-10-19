//! Items drawn by a look: the drawing an item with sockets wears on its blueprint
//! and where its sockets sit on it.

pub mod ratchet;

use crate::ui::art::{Art, SocketPlace};
use crate::ui::cyberdeck::Pen;
use game::{HardwareLook, ItemLook};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

/// An item drawn by its look, with the pieces fitted in its sockets.
#[derive(Clone, Debug, PartialEq)]
pub struct ItemArt {
    pub look: ItemLook,
    /// One per socket the item has: the look of the piece fitted there, if any.
    pub fitted: Vec<Option<HardwareLook>>,
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
            ItemLook::Ratchet => ratchet::draw(&self.fitted, &mut pen),
        }
    }

    fn sockets(&self) -> Vec<SocketPlace> {
        places(self.look, self.fitted.len())
    }
}
