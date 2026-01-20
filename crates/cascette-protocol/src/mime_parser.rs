//! MIME parser for V1 protocol responses
//!
//! This module provides a new MIME parser implementation using the `mail_parser` crate
//! to properly handle V1 MIME responses with correct checksum validation.

use crate::error::{ProtocolError, Result};
use base64::Engine;
use cascette_formats::CascFormat;
use cascette_formats::bpsv::BpsvDocument;
use mail_parser::{HeaderValue, MessageParser, PartType};
use sha2::{Digest, Sha256};
use tracing::{debug, trace};

/// Parsed V1 MIME response data
#[derive(Debug)]
pub struct V1MimeResponse {
    /// The actual BPSV data content
    pub data: String,
    /// Optional signature data if present
    pub signature: Option<Vec<u8>>,
    /// Checksum from epilogue if present
    pub checksum: Option<String>,
}

/// Parse V1 MIME response using `mail_parser` crate
///
/// This function handles:
/// - MIME message parsing
/// - Data part extraction
/// - Signature part extraction (if present)
/// - Checksum validation from epilogue
///
/// # Arguments
/// * `raw_response` - Complete raw response bytes including any checksum epilogue
///
/// # Returns
/// Parsed V1 MIME response with data content
///
/// # Errors
/// Returns error if:
/// - MIME parsing fails
/// - Checksum validation fails
/// - No data content found
// NOTE: Complexity inherent to Battle.net V1 MIME protocol parsing with multi-part messages,
// signature verification, and multiple fallback extraction strategies.
// Future: Extract helpers for part extraction, signature handling, and fallback logic.
#[allow(clippy::cognitive_complexity)]
pub fn parse_v1_mime_response(raw_response: &[u8]) -> Result<V1MimeResponse> {
    debug!("Parsing V1 MIME response: {} bytes", raw_response.len());

    // Extract checksum from epilogue if present (following old implementation pattern)
    let (message_data, checksum) = extract_checksum(raw_response);
    debug!("Extracted checksum from V1 response: {:?}", checksum);

    // Validate checksum if present
    if let Some(ref expected_checksum) = checksum {
        validate_checksum(message_data, expected_checksum)?;
    }

    // Parse the MIME message (without checksum epilogue)
    let message = MessageParser::default()
        .parse(message_data)
        .ok_or_else(|| ProtocolError::Parse("Failed to parse MIME message".to_string()))?;

    trace!(
        "Parsed message - parts count: {}, text_body indices: {:?}",
        message.parts.len(),
        message.text_body
    );

    // Extract the main data part and signature
    let mut data_content = None;
    let mut signature_content = None;

    // Look for multipart content
    for (idx, part) in message.parts.iter().enumerate() {
        let headers_count = part.headers.len();
        trace!("Processing part {}: headers count = {}", idx, headers_count);

        // Debug headers
        for header in &part.headers {
            let value_str = match &header.value {
                HeaderValue::Text(t) => format!("Text: {t}"),
                HeaderValue::TextList(list) => format!("TextList: {list:?}"),
                HeaderValue::ContentType(ct) => format!("ContentType: {ct:?}"),
                _ => format!("Other: {:?}", header.value),
            };
            let name = &header.name;
            trace!("  Header: {} = {}", name, value_str);
        }

        // Check Content-Disposition header
        let disposition = part
            .headers
            .iter()
            .find(|h| {
                let name = h.name.as_str();
                name == "Content-Disposition" || name.to_lowercase() == "content-disposition"
            })
            .map(|h| match &h.value {
                HeaderValue::ContentType(ct) => ct.c_type.as_ref(),
                HeaderValue::Text(t) => t.as_ref(),
                _ => "",
            })
            .unwrap_or_default();

        trace!("Part {} disposition: '{}'", idx, disposition);

        // Get the text content for data parts (following old implementation logic)
        if disposition.contains("version")
            || disposition.contains("cdns")
            || disposition.contains("bgdl")
            || disposition.contains("cert")
            || disposition.contains("ocsp")
            || disposition.contains("summary")
        {
            if let PartType::Text(text) = &part.body {
                data_content = Some(text.as_ref().to_string());
            }
        } else if disposition.contains("signature") {
            // Get content for signature - it might be text or binary
            match &part.body {
                PartType::Binary(binary) => {
                    signature_content = Some(binary.as_ref().to_vec());
                }
                PartType::Text(text) => {
                    // The signature is likely base64 encoded
                    let text_str = text.as_ref().trim();
                    // Try to decode base64
                    match base64::engine::general_purpose::STANDARD.decode(text_str) {
                        Ok(decoded) => signature_content = Some(decoded),
                        Err(_) => {
                            // Try as raw bytes if not base64
                            signature_content = Some(text.as_bytes().to_vec());
                        }
                    }
                }
                _ => {}
            }
        }
    }

    // If no multipart content found, try to extract from message body
    if data_content.is_none() {
        // Check if we have text body indices
        if !message.text_body.is_empty() {
            // Try to get text from the first text body index
            if let Some(text) = message.body_text(0) {
                data_content = Some(text.to_string());
            }
        }

        // If still no content, extract from raw message
        if data_content.is_none() {
            let raw_msg = message.raw_message.as_ref();
            // Find the double CRLF that separates headers from body
            if let Some(body_start) = raw_msg.windows(4).position(|w| w == b"\r\n\r\n") {
                let body_bytes = &raw_msg[body_start + 4..];
                let body_text = String::from_utf8_lossy(body_bytes);
                // Trim any trailing whitespace
                data_content = Some(body_text.trim_end().to_string());
            }
        }
    }

    let data = data_content.ok_or_else(|| {
        ProtocolError::Parse("No data content found in MIME response".to_string())
    })?;

    debug!("Successfully extracted data content: {} bytes", data.len());
    if let Some(ref sig) = signature_content {
        debug!("Successfully extracted signature: {} bytes", sig.len());
    }

    Ok(V1MimeResponse {
        data,
        signature: signature_content,
        checksum,
    })
}

