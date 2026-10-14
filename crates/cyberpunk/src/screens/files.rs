//! Load and save screens over the `./cities` directory.

use crate::app::{AppState, Screen, ScreenCmd, TransitionKind};
use crate::render::Rgb;
use crate::save::{CityFileInfo, default_dir, list_cities, load_city, save_city};
use crate::ui::form::{Field, Form, FormView};
use crate::ui::panel::boxed;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Widget};
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

enum Mode {
    Load,
    Save(Form),
}

pub struct Files {
    dir: PathBuf,
    mode: Mode,
    entries: Vec<CityFileInfo>,
    selected: usize,
    error: Option<String>,
}

/// `YYYY-MM-DD HH:MM` in UTC, without a date crate (civil-from-days algorithm).
fn fmt_time(t: SystemTime) -> String {
    let secs = t
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0) as i64;
    let days = secs.div_euclid(86_400);
    let rem = secs.rem_euclid(86_400);
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!(
        "{y:04}-{m:02}-{d:02} {:02}:{:02}",
        rem / 3600,
        (rem % 3600) / 60
    )
}

impl Files {
    pub fn load(_tick: u64) -> Self {
        Self::load_in(default_dir(), _tick)
    }

    pub fn save(tick: u64, name: &str) -> Self {
        Self::save_in(default_dir(), tick, name)
    }

    pub fn load_in(dir: PathBuf, _tick: u64) -> Self {
        let entries = list_cities(&dir).unwrap_or_default();
        Files {
            dir,
            mode: Mode::Load,
            entries,
            selected: 0,
            error: None,
        }
    }

    pub fn save_in(dir: PathBuf, _tick: u64, name: &str) -> Self {
        let form = Form {
            fields: vec![Field::Text {
                label: "Name".into(),
                value: name.to_string(),
                max: 40,
                numeric: false,
            }],
            selected: 0,
        };
        Files {
            dir,
            mode: Mode::Save(form),
            entries: Vec::new(),
            selected: 0,
            error: None,
        }
    }

    /// Back to whatever pushed this screen.
    fn done() -> ScreenCmd {
        ScreenCmd::Pop {
            fx: TransitionKind::None,
        }
    }
}

impl Screen for Files {
    fn update(&mut self, st: &mut AppState, keys: &[KeyEvent]) -> ScreenCmd {
        for k in keys {
            match &mut self.mode {
                Mode::Load => match k.code {
                    KeyCode::Up | KeyCode::Char('k') => {
                        self.selected = self.selected.saturating_sub(1)
                    }
                    KeyCode::Down | KeyCode::Char('j') => {
                        if self.selected + 1 < self.entries.len() {
                            self.selected += 1;
                        }
                    }
                    KeyCode::Esc | KeyCode::Char('q') => return Self::done(),
                    KeyCode::Enter => {
                        let Some(entry) = self.entries.get(self.selected) else {
                            continue;
                        };
                        match load_city(&entry.path) {
                            Ok(city) => {
                                st.params = city.params.clone();
                                st.enter_city(city);
                                st.say(format!("loaded {}", entry.path.display()));
                                return ScreenCmd::Switch {
                                    to: crate::screens::explore_screen(st.tick),
                                    fx: TransitionKind::Glitch,
                                };
                            }
                            Err(e) => self.error = Some(e.to_string()),
                        }
                    }
                    _ => {}
                },
                Mode::Save(form) => {
                    if k.code == KeyCode::Esc {
                        return Self::done();
                    }
                    if k.code == KeyCode::Enter {
                        let name = form.text("Name").unwrap_or("").trim().to_string();
                        let name = if name.is_empty() {
                            "city".to_string()
                        } else {
                            name
                        };
                        let Some(city) = st.city.as_ref() else {
                            return Self::done();
                        };
                        let path = self.dir.join(format!("{name}.city"));
                        match save_city(&path, city) {
                            Ok(()) => {
                                st.say(format!("saved {}", path.display()));
                                return Self::done();
                            }
                            Err(e) => self.error = Some(e.to_string()),
                        }
                        continue;
                    }
                    form.handle(k.code);
                }
            }
        }
        ScreenCmd::Stay
    }

