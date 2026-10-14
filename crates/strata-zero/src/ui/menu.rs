//! An animated vertical menu: items slide in with a typewriter reveal.

use crate::anim::effects::{pulse, slide_x, typewriter};
use crate::anim::{Easing, Tween};
use crate::render::Rgb;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::Widget;

pub struct Menu {
    pub items: Vec<String>,
    pub selected: usize,
    /// Tick the menu appeared; drives the slide-in.
    pub opened: u64,
}

impl Menu {
    pub fn new(items: Vec<String>, opened: u64) -> Self {
        Menu {
            items,
            selected: 0,
            opened,
        }
    }

    pub fn up(&mut self) {
        let n = self.items.len();
        if n > 0 {
            self.selected = (self.selected + n - 1) % n;
        }
    }

    pub fn down(&mut self) {
        let n = self.items.len();
        if n > 0 {
            self.selected = (self.selected + 1) % n;
        }
    }

    /// Progress of item `i`'s entrance at `tick`.
    pub fn entrance(&self, i: usize, tick: u64) -> f32 {
        Tween::new(self.opened + i as u64 * 4, 8, Easing::OutCubic).progress(tick)
    }
}

pub struct MenuView<'a> {
    pub menu: &'a Menu,
    pub tick: u64,
    pub accent: Rgb,
}

impl Widget for MenuView<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        for (i, item) in self.menu.items.iter().enumerate() {
            let y = area.y + i as u16;
            if y >= area.bottom() {
                break;
            }
            let p = self.menu.entrance(i, self.tick);
            if p <= 0.0 {
                continue;
            }
            let selected = i == self.menu.selected;
            let prefix = if selected { "▸ " } else { "  " };
            let text = format!("{prefix}{}", typewriter(item, p));
            let offset = slide_x(item.chars().count() as u16 + 2, p);
            let style = if selected {
                let glow = 0.6 + 0.4 * pulse(self.tick, 30);
                Style::default()
                    .fg(self.accent.scale(glow).to_color())
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::Rgb(170, 180, 195))
            };
            for (j, ch) in text.chars().enumerate() {
                let x = area.x as i32 + offset + j as i32;
                if x < area.x as i32 || x >= area.right() as i32 {
                    continue;
                }
                buf[(x as u16, y)].set_char(ch).set_style(style);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::Rgb;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use ratatui::layout::Rect;
    use ratatui::widgets::Widget;

    fn row(t: &Terminal<TestBackend>, y: u16) -> String {
        (0..40)
            .map(|x| t.backend().buffer()[(x, y)].symbol().to_string())
            .collect()
    }

    #[test]
    fn menu_slides_in_and_selects() {
        let mut m = Menu::new(
            vec!["NEW CITY".into(), "LOAD CITY".into(), "QUIT".into()],
            0,
        );
        let mut t = Terminal::new(TestBackend::new(40, 10)).unwrap();
        t.draw(|f| {
            MenuView {
                menu: &m,
                tick: 0,
                accent: Rgb(0, 255, 255),
            }
            .render(Rect::new(2, 2, 30, 6), f.buffer_mut())
        })
        .unwrap();
        assert!(!row(&t, 2).contains("NEW CITY"));
        t.draw(|f| {
            MenuView {
                menu: &m,
                tick: 40,
                accent: Rgb(0, 255, 255),
            }
            .render(Rect::new(2, 2, 30, 6), f.buffer_mut())
        })
        .unwrap();
        assert!(row(&t, 2).contains("▸ NEW CITY"), "{}", row(&t, 2));
        assert!(row(&t, 3).contains("LOAD CITY"));
        assert!(row(&t, 4).contains("QUIT"));
        m.down();
        m.down();
        assert_eq!(m.selected, 2);
        m.up();
        assert_eq!(m.selected, 1);
        m.up();
        assert_eq!(m.selected, 0);
    }

    #[test]
    fn menu_wraps_at_both_ends() {
        let mut m = Menu::new(vec!["A".into(), "B".into(), "C".into()], 0);
        m.up();
        assert_eq!(m.selected, 2);
        m.down();
        assert_eq!(m.selected, 0);
        let mut one = Menu::new(vec!["A".into()], 0);
        one.up();
        one.down();
        assert_eq!(one.selected, 0);
    }
}
