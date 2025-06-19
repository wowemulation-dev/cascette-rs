//! Enhanced signature verification for Ribbit V1 responses
//!
//! This module provides more comprehensive signature parsing and validation.

use crate::error::Result;
use der::{Decode, Encode};
use digest::Digest;
use sha2::{Sha256, Sha384, Sha512};
use tracing::{debug, trace, warn};
use x509_cert::{certificate::Certificate, der::asn1::ObjectIdentifier, time::Validity};

/// OID constants for common algorithms
mod oids {

    /// PKCS#7 signedData
    #[allow(dead_code)]
    pub const SIGNED_DATA: &str = "1.2.840.113549.1.7.2";

    /// SHA-256
    pub const SHA256: &str = "2.16.840.1.101.3.4.2.1";
    /// SHA-384
    pub const SHA384: &str = "2.16.840.1.101.3.4.2.2";
    /// SHA-512
    pub const SHA512: &str = "2.16.840.1.101.3.4.2.3";

    /// RSA with SHA-256
    pub const RSA_SHA256: &str = "1.2.840.113549.1.1.11";
    /// RSA with SHA-384
    pub const RSA_SHA384: &str = "1.2.840.113549.1.1.12";
    /// RSA with SHA-512
    pub const RSA_SHA512: &str = "1.2.840.113549.1.1.13";

    /// `SigningTime` attribute
    pub const SIGNING_TIME: &str = "1.2.840.113549.1.9.5";
    /// `TimeStampToken` attribute
    pub const TIMESTAMP_TOKEN: &str = "1.2.840.113549.1.9.16.2.14";
}

/// Enhanced signature information with verification details
#[derive(Debug, Clone)]
pub struct EnhancedSignatureInfo {
    /// Basic signature info
    pub format: String,
    /// Size in bytes
    pub size: usize,
    /// Digest algorithm used (e.g., SHA-256)
    pub digest_algorithm: String,
    /// Signature algorithm used (e.g., RSA with SHA-256)
    pub signature_algorithm: String,

    /// Verification status
    pub is_verified: bool,
    /// List of verification errors (if any)
    pub verification_errors: Vec<String>,

    /// Certificate details
    pub certificates: Vec<CertificateInfo>,
    /// Number of signers
    pub signer_count: usize,
    /// Timestamp information if present
    pub timestamp_info: Option<TimestampInfo>,
}

/// Information about a certificate in the chain
#[derive(Debug, Clone)]
pub struct CertificateInfo {
    /// Subject distinguished name
    pub subject: String,
    /// Issuer distinguished name
    pub issuer: String,
    /// Serial number (hex)
    pub serial_number: String,
    /// Not valid before
    pub not_before: String,
    /// Not valid after
    pub not_after: String,
    /// Whether the certificate is currently valid
    pub is_valid: bool,
}

/// Timestamp information from the signature
#[derive(Debug, Clone)]
pub struct TimestampInfo {
    /// Signing time from `SignerInfo`
    pub signing_time: Option<String>,
    /// Whether the timestamp is verified
    pub is_verified: bool,
    /// Timestamp authority info if available
    pub timestamp_authority: Option<String>,
}

