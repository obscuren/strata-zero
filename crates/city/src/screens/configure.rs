//! The configure screen: a form over `GenParams` with a live preview card.

use crate::app::{AppState, Screen, ScreenCmd};
use citygen::GenParams;
use crossterm::event::KeyEvent;
use ratatui::Frame;

pub struct Configure {
    params: GenParams,
    opened: u64,
}

impl Configure {
    pub fn new(params: GenParams, tick: u64) -> Self {
        Configure {
            params,
            opened: tick,
        }
    }
}

impl Screen for Configure {
    fn update(&mut self, _st: &mut AppState, _keys: &[KeyEvent]) -> ScreenCmd {
        ScreenCmd::Stay
    }
    fn draw(&self, _st: &AppState, _f: &mut Frame) {}
}
