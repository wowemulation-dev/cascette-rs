//! Domain models: Operation, Product, and Progress state machines.

pub mod operation;
pub mod product;
pub mod progress;

pub use operation::{Operation, OperationState, OperationType, Priority};
pub use product::{InstallationMode, Product, ProductStatus};
pub use progress::Progress;
