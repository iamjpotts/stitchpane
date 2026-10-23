//! Selects the UI backend at compile time: exactly one of the `gtk` (default) or `iced`
//! features must be enabled.

use std::process::ExitCode;

#[cfg(all(feature = "gtk", feature = "iced"))]
compile_error!(
    "features `gtk` and `iced` are mutually exclusive; use `--no-default-features --features iced` for iced"
);

#[cfg(not(any(feature = "gtk", feature = "iced")))]
compile_error!("enable exactly one UI feature: `gtk` or `iced`");

#[cfg(all(feature = "gtk", not(feature = "iced")))]
pub fn run() -> ExitCode {
    stitchpane_gtk::run()
}

#[cfg(all(feature = "iced", not(feature = "gtk")))]
pub fn run() -> ExitCode {
    stitchpane_iced::run()
}
