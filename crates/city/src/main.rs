//! `city`: generate, explore and walk a cyberpunk city in the terminal.

mod anim;
mod app;
mod cli;
mod render;
mod save;
mod screens;
mod term;
mod ui;
mod walker;

fn main() -> anyhow::Result<()> {
    cli::run()
}
