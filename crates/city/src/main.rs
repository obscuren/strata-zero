//! `city`: generate, explore and walk a cyberpunk city in the terminal.

// Screens, widgets and overlays land task by task; lifted once the app is wired up.
#![allow(dead_code)]

mod anim;
mod app;
mod cli;
mod save;
mod screens;
mod term;

fn main() -> anyhow::Result<()> {
    cli::run()
}
