//! Integration tests for ZBSDIFF1 format using real CDN fixtures.
//!
//! These tests load `.zbsdiff` files from `test_fixtures/zbsdiff/` and
//! validate that the Rust parser handles real patches from Blizzard's CDN.
//!
//! Two types of fixtures are supported:
//!
//! 1. **Patch-only** (`.zbsdiff`): Tests parsing and structural validation.
//! 2. **Triplets** (`.old` + `.new` + `.zbsdiff`): Tests patch application
//!    by verifying `apply_patch(old, patch) == new`.
//!
//! To populate fixtures, use the cascette-py tools:
//! ```sh
//! # Patch-only fixtures:
//! uv run cascette fetch zbsdiff --product wow_classic --limit 5 --output-dir /tmp/zbsdiff-patches/
//!
//! # Triplets (old + new + patch):
//! uv run python scripts/download_zbsdiff_triplets.py --product wow_classic --limit 5
//! ```
//! Then copy the files to `crates/cascette-formats/test_fixtures/zbsdiff/`.
//!
//! If no fixture files are present, the tests are skipped.

#![allow(clippy::expect_used)]

use std::path::PathBuf;

use cascette_formats::zbsdiff::{
    self, ControlBlock, ZBSDIFF1_SIGNATURE, ZbsDiff, ZbsdiffBuilder, ZbsdiffHeader, ZbsdiffPatcher,
};

fn fixture_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("test_fixtures/zbsdiff")
}

fn collect_zbsdiff_files() -> Vec<PathBuf> {
    let dir = fixture_dir();
    if !dir.exists() {
        return Vec::new();
    }

    let mut files: Vec<PathBuf> = std::fs::read_dir(&dir)
        .unwrap()
        .filter_map(|entry| {
            let entry = entry.ok()?;
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("zbsdiff") {
                Some(path)
            } else {
                None
            }
        })
        .collect();
    files.sort();
    files
}

#[test]
fn zbsdiff_cdn_fixtures_parse_headers() {
    let files = collect_zbsdiff_files();
    if files.is_empty() {
        eprintln!(
            "SKIPPED: no .zbsdiff fixtures in {}",
            fixture_dir().display()
        );
        return;
    }

    for path in &files {
        let data = std::fs::read(path).unwrap();
        let filename = path.file_name().unwrap().to_string_lossy();

        // Parse header using binrw
        let header = {
            use binrw::BinRead;
            use std::io::Cursor;
            let mut cursor = Cursor::new(&data);
            ZbsdiffHeader::read_options(&mut cursor, binrw::Endian::Little, ())
                .unwrap_or_else(|e| panic!("{filename}: header parse failed: {e}"))
        };

        // Validate signature
        assert_eq!(
            header.signature, ZBSDIFF1_SIGNATURE,
            "{filename}: wrong signature"
        );

        // Validate sizes are non-negative
        assert!(
            header.control_size >= 0,
            "{filename}: negative control_size: {}",
            header.control_size
        );
        assert!(
            header.diff_size >= 0,
            "{filename}: negative diff_size: {}",
            header.diff_size
        );
        assert!(
            header.output_size >= 0,
            "{filename}: negative output_size: {}",
            header.output_size
        );

        // Validate header passes full validation
        header
            .validate()
            .unwrap_or_else(|e| panic!("{filename}: validation failed: {e}"));

        eprintln!(
            "  {filename}: control={}, diff={}, output={}",
            header.control_size, header.diff_size, header.output_size
        );
    }

    eprintln!("Parsed {} CDN fixture headers", files.len());
}

#[test]
fn zbsdiff_cdn_fixtures_parse_full() {
    let files = collect_zbsdiff_files();
    if files.is_empty() {
        eprintln!(
            "SKIPPED: no .zbsdiff fixtures in {}",
            fixture_dir().display()
        );
        return;
    }

    for path in &files {
        let data = std::fs::read(path).unwrap();
        let filename = path.file_name().unwrap().to_string_lossy();

        // Parse the full patch structure
        let patch =
            ZbsDiff::parse(&data).unwrap_or_else(|e| panic!("{filename}: full parse failed: {e}"));

        // Verify compressed block sizes match header
        assert_eq!(
            patch.control_data.len(),
            patch.header.control_size as usize,
            "{filename}: control block size mismatch"
        );
        assert_eq!(
            patch.diff_data.len(),
            patch.header.diff_size as usize,
            "{filename}: diff block size mismatch"
        );

        // Decompress and parse the control block
        let control_block = ControlBlock::from_compressed(&patch.control_data)
            .unwrap_or_else(|e| panic!("{filename}: control block decompression failed: {e}"));

        // Validate control entries have reasonable values
        for (i, entry) in control_block.entries.iter().enumerate() {
            assert!(
                entry.diff_size >= 0,
                "{filename}: entry {i} has negative diff_size: {}",
                entry.diff_size
            );
            assert!(
                entry.extra_size >= 0,
                "{filename}: entry {i} has negative extra_size: {}",
                entry.extra_size
            );
            // seek_offset can be negative, so no assertion needed
        }

        eprintln!(
            "  {filename}: {} control entries, output_size={}",
            control_block.entry_count(),
            patch.header.output_size
        );
    }

    eprintln!("Fully parsed {} CDN fixtures", files.len());
}

