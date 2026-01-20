//! PKCS#7/CMS signature verification for V1 MIME responses
//!
//! This module provides PKCS#7 signature parsing and verification capabilities
//! for V1 MIME responses from Ribbit protocol endpoints.

use crate::error::{ProtocolError, Result};
use crate::v1_mime::types::{
    CertificateInfo, PublicKeyInfo, SignatureInfo, SignatureVerification, SignerIdentifier,
    SignerInfo,
};
use cms::cert::CertificateChoices;
use cms::content_info::ContentInfo;
use cms::signed_data::SignerInfo as CmsSignerInfo;
use der::{Decode, Encode};
use rsa::RsaPublicKey;
use rsa::pkcs1::DecodeRsaPublicKey;
use rsa::signature::Verifier;
use sha2::{Sha256, Sha384, Sha512};
use tracing::{debug, trace, warn};
use x509_cert::certificate::Certificate;
use x509_cert::spki::SubjectPublicKeyInfoRef;

/// Parse and verify a `PKCS#7` signature
///
/// # Arguments
/// * `signature_bytes` - The raw signature bytes (PKCS#7/CMS format)
/// * `signed_data` - The data that was signed (for detached signatures)
///
/// # Returns
/// Returns detailed signature information including verification results
///
/// # Errors
/// Returns an error if the signature cannot be parsed or verification fails
// NOTE: Complexity from PKCS#7 signature parsing and cryptographic verification.
// Future: Extract helpers for ASN.1 parsing and certificate extraction.
#[allow(clippy::cognitive_complexity)]
pub fn parse_and_verify_signature(
    signature_bytes: &[u8],
    signed_data: Option<&[u8]>,
) -> Result<SignatureInfo> {
    debug!("Parsing PKCS#7 signature: {} bytes", signature_bytes.len());

    // Parse ContentInfo structure
    let content_info = ContentInfo::from_der(signature_bytes)
        .map_err(|e| ProtocolError::Parse(format!("Failed to parse PKCS#7 ContentInfo: {e}")))?;

    // Verify this is SignedData (OID 1.2.840.113549.1.7.2)
    let signed_data_oid = der::asn1::ObjectIdentifier::new("1.2.840.113549.1.7.2")
        .map_err(|e| ProtocolError::Parse(format!("Invalid SignedData OID: {e}")))?;

    if content_info.content_type != signed_data_oid {
        return Err(ProtocolError::Parse(
            "ContentInfo is not SignedData".to_string(),
        ));
    }

    // Extract and parse SignedData
    let signed_data_bytes = content_info
        .content
        .to_der()
        .map_err(|e| ProtocolError::Parse(format!("Failed to encode SignedData content: {e}")))?;

    let cms_signed_data = cms::signed_data::SignedData::from_der(&signed_data_bytes)
        .map_err(|e| ProtocolError::Parse(format!("Failed to parse SignedData: {e}")))?;

    debug!(
        "Parsed SignedData with {} signers",
        cms_signed_data.signer_infos.0.len()
    );

    // Parse certificates
    let certificates = parse_certificates(&cms_signed_data);
    debug!("Found {} certificates", certificates.len());

    // Parse signers
    let mut signers = Vec::new();
    for (i, cms_signer) in cms_signed_data.signer_infos.0.iter().enumerate() {
        debug!("Processing signer #{}", i);
        let signer = parse_signer_info(cms_signer, &certificates);
        signers.push(signer);
    }

    // Extract digest algorithms
    let digest_algorithm = if let Some(first_signer) = signers.first() {
        first_signer.digest_algorithm.clone()
    } else {
        "Unknown".to_string()
    };

    let signature_algorithm = if let Some(first_signer) = signers.first() {
        first_signer.signature_algorithm.clone()
    } else {
        "Unknown".to_string()
    };

    // Perform signature verification if we have signed data
    let verification = if let Some(data) = signed_data {
        verify_signatures(&signers, data, &cms_signed_data)
    } else {
        SignatureVerification::failure("No data provided for verification".to_string())
    };

    Ok(SignatureInfo {
        format: "PKCS#7".to_string(),
        size: signature_bytes.len(),
        digest_algorithm,
        signature_algorithm,
        signer_count: signers.len(),
        certificate_count: certificates.len(),
        certificates,
        verification,
    })
}

/// Parse certificates from `SignedData`
fn parse_certificates(signed_data: &cms::signed_data::SignedData) -> Vec<CertificateInfo> {
    let mut certificates = Vec::new();

    if let Some(cert_set) = &signed_data.certificates {
        for cert_choice in cert_set.0.iter() {
            if let CertificateChoices::Certificate(cert) = cert_choice {
                match parse_certificate(cert) {
                    Ok(cert_info) => certificates.push(cert_info),
                    Err(e) => warn!("Failed to parse certificate: {}", e),
                }
            }
        }
    }

    certificates
}

