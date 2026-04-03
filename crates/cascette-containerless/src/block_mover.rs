//! Block-level data movement for containerless file updates.
//!
//! The block mover reads source blocks from a base file and writes them
//! to a target file at corresponding positions. This enables incremental
//! updates where only changed blocks are re-downloaded.
//!
//! From the RE docs (`containerless_block_mover.cpp`, 10 functions):
//! - Requires target and base E-headers (BLTE frame descriptors)
//! - Calculates total data size from block descriptors
//! - Reads source blocks, writes target blocks, updates residency
//! - Uses a memory buffer for I/O

use tracing::debug;

use crate::eheader::EHeader;
use crate::error::{ContainerlessError, ContainerlessResult};
use crate::loose::LooseFileStore;
use crate::residency::ResidencyTracker;

/// Descriptor for a single block within a BLTE frame table.
#[derive(Debug, Clone, Copy)]
pub struct BlockDescriptor {
    /// Byte offset within the file.
    pub offset: u64,
    /// Size of the block as BLTE-encoded.
    pub encoded_size: u32,
    /// Size of the block after decoding.
    pub decoded_size: u32,
}

/// Instruction for moving blocks between files.
#[derive(Debug, Clone)]
pub struct BlockMoveInstruction {
    /// Source encoding key.
    pub source_ekey: [u8; 16],
    /// Target encoding key.
    pub target_ekey: [u8; 16],
    /// Block ranges to read from the source.
    pub source_blocks: Vec<BlockDescriptor>,
    /// Block ranges to write in the target.
    pub target_blocks: Vec<BlockDescriptor>,
}

/// State of the block mover execution.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockMoverState {
    /// Initial state, ready to execute.
    Init,
    /// Reading blocks from the source file.
    ReadSource,
    /// Writing blocks to the target file.
    WriteTarget,
    /// Updating residency tracking.
    UpdateResidency,
    /// Operation completed.
    Complete,
    /// Error during execution.
    Error,
}

/// Block-level data mover for containerless file updates.
///
/// Moves block data from a base (source) file to a target file based
/// on E-header frame descriptors. Used during incremental updates to
/// reuse unchanged blocks.
pub struct BlockMover {
    instructions: Vec<BlockMoveInstruction>,
    buffer_size: usize,
    state: BlockMoverState,
    /// Total bytes to be moved across all instructions.
    total_bytes: u64,
    /// Bytes moved so far.
    moved_bytes: u64,
}

impl BlockMover {
    /// Minimum buffer size (64 KiB).
    const MIN_BUFFER_SIZE: usize = 64 * 1024;

    /// Create a new block mover from target and base E-headers.
    ///
    /// Validates that both headers are present and builds the instruction
    /// list. Returns an error if either header is missing.
    pub fn new(
        target_header: &EHeader,
        base_header: &EHeader,
        target_blocks: Vec<BlockDescriptor>,
        base_blocks: Vec<BlockDescriptor>,
        buffer_size: usize,
    ) -> ContainerlessResult<Self> {
        if target_blocks.is_empty() || base_blocks.is_empty() {
            return Err(ContainerlessError::InvalidConfig(
                "block mover requires target and base headers".to_string(),
            ));
        }

        let effective_buffer = if buffer_size < Self::MIN_BUFFER_SIZE {
            debug!(
                requested = buffer_size,
                minimum = Self::MIN_BUFFER_SIZE,
                "memory limit lower than minimum instruction size, increasing"
            );
            Self::MIN_BUFFER_SIZE
        } else {
            buffer_size
        };

        let total_bytes: u64 = base_blocks.iter().map(|b| u64::from(b.encoded_size)).sum();

        let instruction = BlockMoveInstruction {
            source_ekey: base_header.ekey,
            target_ekey: target_header.ekey,
            source_blocks: base_blocks,
            target_blocks,
        };

        Ok(Self {
            instructions: vec![instruction],
            buffer_size: effective_buffer,
            state: BlockMoverState::Init,
            total_bytes,
            moved_bytes: 0,
        })
    }

    /// Create a block mover from pre-built instructions.
    pub fn from_instructions(
        instructions: Vec<BlockMoveInstruction>,
        buffer_size: usize,
    ) -> ContainerlessResult<Self> {
        if instructions.is_empty() {
            return Err(ContainerlessError::InvalidConfig(
                "block mover requires at least one instruction".to_string(),
            ));
        }

        let effective_buffer = buffer_size.max(Self::MIN_BUFFER_SIZE);

        let total_bytes: u64 = instructions
            .iter()
            .flat_map(|i| &i.source_blocks)
            .map(|b| u64::from(b.encoded_size))
            .sum();

        Ok(Self {
            instructions,
            buffer_size: effective_buffer,
            state: BlockMoverState::Init,
            total_bytes,
            moved_bytes: 0,
        })
    }

    /// Current state.
    #[must_use]
    pub fn state(&self) -> BlockMoverState {
        self.state
    }

