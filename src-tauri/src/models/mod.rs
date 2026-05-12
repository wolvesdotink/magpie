pub mod downloader;
pub mod error;
pub mod registry;
pub mod storage;

#[allow(unused_imports)] // Phase 1 scaffolding; consumers migrate in a later phase.
pub use error::{ModelError, Result};
