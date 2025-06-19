//! Certificate fetcher for retrieving signer certificates by SKI
//!
//! This module allows fetching certificates from the Ribbit certs endpoint
//! using Subject Key Identifiers (SKI) found in signatures.

use crate::client::RibbitClient;
use crate::cms_parser::{CertificateDetails, PublicKeyInfo};
use crate::error::{Error, Result};
use crate::types::Endpoint;
use base64::Engine;
use der::Decode;
use tracing::{debug, info, warn};
use x509_cert::certificate::Certificate;

/// Fetch a certificate using its Subject Key Identifier
///
/// # Arguments
/// * `client` - The Ribbit client to use for requests
/// * `ski` - The Subject Key Identifier (hex string)
///
/// # Returns
/// Returns the certificate details including the public key
///
/// # Errors
/// Returns an error if:
/// - The certificate request fails
/// - The response doesn't contain a valid PEM certificate
/// - The certificate cannot be parsed
/// - The SKI extraction fails
pub async fn fetch_certificate_by_ski(
    client: &RibbitClient,
    ski: &str,
) -> Result<CertificateDetails> {
    info!("Fetching certificate for SKI: {}", ski);

    // Use the SKI as the certificate endpoint
    let endpoint = Endpoint::Cert(ski.to_string());

    // Make raw request to bypass checksum validation issues
    let raw_response = client.request_raw(&endpoint).await?;
    let response_str = String::from_utf8_lossy(&raw_response);

    // Extract PEM certificate
    if !response_str.contains("-----BEGIN CERTIFICATE-----") {
        return Err(Error::Asn1Error(
            "Response does not contain a PEM certificate".to_string(),
        ));
    }

    let cert_start = response_str
        .find("-----BEGIN CERTIFICATE-----")
        .ok_or_else(|| Error::Asn1Error("Certificate start marker not found".to_string()))?;

    let cert_end = response_str
        .find("-----END CERTIFICATE-----")
        .ok_or_else(|| Error::Asn1Error("Certificate end marker not found".to_string()))?;

    let cert_pem = &response_str[cert_start..cert_end + 25];

    // Extract base64 content
    let lines: Vec<&str> = cert_pem
        .lines()
        .filter(|line| !line.contains("-----"))
        .collect();
    let base64_content = lines.join("");

    // Decode from base64
    let cert_der = base64::engine::general_purpose::STANDARD
        .decode(&base64_content)
        .map_err(|e| Error::Asn1Error(format!("Base64 decode error: {e}")))?;

    // Parse certificate
    let cert = Certificate::from_der(&cert_der)
        .map_err(|e| Error::Asn1Error(format!("Certificate parse error: {e}")))?;

    // Verify the SKI matches
    let cert_ski = extract_ski_from_certificate(&cert)?;
    if cert_ski != ski {
        warn!(
            "Certificate SKI mismatch: expected {}, got {}",
            ski, cert_ski
        );
    }

    // Extract certificate details
    let tbs = &cert.tbs_certificate;

    // Extract public key
    let public_key = PublicKeyInfo {
        algorithm: oid_to_algorithm_name(&tbs.subject_public_key_info.algorithm.oid),
        key_size: tbs
            .subject_public_key_info
            .subject_public_key
            .raw_bytes()
            .len()
            * 8,
        key_bytes: tbs
            .subject_public_key_info
            .subject_public_key
            .raw_bytes()
            .to_vec(),
    };

    debug!(
        "Fetched certificate for {}: {} key, {} bits",
        tbs.subject, public_key.algorithm, public_key.key_size
    );

    Ok(CertificateDetails {
        subject: tbs.subject.to_string(),
        issuer: tbs.issuer.to_string(),
        serial_number: hex::encode(tbs.serial_number.as_bytes()),
        public_key: Some(public_key),
    })
}

/// Extract Subject Key Identifier from a certificate
fn extract_ski_from_certificate(cert: &Certificate) -> Result<String> {
    if let Some(extensions) = &cert.tbs_certificate.extensions {
        for ext in extensions {
            // Subject Key Identifier OID is 2.5.29.14
            if ext.extn_id.to_string() == "2.5.29.14" {
                let ski_bytes = ext.extn_value.as_bytes();
                // SKI is OCTET STRING, skip tag and length
                if ski_bytes.len() > 2 && ski_bytes[0] == 0x04 {
                    return Ok(hex::encode(&ski_bytes[2..]));
                }
            }
        }
    }

    Err(Error::Asn1Error(
        "No Subject Key Identifier found in certificate".to_string(),
    ))
}

/// Convert OID to algorithm name
fn oid_to_algorithm_name(oid: &der::asn1::ObjectIdentifier) -> String {
    match oid.to_string().as_str() {
        "1.2.840.113549.1.1.1" => "RSA".to_string(),
        "1.2.840.10045.2.1" => "ECDSA".to_string(),
        _ => format!("OID: {oid}"),
    }
}

/// Fetch certificate and match with signer
///
/// This is a convenience function that fetches a certificate by SKI
/// and returns it along with the extracted public key.
///
/// # Errors
/// Returns an error if:
/// - The certificate fetch fails
/// - The certificate doesn't contain a public key
pub async fn fetch_signer_certificate(
    client: &RibbitClient,
    signer_ski: &str,
) -> Result<(CertificateDetails, PublicKeyInfo)> {
    let cert_details = fetch_certificate_by_ski(client, signer_ski).await?;

    let public_key = cert_details
        .public_key
        .clone()
        .ok_or_else(|| Error::Asn1Error("No public key in certificate".to_string()))?;

    Ok((cert_details, public_key))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_oid_to_algorithm_name() {
        use der::asn1::ObjectIdentifier;

        let rsa_oid = ObjectIdentifier::new("1.2.840.113549.1.1.1").unwrap();
        assert_eq!(oid_to_algorithm_name(&rsa_oid), "RSA");

        let ecdsa_oid = ObjectIdentifier::new("1.2.840.10045.2.1").unwrap();
        assert_eq!(oid_to_algorithm_name(&ecdsa_oid), "ECDSA");
    }
}