    /// Total bytes to be moved.
    #[must_use]
    pub fn total_bytes(&self) -> u64 {
        self.total_bytes
    }

    /// Bytes moved so far.
    #[must_use]
    pub fn moved_bytes(&self) -> u64 {
        self.moved_bytes
    }

    /// Execute all block move instructions.
    ///
    /// Reads blocks from source files in the loose store, writes them
    /// to the target files, and updates the residency tracker.
    pub async fn execute(
        &mut self,
        loose: &LooseFileStore,
        residency: &ResidencyTracker,
    ) -> ContainerlessResult<()> {
        self.state = BlockMoverState::ReadSource;

        for idx in 0..self.instructions.len() {
            let source_ekey = self.instructions[idx].source_ekey;
            let target_ekey = self.instructions[idx].target_ekey;

            let source_data = match loose.read(&source_ekey).await {
                Ok(data) => data,
                Err(e) => {
                    self.state = BlockMoverState::Error;
                    return Err(ContainerlessError::Io(std::io::Error::other(format!(
                        "block mover failed to read file {}. {}",
                        hex::encode(source_ekey),
                        e
                    ))));
                }
            };

            self.state = BlockMoverState::WriteTarget;

            // Clone block descriptors to avoid borrow conflict.
            let src_blocks = self.instructions[idx].source_blocks.clone();
            let tgt_blocks = self.instructions[idx].target_blocks.clone();

            let (target_data, bytes_copied) =
                Self::build_target_data(&source_data, &src_blocks, &tgt_blocks)?;
            self.moved_bytes += bytes_copied;

            if let Err(e) = loose.write(&target_ekey, &target_data).await {
                self.state = BlockMoverState::Error;
                return Err(ContainerlessError::Io(std::io::Error::other(format!(
                    "block mover failed to write file {}. {}",
                    hex::encode(target_ekey),
                    e
                ))));
            }

            self.state = BlockMoverState::UpdateResidency;
            residency.mark_resident(&target_ekey);
            debug!(
                source = %hex::encode(source_ekey),
                target = %hex::encode(target_ekey),
                bytes = target_data.len(),
                "block move complete"
            );
        }

        self.state = BlockMoverState::Complete;
        Ok(())
    }

    /// Build target file data by copying blocks from source data.
    ///
    /// Returns the assembled target data and the number of bytes copied.
    fn build_target_data(
        source_data: &[u8],
        source_blocks: &[BlockDescriptor],
        target_blocks: &[BlockDescriptor],
    ) -> ContainerlessResult<(Vec<u8>, u64)> {
        // Calculate total target size.
        let target_size: u64 = target_blocks
            .iter()
            .map(|b| b.offset + u64::from(b.encoded_size))
            .max()
            .unwrap_or(0);

        let mut target = vec![0u8; target_size as usize];
        let mut bytes_copied = 0u64;

        // Copy blocks from source to target using paired descriptors.
        let pairs = source_blocks.len().min(target_blocks.len());
        for i in 0..pairs {
            let src = &source_blocks[i];
            let dst = &target_blocks[i];

            let src_start = src.offset as usize;
            let src_end = src_start + src.encoded_size as usize;

            if src_end > source_data.len() {
                return Err(ContainerlessError::Integrity(format!(
                    "source block {i} extends beyond source data: offset={}, size={}, data_len={}",
                    src.offset,
                    src.encoded_size,
                    source_data.len()
                )));
            }

            let dst_start = dst.offset as usize;
            let copy_len = src.encoded_size as usize;
            let dst_end = dst_start + copy_len;

            if dst_end > target.len() {
                target.resize(dst_end, 0);
            }

            target[dst_start..dst_end].copy_from_slice(&source_data[src_start..src_end]);
            bytes_copied += u64::from(src.encoded_size);
        }

        Ok((target, bytes_copied))
    }

    /// Number of instructions in this mover.
    #[must_use]
    pub fn instruction_count(&self) -> usize {
        self.instructions.len()
    }

