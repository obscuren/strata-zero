//! A keyboard-driven form of numbers, sliders, choices, text and buttons.

use crate::anim::effects::pulse;
use crate::render::Rgb;
use crossterm::event::KeyCode;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::Widget;

#[derive(Clone, Debug, PartialEq)]
pub enum Field {
    Number {
        label: String,
        value: i64,
        min: i64,
        max: i64,
        step: i64,
    },
    Slider {
        label: String,
        value: f32,
        min: f32,
        max: f32,
        step: f32,
    },
    Choice {
        label: String,
        options: Vec<String>,
        idx: usize,
    },
    Text {
        label: String,
        value: String,
        max: usize,
    },
    Button {
        label: String,
    },
}

impl Field {
    pub fn label(&self) -> &str {
        match self {
            Field::Number { label, .. }
            | Field::Slider { label, .. }
            | Field::Choice { label, .. }
            | Field::Text { label, .. }
            | Field::Button { label } => label,
        }
    }

    fn adjust(&mut self, dir: i32) {
        match self {
            Field::Number {
                value,
                min,
                max,
                step,
                ..
            } => {
                *value = (*value + dir as i64 * *step).clamp(*min, *max);
            }
            Field::Slider {
                value,
                min,
                max,
                step,
                ..
            } => {
                *value = (*value + dir as f32 * *step).clamp(*min, *max);
                *value = (*value * 1000.0).round() / 1000.0;
            }
            Field::Choice { options, idx, .. } => {
                let n = options.len().max(1);
                *idx = (*idx as i64 + dir as i64).rem_euclid(n as i64) as usize;
            }
            _ => {}
        }
    }
}

#[derive(Clone, Debug)]
pub struct Form {
    pub fields: Vec<Field>,
    pub selected: usize,
}

impl Form {
    /// Applies a key; returns the index of a button if one was pressed.
    pub fn handle(&mut self, key: KeyCode) -> Option<usize> {
        let n = self.fields.len();
        if n == 0 {
            return None;
        }
        match key {
            KeyCode::Up | KeyCode::BackTab => {
                self.selected = (self.selected + n - 1) % n;
            }
            KeyCode::Down | KeyCode::Tab => {
                self.selected = (self.selected + 1) % n;
            }
            KeyCode::Left => self.fields[self.selected].adjust(-1),
            KeyCode::Right => self.fields[self.selected].adjust(1),
            KeyCode::Enter => {
                if matches!(self.fields[self.selected], Field::Button { .. }) {
                    return Some(self.selected);
                }
                self.selected = (self.selected + 1) % n;
            }
            KeyCode::Backspace => {
                if let Field::Text { value, .. } = &mut self.fields[self.selected] {
                    value.pop();
                }
            }
            KeyCode::Char(c) => {
                if let Field::Text { value, max, .. } = &mut self.fields[self.selected]
                    && c.is_ascii_digit()
                    && value.chars().count() < *max
                {
                    value.push(c);
                }
            }
            _ => {}
        }
        None
    }

    pub fn text(&self, label: &str) -> Option<&str> {
        self.fields.iter().find_map(|f| match f {
            Field::Text {
                label: l, value, ..
            } if l == label => Some(value.as_str()),
            _ => None,
        })
    }

    pub fn number(&self, label: &str) -> Option<i64> {
        self.fields.iter().find_map(|f| match f {
            Field::Number {
                label: l, value, ..
            } if l == label => Some(*value),
            _ => None,
        })
    }

    pub fn slider(&self, label: &str) -> Option<f32> {
        self.fields.iter().find_map(|f| match f {
            Field::Slider {
                label: l, value, ..
            } if l == label => Some(*value),
            _ => None,
        })
    }

    pub fn choice(&self, label: &str) -> Option<&str> {
        self.fields.iter().find_map(|f| match f {
            Field::Choice {
                label: l,
                options,
                idx,
            } if l == label => options.get(*idx).map(String::as_str),
            _ => None,
        })
    }

    pub fn set_text(&mut self, label: &str, new: String) {
        for f in &mut self.fields {
            if let Field::Text {
                label: l, value, ..
            } = f
                && l == label
            {
                *value = new;
                return;
            }
        }
    }
}

const LABEL_W: usize = 14;
const SLIDER_W: usize = 20;

