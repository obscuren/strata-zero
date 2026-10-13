//! `cyberpunk`: a terminal roguelike in a multi-level cyberpunk city.

mod actions;
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
