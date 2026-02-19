# Agent.exe Maintenance Operations

Technical notes on Agent.exe (TACT 3.13.3, CASC 1.5.9) covering
`Compactor`, `ExtractCompactor`, `CompactionFileMover`, and
`ExtractorCompactorBackup` operations.

## Compaction

Agent uses two-phase compaction: archive merge followed by
extract-compact. Two algorithms are available: defrag (moves data
to fill gaps) and fillholes (estimates free space without moves).

### Two Compaction Modes

| Mode | Flag | Init Function | Callback |
|------|------|---------------|----------|
| Archive Merge | 0 | `InitArchiveMerge` | External completion callback |
| Extract Compact | 1 | `InitExtractCompact` | None (direct dispatch) |

### Archive Merge

`Compactor::InitArchiveMerge`:

1. Copy 0x30 bytes of defrag parameters
2. Store completion callback and context
3. Read archive merge threshold
4. Initialize compaction progress tracker
5. Call `Compactor::Execute` to validate container and build work plan
6. On success: dispatch async merge with `OnArchiveMergeComplete`
   as handler
7. On failure: log "Failed to initialize information necessary to run
   compaction"

`Compactor::OnArchiveMergeComplete`:

- Success (0): update progress, transition to `StartCompaction`
  (segment move phase)
- Cancelled (0xF): call `OnCompactionComplete`
- Other error: log "Archive merging failed", record error, complete

`Compactor::FinalizeArchiveMerge`:

1. Flush pending work
2. Iterate segments (stride 0x40 per segment), record errors
3. Update compaction progress
4. Call `ContainerIndex::ValidateSpans` post-merge
5. If validation fails: set error state 9
6. Record elapsed time metrics
7. Handle cancellation race (both cancel and success -> set 0xF)

Error states: 0=success, 9=validation failure, 0xF=cancelled.

### Extract-Compact

`Compactor::InitExtractCompact`:

1. Set extract-compact flag byte to 1
2. Copy defrag parameters
3. Clear external callback pointers (none for extract-compact)
4. Call `Compactor::Execute`
5. On success: directly call `StartCompaction` (no async dispatch)

`ExtractCompactor::ProcessSegment`:

1. Acquire SRW lock
2. Query segment file size
3. Validate size >= 0x1E0 (480 bytes = segment header)
4. Validate entry count >= 0x10 (16 = header entries)
5. Sort spans
6. Call `ValidateSpans` to check for overlaps
7. If content beyond headers: build move plan, dispatch work
8. If header-only (16 entries): call `TruncateArchive`
9. If empty: call `DeleteEmptyArchive`

`ExtractCompactor::ValidateSpans`:

1. If span count < 2: return true
2. Iterate pairwise (12-byte stride: `{offset, size, ekey_index}`)
3. Check `span[i+1].offset >= span[i].offset + span[i].size`
4. Log overlapping spans with both EKeys if detected

### Segment Dispatch

`Compactor::DispatchSegmentWork`:

1. Acquire SRW lock
2. Return completed work to priority queue
3. Check cancellation flag
4. Dequeue next segment
5. Start async I/O for segment move
6. Track single in-flight segment at a time
7. When no delayed segment and no in-flight work: set all-done flag,
   call `OnCompactionComplete`

Concurrency: single in-flight segment. Priority queue holds remaining
work. In-flight counter tracks segments in async I/O.

### Async Pipeline

The `CompactionFileMover` provides an async read/write pipeline for
moving data between archive files during compaction.

`CompactionFileMover::Initialize`:

1. Compute buffer count and size:
   - If total buffer >= 128 KiB: `count = min(total >> 17, 16)`,
     `per_buffer = total / count`
   - If total buffer < 128 KiB: count=1, log performance warning
2. Allocate buffer objects and move contexts
3. Each context stores owner pointer and buffer reference

**Pipeline stages**:

