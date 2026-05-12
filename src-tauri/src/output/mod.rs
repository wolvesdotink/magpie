pub mod clipboard;
pub mod error;
pub mod paste;

#[allow(unused_imports)] // Phase 1 scaffolding; consumers migrate in a later phase.
pub use error::{OutputError, Result};
