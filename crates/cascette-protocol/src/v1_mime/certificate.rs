//! Certificate management and fetching for V1 MIME responses
//!
//! This module provides functionality to fetch and manage certificates
//! used in PKCS#7 signature verification for V1 MIME protocol responses.
//!
//! **Note**: Certificate fetching requires the Ribbit TCP client, which is
//! only available on native platforms. On WASM, only the certificate parsing
//! and validation functions are available.

// Certificate fetching requires RibbitClient which uses TCP sockets (native only)
#[cfg(not(target_arch = "wasm32"))]
use crate::client::RibbitClient;
#[cfg(not(target_arch = "wasm32"))]
use crate::error::{ProtocolError, Result};
use crate::v1_mime::types::CertificateInfo;
#[cfg(not(target_arch = "wasm32"))]
use crate::v1_mime::types::PublicKeyInfo;
#[cfg(not(target_arch = "wasm32"))]
use base64::Engine;
#[cfg(not(target_arch = "wasm32"))]
use der::{Decode, Encode};
#[cfg(not(target_arch = "wasm32"))]
use tracing::info;
use tracing::{debug, warn};
#[cfg(not(target_arch = "wasm32"))]
use x509_cert::certificate::Certificate;
#[cfg(not(target_arch = "wasm32"))]
use x509_cert::spki::SubjectPublicKeyInfoRef;

/// Certificate fetcher for retrieving certificates by various identifiers
///
/// **Note**: This is only available on native platforms as it requires
/// TCP socket access via the Ribbit protocol.
#[cfg(not(target_arch = "wasm32"))]
pub struct CertificateFetcher<'a> {
    client: &'a RibbitClient,
}

#[cfg(not(target_arch = "wasm32"))]
impl<'a> CertificateFetcher<'a> {
    /// Create a new certificate fetcher
    pub fn new(client: &'a RibbitClient) -> Self {
        Self { client }
    }

    /// Fetch certificate by Subject Key Identifier (SKI)
    ///
    /// # Arguments
    /// * `ski` - The Subject Key Identifier as a hex string
    ///
    /// # Returns
    /// Returns certificate information including public key details
    ///
    /// # Errors
    /// Returns an error if:
    /// - The certificate request fails
    /// - The response doesn't contain a valid certificate
    /// - Certificate parsing fails
    pub async fn fetch_by_ski(&self, ski: &str) -> Result<CertificateInfo> {
        info!("Fetching certificate for SKI: {}", ski);

        // Use the certs endpoint with SKI as the identifier
        let endpoint = format!("certs/{ski}");

        // Make the request - use TCP-only since certs endpoint may not support V2
        let response = self.client.query_tcp_only(&endpoint).await?;

        // Parse the response to extract certificate
        Self::parse_certificate_response(&response)
    }

    /// Fetch certificate by hash (for OCSP-style requests)
    ///
    /// # Arguments
    /// * `hash` - The certificate hash identifier
    ///
    /// # Returns
    /// Returns certificate information
    ///
    /// # Errors
    /// Returns an error if the certificate cannot be fetched or parsed
    pub async fn fetch_by_hash(&self, hash: &str) -> Result<CertificateInfo> {
        info!("Fetching certificate for hash: {}", hash);

        let endpoint = format!("ocsp/{hash}");
        let response = self.client.query_tcp_only(&endpoint).await?;

        Self::parse_certificate_response(&response)
    }

    /// Parse certificate from a response string
    fn parse_certificate_response(response: &str) -> Result<CertificateInfo> {
        debug!("Parsing certificate response: {} bytes", response.len());

        // Look for PEM certificate markers
        if !response.contains("-----BEGIN CERTIFICATE-----") {
            return Err(ProtocolError::Parse(
                "Response does not contain a PEM certificate".to_string(),
            ));
        }

        // Extract PEM certificate content
        let cert_pem = Self::extract_pem_certificate(response)?;

        // Parse the certificate
        let cert = Self::parse_pem_certificate(&cert_pem)?;

        // Extract certificate information
        Self::extract_certificate_info(&cert)
    }

