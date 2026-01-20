//! V1 MIME protocol support for Ribbit client
//!
//! This module provides parsing and verification capabilities for V1 MIME responses
//! from Ribbit protocol endpoints. V1 responses use `multipart/alternative` MIME
//! format with PKCS#7 signature verification.
//!
//! ## V1 MIME Format
//!
//! V1 responses contain:
//! - **Data part**: The actual PSV response content
//! - **Signature part**: PKCS#7/CMS detached signature
//! - **Checksum epilogue**: Optional MD5 checksum at the end
//!
//! ## Usage
//!
//! ```rust,no_run
//! use cascette_protocol::v1_mime::parse_v1_mime_response;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let raw_response = b"Content-Type: multipart/alternative...";
//! let parsed = parse_v1_mime_response(raw_response, Some(raw_response))?;
//!
//! println!("Data: {}", parsed.data);
//! if let Some(sig_info) = &parsed.signature_info {
//!     println!("Signature verified: {}", sig_info.verification.is_valid);
//! }
//! # Ok(())
//! # }
//! ```

pub mod certificate;
pub mod signature;
pub mod types;

use crate::error::{ProtocolError, Result};
use crate::v1_mime::signature::parse_and_verify_signature;
use crate::v1_mime::types::V1MimeResponse;
use base64::Engine;
use mail_parser::{HeaderValue, MessageParser, PartType};
use tracing::{debug, trace, warn};

/// Parse a V1 MIME response with signature verification
///
/// # Arguments
/// * `raw_response` - The complete raw response bytes
/// * `signed_data` - The data that was signed (for detached signature verification)
///
/// # Returns
/// Returns parsed V1 MIME response with signature information
///
/// # Errors
/// Returns an error if:
/// - MIME parsing fails
/// - Signature verification encounters an error (but not verification failure)
/// - Required data parts are missing
// NOTE: Complexity from MIME multi-part parsing with signature and certificate handling.
// Future: Extract helpers for part extraction and verification workflows.
#[allow(clippy::cognitive_complexity)]
pub fn parse_v1_mime_response(
    raw_response: &[u8],
    signed_data: Option<&[u8]>,
) -> Result<V1MimeResponse> {
    debug!("Parsing V1 MIME response: {} bytes", raw_response.len());

    // Extract checksum from epilogue if present
    let (message_data, checksum) = extract_checksum_epilogue(raw_response);
    debug!("Checksum from epilogue: {:?}", checksum);

    // Validate checksum if present
    if let Some(ref expected_checksum) = checksum {
        validate_checksum(message_data, expected_checksum)?;
    }

    // Parse MIME message
    let message = MessageParser::default()
        .parse(message_data)
        .ok_or_else(|| ProtocolError::Parse("Failed to parse MIME message".to_string()))?;

    trace!(
        "Parsed MIME message: {} parts, {} text bodies",
        message.parts.len(),
        message.text_body.len()
    );

    // Extract data and signature parts
    let (data_content, signature_bytes) = extract_mime_parts(&message)?;

    // Parse and verify signature if present
    let signature_info = if let Some(sig_bytes) = signature_bytes {
        // Use the message data without checksum for signature verification
        let verification_data = signed_data.unwrap_or(message_data);

        match parse_and_verify_signature(&sig_bytes, Some(verification_data)) {
            Ok(sig_info) => {
                debug!(
                    "Signature parsing successful: {} signers, {} certificates",
                    sig_info.signer_count, sig_info.certificate_count
                );
                Some(sig_info)
            }
            Err(e) => {
                warn!("Signature parsing/verification failed: {}", e);
                // Continue without signature info rather than failing completely
                None
            }
        }
    } else {
        debug!("No signature found in MIME response");
        None
    };

    Ok(V1MimeResponse {
        raw: raw_response.to_vec(),
        data: data_content,
        signature_info,
        checksum,
    })
}