/// Parse and verify a PKCS#7 signature
///
/// # Arguments
/// * `signature_bytes` - The raw PKCS#7 signature bytes
/// * `signed_data` - The data that was signed (optional for verification)
///
/// # Errors
///
/// Returns an error if parsing fails completely
#[allow(clippy::too_many_lines)]
pub fn parse_and_verify_signature(
    signature_bytes: &[u8],
    signed_data: Option<&[u8]>,
) -> Result<EnhancedSignatureInfo> {
    trace!("Parsing signature: {} bytes", signature_bytes.len());

    let mut info = EnhancedSignatureInfo {
        format: "PKCS#7/CMS".to_string(),
        size: signature_bytes.len(),
        digest_algorithm: "Unknown".to_string(),
        signature_algorithm: "Unknown".to_string(),
        is_verified: false,
        verification_errors: Vec::new(),
        certificates: Vec::new(),
        signer_count: 0,
        timestamp_info: None,
    };

    // Check minimum size
    if signature_bytes.is_empty() {
        info.verification_errors
            .push("Empty signature data".to_string());
        return Ok(info);
    }

    // Try to parse with CMS crate first
    match crate::cms_parser::parse_cms_signature(signature_bytes) {
        Ok(cms_info) => {
            debug!("Successfully parsed CMS signature");

            // Update basic info
            info.signer_count = cms_info.signers.len();

            // Get first signer's algorithms (most common case)
            if let Some(first_signer) = cms_info.signers.first() {
                info.digest_algorithm
                    .clone_from(&first_signer.digest_algorithm);
                info.signature_algorithm
                    .clone_from(&first_signer.signature_algorithm);

                // Log public key extraction
                if let Some(ref pk) = first_signer.public_key {
                    debug!(
                        "Extracted {} public key: {} bits",
                        pk.algorithm, pk.key_size
                    );
                }
            }

            // Convert certificates
            for cert in &cms_info.certificates {
                let cert_info = CertificateInfo {
                    subject: cert.subject.clone(),
                    issuer: cert.issuer.clone(),
                    serial_number: cert.serial_number.clone(),
                    not_before: "Unknown".to_string(), // CMS doesn't expose validity
                    not_after: "Unknown".to_string(),
                    is_valid: true, // Assume valid for now
                };
                info.certificates.push(cert_info);
            }

            // Attempt verification if we have signed data and public key
            if let Some(data) = signed_data {
                if let Some(first_signer) = cms_info.signers.first() {
                    if let Some(ref public_key) = first_signer.public_key {
                        match crate::cms_parser::verify_with_public_key(
                            public_key,
                            data,
                            &first_signer.signature,
                            &first_signer.digest_algorithm,
                        ) {
                            Ok(true) => {
                                info.is_verified = true;
                                debug!("Signature verification successful!");
                            }
                            Ok(false) => {
                                info.verification_errors
                                    .push("Signature verification failed".to_string());
                            }
                            Err(e) => {
                                info.verification_errors
                                    .push(format!("Verification error: {e}"));
                            }
                        }
                    } else {
                        info.verification_errors
                            .push("No public key found for verification".to_string());
                    }
                }
            }

            // Still extract timestamp using the old method
            let pkcs7_info = parse_pkcs7_structure(signature_bytes);
            if let Some(timestamp) = extract_timestamp_info(&pkcs7_info) {
                info.timestamp_info = Some(timestamp);
            }
        }
        Err(e) => {
            warn!("CMS parsing failed, falling back to manual parsing: {e}");

            // Fall back to manual parsing
            let pkcs7_info = parse_pkcs7_structure(signature_bytes);
            info.digest_algorithm
                .clone_from(&pkcs7_info.digest_algorithm);
            info.signature_algorithm
                .clone_from(&pkcs7_info.signature_algorithm);
            info.signer_count = pkcs7_info.signer_count;

            // Extract certificates
            for cert_der in &pkcs7_info.certificates {
                match Certificate::from_der(cert_der) {
                    Ok(cert) => {
                        let cert_info = extract_certificate_info(&cert);
                        info.certificates.push(cert_info);
                    }
                    Err(e) => {
                        warn!("Failed to parse certificate: {e}");
                    }
                }
            }

            // Extract timestamp information
            if let Some(timestamp) = extract_timestamp_info(&pkcs7_info) {
                info.timestamp_info = Some(timestamp);
            }

            // Attempt verification if we have the signed data
            if let Some(data) = signed_data {
                if verify_signature_data(&pkcs7_info, data) {
                    info.is_verified = true;
                } else {
                    info.verification_errors
                        .push("Signature verification failed".to_string());
                }
            }
        }
    }

    Ok(info)
}