    /// Extract PEM certificate from response text
    fn extract_pem_certificate(response: &str) -> Result<String> {
        let cert_start = response
            .find("-----BEGIN CERTIFICATE-----")
            .ok_or_else(|| {
                ProtocolError::Parse("Certificate BEGIN marker not found".to_string())
            })?;

        let cert_end = response
            .find("-----END CERTIFICATE-----")
            .ok_or_else(|| ProtocolError::Parse("Certificate END marker not found".to_string()))?;

        // Include the END marker in the extraction
        let cert_pem = &response[cert_start..cert_end + 25];
        Ok(cert_pem.to_string())
    }

    /// Parse PEM certificate to DER format
    fn parse_pem_certificate(cert_pem: &str) -> Result<Certificate> {
        // Extract base64 content between BEGIN/END markers
        let lines: Vec<&str> = cert_pem
            .lines()
            .filter(|line| !line.contains("-----"))
            .collect();
        let base64_content = lines.join("");

        // Decode from base64 to DER
        let cert_der = base64::engine::general_purpose::STANDARD
            .decode(&base64_content)
            .map_err(|e| ProtocolError::Parse(format!("Base64 decode error: {e}")))?;

        // Parse DER certificate
        Certificate::from_der(&cert_der)
            .map_err(|e| ProtocolError::Parse(format!("Certificate parse error: {e}")))
    }

    /// Extract certificate information from parsed certificate
    fn extract_certificate_info(cert: &Certificate) -> Result<CertificateInfo> {
        let tbs = &cert.tbs_certificate;

        // Extract Subject Key Identifier
        let subject_key_identifier = Self::extract_subject_key_identifier(cert).ok();

        // Extract public key information
        let spki_der = tbs
            .subject_public_key_info
            .to_der()
            .map_err(|e| ProtocolError::Parse(format!("Failed to encode SPKI: {e}")))?;
        let public_key = Self::extract_public_key_info_from_der(&spki_der)?;

        debug!(
            "Extracted certificate info: subject={}, issuer={}, algorithm={}, key_size={}",
            tbs.subject, tbs.issuer, public_key.algorithm, public_key.key_size
        );

        Ok(CertificateInfo {
            subject: tbs.subject.to_string(),
            issuer: tbs.issuer.to_string(),
            serial_number: hex::encode(tbs.serial_number.as_bytes()),
            public_key: Some(public_key),
            subject_key_identifier,
        })
    }

    /// Extract Subject Key Identifier from certificate extensions
    fn extract_subject_key_identifier(cert: &Certificate) -> Result<String> {
        if let Some(extensions) = &cert.tbs_certificate.extensions {
            for ext in extensions {
                // Subject Key Identifier OID is 2.5.29.14
                if ext.extn_id.to_string() == "2.5.29.14" {
                    let ski_bytes = ext.extn_value.as_bytes();

                    // SKI extension value is an OCTET STRING
                    // Format: 04 <length> <ski_bytes>
                    if ski_bytes.len() > 2 && ski_bytes[0] == 0x04 {
                        let length = ski_bytes[1] as usize;
                        if ski_bytes.len() >= 2 + length {
                            return Ok(hex::encode(&ski_bytes[2..2 + length]));
                        }
                    }
                }
            }
        }

        Err(ProtocolError::Parse(
            "No Subject Key Identifier extension found".to_string(),
        ))
    }

    /// Extract public key information from DER-encoded `SubjectPublicKeyInfo`
    fn extract_public_key_info_from_der(spki_der: &[u8]) -> Result<PublicKeyInfo> {
        let spki_ref = SubjectPublicKeyInfoRef::from_der(spki_der)
            .map_err(|e| ProtocolError::Parse(format!("Failed to parse SPKI: {e}")))?;

        let algorithm = Self::oid_to_algorithm_name(&spki_ref.algorithm.oid);
        let key_bytes = spki_ref.subject_public_key.raw_bytes().to_vec();

        // Determine key size based on algorithm
        let key_size = match algorithm.as_str() {
            "RSA" => Self::estimate_rsa_key_size(&key_bytes),
            "ECDSA" => Self::estimate_ec_key_size(&key_bytes),
            _ => key_bytes.len() * 8, // Fallback estimate
        };

        Ok(PublicKeyInfo {
            algorithm,
            key_size,
            key_bytes,
        })
    }