/// Extract data and signature parts from parsed MIME message
// NOTE: Complexity from iterating parts with header matching and content extraction.
// Future: Extract helpers for disposition checking and content type handling.
#[allow(clippy::cognitive_complexity)]
fn extract_mime_parts(message: &mail_parser::Message) -> Result<(String, Option<Vec<u8>>)> {
    let mut data_content = None;
    let mut signature_bytes = None;

    // Process each MIME part
    for (i, part) in message.parts.iter().enumerate() {
        trace!("Processing MIME part {}: {} headers", i, part.headers.len());

        // Find Content-Disposition header
        let disposition = find_content_disposition(&part.headers);
        trace!("Part {} Content-Disposition: '{}'", i, disposition);

        // Classify part based on Content-Disposition
        if is_data_part(&disposition) {
            data_content = extract_text_content(part);
            debug!(
                "Extracted data content: {} bytes",
                data_content.as_ref().map_or(0, String::len)
            );
        } else if is_signature_part(&disposition) {
            signature_bytes = extract_signature_content(part);
            debug!(
                "Extracted signature: {} bytes",
                signature_bytes.as_ref().map_or(0, Vec::len)
            );
        }
    }

    // If no multipart content found, try to extract from message body
    if data_content.is_none() {
        data_content = extract_fallback_content(message);
        if data_content.is_some() {
            debug!("Extracted fallback content from message body");
        }
    }

    let data = data_content.ok_or_else(|| {
        ProtocolError::Parse("No data content found in MIME response".to_string())
    })?;

    Ok((data, signature_bytes))
}

/// Find Content-Disposition header value
fn find_content_disposition(headers: &[mail_parser::Header]) -> String {
    for header in headers {
        let name_lower = header.name.as_str().to_lowercase();
        if name_lower == "content-disposition" {
            return match &header.value {
                HeaderValue::ContentType(ct) => ct.c_type.to_string(),
                HeaderValue::Text(t) => t.to_string(),
                HeaderValue::TextList(list) => list.join("; "),
                _ => String::new(),
            };
        }
    }
    String::new()
}

/// Check if a Content-Disposition indicates a data part
fn is_data_part(disposition: &str) -> bool {
    let disposition_lower = disposition.to_lowercase();
    disposition_lower.contains("version")
        || disposition_lower.contains("cdns")
        || disposition_lower.contains("bgdl")
        || disposition_lower.contains("summary")
        || disposition_lower.contains("cert")
        || disposition_lower.contains("ocsp")
}

/// Check if a Content-Disposition indicates a signature part
fn is_signature_part(disposition: &str) -> bool {
    disposition.to_lowercase().contains("signature")
}

/// Extract text content from a MIME part
fn extract_text_content(part: &mail_parser::MessagePart) -> Option<String> {
    match &part.body {
        PartType::Text(text) => {
            let content = text.as_ref().trim().to_string();
            Some(content)
        }
        // Binary and other parts are not text content
        _ => None,
    }
}

/// Extract signature content from a MIME part
fn extract_signature_content(part: &mail_parser::MessagePart) -> Option<Vec<u8>> {
    match &part.body {
        PartType::Binary(binary) => Some(binary.as_ref().to_vec()),
        PartType::Text(text) => {
            // Signature might be base64 encoded text
            let text_str = text.as_ref().trim();

            // Try base64 decoding
            match base64::engine::general_purpose::STANDARD.decode(text_str) {
                Ok(decoded) => Some(decoded),
                Err(_) => {
                    // Not base64, treat as raw bytes
                    Some(text_str.as_bytes().to_vec())
                }
            }
        }
        _ => None,
    }
}

/// Extract content from message body as fallback
fn extract_fallback_content(message: &mail_parser::Message) -> Option<String> {
    // Try text body indices first
    if !message.text_body.is_empty()
        && let Some(text) = message.body_text(0)
    {
        return Some(text.to_string());
    }

    // Extract from raw message after headers
    let raw_msg = message.raw_message.as_ref();
    if let Some(body_start) = find_body_start(raw_msg) {
        let body_bytes = &raw_msg[body_start..];
        let body_text = String::from_utf8_lossy(body_bytes);
        return Some(body_text.trim_end().to_string());
    }

    None
}

/// Find the start of the message body after headers
fn find_body_start(raw_message: &[u8]) -> Option<usize> {
    // Look for double CRLF that separates headers from body
    raw_message
        .windows(4)
        .position(|w| w == b"\r\n\r\n")
        .map(|pos| pos + 4)
}

