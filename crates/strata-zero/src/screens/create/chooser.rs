//! NEW GAME's first screen: a template, or a custom build from nothing.

use super::Create;
use crate::anim::effects::typewriter;
use crate::app::{AppState, Screen, ScreenCmd, TransitionKind};
use crate::ui::chrome::{CYAN, CYAN_RGB, DIM, FAINT, TEXT, solid};
use crate::ui::logo::{self, Logo};
use crate::ui::menu::Menu;
use crossterm::event::{KeyCode, KeyEvent};
use game::{Build, Step};
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Widget;

const CAPTION: &str = "NEW GAME";
const CUSTOM_LINE: &str = "start from nothing";

pub struct Chooser {
    menu: Menu,
    lines: Vec<String>,
}

impl Chooser {
    pub fn new(st: &AppState, tick: u64) -> Self {
        let mut items: Vec<String> = st
            .content
            .templates
            .iter()
            .map(|t| t.handle.clone())
            .collect();
        let mut lines: Vec<String> = st
            .content
            .templates
            .iter()
            .map(|t| t.line.clone())
            .collect();
        items.push("CUSTOM BUILD".into());
        lines.push(CUSTOM_LINE.into());
        Chooser {
            menu: Menu::new(items, tick + 10),
            lines,
        }
    }

    /// Fills the build from the selected template, or blanks it, and opens the wizard
    /// on the sheet or on the name.
    pub fn choose(&self, st: &mut AppState) -> Create {
        let i = self.menu.selected;
        let (build, step) = match st.content.templates.get(i) {
            Some(t) => (Build::from_template(&st.content, t), Step::Summary),
            None => (Build::blank(&st.content), Step::Name),
        };
        st.build = Some(build);
        Create::new(st, step, st.tick)
    }
}

impl Screen for Chooser {
    fn update(&mut self, st: &mut AppState, keys: &[KeyEvent]) -> ScreenCmd {
        for k in keys {
            match k.code {
                KeyCode::Up | KeyCode::Char('k') => self.menu.up(),
                KeyCode::Down | KeyCode::Char('j') => self.menu.down(),
                KeyCode::Esc | KeyCode::Char('q') => {
                    return ScreenCmd::Pop {
                        fx: TransitionKind::SlideUp,
                    };
                }
                KeyCode::Enter => {
                    return ScreenCmd::Push {
                        to: Box::new(self.choose(st)),
                        fx: TransitionKind::Glitch,
                    };
                }
                _ => {}
            }
        }
        ScreenCmd::Stay
    }

    fn draw(&self, st: &AppState, f: &mut Frame) {
        let area = f.area();
        let buf = f.buffer_mut();
        solid(buf, area);
        let tall = area.height >= super::TALL;
        let x = area.x + area.width / 2 - 20;
        let mut y = area.y + area.height / 5;
        if tall {
            let w = logo::width(CAPTION);
            let lx = area.x + area.width.saturating_sub(w) / 2;
            Logo {
                text: CAPTION,
                tick: st.tick,
                accent: CYAN_RGB,
            }
            .render(Rect::new(lx, y, w.min(area.width), logo::ROWS), buf);
            y += logo::ROWS + 2;
        } else {
            buf.set_string(
                x,
                y,
                CAPTION,
                Style::default().fg(CYAN).add_modifier(Modifier::BOLD),
            );
            y += 2;
        }
        buf.set_string(x, y, "who are you tonight", Style::default().fg(DIM));
        y += 2;
        for (i, item) in self.menu.items.iter().enumerate() {
            let p = self.menu.entrance(i, st.tick);
            let selected = i == self.menu.selected;
            let name = typewriter(item, p).to_string();
            let line = typewriter(&self.lines[i], p).to_string();
            let line = Line::from(vec![
                Span::styled(
                    if selected { "▸ " } else { "  " },
                    Style::default().fg(CYAN),
                ),
                Span::styled(
                    format!("{name:<14}"),
                    if selected {
                        Style::default().fg(CYAN).add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().fg(TEXT)
                    },
                ),
                Span::styled(line, Style::default().fg(DIM)),
            ]);
            buf.set_line(x, y + i as u16, &line, area.right().saturating_sub(x));
        }
        let hint = super::keys_line(&[("↑↓", "choose"), ("enter", "build"), ("esc", "back")]);
        buf.set_line(
            area.x + 2,
            area.bottom().saturating_sub(2),
            &hint,
            area.width.saturating_sub(2),
        );
        let _ = FAINT;
    }
}