/// Parse a single certificate
fn parse_certificate(cert: &Certificate) -> Result<CertificateInfo> {
    let tbs = &cert.tbs_certificate;

    // Extract Subject Key Identifier if present
    let subject_key_identifier = extract_subject_key_identifier(cert).ok();

    // Extract public key information
    let spki_der = tbs
        .subject_public_key_info
        .to_der()
        .map_err(|e| ProtocolError::Parse(format!("Failed to encode SubjectPublicKeyInfo: {e}")))?;

    let spki_ref = SubjectPublicKeyInfoRef::from_der(&spki_der)
        .map_err(|e| ProtocolError::Parse(format!("Failed to parse SubjectPublicKeyInfo: {e}")))?;

    let public_key = extract_public_key_info(&spki_ref);

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
                // SKI is OCTET STRING, skip ASN.1 tag and length
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
        "No Subject Key Identifier found".to_string(),
    ))
}

/// Extract public key information from `SubjectPublicKeyInfo`
fn extract_public_key_info(spki: &SubjectPublicKeyInfoRef<'_>) -> PublicKeyInfo {
    let algorithm = oid_to_algorithm_name(&spki.algorithm.oid);
    let key_bytes = spki.subject_public_key.raw_bytes().to_vec();

    // Determine key size based on algorithm
    let key_size = match algorithm.as_str() {
        "RSA" => {
            // Try to decode RSA key to get actual modulus size
            if let Ok(rsa_key) = RsaPublicKey::from_pkcs1_der(&key_bytes) {
                // Import the trait to use the size method
                use rsa::traits::PublicKeyParts;
                rsa_key.size() * 8
            } else {
                // Estimate from key length
                key_bytes.len() * 8
            }
        }
        _ => key_bytes.len() * 8,
    };

    PublicKeyInfo {
        algorithm,
        key_size,
        key_bytes,
    }
}

/// Parse `SignerInfo` from CMS structure
fn parse_signer_info(cms_signer: &CmsSignerInfo, certificates: &[CertificateInfo]) -> SignerInfo {
    // Parse signer identifier
    let identifier = match &cms_signer.sid {
        cms::signed_data::SignerIdentifier::IssuerAndSerialNumber(isn) => {
            SignerIdentifier::IssuerAndSerial {
                issuer: isn.issuer.to_string(),
                serial_number: hex::encode(isn.serial_number.as_bytes()),
            }
        }
        cms::signed_data::SignerIdentifier::SubjectKeyIdentifier(ski) => {
            SignerIdentifier::SubjectKeyIdentifier(hex::encode(ski.0.as_bytes()))
        }
    };

    // Find matching certificate
    let certificate = find_matching_certificate(&identifier, certificates);
    if certificate.is_none() {
        debug!("No matching certificate found for signer: {}", identifier);
    }

    let digest_algorithm = oid_to_algorithm_name(&cms_signer.digest_alg.oid);
    let signature_algorithm = oid_to_algorithm_name(&cms_signer.signature_algorithm.oid);

    SignerInfo {
        identifier,
        digest_algorithm,
        signature_algorithm,
        signature: cms_signer.signature.as_bytes().to_vec(),
        has_signed_attributes: cms_signer.signed_attrs.is_some(),
        certificate,
    }
}

/// Find certificate matching the signer identifier
fn find_matching_certificate(
    identifier: &SignerIdentifier,
    certificates: &[CertificateInfo],
) -> Option<CertificateInfo> {
    certificates
        .iter()
        .find(|cert| match identifier {
            SignerIdentifier::IssuerAndSerial {
                issuer,
                serial_number,
            } => cert.issuer == *issuer && cert.serial_number == *serial_number,
            SignerIdentifier::SubjectKeyIdentifier(ski) => {
                cert.subject_key_identifier.as_ref() == Some(ski)
            }
        })
        .cloned()
}

