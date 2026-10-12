//! Effects as pure functions of progress and tick: typewriter, slide, flicker, pulse, glitch.

use citygen::rng::hash64;
use ratatui::layout::Rect;

/// Characters a glitch effect scatters over the frame.
pub const GLITCH_CHARS: &[char] = &[
    '▓', '▒', '░', '█', '▀', '▄', '▌', '▐', '╳', '╬', '≋', '≈', 'ｦ', 'ｱ', 'ﾂ', 'ﾈ', '0', '1', '/',
    '\\',
];

/// The first `p` share of `text`, by characters, never splitting one.
pub fn typewriter(text: &str, p: f32) -> &str {
    let n = text.chars().count();
    let shown = ((n as f32) * p.clamp(0.0, 1.0)).floor() as usize;
    let end = text
        .char_indices()
        .nth(shown)
        .map(|(i, _)| i)
        .unwrap_or(text.len());
    &text[..end]
}

/// Horizontal offset for something sliding in from the left: -width at 0, 0 at 1.
pub fn slide_x(width: u16, p: f32) -> i32 {
    let p = p.clamp(0.0, 1.0);
    (-(width as f32) * (1.0 - p)).round() as i32
}

/// Brightness of a neon tube in 0..=1. Broken tubes drop out on a per-variant schedule.
pub fn flicker(tick: u64, variant: u8, broken: bool) -> f32 {
    if broken {
        let phase = (tick / 3 + variant as u64 * 7) % 64;
        if matches!(phase, 10..=13 | 20 | 21 | 40) {
            return 0.05;
        }
    }
    0.8 + 0.2 * ((tick as f32) / 9.0 + variant as f32).sin()
}

/// Smooth 0..=1 oscillation with the given period in ticks.
pub fn pulse(tick: u64, period: u64) -> f32 {
    let period = period.max(1) as f32;
    0.5 + 0.5 * (2.0 * std::f32::consts::PI * (tick as f32) / period).sin()
}

/// Cells to corrupt at progress `p` of a glitch wipe: dense at the start, gone at the end.
pub fn glitch_cells(area: Rect, tick: u64, p: f32) -> impl Iterator<Item = (u16, u16, char)> {
    let total = area.width as usize * area.height as usize;
    let n = ((total as f32) * 0.25 * (1.0 - p.clamp(0.0, 1.0))) as usize;
    (0..n).filter_map(move |i| {
        if area.width == 0 || area.height == 0 {
            return None;
        }
        let h = hash64(tick, i as u64);
        let x = area.x + (h % area.width as u64) as u16;
        let y = area.y + ((h >> 16) % area.height as u64) as u16;
        let ch = GLITCH_CHARS[((h >> 32) % GLITCH_CHARS.len() as u64) as usize];
        Some((x, y, ch))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::layout::Rect;

    #[test]
    fn typewriter_is_char_safe() {
        assert_eq!(typewriter("ｦｧｨ", 0.34), "ｦ");
        assert_eq!(typewriter("abc", 1.0), "abc");
        assert_eq!(typewriter("abc", 0.0), "");
    }

    #[test]
    fn slide_offset_runs_from_minus_width_to_zero() {
        assert_eq!(slide_x(20, 0.0), -20);
        assert_eq!(slide_x(20, 1.0), 0);
        assert!(slide_x(20, 0.5) < 0 && slide_x(20, 0.5) > -20);
    }

    #[test]
    fn flicker_is_deterministic_and_bounded() {
        for tick in 0..200 {
            let v = flicker(tick, 7, true);
            assert!((0.0..=1.0).contains(&v));
            assert_eq!(v, flicker(tick, 7, true));
        }
        assert!(flicker(3, 0, false) > 0.6);
        assert!((0..200).any(|t| flicker(t, 7, true) < 0.1));
        assert!((0.0..=1.0).contains(&pulse(17, 30)));
    }

    #[test]
    fn glitch_density_shrinks_with_progress() {
        let a = glitch_cells(Rect::new(0, 0, 50, 10), 1, 0.1).count();
        let b = glitch_cells(Rect::new(0, 0, 50, 10), 1, 0.9).count();
        assert!(a > b);
        for (x, y, _) in glitch_cells(Rect::new(3, 4, 50, 10), 2, 0.2) {
            assert!((3..53).contains(&x) && (4..14).contains(&y));
        }
        assert_eq!(glitch_cells(Rect::new(0, 0, 0, 0), 1, 0.1).count(), 0);
    }
}
