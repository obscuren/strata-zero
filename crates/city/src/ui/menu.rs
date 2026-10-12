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
        self.selected = self.selected.saturating_sub(1);
    }

    pub fn down(&mut self) {
        if self.selected + 1 < self.items.len() {
            self.selected += 1;
        }
    }

    /// Progress of item `i`'s entrance at `tick`.
    pub fn entrance(&self, i: usize, tick: u64) -> f32 {
        Tween::new(self.opened + i as u64 * 4, 8, Easing::OutCubic).progress(tick)
    }

    pub fn settled(&self, tick: u64) -> bool {
        self.items
            .iter()
            .enumerate()
            .all(|(i, _)| self.entrance(i, tick) >= 1.0)
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
        m.down();
        assert_eq!(m.selected, 2);
        m.up();
        assert_eq!(m.selected, 1);
        m.up();
        m.up();
        assert_eq!(m.selected, 0);
    }
}