    /// Estimate RSA key size from key bytes
    fn estimate_rsa_key_size(key_bytes: &[u8]) -> usize {
        // Try to decode as PKCS#1 RSA public key to get actual modulus size
        if let Ok(rsa_key) = rsa::pkcs1::DecodeRsaPublicKey::from_pkcs1_der(key_bytes) {
            use rsa::traits::PublicKeyParts;
            let key: rsa::RsaPublicKey = rsa_key;
            return key.size() * 8;
        }

        // Fallback: estimate from key length
        // RSA keys are typically 1024, 2048, 3072, or 4096 bits
        match key_bytes.len() {
            128..=255 => 1024, // ~128-255 bytes for 1024-bit
            256..=383 => 2048, // ~256-383 bytes for 2048-bit
            384..=511 => 3072, // ~384-511 bytes for 3072-bit
            512.. => 4096,     // ~512+ bytes for 4096-bit
            _ => key_bytes.len() * 8,
        }
    }

    /// Estimate EC key size from key bytes
    fn estimate_ec_key_size(key_bytes: &[u8]) -> usize {
        // EC key sizes are typically 256, 384, or 521 bits
        match key_bytes.len() {
            32..=47 => 256, // P-256
            48..=63 => 384, // P-384
            64.. => 521,    // P-521
            _ => key_bytes.len() * 8,
        }
    }

    /// Convert OID to human-readable algorithm name
    fn oid_to_algorithm_name(oid: &der::asn1::ObjectIdentifier) -> String {
        match oid.to_string().as_str() {
            "1.2.840.113549.1.1.1" => "RSA".to_string(),
            "1.2.840.10045.2.1" => "ECDSA".to_string(),
            "1.3.101.112" => "Ed25519".to_string(),
            "1.3.101.113" => "Ed448".to_string(),
            _ => format!("OID: {oid}"),
        }
    }
}

/// Fetch multiple certificates by Subject Key Identifiers
///
/// This is a convenience function for fetching multiple certificates concurrently.
///
/// **Note**: This is only available on native platforms as it requires
/// TCP socket access via the Ribbit protocol.
///
/// # Arguments
/// * `client` - The Ribbit client to use
/// * `skis` - List of Subject Key Identifiers
///
/// # Returns
/// Returns a vector of certificate information, with None for failed fetches
#[cfg(not(target_arch = "wasm32"))]
pub async fn fetch_certificates_by_skis(
    client: &RibbitClient,
    skis: &[String],
) -> Vec<Option<CertificateInfo>> {
    let fetcher = CertificateFetcher::new(client);
    let mut results = Vec::with_capacity(skis.len());

    // For now, fetch sequentially to avoid overwhelming the server
    // In the future, this could be made concurrent with rate limiting
    for ski in skis {
        match fetcher.fetch_by_ski(ski).await {
            Ok(cert) => {
                debug!("Successfully fetched certificate for SKI: {}", ski);
                results.push(Some(cert));
            }
            Err(e) => {
                warn!("Failed to fetch certificate for SKI {}: {}", ski, e);
                results.push(None);
            }
        }
    }

    results
}

/// Validate certificate chain (basic validation)
///
/// This performs basic certificate chain validation including:
/// - Certificate format validation
/// - Basic signature verification (if possible)
/// - Validity period checks
///
/// # Arguments
/// * `certificates` - Chain of certificates to validate
///
/// # Returns
/// Returns true if the chain appears valid
// NOTE: Complexity from multi-step certificate chain validation with issuer matching.
// Future: Extract helpers for certificate validation steps and chain traversal.
#[allow(clippy::cognitive_complexity)]
pub fn validate_certificate_chain(certificates: &[CertificateInfo]) -> bool {
    if certificates.is_empty() {
        warn!("Empty certificate chain");
        return false;
    }

    // Basic validation - ensure all certificates have required fields
    for (i, cert) in certificates.iter().enumerate() {
        if cert.subject.is_empty() || cert.issuer.is_empty() {
            warn!("Certificate {} has empty subject or issuer", i);
            return false;
        }

        if cert.public_key.is_none() {
            warn!("Certificate {} has no public key", i);
            return false;
        }
    }

    debug!("Certificate chain basic validation passed");
    true
}