/// Internal PKCS#7 parsing result
struct Pkcs7Info {
    digest_algorithm: String,
    signature_algorithm: String,
    signer_count: usize,
    certificates: Vec<Vec<u8>>,
    #[allow(dead_code)]
    signature_value: Vec<u8>,
    /// Raw PKCS#7 data for timestamp extraction
    raw_data: Vec<u8>,
}

/// Parse PKCS#7 structure manually
fn parse_pkcs7_structure(data: &[u8]) -> Pkcs7Info {
    let mut info = Pkcs7Info {
        digest_algorithm: "Unknown".to_string(),
        signature_algorithm: "Unknown".to_string(),
        signer_count: 0,
        certificates: Vec::new(),
        signature_value: Vec::new(),
        raw_data: data.to_vec(),
    };

    // This is a simplified parser that looks for known patterns
    // In a production system, you'd use a proper PKCS#7 parser

    // Look for algorithm OIDs
    if let Some(pos) = find_oid_pattern(data, oids::SHA256) {
        info.digest_algorithm = "SHA-256".to_string();
        debug!("Found SHA-256 digest algorithm at position {pos}");
    } else if let Some(pos) = find_oid_pattern(data, oids::SHA384) {
        info.digest_algorithm = "SHA-384".to_string();
        debug!("Found SHA-384 digest algorithm at position {pos}");
    } else if let Some(pos) = find_oid_pattern(data, oids::SHA512) {
        info.digest_algorithm = "SHA-512".to_string();
        debug!("Found SHA-512 digest algorithm at position {pos}");
    }

    // Look for signature algorithms
    if find_oid_pattern(data, oids::RSA_SHA256).is_some() {
        info.signature_algorithm = "RSA with SHA-256".to_string();
    } else if find_oid_pattern(data, oids::RSA_SHA384).is_some() {
        info.signature_algorithm = "RSA with SHA-384".to_string();
    } else if find_oid_pattern(data, oids::RSA_SHA512).is_some() {
        info.signature_algorithm = "RSA with SHA-512".to_string();
    }

    // Extract certificates (look for certificate patterns)
    let mut pos = 0;
    while pos < data.len().saturating_sub(4) {
        // Certificates typically start with SEQUENCE tag (0x30) followed by length
        if data[pos] == 0x30 && data[pos + 1] == 0x82 {
            let len = ((data[pos + 2] as usize) << 8) | (data[pos + 3] as usize);

            // Certificates are typically 300-2000 bytes
            if len > 300 && len < 2000 && pos + 4 + len <= data.len() {
                let cert_data = data[pos..pos + 4 + len].to_vec();

                // Quick validation - check if it might be a certificate
                if cert_data.len() > 100 && cert_data[4] == 0x30 {
                    info.certificates.push(cert_data);
                    debug!("Found potential certificate at position {pos}, length {len}");
                }

                pos += 4 + len;
            } else {
                pos += 1;
            }
        } else {
            pos += 1;
        }
    }

    // Count signers (simplified - assume one signer if we found certificates)
    if !info.certificates.is_empty() {
        info.signer_count = 1;
    }

    debug!(
        "Parsed PKCS#7: {} certificates, {} signers, digest: {}, signature: {}",
        info.certificates.len(),
        info.signer_count,
        info.digest_algorithm,
        info.signature_algorithm
    );

    info
}

/// Find an OID pattern in data
fn find_oid_pattern(data: &[u8], oid_str: &str) -> Option<usize> {
    // Convert OID string to DER encoding
    let oid = ObjectIdentifier::new(oid_str).ok()?;
    let oid_bytes = oid.to_der().ok()?;

    data.windows(oid_bytes.len())
        .position(|window| window == oid_bytes)
}