/// Extract checksum from the epilogue of a V1 response
///
/// Based on the old implementation pattern from ribbit-client
// NOTE: Complexity from byte-level parsing with multiple boundary conditions.
// Future: Extract helper for boundary detection and checksum parsing.
#[allow(clippy::cognitive_complexity)]
fn extract_checksum(raw: &[u8]) -> (&[u8], Option<String>) {
    const CHECKSUM_PREFIX: &[u8] = b"Checksum: ";

    // Look for the last occurrence of "Checksum: " in the data
    if let Some(checksum_pos) = raw
        .windows(CHECKSUM_PREFIX.len())
        .rposition(|window| window == CHECKSUM_PREFIX)
    {
        trace!("Found checksum at position {}", checksum_pos);
        // Found "Checksum: " - extract the rest of the line
        let checksum_line_start = checksum_pos;

        // Find the end of the line (newline character)
        let checksum_line_end = raw[checksum_line_start..]
            .iter()
            .position(|&b| b == b'\n')
            .map_or(raw.len(), |pos| checksum_line_start + pos + 1);

        // Extract just the hex part (after "Checksum: " and before newline)
        let hex_start = checksum_pos + CHECKSUM_PREFIX.len();
        let mut hex_end = if checksum_line_end > 0 && raw[checksum_line_end - 1] == b'\n' {
            checksum_line_end - 1
        } else {
            checksum_line_end
        };

        // Also strip carriage return if present
        if hex_end > 0 && raw[hex_end - 1] == b'\r' {
            hex_end -= 1;
        }

        if hex_start < hex_end {
            let checksum = String::from_utf8_lossy(&raw[hex_start..hex_end]).to_string();
            // Validate it's a proper hex string (SHA-256 should be 64 chars)
            if checksum.len() == 64 && checksum.chars().all(|c| c.is_ascii_hexdigit()) {
                trace!("Valid checksum found: {}", checksum);
                // Return the message without the checksum line
                let message_bytes = &raw[..checksum_line_start];
                return (message_bytes, Some(checksum));
            }
            let len = checksum.len();
            trace!(
                "Invalid checksum format - length: {}, content: {:?}",
                len, checksum
            );
        }
    }

    // No valid checksum found
    let len = raw.len();
    trace!("No checksum found in {} bytes of data", len);
    (raw, None)
}

