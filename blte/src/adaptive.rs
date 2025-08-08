//! Adaptive compression module for BLTE
//!
//! This module provides intelligent compression algorithm selection based on data characteristics.
//! It analyzes input data to determine the most efficient compression method.

use crate::{CompressionMode, Result};

/// Data characteristics used for compression selection
#[derive(Debug, Clone)]
pub struct DataAnalysis {
    /// Total size in bytes
    pub size: usize,
    /// Entropy score (0.0 = low entropy, 1.0 = high entropy)
    pub entropy: f64,
    /// Percentage of bytes that are zeros
    pub zero_ratio: f64,
    /// Percentage of bytes that repeat
    pub repetition_ratio: f64,
    /// Whether data appears to be text
    pub is_text: bool,
    /// Whether data appears to be already compressed
    pub is_compressed: bool,
    /// Detected file type if known
    pub file_type: Option<FileType>,
}

/// Known file types for specialized compression
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FileType {
    Text,
    Json,
    Xml,
    Binary,
    Image,
    Audio,
    Video,
    Archive,
    Executable,
}

/// Compression recommendation with rationale
#[derive(Debug, Clone)]
pub struct CompressionRecommendation {
    /// Recommended compression mode
    pub mode: CompressionMode,
    /// Recommended compression level (if applicable)
    pub level: Option<u8>,
    /// Expected compression ratio (0.0 = no compression, 1.0 = perfect compression)
    pub expected_ratio: f64,
    /// Reasoning for the recommendation
    pub rationale: String,
}

/// Analyze data characteristics for compression selection
pub fn analyze_data(data: &[u8]) -> DataAnalysis {
    let size = data.len();

    // Calculate entropy
    let entropy = calculate_entropy(data);

    // Calculate zero ratio
    let zero_count = data.iter().filter(|&&b| b == 0).count();
    let zero_ratio = zero_count as f64 / size as f64;

    // Calculate repetition ratio
    let repetition_ratio = calculate_repetition_ratio(data);

    // Check if data appears to be text
    let is_text = is_likely_text(data);

    // Check if data appears to be already compressed
    let is_compressed = entropy > 0.95 || is_likely_compressed(data);

    // Detect file type
    let file_type = detect_file_type(data);

    DataAnalysis {
        size,
        entropy,
        zero_ratio,
        repetition_ratio,
        is_text,
        is_compressed,
        file_type,
    }
}

/// Calculate Shannon entropy of data
fn calculate_entropy(data: &[u8]) -> f64 {
    if data.is_empty() {
        return 0.0;
    }

    let mut frequency = [0u64; 256];
    for &byte in data {
        frequency[byte as usize] += 1;
    }

    let len = data.len() as f64;
    let mut entropy = 0.0;

    for &count in &frequency {
        if count > 0 {
            let probability = count as f64 / len;
            entropy -= probability * probability.log2();
        }
    }

    // Normalize to 0.0 - 1.0 range (max entropy is 8 bits)
    entropy / 8.0
}

/// Calculate repetition ratio in data
fn calculate_repetition_ratio(data: &[u8]) -> f64 {
    if data.len() < 2 {
        return 0.0;
    }

    let mut runs = 0;
    let mut run_length = 0;
    let mut last_byte = data[0];

    for &byte in &data[1..] {
        if byte == last_byte {
            run_length += 1;
        } else {
            if run_length > 0 {
                runs += run_length;
            }
            run_length = 0;
            last_byte = byte;
        }
    }

    if run_length > 0 {
        runs += run_length;
    }

    runs as f64 / data.len() as f64
}

/// Check if data is likely text
fn is_likely_text(data: &[u8]) -> bool {
    if data.is_empty() {
        return false;
    }

    let sample_size = data.len().min(1024);
    let sample = &data[..sample_size];

    let text_chars = sample
        .iter()
        .filter(|&&b| b == b'\t' || b == b'\n' || b == b'\r' || (32..127).contains(&b))
        .count();

    text_chars as f64 / sample_size as f64 > 0.85
}

/// Check if data appears to be already compressed
fn is_likely_compressed(data: &[u8]) -> bool {
    if data.len() < 4 {
        return false;
    }

    // Check for common compressed file signatures
    let signatures = [
        &b"\x1f\x8b"[..],     // gzip
        &b"PK"[..],           // zip
        &b"\x42\x5a"[..],     // bzip2
        &b"\x37\x7a"[..],     // 7z
        &b"\x52\x61\x72"[..], // rar
        &b"\xfd\x37\x7a"[..], // xz
        &b"\x04\x22\x4d"[..], // lz4
        &b"BLTE"[..],         // BLTE
    ];

    for sig in &signatures {
        if data.starts_with(sig) {
            return true;
        }
    }

    false
}