    /// Buffer size used for I/O.
    #[must_use]
    pub fn buffer_size(&self) -> usize {
        self.buffer_size
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    fn make_header(ekey_byte: u8) -> EHeader {
        EHeader {
            ekey: [ekey_byte; 16],
            encoded_size: 4096,
            frame_count: 2,
        }
    }

    #[test]
    fn test_block_descriptor() {
        let desc = BlockDescriptor {
            offset: 0,
            encoded_size: 1024,
            decoded_size: 2048,
        };
        assert_eq!(desc.offset, 0);
        assert_eq!(desc.encoded_size, 1024);
        assert_eq!(desc.decoded_size, 2048);
    }

    #[test]
    fn test_new_requires_blocks() {
        let target = make_header(0xAA);
        let base = make_header(0xBB);

        let result = BlockMover::new(&target, &base, vec![], vec![], 4096);
        assert!(result.is_err());
    }

    #[test]
    fn test_new_increases_small_buffer() {
        let target = make_header(0xAA);
        let base = make_header(0xBB);

        let target_blocks = vec![BlockDescriptor {
            offset: 0,
            encoded_size: 100,
            decoded_size: 200,
        }];
        let base_blocks = vec![BlockDescriptor {
            offset: 0,
            encoded_size: 100,
            decoded_size: 200,
        }];

        let mover = BlockMover::new(&target, &base, target_blocks, base_blocks, 32).unwrap();
        assert_eq!(mover.buffer_size(), BlockMover::MIN_BUFFER_SIZE);
    }

    #[test]
    fn test_instruction_building() {
        let target = make_header(0xAA);
        let base = make_header(0xBB);

        let target_blocks = vec![
            BlockDescriptor {
                offset: 0,
                encoded_size: 100,
                decoded_size: 200,
            },
            BlockDescriptor {
                offset: 100,
                encoded_size: 150,
                decoded_size: 300,
            },
        ];
        let base_blocks = vec![
            BlockDescriptor {
                offset: 0,
                encoded_size: 100,
                decoded_size: 200,
            },
            BlockDescriptor {
                offset: 100,
                encoded_size: 150,
                decoded_size: 300,
            },
        ];

        let mover =
            BlockMover::new(&target, &base, target_blocks, base_blocks, 1024 * 1024).unwrap();
        assert_eq!(mover.instruction_count(), 1);
        assert_eq!(mover.total_bytes(), 250);
        assert_eq!(mover.state(), BlockMoverState::Init);
    }

    #[test]
    fn test_from_instructions_empty() {
        let result = BlockMover::from_instructions(vec![], 4096);
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_execute_moves_blocks() {
        let dir = tempfile::tempdir().unwrap();
        let loose = LooseFileStore::new(dir.path().to_path_buf());
        let residency = ResidencyTracker::new();

        let source_ekey = [0x01; 16];
        let target_ekey = [0x02; 16];

        // Write source file: 200 bytes of known data.
        let mut source_data = vec![0xAA; 100];
        source_data.extend_from_slice(&[0xBB; 100]);
        loose.write(&source_ekey, &source_data).await.unwrap();

        let source_header = EHeader {
            ekey: source_ekey,
            encoded_size: 200,
            frame_count: 2,
        };
        let target_header = EHeader {
            ekey: target_ekey,
            encoded_size: 200,
            frame_count: 2,
        };

        let source_blocks = vec![
            BlockDescriptor {
                offset: 0,
                encoded_size: 100,
                decoded_size: 100,
            },
            BlockDescriptor {
                offset: 100,
                encoded_size: 100,
                decoded_size: 100,
            },
        ];
        let target_blocks = vec![
            BlockDescriptor {
                offset: 0,
                encoded_size: 100,
                decoded_size: 100,
            },
            BlockDescriptor {
                offset: 100,
                encoded_size: 100,
                decoded_size: 100,
            },
        ];

        let mut mover = BlockMover::new(
            &target_header,
            &source_header,
            target_blocks,
            source_blocks,
            1024 * 1024,
        )
        .unwrap();

        mover.execute(&loose, &residency).await.unwrap();

        assert_eq!(mover.state(), BlockMoverState::Complete);
        assert_eq!(mover.moved_bytes(), 200);
        assert!(residency.is_resident(&target_ekey));

        // Verify target data matches source.
        let target_data = loose.read(&target_ekey).await.unwrap();
        assert_eq!(&target_data[..100], &[0xAA; 100]);
        assert_eq!(&target_data[100..], &[0xBB; 100]);
    }

    #[tokio::test]
    async fn test_execute_error_on_missing_source() {
        let dir = tempfile::tempdir().unwrap();
        let loose = LooseFileStore::new(dir.path().to_path_buf());
        let residency = ResidencyTracker::new();

        let source_header = EHeader {
            ekey: [0x01; 16],
            encoded_size: 100,
            frame_count: 1,
        };
        let target_header = EHeader {
            ekey: [0x02; 16],
            encoded_size: 100,
            frame_count: 1,
        };

        let blocks = vec![BlockDescriptor {
            offset: 0,
            encoded_size: 100,
            decoded_size: 100,
        }];

        let mut mover = BlockMover::new(
            &target_header,
            &source_header,
            blocks.clone(),
            blocks,
            1024 * 1024,
        )
        .unwrap();

        let result = mover.execute(&loose, &residency).await;
        assert!(result.is_err());
        assert_eq!(mover.state(), BlockMoverState::Error);
    }

    #[test]
    fn test_build_target_data_out_of_bounds() {
        let source_blocks = vec![BlockDescriptor {
            offset: 0,
            encoded_size: 100,
            decoded_size: 100,
        }];
        let target_blocks = vec![BlockDescriptor {
            offset: 0,
            encoded_size: 100,
            decoded_size: 100,
        }];

        // Source data is only 50 bytes, but block says 100.
        let source_data = vec![0u8; 50];
        let result = BlockMover::build_target_data(&source_data, &source_blocks, &target_blocks);
        assert!(result.is_err());
    }
}