/// Validate the SHA-256 checksum of the message
///
/// Following the pattern from the old implementation
fn validate_checksum(message_bytes: &[u8], expected_checksum: &str) -> Result<()> {
    let mut hasher = Sha256::new();
    hasher.update(message_bytes);
    let computed = hasher.finalize();
    let computed_hex = format!("{computed:x}");

    if computed_hex != expected_checksum {
        return Err(ProtocolError::Parse(format!(
            "Checksum validation failed: expected '{expected_checksum}', got '{computed_hex}'"
        )));
    }

    debug!("Checksum validation successful");
    Ok(())
}

/// Parse V1 MIME response and convert to BPSV document
///
/// This is the main entry point for parsing V1 responses that combines
/// MIME parsing with BPSV document creation.
pub fn parse_v1_mime_to_bpsv(raw_response: &[u8]) -> Result<BpsvDocument> {
    let v1_response = parse_v1_mime_response(raw_response)?;

    // Parse the data content as BPSV
    BpsvDocument::parse(v1_response.data.as_bytes())
        .map_err(|e| ProtocolError::Parse(format!("BPSV parse error: {e}")))
}

/// Detect if raw response is V1 MIME format
///
/// Quick check to determine response format
pub fn is_v1_mime_response(raw_response: &[u8]) -> bool {
    let response_str = String::from_utf8_lossy(raw_response);
    let first_512 = if response_str.len() > 512 {
        &response_str[..512]
    } else {
        &response_str
    };

    // Look for MIME headers indicating multipart content
    first_512.to_lowercase().contains("content-type:")
        && (first_512.to_lowercase().contains("multipart/alternative")
            || first_512.to_lowercase().contains("multipart/mixed"))
}

#[cfg(test)]
#[allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::uninlined_format_args
)]
mod tests {
    use super::*;
    use sha2::{Digest, Sha256};

    #[test]
    fn test_extract_checksum() {
        // Test with valid checksum
        let data_with_checksum = b"Some MIME data here\nChecksum: 1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef\n";
        let (message, checksum) = extract_checksum(data_with_checksum);

        assert_eq!(message, b"Some MIME data here\n");
        assert_eq!(
            checksum,
            Some("1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef".to_string())
        );

        // Test without checksum
        let data_no_checksum = b"Just some data";
        let (message, checksum) = extract_checksum(data_no_checksum);

        assert_eq!(message, data_no_checksum);
        assert!(checksum.is_none());
    }

    #[test]
    fn test_validate_checksum() {
        // Test data
        let message = b"test message";

        // Compute expected checksum
        let mut hasher = Sha256::new();
        hasher.update(message);
        let expected = format!("{:x}", hasher.finalize());

        // Should succeed with correct checksum
        assert!(validate_checksum(message, &expected).is_ok());

        // Should fail with incorrect checksum
        let wrong_checksum = "0000000000000000000000000000000000000000000000000000000000000000";
        assert!(validate_checksum(message, wrong_checksum).is_err());
    }

    #[test]
    fn test_is_v1_mime_response() {
        let v1_response = b"Content-Type: multipart/alternative; boundary=boundary123\r\n\r\n";
        assert!(is_v1_mime_response(v1_response));

        let v2_response =
            b"Region!STRING:0|BuildConfig!HEX:16|CDNConfig!HEX:16\r\nus|abc123|def456\r\n";
        assert!(!is_v1_mime_response(v2_response));
    }

    #[test]
    fn test_parse_simple_mime() {
        // Create a simple MIME message
        let mime_data = concat!(
            "Content-Type: text/plain\r\n",
            "From: Test\r\n",
            "\r\n",
            "Region!STRING:0|BuildConfig!HEX:16\r\n",
            "us|abcdef1234567890\r\n"
        )
        .as_bytes();

        let response = parse_v1_mime_response(mime_data).expect("Operation should succeed");
        assert!(response.data.contains("Region!STRING:0"));
    }

