pub mod engine;
pub mod error;
pub mod registry;

#[allow(unused_imports)] // Phase 1 scaffolding; consumers migrate in a later phase.
pub use error::{CorrectionError, Result};