/// Verify all signatures
fn verify_signatures(
    signers: &[SignerInfo],
    signed_data: &[u8],
    cms_signed_data: &cms::signed_data::SignedData,
) -> SignatureVerification {
    if signers.is_empty() {
        return SignatureVerification::failure("No signers to verify".to_string());
    }

    let mut all_valid = true;
    let mut details = Vec::new();

    // Extract the content to verify from the SignedData structure
    let data_to_verify = extract_signed_content(signed_data, cms_signed_data);

    for (i, signer) in signers.iter().enumerate() {
        let verification_result = verify_single_signature(signer, &data_to_verify);

        match verification_result {
            Ok(valid) => {
                if valid {
                    details.push(format!("Signer #{i} verification: SUCCESS"));
                } else {
                    details.push(format!("Signer #{i} verification: FAILED"));
                    all_valid = false;
                }
            }
            Err(e) => {
                details.push(format!("Signer #{i} verification error: {e}"));
                all_valid = false;
            }
        }
    }

    let message = if all_valid {
        format!("All {} signatures verified successfully", signers.len())
    } else {
        "One or more signature verifications failed".to_string()
    };

    let mut verification = if all_valid {
        SignatureVerification::success(message)
    } else {
        SignatureVerification::failure(message)
    };

    for detail in details {
        verification.add_detail(detail);
    }

    verification
}

/// Extract content to verify from `SignedData` structure
///
/// For detached signatures, uses the provided `signed_data`.
/// For attached signatures, extracts encapsulated content from the CMS structure.
// NOTE: Complexity from handling both attached and detached signature formats.
// Future: Extract separate handlers for attached vs detached signatures.
#[allow(clippy::cognitive_complexity)]
fn extract_signed_content(
    external_data: &[u8],
    cms_signed_data: &cms::signed_data::SignedData,
) -> Vec<u8> {
    // Check if this is an attached signature (has encapContentInfo with content)
    if let Some(ref econtent) = cms_signed_data.encap_content_info.econtent {
        debug!("Extracting content from attached signature");

        // The econtent contains the raw content bytes
        // Use to_der() to get the encoded bytes and then decode as OCTET STRING
        match econtent.to_der() {
            Ok(der_bytes) => {
                // For attached signatures, try to decode as OCTET STRING
                use der::Decode;
                if let Ok(octet_string) = der::asn1::OctetString::from_der(&der_bytes) {
                    let content_bytes = octet_string.as_bytes();
                    debug!(
                        "Extracted {} bytes from attached signature OCTET STRING",
                        content_bytes.len()
                    );
                    return content_bytes.to_vec();
                }
                // Fall back to using raw DER bytes
                debug!(
                    "Using raw DER bytes from attached signature: {} bytes",
                    der_bytes.len()
                );
                return der_bytes;
            }
            Err(e) => {
                warn!("Failed to encode econtent to DER: {}", e);
            }
        }
    }

    debug!("Using external data for detached signature verification");
    external_data.to_vec()
}

/// Extract content from DER-encoded OCTET STRING
#[allow(dead_code)]
fn extract_octet_string_content(der_bytes: &[u8]) -> Result<Vec<u8>> {
    use der::Decode;

    // Parse as OCTET STRING
    let octet_string = der::asn1::OctetString::from_der(der_bytes)
        .map_err(|e| ProtocolError::Parse(format!("Failed to parse OCTET STRING: {e}")))?;

    Ok(octet_string.as_bytes().to_vec())
}

/// Verify a single signature
fn verify_single_signature(signer: &SignerInfo, data: &[u8]) -> Result<bool> {
    // Get the certificate and public key
    let cert = signer.certificate.as_ref().ok_or_else(|| {
        ProtocolError::Parse("No certificate available for signature verification".to_string())
    })?;

    let public_key = cert.public_key.as_ref().ok_or_else(|| {
        ProtocolError::Parse("No public key available in certificate".to_string())
    })?;

    // For RSA signatures, verify using the appropriate digest algorithm
    verify_rsa_signature(
        public_key,
        data,
        &signer.signature,
        &signer.digest_algorithm,
    )
}

/// Verify RSA signature with specified digest algorithm
fn verify_rsa_signature(
    public_key: &PublicKeyInfo,
    data: &[u8],
    signature: &[u8],
    digest_algorithm: &str,
) -> Result<bool> {
    if public_key.algorithm != "RSA" && !public_key.algorithm.starts_with("RSA with") {
        return Err(ProtocolError::Parse(format!(
            "Unsupported algorithm for RSA verification: {}",
            public_key.algorithm
        )));
    }

    // Parse the RSA public key
    let rsa_key = parse_rsa_public_key(&public_key.key_bytes)?;

    // Verify signature based on digest algorithm
    let result = match digest_algorithm {
        "SHA-256" => {
            let verifying_key = rsa::pkcs1v15::VerifyingKey::<Sha256>::new(rsa_key);
            let signature_obj = rsa::pkcs1v15::Signature::try_from(signature)
                .map_err(|e| ProtocolError::Parse(format!("Invalid signature format: {e}")))?;
            verifying_key.verify(data, &signature_obj).is_ok()
        }
        "SHA-384" => {
            let verifying_key = rsa::pkcs1v15::VerifyingKey::<Sha384>::new(rsa_key);
            let signature_obj = rsa::pkcs1v15::Signature::try_from(signature)
                .map_err(|e| ProtocolError::Parse(format!("Invalid signature format: {e}")))?;
            verifying_key.verify(data, &signature_obj).is_ok()
        }
        "SHA-512" => {
            let verifying_key = rsa::pkcs1v15::VerifyingKey::<Sha512>::new(rsa_key);
            let signature_obj = rsa::pkcs1v15::Signature::try_from(signature)
                .map_err(|e| ProtocolError::Parse(format!("Invalid signature format: {e}")))?;
            verifying_key.verify(data, &signature_obj).is_ok()
        }
        _ => {
            return Err(ProtocolError::Parse(format!(
                "Unsupported digest algorithm: {digest_algorithm}"
            )));
        }
    };

    trace!(
        "RSA signature verification with {}: {}",
        digest_algorithm,
        if result { "SUCCESS" } else { "FAILED" }
    );

    Ok(result)
}