    #[test]
    fn test_parse_mime_with_checksum() {
        // Create MIME with checksum
        let mime_data =
            concat!("Content-Type: text/plain\r\n", "\r\n", "test data\r\n",).as_bytes();

        // Add checksum
        let mut data_with_checksum = mime_data.to_vec();

        // Calculate real checksum
        let mut hasher = Sha256::new();
        hasher.update(&data_with_checksum);
        let checksum = format!("Checksum: {:x}\n", hasher.finalize());
        data_with_checksum.extend_from_slice(checksum.as_bytes());

        let response =
            parse_v1_mime_response(&data_with_checksum).expect("Operation should succeed");
        assert!(response.checksum.is_some());
    }

    /// Test checksum extraction working correctly
    #[test]
    fn test_debug_checksum() {
        // Create message that includes the newline since that's what will be in the extracted message
        let message = b"simple test message\n";

        // Calculate correct checksum for the message with newline
        let mut hasher = Sha256::new();
        hasher.update(message);
        let correct_checksum = format!("{:x}", hasher.finalize());

        // Create response with checksum - no extra newline needed since message already has it
        let mut response = message.to_vec();
        response.extend_from_slice(b"Checksum: ");
        response.extend_from_slice(correct_checksum.as_bytes());
        response.extend_from_slice(b"\n");

        // Extract and validate
        let (extracted_message, extracted_checksum) = extract_checksum(&response);
        assert_eq!(extracted_checksum, Some(correct_checksum.clone()));
        assert_eq!(extracted_message, message);

        // Validate checksum function should work
        assert!(validate_checksum(extracted_message, &correct_checksum).is_ok());
    }

    /// Test to verify checksum validation works like old implementation
    #[test]
    fn test_checksum_validation_compatibility() {
        // Create a proper MIME message
        let mime_content = b"MIME-Version: 1.0\r\nContent-Type: text/plain\r\n\r\nRegion!STRING:0|BuildConfig!HEX:16\r\nus|abcdef1234567890\n";

        // Calculate checksum for the MIME content (including the final newline)
        let mut hasher = Sha256::new();
        hasher.update(mime_content);
        let expected_checksum = format!("{:x}", hasher.finalize());

        // Create full response
        let mut full_response = mime_content.to_vec();
        full_response.extend_from_slice(b"Checksum: ");
        full_response.extend_from_slice(expected_checksum.as_bytes());
        full_response.extend_from_slice(b"\n");

        // Should parse successfully
        let result = parse_v1_mime_response(&full_response);
        assert!(
            result.is_ok(),
            "Should parse MIME with valid checksum: {:?}",
            result.err()
        );

        let parsed = result.expect("Operation should succeed");
        assert!(parsed.checksum.is_some());
        assert_eq!(
            parsed.checksum.expect("Operation should succeed"),
            expected_checksum
        );
        assert!(parsed.data.contains("Region!STRING:0"));
    }

    /// Test the old error case to ensure it's fixed
    #[test]
    fn test_checksum_mismatch_error() {
        // Create test data with an intentionally wrong checksum
        let mime_content = b"MIME-Version: 1.0\r\nContent-Type: text/plain\r\n\r\ntest data";

        // Use a wrong checksum that would cause validation to fail
        let wrong_checksum = "0000000000000000000000000000000000000000000000000000000000000000";

        // Create the full response with wrong checksum
        let mut full_response = mime_content.to_vec();
        full_response.extend_from_slice(b"\nChecksum: ");
        full_response.extend_from_slice(wrong_checksum.as_bytes());
        full_response.extend_from_slice(b"\n");

        // This should fail with checksum validation error
        let result = parse_v1_mime_response(&full_response);
        assert!(result.is_err(), "Should fail with invalid checksum");

        let error = result.expect_err("Test operation should fail");
        assert!(
            error.to_string().contains("Checksum validation failed"),
            "Error should indicate checksum validation failure: {}",
            error
        );
    }
}
