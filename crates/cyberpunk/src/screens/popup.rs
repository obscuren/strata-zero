//! A popup of verbs over whatever opened it: hotkey letters, a selected row, a card view.

use crate::actions::{perform, report};
use crate::anim::effects::{pulse, typewriter};
use crate::app::{AppState, Screen, ScreenCmd, TransitionKind};
use crate::ui::chrome::{
    BAR_BG, CYAN, CYAN_RGB, DIM, MAGENTA, PANEL_BG, TEXT, frame, glass, put, put_paragraph, reveal,
};
use crossterm::event::{KeyCode, KeyEvent};
use game::Action;
use ratatui::Frame;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::Line;

/// What a row does when fired.
#[derive(Clone, Debug)]
pub enum Pick {
    /// Runs through `actions::perform`; the result lands in the status strip.
    Act(Action),
    /// Shows these lines in place of the rows; Esc returns to the rows.
    Card(Vec<Line<'static>>),
    #[allow(dead_code)] // the abilities popup (world-interact spec) builds disabled rows
    Close,
}

#[derive(Clone, Debug)]
pub struct Choice {
    pub key: char,
    pub label: String,
    pub pick: Pick,
    pub enabled: bool,
}

impl Choice {
    pub fn new(key: char, label: impl Into<String>, pick: Pick) -> Self {
        Choice {
            key,
            label: label.into(),
            pick,
            enabled: true,
        }
    }

    /// A greyed row that cannot be fired; shown so the player knows it exists.
    #[allow(dead_code)] // the abilities popup (world-interact spec) builds disabled rows
    pub fn disabled(key: char, label: impl Into<String>) -> Self {
        Choice {
            key,
            label: label.into(),
            pick: Pick::Close,
            enabled: false,
        }
    }
}

const REVEAL_TICKS: u64 = 10;
const MIN_W: u16 = 24;
const MAX_W: u16 = 48;

pub struct Popup {
    title: String,
    choices: Vec<Choice>,
    pub selected: usize,
    anchor: Option<(u16, u16)>,
    /// Tick the rows (or the card) appeared; drives the reveal.
    opened: u64,
    /// When set, the card's lines replace the rows.
    card: Option<Vec<Line<'static>>>,
    /// Whether closing the card returns to rows (verbs) or closes the popup.
    verbs_behind: bool,
}

impl Popup {
    pub fn verbs(title: &str, choices: Vec<Choice>, tick: u64) -> Self {
        let selected = choices.iter().position(|c| c.enabled).unwrap_or(0);
        Popup {
            title: title.to_string(),
            choices,
            selected,
            anchor: None,
            opened: tick,
            card: None,
            verbs_behind: true,
        }
    }

    #[allow(dead_code)] // the world's `look` (world-interact spec) opens a plain card
    pub fn card(title: &str, lines: Vec<Line<'static>>, tick: u64) -> Self {
        Popup {
            title: title.to_string(),
            choices: Vec::new(),
            selected: 0,
            anchor: None,
            opened: tick,
            card: Some(lines),
            verbs_behind: false,
        }
    }

    /// Pins the top-left corner near `(x, y)`; clamped so the whole panel stays on screen.
    pub fn anchored(mut self, x: u16, y: u16) -> Self {
        self.anchor = Some((x, y));
        self
    }

    fn move_sel(&mut self, delta: i32) {
        let n = self.choices.len() as i32;
        let mut i = self.selected as i32;
        loop {
            i += delta;
            if i < 0 || i >= n {
                return;
            }
            if self.choices[i as usize].enabled {
                self.selected = i as usize;
                return;
            }
        }
    }

    /// Where the panel goes inside `area`.
    pub(crate) fn rect(&self, area: Rect) -> Rect {
        let title_w = self.title.chars().count() + 2;
        let (w, h) = match &self.card {
            Some(lines) => {
                let longest = lines.iter().map(|l| l.width()).max().unwrap_or(0);
                let w = (longest.max(title_w) as u16 + 6).clamp(MIN_W, MAX_W);
                let inner_w = w.saturating_sub(4).max(1) as usize;
                let rows: usize = lines
                    .iter()
                    .map(|l| l.width().max(1).div_ceil(inner_w))
                    .sum();
                (w, rows as u16 + 4)
            }
            None => {
                let longest = self
                    .choices
                    .iter()
                    .map(|c| c.label.chars().count() + 5)
                    .max()
                    .unwrap_or(0);
                let w = (longest.max(title_w) as u16 + 6).clamp(MIN_W, MAX_W);
                (w, self.choices.len() as u16 + 4)
            }
        };
        let w = w.min(area.width);
        let h = h.min(area.height);
        let (x, y) = match self.anchor {
            Some((ax, ay)) => (
                ax.clamp(area.x, area.right().saturating_sub(w).max(area.x)),
                ay.clamp(area.y, area.bottom().saturating_sub(h).max(area.y)),
            ),
            None => (
                area.x + area.width.saturating_sub(w) / 2,
                area.y + area.height.saturating_sub(h) / 2,
            ),
        };
        Rect::new(x, y, w, h)
    }