/// A triplet of old file, new file, and patch for application testing.
struct PatchTriplet {
    name: String,
    old_data: Vec<u8>,
    new_data: Vec<u8>,
    patch_data: Vec<u8>,
}

/// Collect patch triplets where .old, .new, and .zbsdiff all exist.
fn collect_triplets() -> Vec<PatchTriplet> {
    let dir = fixture_dir();
    if !dir.exists() {
        return Vec::new();
    }

    let mut triplets = Vec::new();
    let zbsdiff_files = collect_zbsdiff_files();

    for zbsdiff_path in &zbsdiff_files {
        let stem = zbsdiff_path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("");
        let old_path = dir.join(format!("{stem}.old"));
        let new_path = dir.join(format!("{stem}.new"));

        if old_path.exists() && new_path.exists() {
            triplets.push(PatchTriplet {
                name: stem.to_string(),
                old_data: std::fs::read(&old_path)
                    .unwrap_or_else(|e| panic!("{stem}.old: read failed: {e}")),
                new_data: std::fs::read(&new_path)
                    .unwrap_or_else(|e| panic!("{stem}.new: read failed: {e}")),
                patch_data: std::fs::read(zbsdiff_path)
                    .unwrap_or_else(|e| panic!("{stem}.zbsdiff: read failed: {e}")),
            });
        }
    }

    triplets
}

#[test]
fn zbsdiff_cdn_apply_patch_memory() {
    let triplets = collect_triplets();
    if triplets.is_empty() {
        eprintln!(
            "SKIPPED: no .old/.new/.zbsdiff triplets in {}",
            fixture_dir().display()
        );
        return;
    }

    for triplet in &triplets {
        let result = zbsdiff::apply_patch_memory(&triplet.old_data, &triplet.patch_data)
            .unwrap_or_else(|e| {
                panic!(
                    "{}: apply_patch_memory failed: {e}\n  old={} bytes, patch={} bytes",
                    triplet.name,
                    triplet.old_data.len(),
                    triplet.patch_data.len(),
                )
            });

        assert_eq!(
            result.len(),
            triplet.new_data.len(),
            "{}: output size mismatch (got {}, expected {})",
            triplet.name,
            result.len(),
            triplet.new_data.len(),
        );

        assert_eq!(
            result,
            triplet.new_data,
            "{}: patched output does not match expected new file ({} bytes)",
            triplet.name,
            triplet.new_data.len(),
        );

        eprintln!(
            "  {}: {} -> {} bytes (patch {} bytes) OK",
            triplet.name,
            triplet.old_data.len(),
            triplet.new_data.len(),
            triplet.patch_data.len(),
        );
    }

    eprintln!("Applied {} CDN patches with correct output", triplets.len());
}

#[test]
fn zbsdiff_cdn_apply_patch_streaming() {
    let triplets = collect_triplets();
    if triplets.is_empty() {
        eprintln!(
            "SKIPPED: no .old/.new/.zbsdiff triplets in {}",
            fixture_dir().display()
        );
        return;
    }

    for triplet in &triplets {
        let old_cursor = std::io::Cursor::new(&triplet.old_data);
        let header = ZbsdiffHeader::parse_from_patch(&triplet.patch_data)
            .unwrap_or_else(|e| panic!("{}: header parse failed: {e}", triplet.name));

        let patcher = ZbsdiffPatcher::new(old_cursor, header.output_size as usize);
        let result = patcher
            .apply_patch_from_data(&triplet.patch_data)
            .unwrap_or_else(|e| {
                panic!(
                    "{}: streaming apply failed: {e}\n  old={} bytes, patch={} bytes",
                    triplet.name,
                    triplet.old_data.len(),
                    triplet.patch_data.len(),
                )
            });

        assert_eq!(
            result, triplet.new_data,
            "{}: streaming patched output does not match expected new file",
            triplet.name,
        );

        eprintln!(
            "  {}: streaming {} -> {} bytes OK",
            triplet.name,
            triplet.old_data.len(),
            triplet.new_data.len(),
        );
    }

    eprintln!(
        "Streaming-applied {} CDN patches with correct output",
        triplets.len()
    );
}

