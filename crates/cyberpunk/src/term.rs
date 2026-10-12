//! Raw-mode terminal setup that always restores the shell, even on panic.

use anyhow::Result;
use crossterm::{cursor, execute, terminal};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use std::io::{Stdout, stdout};

/// Restores the terminal when dropped.
pub struct TerminalGuard;

impl TerminalGuard {
    pub fn enter() -> Result<(Self, Terminal<CrosstermBackend<Stdout>>)> {
        terminal::enable_raw_mode()?;
        execute!(stdout(), terminal::EnterAlternateScreen, cursor::Hide)?;
        let term = Terminal::new(CrosstermBackend::new(stdout()))?;
        Ok((TerminalGuard, term))
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        restore();
    }
}

/// Leaves raw mode and the alternate screen. Safe to call more than once.
pub fn restore() {
    let _ = terminal::disable_raw_mode();
    let _ = execute!(stdout(), terminal::LeaveAlternateScreen, cursor::Show);
}

/// Makes a panic restore the terminal before printing its message.
pub fn install_panic_hook() {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        restore();
        previous(info);
    }));
}
