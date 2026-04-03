//! Operation state machine.
//!
//! Operations represent background tasks (install, update, repair, verify,
//! uninstall) that progress through a well-defined state machine.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::progress::Progress;

/// Operation type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OperationType {
    /// Fresh product installation.
    Install,
    /// Update existing installation.
    Update,
    /// Repair corrupted files.
    Repair,
    /// Verify installation integrity.
    Verify,
    /// Remove product installation.
    Uninstall,
    /// Background content fill.
    Backfill,
    /// Extract CASC content to a directory tree.
    Extract,
}

impl std::fmt::Display for OperationType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Install => write!(f, "install"),
            Self::Update => write!(f, "update"),
            Self::Repair => write!(f, "repair"),
            Self::Verify => write!(f, "verify"),
            Self::Uninstall => write!(f, "uninstall"),
            Self::Backfill => write!(f, "backfill"),
            Self::Extract => write!(f, "extract"),
        }
    }
}

/// Operation state in the state machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OperationState {
    /// Waiting in queue to begin.
    Queued,
    /// Preparing resources and resolving metadata.
    Initializing,
    /// Downloading content from CDN.
    Downloading,
    /// Verifying file integrity.
    Verifying,
    /// Finished.
    Complete,
    /// Encountered a fatal error.
    Failed,
    /// Cancelled by user or system.
    Cancelled,
}

impl OperationState {
    /// Check whether the transition from `self` to `target` is valid.
    #[must_use]
    pub fn can_transition_to(self, target: Self) -> bool {
        matches!(
            (self, target),
            // Forward progression
            (Self::Queued, Self::Initializing | Self::Cancelled)
                | (Self::Initializing | Self::Verifying, Self::Downloading)
                | (
                    Self::Downloading,
                    Self::Verifying | Self::Queued | Self::Cancelled | Self::Failed
                )
                | (
                    Self::Verifying,
                    Self::Complete | Self::Queued | Self::Cancelled | Self::Failed
                )
                | (
                    Self::Initializing,
                    Self::Queued | Self::Cancelled | Self::Failed
                )
        )
    }

    /// Whether this state is terminal (no further transitions possible).
    #[must_use]
    pub fn is_terminal(self) -> bool {
        matches!(self, Self::Complete | Self::Failed | Self::Cancelled)
    }

    /// Whether this state represents an active (in-progress) operation.
    #[must_use]
    pub fn is_active(self) -> bool {
        matches!(
            self,
            Self::Queued | Self::Initializing | Self::Downloading | Self::Verifying
        )
    }
}

impl std::fmt::Display for OperationState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Queued => write!(f, "queued"),
            Self::Initializing => write!(f, "initializing"),
            Self::Downloading => write!(f, "downloading"),
            Self::Verifying => write!(f, "verifying"),
            Self::Complete => write!(f, "complete"),
            Self::Failed => write!(f, "failed"),
            Self::Cancelled => write!(f, "cancelled"),
        }
    }
}

/// Priority for operation scheduling.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[derive(Default)]
pub enum Priority {
    /// Background operation.
    Low,
    /// Standard priority.
    #[default]
    Normal,
    /// User-initiated, needs fast execution.
    High,
}

impl Priority {
    /// Convert from the numeric priority used in the real agent API.
    /// The real agent uses 700 as normal priority.
    #[must_use]
    pub fn from_agent_priority(value: u32) -> Self {
        if value >= 900 {
            Self::High
        } else if value >= 500 {
            Self::Normal
        } else {
            Self::Low
        }
    }
}

impl std::fmt::Display for Priority {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Low => write!(f, "low"),
            Self::Normal => write!(f, "normal"),
            Self::High => write!(f, "high"),
        }
    }
}

/// Error details stored with a failed operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorInfo {
    /// Machine-readable error code.
    pub code: String,
    /// Human-readable error message.
    pub message: String,
    /// Additional context (serialized as JSON).
    pub details: Option<serde_json::Value>,
}

/// A background operation tracking install/update/repair/verify/uninstall.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Operation {
    /// Unique operation identifier.
    pub operation_id: Uuid,
    /// Target product code.
    pub product_code: String,
    /// Type of operation.
    pub operation_type: OperationType,
    /// Current state.
    pub state: OperationState,
    /// Scheduling priority.
    pub priority: Priority,
    /// Operation-specific parameters (JSON).
    pub parameters: Option<serde_json::Value>,
    /// Operation metadata (verification results, stats, etc.).
    pub metadata: Option<serde_json::Value>,
    /// Current progress metrics.
    pub progress: Option<Progress>,
    /// Error details if failed.
    pub error: Option<ErrorInfo>,
    /// When the operation was created.
    pub created_at: DateTime<Utc>,
    /// Last state or progress update.
    pub updated_at: DateTime<Utc>,
    /// When execution started.
    pub started_at: Option<DateTime<Utc>>,
    /// When the operation finished (complete, failed, or cancelled).
    pub completed_at: Option<DateTime<Utc>>,
}

impl Operation {
    /// Create a new operation in the Queued state.
    #[must_use]
    pub fn new(
        product_code: String,
        operation_type: OperationType,
        priority: Priority,
        parameters: Option<serde_json::Value>,
    ) -> Self {
        let now = Utc::now();
        Self {
            operation_id: Uuid::new_v4(),
            product_code,
            operation_type,
            state: OperationState::Queued,
            priority,
            parameters,
            metadata: None,
            progress: None,
            error: None,
            created_at: now,
            updated_at: now,
            started_at: None,
            completed_at: None,
        }
    }

    /// Attempt to transition to a new state.
    ///
    /// # Errors
    ///
    /// Returns `AgentError::InvalidTransition` if the transition is not valid.
    pub fn transition_to(
        &mut self,
        new_state: OperationState,
    ) -> Result<(), crate::error::AgentError> {
        if !self.state.can_transition_to(new_state) {
            return Err(crate::error::AgentError::InvalidTransition {
                from: self.state.to_string(),
                to: new_state.to_string(),
            });
        }

        let now = Utc::now();
        self.state = new_state;
        self.updated_at = now;

        match new_state {
            OperationState::Initializing if self.started_at.is_none() => {
                self.started_at = Some(now);
            }
            OperationState::Complete | OperationState::Failed | OperationState::Cancelled => {
                self.completed_at = Some(now);
            }
            _ => {}
        }

        Ok(())
    }

    /// Set the error info and transition to Failed state.
    ///
    /// # Errors
    ///
    /// Returns an error if the transition to Failed is not valid.
    pub fn fail(&mut self, error: ErrorInfo) -> Result<(), crate::error::AgentError> {
        self.error = Some(error);
        self.transition_to(OperationState::Failed)
    }

    /// Update progress metrics.
    pub fn update_progress(&mut self, progress: Progress) {
        self.progress = Some(progress);
        self.updated_at = Utc::now();
    }
}