/// Detect file type from magic bytes
fn detect_file_type(data: &[u8]) -> Option<FileType> {
    if data.len() < 4 {
        return None;
    }

    // Check common file signatures
    if data.starts_with(b"\x89PNG") {
        return Some(FileType::Image);
    }
    if data.starts_with(b"\xff\xd8\xff") {
        return Some(FileType::Image); // JPEG
    }
    if data.starts_with(b"GIF8") {
        return Some(FileType::Image);
    }
    if data.starts_with(b"BM") {
        return Some(FileType::Image); // BMP
    }
    if data.starts_with(b"BLP2") {
        return Some(FileType::Image); // Blizzard BLP
    }

    if data.starts_with(b"<?xml") || data.starts_with(b"<html") {
        return Some(FileType::Xml);
    }

    if data.starts_with(b"{\"") || data.starts_with(b"[{") {
        return Some(FileType::Json);
    }

    if data.starts_with(b"MZ") {
        return Some(FileType::Executable); // Windows PE
    }
    if data.starts_with(b"\x7fELF") {
        return Some(FileType::Executable); // Linux ELF
    }

    if data.starts_with(b"OggS") {
        return Some(FileType::Audio); // Ogg
    }
    if data.starts_with(b"ID3") || data.starts_with(b"\xff\xfb") {
        return Some(FileType::Audio); // MP3
    }

    if is_likely_text(data) {
        return Some(FileType::Text);
    }

    None
}

/// Select optimal compression mode based on data analysis
pub fn select_compression_mode(analysis: &DataAnalysis) -> CompressionRecommendation {
    // Very small files - no compression overhead worth it
    if analysis.size < 256 {
        return CompressionRecommendation {
            mode: CompressionMode::None,
            level: None,
            expected_ratio: 0.0,
            rationale: "File too small for compression to be beneficial".into(),
        };
    }

    // Already compressed data
    if analysis.is_compressed {
        return CompressionRecommendation {
            mode: CompressionMode::None,
            level: None,
            expected_ratio: 0.0,
            rationale: "Data appears to be already compressed".into(),
        };
    }

    // Check for specific file types
    if let Some(file_type) = analysis.file_type {
        match file_type {
            FileType::Image | FileType::Audio | FileType::Video | FileType::Archive => {
                return CompressionRecommendation {
                    mode: CompressionMode::None,
                    level: None,
                    expected_ratio: 0.0,
                    rationale: format!(
                        "Media file type ({file_type:?}) is typically already compressed"
                    ),
                };
            }
            FileType::Text | FileType::Json | FileType::Xml => {
                // Text files compress well with ZLib
                return CompressionRecommendation {
                    mode: CompressionMode::ZLib,
                    level: Some(6),
                    expected_ratio: 0.6,
                    rationale: "Text-based data compresses well with ZLib".into(),
                };
            }
            _ => {}
        }
    }

    // High zero ratio - excellent for compression
    if analysis.zero_ratio > 0.3 {
        return CompressionRecommendation {
            mode: CompressionMode::ZLib,
            level: Some(9),
            expected_ratio: 0.8,
            rationale: format!(
                "High zero ratio ({:.1}%) indicates excellent compression potential",
                analysis.zero_ratio * 100.0
            ),
        };
    }

    // High repetition - good for LZ4
    if analysis.repetition_ratio > 0.2 {
        return CompressionRecommendation {
            mode: CompressionMode::LZ4,
            level: None,
            expected_ratio: 0.5,
            rationale: format!(
                "High repetition ratio ({:.1}%) suits LZ4 compression",
                analysis.repetition_ratio * 100.0
            ),
        };
    }

    // Low entropy - compressible
    if analysis.entropy < 0.7 {
        // For larger files with low entropy, use ZLib for better ratio
        if analysis.size > 10_000 {
            return CompressionRecommendation {
                mode: CompressionMode::ZLib,
                level: Some(6),
                expected_ratio: 0.6,
                rationale: format!(
                    "Low entropy ({:.2}) indicates good compression potential",
                    analysis.entropy
                ),
            };
        } else {
            // For smaller files, use LZ4 for speed
            return CompressionRecommendation {
                mode: CompressionMode::LZ4,
                level: None,
                expected_ratio: 0.4,
                rationale: format!(
                    "Low entropy ({:.2}) with small size favors fast LZ4",
                    analysis.entropy
                ),
            };
        }
    }

    // Medium entropy - use LZ4 for balance
    if analysis.entropy < 0.85 {
        return CompressionRecommendation {
            mode: CompressionMode::LZ4,
            level: None,
            expected_ratio: 0.3,
            rationale: format!(
                "Medium entropy ({:.2}) - LZ4 provides good speed/ratio balance",
                analysis.entropy
            ),
        };
    }

    // High entropy - compression unlikely to help
    CompressionRecommendation {
        mode: CompressionMode::None,
        level: None,
        expected_ratio: 0.0,
        rationale: format!(
            "High entropy ({:.2}) indicates poor compression potential",
            analysis.entropy
        ),
    }
}

/// Automatically compress data with optimal settings (returns full BLTE file)
pub fn auto_compress(data: &[u8]) -> Result<Vec<u8>> {
    let analysis = analyze_data(data);
    let recommendation = select_compression_mode(&analysis);

    // Use compress_data_single to create a full BLTE file
    crate::compress::compress_data_single(data.to_vec(), recommendation.mode, recommendation.level)
}

