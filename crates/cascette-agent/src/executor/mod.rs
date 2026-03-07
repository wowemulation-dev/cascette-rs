//! Background operation executor with pipeline integration.
//!
//! The executor picks operations from the queue and runs them using the
//! cascette-installation pipelines. It supports configurable concurrency
//! and graceful shutdown via `CancellationToken`.

pub mod backfill;
pub mod extract;
pub mod helpers;
pub mod install;
pub mod repair;
pub mod runner;
pub mod uninstall;
pub mod update;
pub mod verify;

pub use runner::OperationRunner;
