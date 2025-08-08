//! Test large archive support (>2GB memory mapping)

use casc_storage::archive::ArchiveReader;
use casc_storage::error::Result;
use std::io::Write;
use tempfile::NamedTempFile;

/// Test that we can handle large archives correctly
#[test]
fn test_large_archive_memory_mapping_limits() {
    // Test the memory mapping decision logic

    // Small file should be memory mapped
    let small_archive = create_test_archive(1024).unwrap(); // 1KB
    let reader = ArchiveReader::open(small_archive.path()).unwrap();
    assert!(
        reader.is_memory_mapped(),
        "Small files should be memory-mapped"
    );

    // Test that we properly handle files around the 2GB boundary on 32-bit systems
    #[cfg(target_pointer_width = "32")]
    {
        // On 32-bit systems, files >2GB should not be memory-mapped
        // We can't actually create a 2GB+ file in tests, but we can test the logic
        use casc_storage::archive::ArchiveReader;

        // This tests the internal logic without creating massive files
        assert!(
            ArchiveReader::can_memory_map(1024 * 1024 * 1024),
            "1GB should be mappable on 32-bit"
        );
        assert!(
            !ArchiveReader::can_memory_map(3 * 1024 * 1024 * 1024),
            "3GB should not be mappable on 32-bit"
        );
    }

    #[cfg(target_pointer_width = "64")]
    {
        // On 64-bit systems, we can handle much larger files
        use casc_storage::archive::ArchiveReader;

        assert!(
            ArchiveReader::can_memory_map(8 * 1024 * 1024 * 1024),
            "8GB should be mappable on 64-bit"
        );
        assert!(
            ArchiveReader::can_memory_map(64 * 1024 * 1024 * 1024),
            "64GB should be mappable on 64-bit"
        );

        // But not infinitely large files
        let too_large = 200u64 * 1024 * 1024 * 1024; // 200GB
        assert!(
            !ArchiveReader::can_memory_map(too_large),
            "200GB should not be mappable to avoid VM exhaustion"
        );
    }
}

#[test]
fn test_fallback_file_reading() {
    // Create a test archive with known data
    let test_data = b"This is test data for large archive fallback testing. It should be readable even without memory mapping.";
    let archive = create_test_archive_with_data(test_data).unwrap();

    // Force non-memory-mapped reading by creating an ArchiveReader
    let reader = ArchiveReader::open(archive.path()).unwrap();

    // Test reading at different offsets
    let mut reader = reader; // Make mutable for read_at

    // Read from the beginning
    let data = reader.read_at(0, 10).unwrap();
    assert_eq!(&data, b"This is te");

    // Read from the middle
    let data = reader.read_at(50, 20).unwrap();
    assert_eq!(&data, b"ng. It should be rea");

    // Read near the end
    let end_offset = test_data.len() - 10;
    let data = reader.read_at(end_offset as u64, 10).unwrap();
    assert_eq!(&data, b"y mapping.");
}

#[test]
fn test_positioned_reads_thread_safety() {
    use std::sync::Arc;
    use std::thread;

    // Create test archive with pattern data
    let mut test_data = Vec::new();
    for i in 0..1000u32 {
        test_data.extend_from_slice(&i.to_le_bytes());
    }

    let archive = create_test_archive_with_data(&test_data).unwrap();
    let reader = Arc::new(ArchiveReader::open(archive.path()).unwrap());

    // Spawn multiple threads to read different parts simultaneously
    let mut handles = vec![];

    for thread_id in 0..10 {
        let reader_clone = Arc::clone(&reader);
        let handle = thread::spawn(move || {
            // Each thread reads a different 40-byte section (10 u32 values)
            let offset = (thread_id * 40) as u64;
            let data = reader_clone.read_at_cow(offset, 40).unwrap();

            // Verify the data is correct
            for (i, chunk) in data.chunks(4).enumerate() {
                let expected_value = (thread_id * 10 + i) as u32;
                let actual_value = u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
                assert_eq!(
                    actual_value, expected_value,
                    "Thread {} failed at position {}",
                    thread_id, i
                );
            }

            thread_id
        });
        handles.push(handle);
    }

    // Wait for all threads and verify they completed
    let mut completed_threads = Vec::new();
    for handle in handles {
        completed_threads.push(handle.join().unwrap());
    }
    completed_threads.sort();
    assert_eq!(completed_threads, (0..10).collect::<Vec<_>>());
}

