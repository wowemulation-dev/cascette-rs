//! Installation pipeline state machine.
//!
//! The install pipeline uses explicit enum states. Each transition is an async
//! function that consumes its input state and produces the next. No `Arc<Mutex<>>`
//! is needed -- each state owns its data and passes it forward.

pub mod classify;
pub mod download;
pub mod install;
pub mod install_loose;
pub mod loose;
pub mod manifests;
pub mod metadata;
pub mod root_content;
pub mod update;
