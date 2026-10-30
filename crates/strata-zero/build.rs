//! The Windows app's resources: its icon, and the name Explorer and Task Manager show.
//! Only an `app` build for Windows has any. `STRATA_NO_WINDOWS_RESOURCES` leaves them
//! out, for checking that build from a machine with no Windows resource compiler.

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    #[cfg(feature = "app")]
    windows_resources();
}

#[cfg(feature = "app")]
fn windows_resources() {
    println!("cargo:rerun-if-changed=windows/AppIcon.ico");
    println!("cargo:rerun-if-env-changed=STRATA_NO_WINDOWS_RESOURCES");
    let windows = std::env::var("CARGO_CFG_TARGET_OS").is_ok_and(|os| os == "windows");
    if !windows || std::env::var_os("STRATA_NO_WINDOWS_RESOURCES").is_some() {
        return;
    }
    let mut res = winresource::WindowsResource::new();
    // Ordinal 1: the window loads it from there (`window::open`).
    res.set_icon_with_id("windows/AppIcon.ico", "1")
        .set("ProductName", "Strata Zero")
        .set("FileDescription", "Strata Zero");
    res.compile().expect("compiling the Windows resources");
}
