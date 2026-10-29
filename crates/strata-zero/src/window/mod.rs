//! The game in its own window instead of a terminal (`strata --window`, the `window`
//! feature): a ratatui backend that paints the cell grid into pixels, driven by a winit
//! loop that runs the same `Runner` frame as the terminal does.

pub mod keys;