#[test]
fn zbsdiff_cdn_apply_patch_streaming_small_buffer() {
    let triplets = collect_triplets();
    if triplets.is_empty() {
        eprintln!(
            "SKIPPED: no .old/.new/.zbsdiff triplets in {}",
            fixture_dir().display()
        );
        return;
    }

    // Use a small buffer (1KB) to exercise chunked I/O paths
    for triplet in &triplets {
        let old_cursor = std::io::Cursor::new(&triplet.old_data);
        let header = ZbsdiffHeader::parse_from_patch(&triplet.patch_data)
            .unwrap_or_else(|e| panic!("{}: header parse failed: {e}", triplet.name));

        let patcher =
            ZbsdiffPatcher::new(old_cursor, header.output_size as usize).with_buffer_size(1024);
        let result = patcher
            .apply_patch_from_data(&triplet.patch_data)
            .unwrap_or_else(|e| {
                panic!("{}: small-buffer streaming apply failed: {e}", triplet.name)
            });

        assert_eq!(
            result, triplet.new_data,
            "{}: small-buffer output does not match expected new file",
            triplet.name,
        );

        eprintln!(
            "  {}: small-buffer streaming {} -> {} bytes OK",
            triplet.name,
            triplet.old_data.len(),
            triplet.new_data.len(),
        );
    }

    eprintln!(
        "Small-buffer streaming-applied {} CDN patches",
        triplets.len()
    );
}

#[test]
fn zbsdiff_cdn_builder_round_trip() {
    let triplets = collect_triplets();
    if triplets.is_empty() {
        eprintln!(
            "SKIPPED: no .old/.new/.zbsdiff triplets in {}",
            fixture_dir().display()
        );
        return;
    }

    for triplet in &triplets {
        // Build a patch from old -> new using the bsdiff algorithm
        let builder = ZbsdiffBuilder::new(triplet.old_data.clone(), triplet.new_data.clone());
        let our_patch = builder
            .build()
            .unwrap_or_else(|e| panic!("{}: builder.build() failed: {e}", triplet.name));

        // Apply our patch and verify the output matches the expected new data.
        // The patch bytes will differ from Blizzard's (different zlib params,
        // possibly different tie-breaking in the algorithm), but the patched
        // output must be identical.
        let result =
            zbsdiff::apply_patch_memory(&triplet.old_data, &our_patch).unwrap_or_else(|e| {
                panic!(
                    "{}: apply our patch failed: {e}\n  old={} bytes, our_patch={} bytes",
                    triplet.name,
                    triplet.old_data.len(),
                    our_patch.len(),
                )
            });

        assert_eq!(
            result,
            triplet.new_data,
            "{}: builder round-trip output does not match expected new file ({} bytes)",
            triplet.name,
            triplet.new_data.len(),
        );

        eprintln!(
            "  {}: builder {} -> {} bytes (our patch {} bytes, cdn patch {} bytes)",
            triplet.name,
            triplet.old_data.len(),
            triplet.new_data.len(),
            our_patch.len(),
            triplet.patch_data.len(),
        );
    }

    eprintln!(
        "Builder round-trip verified {} CDN triplets",
        triplets.len()
    );
}

/// Parse a real CDN `.zbsdiff` file into `ZbsDiff`, rebuild it to bytes,
/// and verify the rebuilt patch applies correctly against the `.old`/`.new`
/// fixture data. This tests the parse-serialize round-trip of Blizzard-
/// produced patches, separate from our own diff algorithm.
#[test]
fn zbsdiff_cdn_parse_rebuild_apply() {
    let triplets = collect_triplets();
    if triplets.is_empty() {
        eprintln!(
            "SKIPPED: no .old/.new/.zbsdiff triplets in {}",
            fixture_dir().display()
        );
        return;
    }

    for triplet in &triplets {
        // Parse the CDN patch into its structured form
        let parsed = ZbsDiff::parse(&triplet.patch_data)
            .unwrap_or_else(|e| panic!("{}: ZbsDiff::parse failed: {e}", triplet.name));

        // Rebuild to bytes
        let rebuilt_bytes = parsed
            .build()
            .unwrap_or_else(|e| panic!("{}: ZbsDiff::build failed: {e}", triplet.name));

        // The rebuilt bytes should be identical to the original CDN patch.
        // ZbsDiff::parse stores the compressed blocks as-is, and build()
        // re-serializes them without re-compressing.
        assert_eq!(
            rebuilt_bytes,
            triplet.patch_data,
            "{}: rebuilt patch bytes differ from original CDN patch ({} vs {} bytes)",
            triplet.name,
            rebuilt_bytes.len(),
            triplet.patch_data.len(),
        );

        // Apply the rebuilt patch and verify the output matches .new
        let result =
            zbsdiff::apply_patch_memory(&triplet.old_data, &rebuilt_bytes).unwrap_or_else(|e| {
                panic!(
                    "{}: apply rebuilt patch failed: {e}\n  old={} bytes, rebuilt={} bytes",
                    triplet.name,
                    triplet.old_data.len(),
                    rebuilt_bytes.len(),
                )
            });

        assert_eq!(
            result,
            triplet.new_data,
            "{}: rebuilt patch output does not match expected new file ({} bytes)",
            triplet.name,
            triplet.new_data.len(),
        );

        eprintln!(
            "  {}: parse-rebuild-apply OK ({} bytes patch, {} -> {} bytes)",
            triplet.name,
            triplet.patch_data.len(),
            triplet.old_data.len(),
            triplet.new_data.len(),
        );
    }

    eprintln!(
        "Parse-rebuild-apply verified {} CDN triplets",
        triplets.len()
    );
}