    fn fire(&mut self, st: &mut AppState, i: usize) -> ScreenCmd {
        let Some(choice) = self.choices.get(i) else {
            return ScreenCmd::Stay;
        };
        if !choice.enabled {
            return ScreenCmd::Stay;
        }
        match choice.pick.clone() {
            Pick::Act(action) => {
                let result = perform(st, action);
                report(st, result);
                ScreenCmd::Pop {
                    fx: TransitionKind::None,
                }
            }
            Pick::Card(lines) => {
                self.card = Some(lines);
                self.opened = st.tick;
                ScreenCmd::Stay
            }
            Pick::Close => ScreenCmd::Pop {
                fx: TransitionKind::None,
            },
        }
    }

    fn draw_rows(&self, st: &AppState, inner: Rect, bar: Rect, buf: &mut Buffer) {
        let p = st.tick.saturating_sub(self.opened) as f32 / REVEAL_TICKS as f32;
        let n = self.choices.len().max(1) as f32;
        let glow = 0.7 + 0.3 * pulse(st.tick, 30);
        for (i, c) in self.choices.iter().enumerate() {
            let ry = inner.y + i as u16;
            if ry >= inner.bottom() {
                break;
            }
            let selected = i == self.selected && c.enabled;
            let bg = if selected { BAR_BG } else { PANEL_BG };
            if selected {
                for cx in bar.x..bar.right() {
                    buf[(cx, ry)].set_bg(BAR_BG);
                }
            }
            let label_style = if selected {
                Style::default()
                    .fg(CYAN_RGB.scale(glow).to_color())
                    .add_modifier(Modifier::BOLD)
            } else if c.enabled {
                Style::default().fg(TEXT)
            } else {
                Style::default().fg(DIM)
            };
            let key_style = if c.enabled {
                Style::default().fg(MAGENTA).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(DIM)
            };
            let frac = (p * n - i as f32).clamp(0.0, 1.0);
            let key = if c.enabled {
                c.key.to_string()
            } else {
                " ".to_string()
            };
            put(
                buf,
                inner.x,
                ry,
                if selected { "▸ " } else { "  " },
                label_style.bg(bg),
                inner.right(),
            );
            put(buf, inner.x + 2, ry, &key, key_style.bg(bg), inner.right());
            put(
                buf,
                inner.x + 3,
                ry,
                &format!("  {}", typewriter(&c.label, frac)),
                label_style.bg(bg),
                inner.right(),
            );
        }
    }
}

impl Screen for Popup {
    fn update(&mut self, st: &mut AppState, keys: &[KeyEvent]) -> ScreenCmd {
        for k in keys {
            if self.card.is_some() {
                if matches!(
                    k.code,
                    KeyCode::Esc | KeyCode::Char('q') | KeyCode::Enter | KeyCode::Char(' ')
                ) {
                    if self.verbs_behind {
                        self.card = None;
                        self.opened = st.tick;
                    } else {
                        return ScreenCmd::Pop {
                            fx: TransitionKind::None,
                        };
                    }
                }
                continue;
            }
            match k.code {
                KeyCode::Up | KeyCode::Char('k') => self.move_sel(-1),
                KeyCode::Down | KeyCode::Char('j') => self.move_sel(1),
                KeyCode::Esc | KeyCode::Char('q') => {
                    return ScreenCmd::Pop {
                        fx: TransitionKind::None,
                    };
                }
                KeyCode::Enter | KeyCode::Char(' ') => return self.fire(st, self.selected),
                KeyCode::Char(c) => {
                    if let Some(i) = self.choices.iter().position(|ch| ch.enabled && ch.key == c) {
                        return self.fire(st, i);
                    }
                }
                _ => {}
            }
        }
        ScreenCmd::Stay
    }

