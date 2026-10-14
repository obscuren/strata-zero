//! The generation screen: the city grows in the viewport while a stage checklist fills.

use crate::app::{AppState, Screen, ScreenCmd, TransitionKind};
use crate::render::viewport::{Camera, MapView};
use crate::render::weather::Weather;
use crate::ui::panel::boxed;
use citygen::rng::hash64;
use citygen::{GenParams, Generator, Pos, Progress, Stage, StratumKind};
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Widget};

const PANEL_W: u16 = 36;
const BAR_W: usize = 16;

pub struct Generating {
    generator: Option<Generator>,
    params: GenParams,
    pub steps_per_frame: u32,
    pub paused: bool,
    current: Option<(Stage, f32)>,
    done: Vec<Stage>,
    cam: Camera,
    error: Option<String>,
    started: u64,
}

/// Short stage names for the checklist.
fn short(stage: Stage) -> &'static str {
    match stage {
        Stage::Terrain => "Terrain",
        Stage::Districts => "Districts",
        Stage::Arterials => "Arterials",
        Stage::Streets => "Streets",
        Stage::Lots => "Buildings",
        Stage::Verticals => "Strata",
        Stage::Interiors => "Interiors",
        Stage::Connectivity => "Links",
        Stage::Dressing => "Neon",
        Stage::Naming => "Names",
        Stage::Net => "Net",
        Stage::Cyberspace => "Cyber",
    }
}

impl Generating {
    pub fn new(params: GenParams, tick: u64) -> anyhow::Result<Self> {
        let generator = Generator::new(params.clone())?;
        let mut cam = Camera::default();
        cam.snap(generator.city().size.center());
        Ok(Generating {
            generator: Some(generator),
            params,
            steps_per_frame: 8,
            paused: false,
            current: None,
            done: Vec::new(),
            cam,
            error: None,
            started: tick,
        })
    }

    /// Something that changes whenever generation advances; used by tests.
    #[cfg(test)]
    pub fn progress_marker(&self) -> (usize, Option<(Stage, u32)>) {
        (
            self.done.len(),
            self.current.map(|(s, p)| (s, (p * 1000.0) as u32)),
        )
    }

    fn step_once(&mut self) -> bool {
        let Some(generator) = self.generator.as_mut() else {
            return false;
        };
        match generator.step() {
            Progress::Running { stage, pct } => {
                if let Some((prev, _)) = self.current
                    && prev != stage
                    && !self.done.contains(&prev)
                {
                    self.done.push(prev);
                }
                self.current = Some((stage, pct));
                true
            }
            Progress::Done => {
                if let Some((prev, _)) = self.current
                    && !self.done.contains(&prev)
                {
                    self.done.push(prev);
                }
                false
            }
        }
    }

    fn finish(&mut self, st: &mut AppState) -> ScreenCmd {
        let Some(generator) = self.generator.take() else {
            return ScreenCmd::Stay;
        };
        match generator.finish() {
            Ok(city) => {
                st.params = self.params.clone();
                st.enter_city(city);
                ScreenCmd::Switch {
                    to: crate::screens::explore_screen(st.tick),
                    fx: TransitionKind::Glitch,
                }
            }
            Err(e) => {
                self.error = Some(e.to_string());
                ScreenCmd::Stay
            }
        }
    }
}

impl Screen for Generating {
    fn update(&mut self, st: &mut AppState, keys: &[KeyEvent]) -> ScreenCmd {
        let mut skip = false;
        for k in keys {
            match k.code {
                KeyCode::Char(' ') => self.paused = !self.paused,
                KeyCode::Char('+') | KeyCode::Char('=') => {
                    self.steps_per_frame = (self.steps_per_frame * 2).min(64)
                }
                KeyCode::Char('-') => self.steps_per_frame = (self.steps_per_frame / 2).max(1),
                KeyCode::Enter => skip = true,
                KeyCode::Char('r') => {
                    let seed = hash64(self.params.seed, st.tick);
                    match Generating::new(self.params.clone().with_seed(seed), st.tick) {
                        Ok(next) => {
                            return ScreenCmd::Switch {
                                to: Box::new(next),
                                fx: TransitionKind::Glitch,
                            };
                        }
                        Err(e) => st.warn(e.to_string()),
                    }
                }
                KeyCode::Esc => {
                    return ScreenCmd::Switch {
                        to: Box::new(crate::screens::configure::Configure::new(
                            self.params.clone(),
                            st.tick,
                        )),
                        fx: TransitionKind::SlideUp,
                    };
                }
                _ => {}
            }
        }
        if self.error.is_some() || self.generator.is_none() {
            return ScreenCmd::Stay;
        }
        let mut running = true;
        if skip {
            while running {
                running = self.step_once();
            }
        } else if !self.paused {
            for _ in 0..self.steps_per_frame {
                running = self.step_once();
                if !running {
                    break;
                }
            }
        }
        if !running {
            return self.finish(st);
        }
        ScreenCmd::Stay
    }

