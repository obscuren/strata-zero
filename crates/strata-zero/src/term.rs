//! Raw-mode terminal setup that always restores the shell, even on panic. The mouse is
//! captured so a drag over the game does not select text; its events are dropped until
//! the game learns to use them.

use anyhow::Result;
use crossterm::event::{DisableMouseCapture, EnableMouseCapture};
use crossterm::{cursor, execute, terminal};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use std::io::{Stdout, stdout};

/// Restores the terminal when dropped.
pub struct TerminalGuard;

impl TerminalGuard {
    pub fn enter() -> Result<(Self, Terminal<CrosstermBackend<Stdout>>)> {
        terminal::enable_raw_mode()?;
        execute!(
            stdout(),
            terminal::EnterAlternateScreen,
            EnableMouseCapture,
            cursor::Hide
        )?;
        let term = Terminal::new(CrosstermBackend::new(stdout()))?;
        Ok((TerminalGuard, term))
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        restore();
    }
}

/// Releases the mouse, leaves raw mode and the alternate screen. Safe to call more
/// than once.
pub fn restore() {
    let _ = terminal::disable_raw_mode();
    let _ = execute!(
        stdout(),
        DisableMouseCapture,
        terminal::LeaveAlternateScreen,
        cursor::Show
    );
}

/// Makes a panic restore the terminal before printing its message. A city's build in
/// the background is left be: its panic is said as a warning when the build is
/// collected, and the game goes on without that city (`outside::stream::say_broke`).
pub fn install_panic_hook() {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        if crate::outside::stream::held_by_build(info) {
            return;
        }
        restore();
        previous(info);
    }));
}