    fn draw(&self, st: &AppState, f: &mut Frame) {
        let r = self.rect(f.area());
        if r.width < 6 || r.height < 4 {
            return;
        }
        let buf = f.buffer_mut();
        glass(buf, r);
        frame(buf, r);
        put(
            buf,
            r.x + 2,
            r.y,
            &format!(" {} ", self.title),
            Style::default().fg(CYAN).add_modifier(Modifier::BOLD),
            r.right() - 1,
        );
        let inner = Rect::new(r.x + 2, r.y + 2, r.width - 4, r.height - 4);
        let bar = Rect::new(r.x + 1, r.y + 2, r.width - 2, r.height - 4);
        match &self.card {
            Some(lines) => {
                let p = st.tick.saturating_sub(self.opened) as f32 / REVEAL_TICKS as f32;
                let shown = reveal(lines, p, (st.tick / 4).is_multiple_of(2));
                put_paragraph(buf, inner, shown);
            }
            None => self.draw_rows(st, inner, bar, buf),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::screens::testutil::{buffer_text, key};
    use citygen::{GenParams, Generator, SizePreset};
    use game::ItemId;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    fn state() -> AppState {
        let mut st = AppState::new();
        let city = Generator::run(GenParams {
            seed: 6,
            size: SizePreset::Custom { w: 120, h: 60 },
            districts: 5,
            ..Default::default()
        })
        .unwrap();
        st.player = Some(crate::walker::Player::spawn(&city, 1));
        st.city = Some(city);
        st
    }

    fn draw(p: &Popup, st: &AppState, w: u16, h: u16) -> String {
        let mut t = Terminal::new(TestBackend::new(w, h)).unwrap();
        t.draw(|f| p.draw(st, f)).unwrap();
        buffer_text(t.backend().buffer())
    }

    #[test]
    fn popup_lists_verbs_and_fires_by_letter() {
        let mut st = state();
        st.character.hp = 20;
        let mut p = Popup::verbs(
            "STIM PATCH",
            vec![
                Choice::new(
                    'u',
                    "use",
                    Pick::Act(Action::Use(ItemId::new("stim_patch"))),
                ),
                Choice::disabled('x', "nothing"),
                Choice::new(
                    'd',
                    "drop one",
                    Pick::Act(Action::Drop {
                        item: ItemId::new("stim_patch"),
                        qty: 1,
                    }),
                ),
            ],
            0,
        );
        st.tick = 40;
        let text = draw(&p, &st, 120, 35);
        assert!(
            text.contains("STIM PATCH") && text.contains("u  use") && text.contains("d  drop one"),
            "{text}"
        );
        p.update(&mut st, &[key(KeyCode::Down), key(KeyCode::Down)]);
        assert_eq!(p.selected, 2, "skips the disabled row and clamps");
        p.update(&mut st, &[key(KeyCode::Up)]);
        assert_eq!(p.selected, 0);
        assert!(matches!(
            p.update(&mut st, &[key(KeyCode::Char('u'))]),
            ScreenCmd::Pop { .. }
        ));
        assert_eq!(st.character.hp, 35);
        assert_eq!(st.status_text(), Some("used Stim patch · +15 hp"));
        assert!(!st.status_is_warning());
    }

    #[test]
    fn popup_refusal_warns_and_cards_open_and_close() {
        let mut st = state();
        let mut p = Popup::verbs(
            "COIL",
            vec![Choice::new(
                'e',
                "equip",
                Pick::Act(Action::Equip(ItemId::new("copper_coil"))),
            )],
            0,
        );
        assert!(matches!(
            p.update(&mut st, &[key(KeyCode::Enter)]),
            ScreenCmd::Pop { .. }
        ));
        assert!(st.status_is_warning());
        let mut p = Popup::verbs(
            "COIL",
            vec![Choice::new(
                'i',
                "inspect",
                Pick::Card(vec![Line::from("a spool")]),
            )],
            0,
        );
        assert!(matches!(
            p.update(&mut st, &[key(KeyCode::Char('i'))]),
            ScreenCmd::Stay
        ));
        st.tick = 40;
        assert!(draw(&p, &st, 120, 35).contains("a spool"));
        assert!(
            matches!(p.update(&mut st, &[key(KeyCode::Esc)]), ScreenCmd::Stay),
            "esc leaves the card"
        );
        assert!(matches!(
            p.update(&mut st, &[key(KeyCode::Esc)]),
            ScreenCmd::Pop { .. }
        ));
        let mut c = Popup::card("CARD", vec![Line::from("x")], 0);
        assert!(matches!(
            c.update(&mut st, &[key(KeyCode::Char(' '))]),
            ScreenCmd::Pop { .. }
        ));
    }

    #[test]
    fn popup_clamps_inside_the_minimum_terminal() {
        let mut st = state();
        let p = Popup::verbs(
            "EDGE",
            vec![Choice::new(
                'a',
                "a very long label that pushes the width",
                Pick::Close,
            )],
            0,
        )
        .anchored(95, 28);
        let r = p.rect(Rect::new(0, 0, 100, 30));
        assert!(
            r.right() <= 100 && r.bottom() <= 30 && r.width >= 24,
            "{r:?}"
        );
        st.tick = 40;
        let text = draw(&p, &st, 100, 30);
        assert!(
            text.contains("EDGE") && text.contains("a very long"),
            "{text}"
        );
        for c in ['┍', '┑', '┕', '┙', '▸', '▮'] {
            assert_eq!(unicode_width::UnicodeWidthChar::width(c), Some(1));
        }
    }
}