/// Test multiple compression modes and return the best result (returns full BLTE file)
pub fn compress_with_best_ratio(data: &[u8]) -> Result<(Vec<u8>, CompressionMode)> {
    let mut best_result =
        crate::compress::compress_data_single(data.to_vec(), CompressionMode::None, None)?;
    let mut best_mode = CompressionMode::None;
    let mut best_size = best_result.len();

    // Try ZLib with different levels
    for level in [1, 6, 9] {
        if let Ok(compressed) =
            crate::compress::compress_data_single(data.to_vec(), CompressionMode::ZLib, Some(level))
        {
            if compressed.len() < best_size {
                best_size = compressed.len();
                best_result = compressed;
                best_mode = CompressionMode::ZLib;
            }
        }
    }

    // Try LZ4
    if let Ok(compressed) =
        crate::compress::compress_data_single(data.to_vec(), CompressionMode::LZ4, None)
    {
        if compressed.len() < best_size {
            best_result = compressed;
            best_mode = CompressionMode::LZ4;
        }
    }

    Ok((best_result, best_mode))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_entropy_calculation() {
        // All zeros - minimum entropy
        let data = vec![0u8; 1000];
        let entropy = calculate_entropy(&data);
        assert!(entropy < 0.01);

        // Random data - high entropy
        let data: Vec<u8> = (0..1000).map(|i| (i % 256) as u8).collect();
        let entropy = calculate_entropy(&data);
        assert!(entropy > 0.9);

        // Repeating pattern - medium entropy
        let data = b"ABCDABCDABCDABCD".repeat(100);
        let entropy = calculate_entropy(&data);
        assert!(entropy > 0.2 && entropy < 0.5);
    }

    #[test]
    fn test_repetition_detection() {
        // High repetition
        let data = vec![b'A'; 100];
        let ratio = calculate_repetition_ratio(&data);
        assert!(ratio > 0.95);

        // No repetition
        let data: Vec<u8> = (0..100).map(|i| i as u8).collect();
        let ratio = calculate_repetition_ratio(&data);
        assert!(ratio < 0.05);

        // Some repetition
        let data = b"AABBCCDD".repeat(10);
        let ratio = calculate_repetition_ratio(&data);
        assert!(ratio > 0.4);
    }

    #[test]
    fn test_text_detection() {
        assert!(is_likely_text(b"Hello, World! This is a test."));
        assert!(is_likely_text(b"fn main() {\n    println!(\"Hello\");\n}"));
        assert!(!is_likely_text(&[0, 1, 2, 3, 4, 5, 6, 7, 8, 9]));
        assert!(!is_likely_text(&[200u8; 100]));
    }

    #[test]
    fn test_compression_selection() {
        // Small file
        let analysis = DataAnalysis {
            size: 100,
            entropy: 0.5,
            zero_ratio: 0.1,
            repetition_ratio: 0.1,
            is_text: false,
            is_compressed: false,
            file_type: None,
        };
        let rec = select_compression_mode(&analysis);
        assert_eq!(rec.mode, CompressionMode::None);

        // Text file
        let analysis = DataAnalysis {
            size: 10000,
            entropy: 0.6,
            zero_ratio: 0.05,
            repetition_ratio: 0.1,
            is_text: true,
            is_compressed: false,
            file_type: Some(FileType::Text),
        };
        let rec = select_compression_mode(&analysis);
        assert_eq!(rec.mode, CompressionMode::ZLib);

        // High zero ratio
        let analysis = DataAnalysis {
            size: 10000,
            entropy: 0.3,
            zero_ratio: 0.5,
            repetition_ratio: 0.1,
            is_text: false,
            is_compressed: false,
            file_type: None,
        };
        let rec = select_compression_mode(&analysis);
        assert_eq!(rec.mode, CompressionMode::ZLib);
        assert_eq!(rec.level, Some(9));

        // Already compressed
        let analysis = DataAnalysis {
            size: 10000,
            entropy: 0.98,
            zero_ratio: 0.01,
            repetition_ratio: 0.01,
            is_text: false,
            is_compressed: true,
            file_type: None,
        };
        let rec = select_compression_mode(&analysis);
        assert_eq!(rec.mode, CompressionMode::None);
    }

    #[test]
    fn test_auto_compress() {
        // Text data should compress
        let data = b"This is a test string that should compress well. ".repeat(100);
        let compressed = auto_compress(&data).unwrap();
        assert!(compressed.len() < data.len());

        // Random data shouldn't compress much
        let data: Vec<u8> = (0..1000).map(|i| (i % 256) as u8).collect();
        let compressed = auto_compress(&data).unwrap();
        // BLTE files always start with "BLTE" magic
        assert_eq!(&compressed[0..4], b"BLTE");
        // After decompressing, verify it used no compression (high entropy data)
        // The compression mode byte would be after the header
        // For a single-chunk BLTE with header size 0, mode byte is at position 8
        assert_eq!(compressed[8], b'N');
    }
}