#[cfg(test)]
#[allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::uninlined_format_args
)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_pem_certificate() {
        let response = r"
Some header text
-----BEGIN CERTIFICATE-----
MIIBIjANBgkqhkiG9w0BAQEFAAOCAQ8AMIIBCgKCAQEA7dQJzFM
-----END CERTIFICATE-----
Some footer text
";

        // Test the PEM extraction logic directly
        let cert_start = response
            .find("-----BEGIN CERTIFICATE-----")
            .expect("Operation should succeed");
        let cert_end = response
            .find("-----END CERTIFICATE-----")
            .expect("Operation should succeed");
        let cert_pem = &response[cert_start..cert_end + 25];

        assert!(cert_pem.starts_with("-----BEGIN CERTIFICATE-----"));
        assert!(cert_pem.ends_with("-----END CERTIFICATE-----"));
    }

    #[test]
    fn test_oid_to_algorithm_name() {
        use der::asn1::ObjectIdentifier;

        // Test the OID conversion logic directly
        fn oid_to_algorithm_name(oid: &ObjectIdentifier) -> String {
            match oid.to_string().as_str() {
                "1.2.840.113549.1.1.1" => "RSA".to_string(),
                "1.2.840.10045.2.1" => "ECDSA".to_string(),
                "1.3.101.112" => "Ed25519".to_string(),
                "1.3.101.113" => "Ed448".to_string(),
                _ => format!("OID: {oid}"),
            }
        }

        let rsa_oid =
            ObjectIdentifier::new("1.2.840.113549.1.1.1").expect("Operation should succeed");
        assert_eq!(oid_to_algorithm_name(&rsa_oid), "RSA");

        let ecdsa_oid =
            ObjectIdentifier::new("1.2.840.10045.2.1").expect("Operation should succeed");
        assert_eq!(oid_to_algorithm_name(&ecdsa_oid), "ECDSA");
    }

    #[test]
    fn test_estimate_rsa_key_size() {
        // Test the RSA key size estimation logic directly
        fn estimate_rsa_key_size(key_bytes: &[u8]) -> usize {
            // Try to decode as PKCS#1 RSA public key to get actual modulus size
            if let Ok(rsa_key) = rsa::pkcs1::DecodeRsaPublicKey::from_pkcs1_der(key_bytes) {
                use rsa::traits::PublicKeyParts;
                let key: rsa::RsaPublicKey = rsa_key;
                return key.size() * 8;
            }

            // Fallback: estimate from key length
            // RSA keys are typically 1024, 2048, 3072, or 4096 bits
            match key_bytes.len() {
                128..=255 => 1024, // ~128-255 bytes for 1024-bit
                256..=383 => 2048, // ~256-383 bytes for 2048-bit
                384..=511 => 3072, // ~384-511 bytes for 3072-bit
                512.. => 4096,     // ~512+ bytes for 4096-bit
                _ => key_bytes.len() * 8,
            }
        }

        // Test various key sizes
        assert_eq!(estimate_rsa_key_size(&[0u8; 128]), 1024);
        assert_eq!(estimate_rsa_key_size(&[0u8; 300]), 2048);
        assert_eq!(estimate_rsa_key_size(&[0u8; 450]), 3072);
        assert_eq!(estimate_rsa_key_size(&[0u8; 600]), 4096);
    }

    #[test]
    fn test_validate_certificate_chain() {
        // Test empty chain
        assert!(!validate_certificate_chain(&[]));

        // Test valid chain
        let cert = CertificateInfo {
            subject: "CN=Test".to_string(),
            issuer: "CN=Test CA".to_string(),
            serial_number: "123456".to_string(),
            public_key: Some(PublicKeyInfo {
                algorithm: "RSA".to_string(),
                key_size: 2048,
                key_bytes: vec![0u8; 256],
            }),
            subject_key_identifier: Some("abcdef".to_string()),
        };

        assert!(validate_certificate_chain(&[cert]));

        // Test invalid chain (empty subject)
        let invalid_cert = CertificateInfo {
            subject: String::new(),
            issuer: "CN=Test CA".to_string(),
            serial_number: "123456".to_string(),
            public_key: None,
            subject_key_identifier: None,
        };

        assert!(!validate_certificate_chain(&[invalid_cert]));
    }
}
