//! Which desktop the window runs on. Fonts, chords and the app folder are answers on a
//! `Platform`, so every platform's answer is tested on any machine; only the code that
//! calls the operating system is behind `cfg`.

use std::ffi::OsString;
use std::path::PathBuf;

/// Reads an environment variable: the process's (`process_env`), or a test's own.
pub type Env<'a> = &'a dyn Fn(&str) -> Option<OsString>;

/// The process's environment, as an `Env`.
pub fn process_env(key: &str) -> Option<OsString> {
    std::env::var_os(key)
}

/// An environment of exactly `vars`, for tests.
#[cfg(test)]
pub fn env_of(vars: &[(&str, &str)]) -> impl Fn(&str) -> Option<OsString> + use<> {
    let vars: Vec<(String, OsString)> = vars
        .iter()
        .map(|(k, v)| (k.to_string(), OsString::from(v)))
        .collect();
    move |key| vars.iter().find(|(k, _)| k == key).map(|(_, v)| v.clone())
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

    /// The folder the desktop app keeps its files in (and so `cities/`), from `env`;
    /// `None` when the variables it is found from aren't set.
    /// - Mac: `~/Library/Application Support/Strata Zero`
    /// - Windows: `%APPDATA%\Strata Zero`, else `%USERPROFILE%\AppData\Roaming\Strata Zero`
    /// - Linux: `$XDG_DATA_HOME/strata-zero`, else `~/.local/share/strata-zero`
    pub fn app_dir(self, env: Env) -> Option<PathBuf> {
        let from = |key: &str| set(env, key).map(PathBuf::from);
        Some(match self {
            Platform::Mac => from("HOME")?.join("Library/Application Support/Strata Zero"),
            Platform::Windows => from("APPDATA")
                .or_else(|| Some(from("USERPROFILE")?.join("AppData").join("Roaming")))?
                .join("Strata Zero"),
            Platform::Linux => from("XDG_DATA_HOME")
                .or_else(|| Some(from("HOME")?.join(".local/share")))?
                .join("strata-zero"),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn mac_app_dir() {
        let dir = Platform::Mac.app_dir(&env_of(&[("HOME", "/Users/x")]));
        let want = Path::new("/Users/x").join("Library/Application Support/Strata Zero");
        assert_eq!(dir, Some(want));
        assert_eq!(Platform::Mac.app_dir(&env_of(&[])), None);
    }

    #[test]
    fn windows_app_dir() {
        let roaming = "C:\\Users\\x\\AppData\\Roaming";
        let dir = Platform::Windows.app_dir(&env_of(&[("APPDATA", roaming)]));
        assert_eq!(dir, Some(Path::new(roaming).join("Strata Zero")));

        let dir = Platform::Windows.app_dir(&env_of(&[("USERPROFILE", "C:\\Users\\x")]));
        let want = Path::new("C:\\Users\\x").join("AppData").join("Roaming");
        assert_eq!(dir, Some(want.join("Strata Zero")));

        assert_eq!(Platform::Windows.app_dir(&env_of(&[("HOME", "/h")])), None);
    }

    #[test]
    fn linux_app_dir() {
        let dir = Platform::Linux.app_dir(&env_of(&[("XDG_DATA_HOME", "/d"), ("HOME", "/h")]));
        assert_eq!(dir, Some(PathBuf::from("/d/strata-zero")));
        for xdg in [
            &[("XDG_DATA_HOME", ""), ("HOME", "/h")][..],
            &[("HOME", "/h")],
        ] {
            let dir = Platform::Linux.app_dir(&env_of(xdg));
            assert_eq!(dir, Some(PathBuf::from("/h/.local/share/strata-zero")));
        }
        assert_eq!(Platform::Linux.app_dir(&env_of(&[])), None);
    }

    #[test]
    fn app_dir_keeps_non_ascii() {
        let roaming = "C:\\Users\\Jürgen\\AppData\\Roaming";
        let dir = Platform::Windows.app_dir(&env_of(&[("APPDATA", roaming)]));
        assert_eq!(dir, Some(Path::new(roaming).join("Strata Zero")));
    }
}
