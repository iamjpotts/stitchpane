//! UI-independent model and image operations. Both UI crates depend on this crate; it
//! depends on no UI toolkit.

pub mod codec;
pub mod editor;
pub mod geometry;
pub mod metadata;
pub mod model;
pub mod project;
pub mod project_file;
pub mod render;
pub mod retouch;

/// Human-readable application name.
pub const APP_NAME: &str = "Imaginer";

/// Application identifier used by desktop integration (GTK application id, etc.).
pub const APP_ID: &str = "app.imaginer.Imaginer";
