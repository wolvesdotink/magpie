pub mod capture;
pub mod error;
pub mod resample;
pub mod ring_buffer;

#[allow(unused_imports)] // Phase 1 scaffolding; consumers migrate in a later phase.
pub use error::{AudioError, Result};
pub use ring_buffer::AudioRingBuffer;
