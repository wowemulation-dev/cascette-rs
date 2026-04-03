//! Patch chain types for constructing patch application sequences
//!
//! A patch chain represents a sequence of patch steps needed to transform
//! a file from one encoding key to another. The chain is constructed from
//! `PatchConfig` entries by finding the shortest path from source to target.
//!
//! This module only contains data types and graph traversal logic. No I/O
//! or patch application happens here.

use std::collections::HashSet;
use std::fmt;

/// Maximum chain length, matching the agent.exe limit
pub const MAX_CHAIN_LENGTH: usize = 10;

/// A sequence of patch steps from start to end
#[derive(Debug, Clone)]
pub struct PatchChain {
    /// Ordered patch steps from start to end
    pub steps: Vec<PatchStep>,
    /// Starting file encoding key
    pub start_key: [u8; 16],
    /// Final result encoding key
    pub end_key: [u8; 16],
}

/// A single step in a patch chain
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PatchStep {
    /// Base file encoding key (input to this step)
    pub original_ekey: [u8; 16],
    /// Base file content size
    pub original_size: u64,
    /// Patch data encoding key
    pub patch_key: [u8; 16],
    /// Patch data encoded size
    pub patch_size: u64,
    /// Result encoding key after applying this step
    pub result_key: [u8; 16],
    /// Result content size
    pub result_size: u64,
}

/// Errors during patch chain construction
#[derive(Debug, Clone)]
pub enum PatchChainError {
    /// A cycle was detected in the patch graph
    CycleDetected,
    /// Chain exceeds the maximum allowed length
    ChainTooLong {
        /// Actual chain length
        length: usize,
        /// Maximum allowed
        max: usize,
    },
    /// No path exists between source and target
    NoPathFound {
        /// Source encoding key
        from: [u8; 16],
        /// Target encoding key
        to: [u8; 16],
    },
}

impl fmt::Display for PatchChainError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CycleDetected => write!(f, "cycle detected in patch chain"),
            Self::ChainTooLong { length, max } => {
                write!(f, "patch chain too long: {length} steps (max {max})")
            }
            Self::NoPathFound { from, to } => {
                write!(
                    f,
                    "no patch path from {} to {}",
                    hex::encode(from),
                    hex::encode(to)
                )
            }
        }
    }
}

impl std::error::Error for PatchChainError {}

/// An edge in the patch graph (used for chain construction)
#[derive(Debug, Clone)]
pub struct PatchEdge {
    /// Source encoding key
    pub from_ekey: [u8; 16],
    /// Source content size
    pub from_size: u64,
    /// Target encoding key
    pub to_ekey: [u8; 16],
    /// Target content size
    pub to_size: u64,
    /// Patch data encoding key
    pub patch_key: [u8; 16],
    /// Patch data size
    pub patch_size: u64,
}

impl PatchChain {
    /// Build a patch chain from a set of edges
    ///
    /// Uses depth-first search with cycle detection to find a path from
    /// `start` to `end` in the patch graph.
    pub fn build(
        edges: &[PatchEdge],
        start: [u8; 16],
        end: [u8; 16],
    ) -> Result<Self, PatchChainError> {
        if start == end {
            return Ok(Self {
                steps: Vec::new(),
                start_key: start,
                end_key: end,
            });
        }

        let mut visited = HashSet::new();
        visited.insert(start);

        let mut path = Vec::new();
        if Self::dfs(edges, start, end, &mut visited, &mut path)? {
            Ok(Self {
                steps: path,
                start_key: start,
                end_key: end,
            })
        } else {
            Err(PatchChainError::NoPathFound {
                from: start,
                to: end,
            })
        }
    }

    /// DFS traversal of the patch graph
    fn dfs(
        edges: &[PatchEdge],
        current: [u8; 16],
        target: [u8; 16],
        visited: &mut HashSet<[u8; 16]>,
        path: &mut Vec<PatchStep>,
    ) -> Result<bool, PatchChainError> {
        if path.len() >= MAX_CHAIN_LENGTH {
            return Err(PatchChainError::ChainTooLong {
                length: path.len() + 1,
                max: MAX_CHAIN_LENGTH,
            });
        }

        for edge in edges {
            if edge.from_ekey != current {
                continue;
            }

            if visited.contains(&edge.to_ekey) {
                // Skip already-visited nodes (avoids cycles)
                continue;
            }

            visited.insert(edge.to_ekey);
            path.push(PatchStep {
                original_ekey: edge.from_ekey,
                original_size: edge.from_size,
                patch_key: edge.patch_key,
                patch_size: edge.patch_size,
                result_key: edge.to_ekey,
                result_size: edge.to_size,
            });

            if edge.to_ekey == target {
                return Ok(true);
            }

            if Self::dfs(edges, edge.to_ekey, target, visited, path)? {
                return Ok(true);
            }

            // Backtrack
            path.pop();
        }

        Ok(false)
    }

    /// Number of steps in the chain
    pub fn len(&self) -> usize {
        self.steps.len()
    }

    /// Whether the chain is empty (start == end)
    pub fn is_empty(&self) -> bool {
        self.steps.is_empty()
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    fn make_key(byte: u8) -> [u8; 16] {
        [byte; 16]
    }

    fn edge(from: u8, to: u8) -> PatchEdge {
        PatchEdge {
            from_ekey: make_key(from),
            from_size: 1000,
            to_ekey: make_key(to),
            to_size: 2000,
            patch_key: make_key(from ^ to),
            patch_size: 500,
        }
    }

    #[test]
    fn test_direct_path() {
        let edges = vec![edge(1, 2)];
        let chain = PatchChain::build(&edges, make_key(1), make_key(2)).unwrap();
        assert_eq!(chain.len(), 1);
        assert_eq!(chain.steps[0].original_ekey, make_key(1));
        assert_eq!(chain.steps[0].result_key, make_key(2));
    }

    #[test]
    fn test_multi_step_path() {
        let edges = vec![edge(1, 2), edge(2, 3), edge(3, 4)];
        let chain = PatchChain::build(&edges, make_key(1), make_key(4)).unwrap();
        assert_eq!(chain.len(), 3);
        assert_eq!(chain.start_key, make_key(1));
        assert_eq!(chain.end_key, make_key(4));
    }

    #[test]
    fn test_no_path() {
        let edges = vec![edge(1, 2), edge(3, 4)];
        let result = PatchChain::build(&edges, make_key(1), make_key(4));
        assert!(matches!(
            result.unwrap_err(),
            PatchChainError::NoPathFound { .. }
        ));
    }

    #[test]
    fn test_same_start_end() {
        let edges = vec![edge(1, 2)];
        let chain = PatchChain::build(&edges, make_key(1), make_key(1)).unwrap();
        assert!(chain.is_empty());
    }

    #[test]
    fn test_cycle_avoidance() {
        // A -> B -> C -> A (cycle), but looking for A -> C
        let edges = vec![edge(1, 2), edge(2, 3), edge(3, 1)];
        let chain = PatchChain::build(&edges, make_key(1), make_key(3)).unwrap();
        assert_eq!(chain.len(), 2);
    }

    #[test]
    fn test_chain_too_long() {
        // Chain of 11 edges exceeds MAX_CHAIN_LENGTH
        let edges: Vec<PatchEdge> = (0..11u8).map(|i| edge(i, i + 1)).collect();
        let result = PatchChain::build(&edges, make_key(0), make_key(11));
        assert!(matches!(
            result.unwrap_err(),
            PatchChainError::ChainTooLong { .. }
        ));
    }
}
