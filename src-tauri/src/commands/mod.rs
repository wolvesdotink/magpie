//! Tauri command surface.
//!
//! Each Tauri command lives in a domain submodule; this `mod.rs` only
//! declares them and re-exports under the flat `commands::name` path so
//! `lib.rs::run`'s `invoke_handler!` macro and cross-module callers
//! (tray.rs, lib.rs) keep working unchanged.
//!
//! Add a new command → put it in the right domain file (or create a new
//! one), make sure the domain is listed below in both the `pub mod` and
//! `pub use` blocks, and register the function in `lib.rs::run`'s
//! `tauri::generate_handler!` macro.

pub mod app;
pub mod correction_models;
pub mod hotkey;
pub mod models;
pub mod permissions;
pub mod recording;
pub mod settings;
#[cfg(not(any(target_os = "android", target_os = "ios")))]
pub mod updater;
pub mod vocabulary;

pub use app::*;
pub use correction_models::*;
pub use hotkey::*;
pub use models::*;
pub use permissions::*;
pub use recording::*;
pub use settings::*;
#[cfg(not(any(target_os = "android", target_os = "ios")))]
pub use updater::*;
pub use vocabulary::*;