/// Extract checksum from response epilogue
///
/// V1 responses may have a checksum appended after the MIME message:
/// ```text
/// <MIME message>
/// Checksum: <hex_checksum>
/// ```
fn extract_checksum_epilogue(raw_response: &[u8]) -> (&[u8], Option<String>) {
    const CHECKSUM_PREFIX: &[u8] = b"Checksum: ";

    // Look for the last occurrence of "Checksum: "
    if let Some(checksum_start) = raw_response
        .windows(CHECKSUM_PREFIX.len())
        .rposition(|w| w == CHECKSUM_PREFIX)
    {
        let checksum_data_start = checksum_start + CHECKSUM_PREFIX.len();

        // Find the end of the checksum (newline or end of data)
        let checksum_end = raw_response[checksum_data_start..]
            .iter()
            .position(|&b| b == b'\r' || b == b'\n')
            .map_or(raw_response.len(), |pos| checksum_data_start + pos);

        if checksum_end > checksum_data_start {
            let checksum_bytes = &raw_response[checksum_data_start..checksum_end];
            let checksum_str = String::from_utf8_lossy(checksum_bytes).trim().to_string();

            debug!("Extracted checksum: '{}'", checksum_str);

            // Return message data without checksum epilogue
            (&raw_response[..checksum_start], Some(checksum_str))
        } else {
            // No valid checksum found, return entire response
            (raw_response, None)
        }
    } else {
        // No checksum found, return entire response
        (raw_response, None)
    }
}

/// Validate MD5 checksum
fn validate_checksum(data: &[u8], expected_checksum: &str) -> Result<()> {
    let calculated = md5::compute(data);
    let calculated_hex = hex::encode(calculated.0);

    if calculated_hex.eq_ignore_ascii_case(expected_checksum) {
        debug!("Checksum validation successful: {}", calculated_hex);
        Ok(())
    } else {
        Err(ProtocolError::Parse(format!(
            "Checksum validation failed: expected '{expected_checksum}', got '{calculated_hex}'"
        )))
    }
}

/// Detect if raw response is V1 MIME format
///
/// This function performs a quick check to determine if the response
/// uses V1 MIME format based on content type headers.
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
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn test_is_v1_mime_response() {
        let v1_response = b"Content-Type: multipart/alternative; boundary=boundary123\r\n\r\n";
        assert!(is_v1_mime_response(v1_response));

        let v2_response =
            b"Region!STRING:0|BuildConfig!HEX:16|CDNConfig!HEX:16\r\nus|abc123|def456\r\n";
        assert!(!is_v1_mime_response(v2_response));
    }

    #[test]
    fn test_extract_checksum_epilogue() {
        let response_with_checksum = b"MIME message content\r\nChecksum: abc123def456\r\n";
        let (data, checksum) = extract_checksum_epilogue(response_with_checksum);

        assert_eq!(data, b"MIME message content\r\n");
        assert_eq!(checksum, Some("abc123def456".to_string()));

        let response_without_checksum = b"MIME message content\r\n";
        let (data, checksum) = extract_checksum_epilogue(response_without_checksum);

        assert_eq!(data, response_without_checksum);
        assert_eq!(checksum, None);
    }

    #[test]
    fn test_is_data_part() {
        assert!(is_data_part("attachment; filename=version"));
        assert!(is_data_part("attachment; filename=cdns"));
        assert!(is_data_part("attachment; filename=summary"));
        assert!(!is_data_part("attachment; filename=signature"));
        assert!(!is_data_part("attachment; filename=other"));
    }

    #[test]
    fn test_is_signature_part() {
        assert!(is_signature_part("attachment; filename=signature"));
        assert!(is_signature_part("attachment; filename=data.signature"));
        assert!(!is_signature_part("attachment; filename=version"));
        assert!(!is_signature_part("attachment; filename=data"));
    }

    #[test]
    fn test_find_body_start() {
        let message = b"Header1: value1\r\nHeader2: value2\r\n\r\nBody content here";
        // "Header1: value1\r\nHeader2: value2\r\n\r\n" = 36 bytes
        assert_eq!(find_body_start(message), Some(36));

        let message_no_body = b"Header1: value1\r\nHeader2: value2\r\n";
        assert_eq!(find_body_start(message_no_body), None);
    }

    #[test]
    fn test_validate_checksum() {
        let data = b"test data";
        let expected = "eb733a00c0c9d336e65691a37ab54293"; // MD5 of "test data"

        assert!(validate_checksum(data, expected).is_ok());
        assert!(validate_checksum(data, "wrong_checksum").is_err());
    }
}
