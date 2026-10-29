//! The game in its own window instead of a terminal (`strata --window`, the `window`
//! feature): a ratatui backend that paints the cell grid into pixels, driven by a winit
//! loop that runs the same `Runner` frame as the terminal does.

pub mod backend;
pub mod font;
pub mod keys;
pub mod png;
pub mod raster;
pub mod shapes;

#[cfg(test)]
mod tests {
    use crate::app::{AppState, Screen};

    /// The title screen as the window paints it, to `SHOT_DIR/title.png`. The street, the
    /// deck and the ATTRIBUTES sheet come out of the shot harness (`screens::shots`) as
    /// PNGs too when this feature is on.
    #[test]
    #[ignore]
    fn window_shots() {
        let Ok(dir) = std::env::var("SHOT_DIR") else {
            return;
        };
        let mut st = AppState::new();
        let mut title = crate::screens::initial();
        for _ in 0..90 {
            st.tick += 1;
            title.update(&mut st, &[]);
        }
        let buf = crate::screens::shots::draw_stack(&st, &[&title], (120, 40));
        super::png::write_buffer(&dir, "title", &buf).unwrap();
    }
}
