//! The Windows app's errors where a player sees them: it has no console, so a fatal error
//! or a panic goes to a message box.

/// Shows `message` in a message box titled "Strata Zero".
pub fn show(message: &str) {
    use windows_sys::Win32::UI::WindowsAndMessaging::{MB_ICONERROR, MB_OK, MessageBoxW};
    let wide = |s: &str| s.encode_utf16().chain(Some(0)).collect::<Vec<u16>>();
    let (text, title) = (wide(message), wide("Strata Zero"));
    // SAFETY: both are NUL-terminated UTF-16 strings that outlive the call; no owner
    // window is needed.
    unsafe {
        MessageBoxW(0, text.as_ptr(), title.as_ptr(), MB_OK | MB_ICONERROR);
    }
}

/// Makes a panic show its message in the box too, then do what it did before. A city's
/// build in the background is left be: its panic is said as a warning when the build is
/// collected, and the game goes on without that city (`outside::stream::say_broke`).
pub fn catch_panics() {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        if crate::outside::stream::held_by_build(info) {
            return;
        }
        show(&format!("Strata Zero stopped.\n\n{info}"));
        previous(info);
    }));
}
