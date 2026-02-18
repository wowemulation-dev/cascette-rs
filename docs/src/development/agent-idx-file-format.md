# Agent.exe IDX / KMT File Format

Technical notes on Agent.exe (TACT 3.13.3, CASC 1.5.9) covering
`casc::KeyMappingTable` operations.

## Overview

The Key Mapping Table (KMT) is the primary on-disk index for CASC local
storage. KMT files and IDX files are the same format -- the terms are
interchangeable. Agent.exe uses the KMT terminology internally
(`casc::KeyMappingTable`).

Each bucket (0-15) has its own KMT file named `{bucket:02x}{version:08x}.idx`
in the `indices/` directory.

## File Layout

A KMT file has three regions:

```text
+---------------------------+
| Guarded Block: Header     |  8-byte section header + 16-byte file header
+---------------------------+
| Guarded Block: Sorted     |  Sorted entries (18 bytes each)
+---------------------------+
| Padding to 16-byte align  |
+---------------------------+
| Empty Section Header      |  8 bytes (zeros) marking sorted section end
+---------------------------+
| Padding to 64KB boundary  |  Round up with (pos + 0x17FFF) & 0xFFFF0000
+---------------------------+
| Update Section            |  512-byte pages, append-only
+---------------------------+
```

## File Header (16 bytes)

Written by `casc::KeyMappingTable::WriteHeader`.

```text
Offset  Size  Field
0x00    2     Version (7 for V7, little-endian)
0x02    1     Bucket index (0-15)
0x03    1     Extra bytes (0x00)
0x04    1     Encoded size bytes (0x04)
0x05    1     Storage offset bytes (0x05)
0x06    1     EKey bytes (0x09)
0x07    1     Segment bits (0x1E = 30)
0x08    4     Entry count (low 32 bits)
0x0C    4     Entry count (high 32 bits)
```

The header is wrapped in a guarded block: an 8-byte section header
containing the block size and a Jenkins `hashlittle()` hash of the
block content.

## Sorted Section

The sorted section contains entries in ascending EKey order. Each entry
is 18 bytes:

```text
Offset  Size  Field
0x00    9     EKey (truncated encoding key, big-endian)
0x09    5     StorageOffset (big-endian)
0x0E    4     EncodedSize (big-endian)
```

All fields are big-endian (verified via CascLib `ConvertBytesToInteger_BE`
and analysis of `BinarySearchEKey`).

Lookup uses binary search (`casc::KeyMappingTable::BinarySearchEKey`).

## Update Section

The update section is an append-only log for recent changes. It starts
at a 64KB-aligned boundary after the sorted section.

### Page Format

Each page is 512 bytes (0x200) and holds up to 21 entries (0x15).

```text
Page (512 bytes):
+-------+-------+-------+-----+--------+
| Entry | Entry | Entry | ... | Unused |
|  0    |  1    |  2    |     |        |
+-------+-------+-------+-----+--------+
  24B     24B     24B          (512 - 21*24 = 8 bytes unused)
```

An empty page has its first 4 bytes set to zero. Parsing stops at the
first empty page.

### Update Entry Format (24 bytes)

```text
Offset  Size  Field
0x00    4     Hash guard (Jenkins hashlittle | 0x80000000)
0x04    9     EKey (truncated encoding key)
0x0D    5     StorageOffset (big-endian)
0x12    4     EncodedSize (big-endian)
0x16    1     Status byte
0x17    1     Padding
```

The hash guard covers bytes 0x04 through 0x16 (19 bytes = 0x13) using
Jenkins `hashlittle()` with seed 0, OR'd with 0x80000000 to distinguish
from empty entries.

### Status Byte Values

The status byte indicates the entry's purpose:

| Value | Meaning |
|-------|---------|
| 0     | Normal entry (insert/update) |
| 3     | Delete entry |
| 6     | Header non-resident |
| 7     | Data non-resident |

Status byte encoding from `UpdateResidency`: `(is_header ^ 1) + 6`,
where `is_header=0` yields 7 (data non-resident) and `is_header=1`
yields 6 (header non-resident).

### Page Sync

Every 8th page (when `page_index & 7 == 7`) and the last entry slot
(index 0x14 = 20) is filled, a 4KB block (8 pages) is synced to disk
to disk. This provides write-ahead durability without flushing
the entire file.

### Minimum Section Size

The update section requires at least 0x7800 bytes (30,720 bytes = 60
pages). If the remaining file space is less than this, parsing logs
"Truncated KMT update section detected" and returns error 9.

## Search Algorithm

`casc::KeyMappingTable::SearchBothSections` searches both
sections and merges results:

1. **Sorted section**: Binary search on the 9-byte EKey
   (`BinarySearchEKey`). Returns a range of matching entries.

2. **Update section**: Linear scan through all pages. For each
   non-empty page, compares the 9-byte EKey at bytes 0x04-0x0C of
   each entry (three 4-byte comparisons for the first 8 bytes plus
   a single byte comparison for byte 8).

3. **Merge**: Results from both sections are merged and deduplicated
   by EKey. Update section entries take precedence
   over sorted section entries (newer wins).

## Compaction (Flush)

When the update section is full, `casc::ContainerIndex::FlushTable`
triggers compaction:

1. Calls `casc::IndexTables::FlushAndBindLoose` which merges the
   update section entries into the sorted section
2. Writes a new KMT file with all entries in sorted order
3. The new file atomically replaces the old one (rename)
4. Updates the shared memory control block with the new file handle

`FlushTableWithDeletes` is a variant that also processes
delete entries (status byte 3), removing them from the sorted section
during the merge.

## Insert Flow

`casc::KeyMappingTable::InsertEntry`:

1. If status is 3 (delete) and no update section exists: return
   error 0x0A (not supported without update section)
2. Search both sections for the key
3. If found and `arg5` (span info) is provided: adjust the storage
   offset by adding the existing offset (accumulate)
4. Write entry to the next available slot in the current update page:
   - Copy 9-byte EKey to offset 0x04
   - Write 5-byte StorageOffset at offset 0x0D
   - Write 4-byte EncodedSize at offset 0x12
   - Write status byte at offset 0x16
   - Compute and write hash guard at offset 0x00
5. If the current page is the 21st entry (0x14) on an 8th page
   boundary: sync the 4KB block to disk

If the insert fails with ERROR_INVALID_DRIVE (update section full),
the caller (`ContainerIndex::UpdateKeyMapping`) triggers a flush and
retries.