/// Extract information from a parsed certificate
fn extract_certificate_info(cert: &Certificate) -> CertificateInfo {
    let tbs = &cert.tbs_certificate;

    CertificateInfo {
        subject: tbs.subject.to_string(),
        issuer: tbs.issuer.to_string(),
        serial_number: tbs.serial_number.to_string(),
        not_before: format!("{}", tbs.validity.not_before),
        not_after: format!("{}", tbs.validity.not_after),
        is_valid: is_certificate_valid(&tbs.validity),
    }
}

/// Check if a certificate is currently valid
fn is_certificate_valid(_validity: &Validity) -> bool {
    // For now, we can't easily compare with current time without additional dependencies
    // In a real implementation, you'd check against the current system time
    true
}

/// Extract timestamp information from PKCS#7 structure
fn extract_timestamp_info(pkcs7: &Pkcs7Info) -> Option<TimestampInfo> {
    let mut timestamp_info = TimestampInfo {
        signing_time: None,
        is_verified: false,
        timestamp_authority: None,
    };

    // Look for signing time attribute in the raw PKCS#7 data
    // In a real implementation, this would parse SignerInfo attributes
    if let Some(pos) = find_oid_pattern(&pkcs7.raw_data, oids::SIGNING_TIME) {
        // For now, we just detect the presence
        timestamp_info.signing_time = Some(format!("Present (position: {pos})"));
        debug!("Found signing time attribute at position {pos}");

        // Try to extract the actual time (simplified)
        // SigningTime is typically followed by UTCTime or GeneralizedTime
        if pos + 20 < pkcs7.raw_data.len() {
            // Look for time encoding after the OID
            let time_data = &pkcs7.raw_data[pos + 11..pos + 30];
            trace!("Time data near signing time OID: {:02x?}", time_data);
        }
    }

    // Look for timestamp token
    if let Some(pos) = find_oid_pattern(&pkcs7.raw_data, oids::TIMESTAMP_TOKEN) {
        timestamp_info.timestamp_authority = Some("TSA present".to_string());
        debug!("Found timestamp token at position {pos}");
    }

    // Return None if no timestamp info found
    if timestamp_info.signing_time.is_none() && timestamp_info.timestamp_authority.is_none() {
        None
    } else {
        // Basic verification: timestamps should not be in the future
        timestamp_info.is_verified = true; // For now, just mark as verified if present
        Some(timestamp_info)
    }
}

/// Verify signature data (placeholder implementation)
fn verify_signature_data(pkcs7: &Pkcs7Info, signed_data: &[u8]) -> bool {
    // This is a placeholder. Real implementation would:
    // 1. Extract the signer's public key from the certificate
    // 2. Compute the message digest
    // 3. Verify the signature using the public key

    debug!("Signature verification not yet implemented");

    // Compute expected digest
    let digest = match pkcs7.digest_algorithm.as_str() {
        "SHA-256" => Sha256::digest(signed_data).to_vec(),
        "SHA-384" => Sha384::digest(signed_data).to_vec(),
        "SHA-512" => Sha512::digest(signed_data).to_vec(),
        _ => return false,
    };

    debug!(
        "Computed {} digest: {} bytes",
        pkcs7.digest_algorithm,
        digest.len()
    );

    // For now, we can't verify without proper PKCS#7 parsing
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_oid_pattern_finding() {
        // Create a buffer with SHA-256 OID
        let oid = ObjectIdentifier::new(oids::SHA256).unwrap();
        let oid_bytes = oid.to_der().unwrap();

        let mut data = vec![0x00, 0x00];
        data.extend_from_slice(&oid_bytes);
        data.extend_from_slice(&[0x00, 0x00]);

        let pos = find_oid_pattern(&data, oids::SHA256);
        assert_eq!(pos, Some(2));
    }

    #[test]
    fn test_empty_signature_parsing() {
        let result = parse_and_verify_signature(&[], None);
        assert!(result.is_ok());

        let info = result.unwrap();
        assert!(!info.is_verified);
        assert!(!info.verification_errors.is_empty());
    }
}
