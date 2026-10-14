//! Light and visibility applied to cells.

use crate::render::{Cell, Rgb, Visibility};

/// Darkens a colour by the tile's light level; never below a quarter so the map stays legible.
pub fn lit(c: Rgb, light: u8) -> Rgb {
    c.scale(0.25 + 0.75 * (light as f32) / 255.0)
}

/// Remembered tiles are dim and static; unseen tiles are black.
pub fn apply_vis(cell: Cell, vis: Visibility) -> Cell {
    match vis {
        Visibility::Visible => cell,
        Visibility::Remembered => Cell {
            fg: cell.fg.scale(0.35),
            bg: cell.bg.scale(0.35),
            bold: false,
            dim: true,
            ..cell
        },
        Visibility::Unseen => Cell {
            ch: ' ',
            fg: Rgb(0, 0, 0),
            bg: Rgb(0, 0, 0),
            bold: false,
            dim: false,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::{Cell, Rgb};

    #[test]
    fn lit_scales_between_quarter_and_full() {
        assert_eq!(lit(Rgb(200, 100, 40), 255), Rgb(200, 100, 40));
        assert_eq!(lit(Rgb(200, 100, 40), 0), Rgb(50, 25, 10));
        let c = Cell {
            ch: 'x',
            fg: Rgb(200, 200, 200),
            bg: Rgb(20, 20, 20),
            bold: true,
            dim: false,
        };
        let r = apply_vis(c, Visibility::Remembered);
        assert!(r.fg.0 < 100 && r.dim && !r.bold);
        assert_eq!(apply_vis(c, Visibility::Visible), c);
    }
}
