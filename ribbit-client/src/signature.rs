//! ASN.1 signature parsing for Ribbit V1 responses
//!
//! This module provides basic parsing of PKCS#7/CMS signatures.

use crate::error::Result;
use tracing::{debug, trace};

/// Parse a PKCS#7 signature and extract basic information
///
/// # Errors
///
/// Returns an error if:
/// - The signature is too short (less than 20 bytes)
/// - ASN.1 parsing fails
/// - The signature structure is invalid
pub fn parse_signature(signature_bytes: &[u8]) -> Result<SignatureInfo> {
    trace!("Parsing {} bytes of signature data", signature_bytes.len());

    // First few bytes tell us about the structure
    if signature_bytes.len() < 20 {
        return Err(crate::error::Error::Asn1Error(
            "Signature too short".to_string(),
        ));
    }

    // Parse as raw ASN.1 to extract basic structure information
    let tlv = asn1::parse_single::<asn1::Tlv>(signature_bytes)
        .map_err(|e| crate::error::Error::Asn1Error(format!("Failed to parse ASN.1: {e:?}")))?;

    let tag = tlv.tag();
    let len = tlv.data().len();
    trace!("ASN.1 tag: {tag:?}, length: {len}");

    // Try to parse inner content to get more details
    let mut algorithm = "Unknown".to_string();
    let mut signer_count = 0;
    let mut certificate_count = 0;

    // PKCS#7 has OID 1.2.840.113549.1.7.2 for signedData
    // Let's look for this pattern
    let data = tlv.data();
    if data.len() > 20 {
        // Look for the signedData OID pattern (06 09 2a 86 48 86 f7 0d 01 07 02)
        if data[0] == 0x06 && data[1] == 0x09 {
            let oid_bytes = &data[2..11];
            if oid_bytes == [0x2a, 0x86, 0x48, 0x86, 0xf7, 0x0d, 0x01, 0x07, 0x02] {
                debug!("Found PKCS#7 signedData OID");

                // Try to find algorithm identifiers (SHA-256 = 2.16.840.1.101.3.4.2.1)
                if let Some(sha256_pos) = find_pattern(
                    data,
                    &[0x60, 0x86, 0x48, 0x01, 0x65, 0x03, 0x04, 0x02, 0x01],
                ) {
                    algorithm = "SHA-256".to_string();
                    debug!("Found SHA-256 algorithm at position {}", sha256_pos);
                }

                // Count SEQUENCE tags that might be certificates (very rough estimate)
                let mut pos = 0;
                while pos < data.len() - 4 {
                    if data[pos] == 0x30 && data[pos + 1] == 0x82 {
                        // This might be a certificate (they're usually large SEQUENCEs)
                        let len = ((data[pos + 2] as usize) << 8) | (data[pos + 3] as usize);
                        if len > 300 && len < 2000 {
                            certificate_count += 1;
                        }
                        pos += 4 + len;
                    } else {
                        pos += 1;
                    }
                }

                // SignerInfo structures usually contain specific patterns
                signer_count = 1; // Assume at least one signer if we have a valid signature
            }
        }
    }

    let signature_info = SignatureInfo {
        format: "PKCS#7/CMS".to_string(),
        size: signature_bytes.len(),
        algorithm,
        signer_count,
        certificate_count,
    };

    debug!("Parsed signature: {:?}", signature_info);
    Ok(signature_info)
}

/// Find a byte pattern in data
fn find_pattern(data: &[u8], pattern: &[u8]) -> Option<usize> {
    data.windows(pattern.len())
        .position(|window| window == pattern)
}

/// Information extracted from a parsed signature
#[derive(Debug, Clone)]
pub struct SignatureInfo {
    /// Signature format (e.g., "PKCS#7")
    pub format: String,
    /// Size of the signature in bytes
    pub size: usize,
    /// Signature algorithm (if detected)
    pub algorithm: String,
    /// Number of signers
    pub signer_count: usize,
    /// Number of certificates included
    pub certificate_count: usize,
}