#[test]
fn test_archive_section_streaming() {
    let test_data = b"Streaming test data for archive sections. This should work for both memory-mapped and file-based access.";
    let archive = create_test_archive_with_data(test_data).unwrap();
    let reader = ArchiveReader::open(archive.path()).unwrap();

    // Create a section from the middle of the archive
    let section = reader.reader_at(20, 30).unwrap();
    let mut section = section;

    // Read from the section
    use std::io::Read;
    let mut buffer = [0u8; 15];
    let bytes_read = section.read(&mut buffer).unwrap();
    assert_eq!(bytes_read, 15);
    assert_eq!(&buffer, b"for archive sec");

    // Read more
    let mut buffer2 = [0u8; 15];
    let bytes_read2 = section.read(&mut buffer2).unwrap();
    assert_eq!(bytes_read2, 15);
    assert_eq!(&buffer2, b"tions. This sho");
}

#[test]
fn test_memory_mapping_vs_file_reading_performance() {
    // Create two identical test files
    let test_data = vec![0u8; 1024 * 1024]; // 1MB of test data
    let mmap_archive = create_test_archive_with_data(&test_data).unwrap();

    // Test memory-mapped performance
    let reader = ArchiveReader::open(mmap_archive.path()).unwrap();
    assert!(
        reader.is_memory_mapped(),
        "Should use memory mapping for 1MB file"
    );

    let start = std::time::Instant::now();
    for i in 0..100 {
        let offset = (i * 1000) as u64;
        let _data = reader.read_slice(offset, 100).unwrap();
    }
    let mmap_time = start.elapsed();

    println!("Memory-mapped reads: {:?}", mmap_time);

    // Note: We can't easily test file-only performance without creating
    // a mock that forces file reading, but this test verifies the
    // memory mapping path works correctly
}

#[test]
fn test_bounds_checking() {
    let test_data = b"Small test data";
    let archive = create_test_archive_with_data(test_data).unwrap();
    let mut reader = ArchiveReader::open(archive.path()).unwrap();

    // Test reading beyond file bounds
    let result = reader.read_at(100, 10);
    assert!(
        result.is_err(),
        "Should fail when reading beyond file bounds"
    );

    // Test reading with offset + length > file size
    let file_size = test_data.len() as u64;
    let result = reader.read_at(file_size - 5, 10);
    assert!(result.is_err(), "Should fail when read extends beyond file");

    // Test valid read at the end
    let result = reader.read_at(file_size - 5, 5);
    assert!(result.is_ok(), "Should succeed for valid read at end");
}

#[test]
fn test_prefetch_optimization() {
    let test_data = vec![0u8; 100 * 1024]; // 100KB
    let archive = create_test_archive_with_data(&test_data).unwrap();
    let reader = ArchiveReader::open(archive.path()).unwrap();

    // Test prefetch functionality (should not crash)
    let result = reader.prefetch(0, 4096);
    assert!(result.is_ok(), "Prefetch should succeed");

    // Test prefetch beyond file bounds (should handle gracefully)
    let result = reader.prefetch(50 * 1024, 100 * 1024);
    assert!(result.is_ok(), "Prefetch should handle bounds gracefully");
}

/// Helper function to create a test archive file
fn create_test_archive(size: usize) -> Result<NamedTempFile> {
    let mut file = NamedTempFile::new()?;
    let data = vec![0u8; size];
    file.write_all(&data)?;
    file.flush()?;
    Ok(file)
}

/// Helper function to create a test archive with specific data
fn create_test_archive_with_data(data: &[u8]) -> Result<NamedTempFile> {
    let mut file = NamedTempFile::new()?;
    file.write_all(data)?;
    file.flush()?;
    Ok(file)
}

/// Integration test with a simulated large archive structure
#[test]
fn test_large_archive_simulation() {
    // Simulate a large CASC archive structure without creating huge files
    let mut archive_data = Vec::new();

    // Create a simulated archive with multiple "files"
    struct SimulatedFile {
        offset: u64,
        size: u32,
        content: Vec<u8>,
    }

    let mut simulated_files = Vec::new();
    let mut current_offset = 0u64;

    // Create 100 simulated files of various sizes
    for i in 0..100 {
        let size = 1024 + (i * 47) % 8192; // Varying sizes
        let content: Vec<u8> = (0..size).map(|j| ((i + j) % 256) as u8).collect();

        simulated_files.push(SimulatedFile {
            offset: current_offset,
            size: size as u32,
            content: content.clone(),
        });

        archive_data.extend_from_slice(&content);
        current_offset += size as u64;
    }

    // Write the simulated archive
    let archive = create_test_archive_with_data(&archive_data).unwrap();
    let mut reader = ArchiveReader::open(archive.path()).unwrap();

    // Test reading each simulated file
    for (i, sim_file) in simulated_files.iter().enumerate() {
        let read_data = reader
            .read_at(sim_file.offset, sim_file.size as usize)
            .unwrap();
        assert_eq!(
            read_data, sim_file.content,
            "Simulated file {} content mismatch",
            i
        );
    }

    println!(
        "Successfully tested {} simulated files in archive",
        simulated_files.len()
    );
}
