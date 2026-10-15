//! TurboFix core: diagnostics, performance sampling and history storage.
//!
//! No GUI dependencies, so every measurement calculation can be unit-tested on
//! any OS. Windows-only features that lack a verified implementation are
//! reported as unavailable (see `platform::capabilities`), never simulated.

pub mod cleanup;
pub mod config;
pub mod diagnosis;
pub mod error;
pub mod monitor;
pub mod network;
pub mod optimizer;
pub mod performance;
pub mod platform;
pub mod scan;
pub mod speedtest;
pub mod stats;
pub mod storage;
pub mod wifi;

pub use error::{ApiError, CoreError, Result};
