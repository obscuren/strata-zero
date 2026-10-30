//! Which desktop the window runs on. Fonts, chords and the app folder are answers on a
//! `Platform`, so every platform's answer is tested on any machine; only the code that
//! calls the operating system is behind `cfg`.

use std::ffi::OsString;

/// Reads an environment variable: the process's (`process_env`), or a test's own.
pub type Env<'a> = &'a dyn Fn(&str) -> Option<OsString>;

/// The process's environment, as an `Env`.
pub fn process_env(key: &str) -> Option<OsString> {
    std::env::var_os(key)
}

/// An environment variable that is set and not empty.
pub fn set(env: Env, key: &str) -> Option<OsString> {
    env(key).filter(|v| !v.is_empty())
}

/// A desktop the window runs on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Platform {
    Mac,
    Windows,
    /// Linux, and any other Unix with X11 or Wayland.
    Linux,
}

impl Platform {
    /// The platform this binary was built for.
    pub const NATIVE: Platform = if cfg!(target_os = "macos") {
        Platform::Mac
    } else if cfg!(windows) {
        Platform::Windows
    } else {
        Platform::Linux
    };
}