/// Parse RSA public key from various formats
fn parse_rsa_public_key(key_bytes: &[u8]) -> Result<RsaPublicKey> {
    // Try PKCS#1 format first
    if let Ok(key) = RsaPublicKey::from_pkcs1_der(key_bytes) {
        return Ok(key);
    }

    // Try SubjectPublicKeyInfo format
    if let Ok(spki) = x509_cert::spki::SubjectPublicKeyInfoOwned::from_der(key_bytes)
        && let Ok(key) = RsaPublicKey::from_pkcs1_der(spki.subject_public_key.raw_bytes())
    {
        return Ok(key);
    }

    Err(ProtocolError::Parse(
        "Failed to parse RSA public key".to_string(),
    ))
}

/// Convert ASN.1 OID to human-readable algorithm name
fn oid_to_algorithm_name(oid: &der::asn1::ObjectIdentifier) -> String {
    match oid.to_string().as_str() {
        // Digest algorithms
        "2.16.840.1.101.3.4.2.1" => "SHA-256".to_string(),
        "2.16.840.1.101.3.4.2.2" => "SHA-384".to_string(),
        "2.16.840.1.101.3.4.2.3" => "SHA-512".to_string(),
        "1.3.14.3.2.26" => "SHA-1".to_string(),
        "1.2.840.113549.2.5" => "MD5".to_string(),

        // Signature algorithms
        "1.2.840.113549.1.1.11" => "RSA with SHA-256".to_string(),
        "1.2.840.113549.1.1.12" => "RSA with SHA-384".to_string(),
        "1.2.840.113549.1.1.13" => "RSA with SHA-512".to_string(),
        "1.2.840.113549.1.1.5" => "RSA with SHA-1".to_string(),
        "1.2.840.113549.1.1.4" => "RSA with MD5".to_string(),
        "1.2.840.113549.1.1.1" => "RSA".to_string(),

        // ECDSA algorithms
        "1.2.840.10045.4.3.2" => "ECDSA with SHA-256".to_string(),
        "1.2.840.10045.4.3.3" => "ECDSA with SHA-384".to_string(),
        "1.2.840.10045.4.3.4" => "ECDSA with SHA-512".to_string(),
        "1.2.840.10045.2.1" => "ECDSA".to_string(),

        _ => format!("OID: {oid}"),
    }
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
    fn test_oid_to_algorithm_name() {
        use der::asn1::ObjectIdentifier;

        let sha256_oid =
            ObjectIdentifier::new("2.16.840.1.101.3.4.2.1").expect("Operation should succeed");
        assert_eq!(oid_to_algorithm_name(&sha256_oid), "SHA-256");

        let rsa_sha256_oid =
            ObjectIdentifier::new("1.2.840.113549.1.1.11").expect("Operation should succeed");
        assert_eq!(oid_to_algorithm_name(&rsa_sha256_oid), "RSA with SHA-256");

        let rsa_oid =
            ObjectIdentifier::new("1.2.840.113549.1.1.1").expect("Operation should succeed");
        assert_eq!(oid_to_algorithm_name(&rsa_oid), "RSA");
    }

    #[test]
    fn test_signer_identifier_display() {
        let issuer_serial = SignerIdentifier::IssuerAndSerial {
            issuer: "CN=Test CA".to_string(),
            serial_number: "123456".to_string(),
        };
        assert_eq!(
            format!("{}", issuer_serial),
            "Issuer: CN=Test CA, Serial: 123456"
        );

        let ski = SignerIdentifier::SubjectKeyIdentifier("abcdef".to_string());
        assert_eq!(format!("{}", ski), "SubjectKeyIdentifier: abcdef");
    }
}