    fn draw(&self, st: &AppState, f: &mut Frame) {
        let area = f.area();
        let cols = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Min(10), Constraint::Length(PANEL_W)])
            .split(area);
        let map_area = cols[0];
        let panel = cols[1];

        if let Some(generator) = &self.generator {
            let city = generator.city();
            let mut cam = self.cam;
            let drift = ((st.tick as f32) / 90.0).sin() * city.size.w as f32 / 6.0;
            cam.target(Pos::new(
                city.size.center().x + drift as i32,
                city.size.center().y,
            ));
            cam.clamp(city.size, map_area.width, map_area.height);
            cam.x += (cam.tx as f32 - cam.x) * 0.1;
            cam.y += (cam.ty as f32 - cam.y) * 0.1;
            MapView {
                city,
                stratum: StratumKind::Street,
                origin: cam.origin(map_area.width, map_area.height),
                tick: st.tick,
                style: &st.style,
                vis: None,
                overlays: &[&Weather],
                cursor: None,
            }
            .render(map_area, f.buffer_mut());
        }

        let title = format!("GENERATING  seed {}", self.params.seed);
        let block = boxed(&title);
        let inner = block.inner(panel);
        f.render_widget(block, panel);
        let mut lines: Vec<Line> = Vec::new();
        lines.push(Line::from(Span::styled(
            format!(
                " {}x{}  {} districts",
                self.params.size.size().w,
                self.params.size.size().h,
                self.params.districts
            ),
            Style::default().fg(Color::Rgb(160, 170, 185)),
        )));
        lines.push(Line::from(""));
        for stage in Stage::ALL {
            let done = self.done.contains(&stage);
            let active = self.current.map(|(s, _)| s) == Some(stage) && !done;
            let line = if done {
                Line::from(Span::styled(
                    format!(" [✓] {:<10}", short(stage)),
                    Style::default().fg(Color::Rgb(90, 160, 120)),
                ))
            } else if active {
                let pct = self.current.map(|(_, p)| p).unwrap_or(0.0);
                let filled = (pct * BAR_W as f32).round() as usize;
                let bar: String = (0..BAR_W)
                    .map(|i| if i < filled { '▓' } else { '░' })
                    .collect();
                Line::from(vec![
                    Span::styled(
                        format!(" [▸] {:<10} ", short(stage)),
                        Style::default()
                            .fg(Color::Rgb(0, 229, 255))
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(bar, Style::default().fg(Color::Rgb(0, 200, 230))),
                    Span::styled(
                        format!(" {:>3.0}%", pct * 100.0),
                        Style::default().fg(Color::Rgb(180, 190, 200)),
                    ),
                ])
            } else {
                Line::from(Span::styled(
                    format!(" [ ] {:<10}", short(stage)),
                    Style::default().fg(Color::Rgb(80, 88, 100)),
                ))
            };
            lines.push(line);
        }
        lines.push(Line::from(""));
        if let Some((stage, _)) = self.current {
            lines.push(Line::from(Span::styled(
                format!(" {}…", stage.label()),
                Style::default().fg(Color::Rgb(160, 170, 185)),
            )));
        }
        if let Some(e) = &self.error {
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                format!(" {e}"),
                Style::default().fg(Color::Rgb(255, 80, 100)),
            )));
            lines.push(Line::from(Span::styled(
                " r reseed · esc back",
                Style::default().fg(Color::Rgb(160, 170, 185)),
            )));
        } else {
            let state = if self.paused { "paused" } else { "running" };
            lines.push(Line::from(Span::styled(
                format!(" {state} · x{} per frame", self.steps_per_frame),
                Style::default().fg(Color::Rgb(160, 170, 185)),
            )));
        }
        let hint_y = inner.bottom().saturating_sub(1);
        f.render_widget(Paragraph::new(lines), inner);
        if hint_y > inner.y {
            f.render_widget(
                Paragraph::new(Span::styled(
                    " space pause · +/- speed · enter skip",
                    Style::default().fg(Color::Rgb(100, 110, 125)),
                )),
                Rect::new(inner.x, hint_y, inner.width, 1),
            );
        }
        let _ = self.started;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::{AppState, Screen, ScreenCmd};
    use crate::screens::testutil::{buffer_text, key};
    use citygen::{GenParams, SizePreset};
    use crossterm::event::KeyCode;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    fn params() -> GenParams {
        GenParams {
            seed: 2,
            size: SizePreset::Custom { w: 80, h: 40 },
            districts: 4,
            ..Default::default()
        }
    }

    #[test]
    fn generating_reaches_explore() {
        let mut st = AppState::new();
        let mut g = Generating::new(params(), 0).unwrap();
        let mut t = Terminal::new(TestBackend::new(100, 30)).unwrap();
        t.draw(|f| g.draw(&st, f)).unwrap();
        let text = buffer_text(t.backend().buffer());
        assert!(text.contains("Terrain") && text.contains("Net"), "{text}");
        let mut cmd = ScreenCmd::Stay;
        for _ in 0..100_000 {
            st.tick += 1;
            cmd = g.update(&mut st, &[key(KeyCode::Enter)]);
            if !matches!(cmd, ScreenCmd::Stay) {
                break;
            }
        }
        assert!(matches!(cmd, ScreenCmd::Switch { .. }));
        assert!(st.city.is_some());
        assert_eq!(st.params.seed, 2);
    }

    #[test]
    fn generating_pauses_and_changes_speed() {
        let mut st = AppState::new();
        let mut g = Generating::new(params(), 0).unwrap();
        g.update(&mut st, &[key(KeyCode::Char(' '))]);
        assert!(g.paused);
        let before = g.progress_marker();
        for _ in 0..5 {
            st.tick += 1;
            g.update(&mut st, &[]);
        }
        assert_eq!(g.progress_marker(), before);
        g.update(&mut st, &[key(KeyCode::Char(' '))]);
        g.update(&mut st, &[key(KeyCode::Char('+'))]);
        assert_eq!(g.steps_per_frame, 16);
        for _ in 0..10 {
            g.update(&mut st, &[key(KeyCode::Char('-'))]);
        }
        assert_eq!(g.steps_per_frame, 1);
        assert!(matches!(
            g.update(&mut st, &[key(KeyCode::Esc)]),
            ScreenCmd::Switch { .. }
        ));
        let mut t = Terminal::new(TestBackend::new(100, 30)).unwrap();
        t.draw(|f| g.draw(&st, f)).unwrap();
    }
}