    fn draw(&self, st: &AppState, f: &mut Frame) {
        let area = f.area();
        let w = area.width.min(80);
        let h = area.height.min(24);
        let panel = Rect::new(
            area.x + (area.width - w) / 2,
            area.y + (area.height - h) / 2,
            w,
            h,
        );
        let dim = Style::default().fg(Color::Rgb(160, 170, 185));
        match &self.mode {
            Mode::Load => {
                let block = boxed("LOAD CITY");
                let inner = block.inner(panel);
                f.render_widget(block, panel);
                let mut lines: Vec<Line> = Vec::new();
                if self.entries.is_empty() {
                    lines.push(Line::from(Span::styled(
                        format!(" no saved cities in {}", self.dir.display()),
                        dim,
                    )));
                }
                for (i, e) in self.entries.iter().enumerate() {
                    let sel = i == self.selected;
                    let style = if sel {
                        Style::default()
                            .fg(Color::Rgb(0, 255, 230))
                            .add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().fg(Color::Rgb(205, 212, 222))
                    };
                    lines.push(Line::from(Span::styled(
                        format!(
                            "{} {:<24} seed {:<12} {}x{:<5} {}",
                            if sel { "▸" } else { " " },
                            e.name,
                            e.seed,
                            e.size.w,
                            e.size.h,
                            fmt_time(e.modified)
                        ),
                        style,
                    )));
                }
                lines.push(Line::from(""));
                lines.push(Line::from(Span::styled(
                    " ↑↓ select · enter load · esc back",
                    dim,
                )));
                if let Some(e) = &self.error {
                    lines.push(Line::from(Span::styled(
                        format!(" {e}"),
                        Style::default().fg(Color::Rgb(255, 80, 100)),
                    )));
                }
                f.render_widget(Paragraph::new(lines), inner);
            }
            Mode::Save(form) => {
                let block = boxed("SAVE CITY");
                let inner = block.inner(panel);
                f.render_widget(block, panel);
                FormView {
                    form,
                    tick: st.tick,
                    accent: Rgb(0, 255, 230),
                }
                .render(
                    Rect::new(inner.x + 1, inner.y + 1, inner.width.saturating_sub(2), 1),
                    f.buffer_mut(),
                );
                let mut lines = vec![
                    Line::from(Span::styled(
                        format!(" saves to {}/<name>.city", self.dir.display()),
                        dim,
                    )),
                    Line::from(Span::styled(" enter save · esc cancel", dim)),
                ];
                if let Some(e) = &self.error {
                    lines.push(Line::from(Span::styled(
                        format!(" {e}"),
                        Style::default().fg(Color::Rgb(255, 80, 100)),
                    )));
                }
                f.render_widget(
                    Paragraph::new(lines),
                    Rect::new(
                        inner.x,
                        inner.y + 3,
                        inner.width,
                        inner.height.saturating_sub(3),
                    ),
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::{AppState, Screen, ScreenCmd};
    use crate::screens::testutil::{buffer_text, key};
    use citygen::{GenParams, Generator, SizePreset};
    use crossterm::event::KeyCode;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    fn tmp(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("city-files-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn save_then_load_round_trip_through_screens() {
        let dir = tmp("rt");
        let mut st = AppState::new();
        st.city = Some(
            Generator::run(GenParams {
                seed: 9,
                size: SizePreset::Custom { w: 80, h: 40 },
                districts: 4,
                ..Default::default()
            })
            .unwrap(),
        );
        let name = st.city.as_ref().unwrap().name.clone();
        let mut save = Files::save_in(dir.clone(), 0, &name);
        let mut t = Terminal::new(TestBackend::new(100, 30)).unwrap();
        t.draw(|f| save.draw(&st, f)).unwrap();
        assert!(buffer_text(t.backend().buffer()).contains("SAVE"));
        assert!(matches!(
            save.update(&mut st, &[key(KeyCode::Enter)]),
            ScreenCmd::Pop { .. }
        ));
        assert!(dir.join(format!("{name}.city")).exists());

        st.city = None;
        let mut load = Files::load_in(dir.clone(), 0);
        t.draw(|f| load.draw(&st, f)).unwrap();
        let text = buffer_text(t.backend().buffer());
        assert!(text.contains(&name) && text.contains("seed 9"), "{text}");
        assert!(matches!(
            load.update(&mut st, &[key(KeyCode::Enter)]),
            ScreenCmd::Switch { .. }
        ));
        assert_eq!(st.city.as_ref().map(|c| c.seed), Some(9));
        // A second save under another name pops back to whatever pushed it.
        let mut save = Files::save_in(dir.clone(), 0, "again");
        assert!(matches!(
            save.update(&mut st, &[key(KeyCode::Enter)]),
            ScreenCmd::Pop { .. }
        ));
        assert!(dir.join("again.city").exists());
        assert!(matches!(
            save.update(&mut st, &[key(KeyCode::Esc)]),
            ScreenCmd::Pop { .. }
        ));

        let mut empty = Files::load_in(tmp("empty"), 0);
        t.draw(|f| empty.draw(&st, f)).unwrap();
        assert!(buffer_text(t.backend().buffer()).contains("no saved cities"));
        assert!(matches!(
            empty.update(&mut st, &[key(KeyCode::Enter)]),
            ScreenCmd::Stay
        ));
        assert!(matches!(
            empty.update(&mut st, &[key(KeyCode::Esc)]),
            ScreenCmd::Pop { .. }
        ));
    }
}