fn render_value(field: &Field, selected: bool, tick: u64) -> String {
    match field {
        Field::Number { value, .. } => format!("◂ {value:>4} ▸"),
        Field::Slider {
            value, min, max, ..
        } => {
            let t = if max > min {
                (value - min) / (max - min)
            } else {
                0.0
            };
            let knob = (t * (SLIDER_W as f32 - 1.0)).round() as usize;
            let bar: String = (0..SLIDER_W)
                .map(|i| if i == knob { '◉' } else { '━' })
                .collect();
            format!("{bar} {value:.2}")
        }
        Field::Choice { options, idx, .. } => {
            format!(
                "◂ {:^8} ▸",
                options.get(*idx).map(String::as_str).unwrap_or("")
            )
        }
        Field::Text { value, .. } => {
            let cursor = if selected && (tick / 15).is_multiple_of(2) {
                "▏"
            } else {
                " "
            };
            if value.is_empty() {
                format!("(random){cursor}")
            } else {
                format!("{value}{cursor}")
            }
        }
        Field::Button { label } => format!("[ {label} ]"),
    }
}

pub struct FormView<'a> {
    pub form: &'a Form,
    pub tick: u64,
    pub accent: Rgb,
}

impl Widget for FormView<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        for (i, field) in self.form.fields.iter().enumerate() {
            let y = area.y + i as u16;
            if y >= area.bottom() {
                break;
            }
            let selected = i == self.form.selected;
            let is_button = matches!(field, Field::Button { .. });
            let line = if is_button {
                format!(
                    "{}{:>w$}",
                    if selected { "▸ " } else { "  " },
                    render_value(field, selected, self.tick),
                    w = LABEL_W + 2
                )
            } else {
                format!(
                    "{}{:<w$} {}",
                    if selected { "▸ " } else { "  " },
                    field.label(),
                    render_value(field, selected, self.tick),
                    w = LABEL_W
                )
            };
            let style = if selected {
                let glow = 0.7 + 0.3 * pulse(self.tick, 30);
                Style::default()
                    .fg(self.accent.scale(glow).to_color())
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::Rgb(175, 185, 200))
            };
            for (j, ch) in line.chars().enumerate() {
                let x = area.x + j as u16;
                if x >= area.right() {
                    break;
                }
                buf[(x, y)].set_char(ch).set_style(style);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::Rgb;
    use crossterm::event::KeyCode;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use ratatui::widgets::Widget;

    #[test]
    fn form_adjusts_and_fires_button() {
        let mut f = Form {
            fields: vec![
                Field::Number {
                    label: "Districts".into(),
                    value: 7,
                    min: 4,
                    max: 12,
                    step: 1,
                },
                Field::Slider {
                    label: "Density".into(),
                    value: 0.6,
                    min: 0.0,
                    max: 1.0,
                    step: 0.05,
                },
                Field::Choice {
                    label: "Water".into(),
                    options: vec!["none".into(), "bay".into(), "river".into()],
                    idx: 1,
                },
                Field::Text {
                    label: "Seed".into(),
                    value: "12".into(),
                    max: 5,
                },
                Field::Button {
                    label: "GENERATE".into(),
                },
            ],
            selected: 0,
        };
        assert_eq!(f.handle(KeyCode::Right), None);
        assert!(matches!(f.fields[0], Field::Number { value: 8, .. }));
        for _ in 0..10 {
            f.handle(KeyCode::Right);
        }
        assert!(matches!(f.fields[0], Field::Number { value: 12, .. }));
        f.handle(KeyCode::Down);
        f.handle(KeyCode::Right);
        assert!(matches!(f.fields[1], Field::Slider { value, .. } if (value - 0.65).abs() < 1e-4));
        f.handle(KeyCode::Down);
        f.handle(KeyCode::Right);
        f.handle(KeyCode::Right);
        assert!(matches!(f.fields[2], Field::Choice { idx: 0, .. }));
        f.handle(KeyCode::Down);
        f.handle(KeyCode::Char('3'));
        f.handle(KeyCode::Char('x'));
        assert!(matches!(&f.fields[3], Field::Text { value, .. } if value == "123"));
        f.handle(KeyCode::Backspace);
        assert!(matches!(&f.fields[3], Field::Text { value, .. } if value == "12"));
        f.handle(KeyCode::Down);
        assert_eq!(f.handle(KeyCode::Enter), Some(4));
        f.handle(KeyCode::Down);
        assert_eq!(f.selected, 0);
        let mut t = Terminal::new(TestBackend::new(60, 10)).unwrap();
        t.draw(|fr| {
            FormView {
                form: &f,
                tick: 3,
                accent: Rgb(0, 255, 255),
            }
            .render(fr.area(), fr.buffer_mut())
        })
        .unwrap();
        let text: String = (0..10)
            .map(|y| {
                (0..60)
                    .map(|x| t.backend().buffer()[(x, y)].symbol().to_string())
                    .collect::<String>()
                    + "\n"
            })
            .collect();
        assert!(text.contains("Districts") && text.contains("GENERATE") && text.contains("◉"));
    }
}
