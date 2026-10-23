//! A pool meter: one cell per point or per step of points, the rest in words.

use crate::render::Rgb;
use crate::ui::chrome::{DIM, MAGENTA};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};

/// `LABEL  ▮▮▮▮▯▯  2 left`, or `over by N` in magenta when `left` is negative. A cell
/// holds `per` points; a step only partly spent still fills its cell.
pub fn meter(label: &str, left: i64, total: u32, per: u32, colour: Rgb) -> Line<'static> {
    let total_i = total as i64;
    let per = per.max(1) as usize;
    let cells = (total as usize).div_ceil(per);
    let used = ((total_i - left).clamp(0, total_i) as usize).div_ceil(per);
    let over = left < 0;
    let fill = if over { MAGENTA } else { colour.to_color() };
    let tail = if over {
        format!("  over by {}", -left)
    } else {
        format!("  {left} left")
    };
    Line::from(vec![
        Span::styled(
            format!("{label}  "),
            Style::default().fg(DIM).add_modifier(Modifier::BOLD),
        ),
        Span::styled("▮".repeat(used), Style::default().fg(fill)),
        Span::styled("▯".repeat(cells - used), Style::default().fg(DIM)),
        Span::styled(tail, Style::default().fg(if over { MAGENTA } else { DIM })),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::chrome::CYAN_RGB;

    #[test]
    fn meter_shows_cells_and_the_rest() {
        assert_eq!(
            meter("SP", 2, 6, 1, CYAN_RGB).to_string(),
            "SP  ▮▮▮▮▯▯  2 left"
        );
        assert_eq!(
            meter("POINTS", 0, 4, 1, CYAN_RGB).to_string(),
            "POINTS  ▮▮▮▮  0 left"
        );
        assert_eq!(
            meter("POINTS", -2, 4, 1, CYAN_RGB).to_string(),
            "POINTS  ▮▮▮▮  over by 2"
        );
        // A cell a step: 600 in fifties is twelve, and a part step fills its cell.
        assert_eq!(
            meter("SP", 200, 600, 50, CYAN_RGB).to_string(),
            "SP  ▮▮▮▮▮▮▮▮▯▯▯▯  200 left"
        );
        assert_eq!(
            meter("SP", 575, 600, 50, CYAN_RGB).to_string(),
            "SP  ▮▯▯▯▯▯▯▯▯▯▯▯  575 left"
        );
    }
}