```text
QueueAsyncRead -> OnReadComplete -> (queue write) -> OnWriteComplete
     ^                                                    |
     |_____________ (more data to move) __________________|
     |
     v (all done)
ReconstructAndUpdateIndex
```

`QueueAsyncRead`:

1. Check cancellation
2. Compute read offset: `segment_index * 0x40000000 + current_position`
3. Read size: `min(remaining, per_buffer_size)`
4. Issue async file read with completion callback

`OnReadComplete`:

1. Handle errors (cancelled=0xF, truncated=0 bytes read)
2. Process/transform read data
3. If all source bytes consumed: remove index keys
4. Queue async write to destination offset

`OnWriteComplete`:

1. Handle errors and partial writes
2. If partial write: shift unwritten data, re-queue write
3. If full write, more data: `QueueAsyncRead` for next chunk
4. If all bytes moved: `ReconstructAndUpdateIndex`

`ReconstructAndUpdateIndex`:

1. Iterate entries in move item (12-byte stride per entry)
2. For each entry:
   - Prepare 30-byte (0x1E) reconstruction record (LocalHeader)
   - Remove old index key (if not already removed)
   - Insert new key mapping with updated offset
   - Update residency spans
3. Log partial span update failures (non-fatal unless
   `ERROR_INVALID_BLOCK` or `ERROR_ARENA_TRASHED`)

Move work item layout:

| Offset | Field |
|--------|-------|
| [0] | Owner pointer |
| [1] | File handle / buffer pointer |
| [3] | Segment data pointer |
| [6..7] | Destination offset (u64) |
| [0x10] | Cancelled marker byte |
| [0x11] | Total bytes to move |
| [0x12] | Bytes moved so far |
| [0x13] | Bytes in current read |
| [0x14..0x15] | OVERLAPPED structure |

### Backup / Recovery

The `ExtractorCompactorBackup` provides crash recovery for
in-progress compaction operations using a memory-mapped file.

`ExtractorCompactorBackup::Open`:

1. Construct backup file path: `<data_dir>.extract_bu`
2. Open file, resize to 0x1005 (4101) bytes
3. Memory-map the file
4. Read header: if version != 1, initialize fresh
5. Validate entries: remove any with segment index >= 0x3FF

**File format**:

| Offset | Size | Field |
|--------|------|-------|
| 0 | 1 | Version (must be 1) |
| 1 | 4 | Max entries (u32 = 0x3FF = 1023) |
| 5 | 4 | Current entry count (u32) |
| 9 | N*4 | Entries (u32 segment indices) |

File size: 0x1005 (4101) bytes. Max entries: 1023.

`AddEntry`: append-only, no duplicate checking, flush after every add.

`GetEntries`: copy entries to output buffer, return total count.

### Metrics

Telemetry counters emitted during compaction:

- `dynamic.compaction.time.data_transfer_cancel`
- `dynamic.compaction.time.data_transfer_success`
- `dynamic.compaction.time.init`
- `dynamic.compaction.error.async_read`
- `dynamic.compaction.error.async_write`
- `dynamic.compaction.error.remove_key`
- `dynamic.compaction.error.reconstruct_key_mapping`
- `dynamic.compaction.error.update_residency`
- `dynamic.compaction.error.truncate_archive`

## Garbage Collection

4-stage pipeline:

1. **BuildPreservationFilter**: Collect keys from active builds
2. **GarbageCollectorFilter**: Mark unreferenced data
3. **Compaction**: Defrag or fillholes algorithm
4. **CleanupDirectory**: Remove empty archive files

## Build Repair

5-stage pipeline using marker files (RepairMarker.psv) for
crash recovery:

1. **ReadBuildConfig**: Load build configuration
2. **InitCdnIndexSet**: Initialize CDN index set
3. **RepairContainers**: Repair data, ecache, hardlink (sequential)
4. **RepairHardLinks**: Verify and fix hard links
5. **PostRepairCleanup**: Remove repair markers
