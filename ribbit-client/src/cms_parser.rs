//! CMS/PKCS#7 parser for extracting signer certificates and public keys
//!
//! This module provides proper PKCS#7 parsing using the cms crate to:
//! - Parse `SignedData` structures
//! - Extract signer certificates
//! - Extract public keys for signature verification

use crate::error::{Error, Result};
use cms::cert::CertificateChoices;
use cms::content_info::ContentInfo;
use cms::signed_data::SignerInfo;
use der::{Decode, Encode};
use rsa::RsaPublicKey;
use rsa::pkcs1::DecodeRsaPublicKey;
use rsa::signature::Verifier;
use rsa::traits::PublicKeyParts;
use sha2::{Sha256, Sha384, Sha512};
use tracing::{debug, trace, warn};
use x509_cert::certificate::Certificate;
use x509_cert::spki::SubjectPublicKeyInfoRef;

/// Information about a parsed CMS/PKCS#7 signature
#[derive(Debug, Clone)]
pub struct CmsSignatureInfo {
    /// The `SignedData` structure
    pub signed_data: SignedDataInfo,
    /// Information about each signer
    pub signers: Vec<SignerDetails>,
    /// All certificates in the signature
    pub certificates: Vec<CertificateDetails>,
    /// Raw `SignedData` for verification
    pub raw_signed_data: Vec<u8>,
}

/// Parsed `SignedData` information
#[derive(Debug, Clone)]
pub struct SignedDataInfo {
    /// CMS version
    pub version: u8,
    /// Digest algorithms used
    pub digest_algorithms: Vec<String>,
    /// Whether this contains detached signature
    pub is_detached: bool,
}

/// Details about a signer
#[derive(Debug, Clone)]
pub struct SignerDetails {
    /// Signer identifier (issuer and serial)
    pub identifier: SignerIdentifier,
    /// Digest algorithm used
    pub digest_algorithm: String,
    /// Signature algorithm used
    pub signature_algorithm: String,
    /// The signature value
    pub signature: Vec<u8>,
    /// The signer's certificate (if found)
    pub certificate: Option<CertificateDetails>,
    /// The public key (if extracted)
    pub public_key: Option<PublicKeyInfo>,
    /// Whether signed attributes are present
    pub has_signed_attributes: bool,
    /// DER-encoded signed attributes (if present)
    pub signed_attributes_der: Option<Vec<u8>>,
}

/// Signer identifier
#[derive(Debug, Clone)]
pub struct SignerIdentifier {
    /// Issuer distinguished name
    pub issuer: String,
    /// Serial number (hex)
    pub serial_number: String,
}

/// Certificate details
#[derive(Debug, Clone)]
pub struct CertificateDetails {
    /// Subject DN
    pub subject: String,
    /// Issuer DN
    pub issuer: String,
    /// Serial number (hex)
    pub serial_number: String,
    /// Public key info
    pub public_key: Option<PublicKeyInfo>,
}

/// Public key information
#[derive(Debug, Clone)]
pub struct PublicKeyInfo {
    /// Algorithm (e.g., "RSA", "ECDSA")
    pub algorithm: String,
    /// Key size in bits
    pub key_size: usize,
    /// The actual public key bytes (DER encoded)
    pub key_bytes: Vec<u8>,
}

/// Parse a CMS/PKCS#7 signature and extract signer information
///
/// # Errors
/// Returns an error if:
/// - The input is not a valid CMS/PKCS#7 structure
/// - The `ContentInfo` cannot be parsed
/// - The `SignedData` structure is malformed
/// - Certificate parsing fails
pub fn parse_cms_signature(signature_bytes: &[u8]) -> Result<CmsSignatureInfo> {
    trace!("Parsing CMS signature: {} bytes", signature_bytes.len());

    // Parse as ContentInfo
    let content_info = ContentInfo::from_der(signature_bytes)
        .map_err(|e| Error::Asn1Error(format!("Failed to parse ContentInfo: {e:?}")))?;

    // Check content type - SignedData OID is 1.2.840.113549.1.7.2
    let signed_data_oid = der::asn1::ObjectIdentifier::new("1.2.840.113549.1.7.2")
        .map_err(|e| Error::Asn1Error(format!("Invalid OID: {e}")))?;

    if content_info.content_type != signed_data_oid {
        return Err(Error::Asn1Error(
            "ContentInfo is not SignedData".to_string(),
        ));
    }

    // Re-encode the AnyRef to get the SignedData bytes
    let signed_data_bytes = content_info
        .content
        .to_der()
        .map_err(|e| Error::Asn1Error(format!("Failed to encode content: {e:?}")))?;

    // Parse as SignedData
    let signed_data = cms::signed_data::SignedData::from_der(&signed_data_bytes)
        .map_err(|e| Error::Asn1Error(format!("Failed to parse SignedData: {e:?}")))?;

    debug!(
        "Parsed SignedData: {} signers",
        signed_data.signer_infos.0.len()
    );

    // Parse digest algorithms
    let digest_algorithms: Vec<String> = signed_data
        .digest_algorithms
        .iter()
        .map(|alg| oid_to_algorithm_name(&alg.oid))
        .collect();

    // Check if detached signature (no encapsulated content)
    let is_detached = signed_data.encap_content_info.econtent.is_none();

    // Parse all certificates
    let mut certificates = Vec::new();
    if let Some(cert_set) = &signed_data.certificates {
        debug!("Certificate set present with {} entries", cert_set.0.len());
        for (i, cert_choice) in cert_set.0.iter().enumerate() {
            match cert_choice {
                CertificateChoices::Certificate(cert) => {
                    debug!("Entry {} is a Certificate", i);
                    if let Ok(details) = extract_certificate_details(cert) {
                        certificates.push(details);
                    }
                }
                CertificateChoices::Other(_) => {
                    debug!(
                        "Entry {} is not a Certificate (different CertificateChoice variant)",
                        i
                    );
                }
            }
        }
    } else {
        debug!("No certificate set in SignedData");
    }

    debug!("Found {} certificates", certificates.len());

    // Parse each signer
    let mut signers = Vec::new();
    debug!("Processing {} signers", signed_data.signer_infos.0.len());
    for (i, signer_info) in signed_data.signer_infos.0.iter().enumerate() {
        debug!("Processing signer #{}", i);
        match parse_signer_info(signer_info, &certificates) {
            Ok(signer) => {
                debug!("Successfully parsed signer #{}", i);
                signers.push(signer);
            }
            Err(e) => {
                warn!("Failed to parse signer #{}: {}", i, e);
            }
        }
    }

    Ok(CmsSignatureInfo {
        signed_data: SignedDataInfo {
            version: 1, // CMS version is usually 1 or 3
            digest_algorithms,
            is_detached,
        },
        signers,
        certificates,
        raw_signed_data: signed_data_bytes,
    })
}

/// Parse a `SignerInfo` and match with certificate
fn parse_signer_info(
    signer_info: &SignerInfo,
    certificates: &[CertificateDetails],
) -> Result<SignerDetails> {
    let identifier = match &signer_info.sid {
        cms::signed_data::SignerIdentifier::IssuerAndSerialNumber(isn) => {
            debug!("Signer uses IssuerAndSerialNumber");
            SignerIdentifier {
                issuer: isn.issuer.to_string(),
                serial_number: hex::encode(isn.serial_number.as_bytes()),
            }
        }
        cms::signed_data::SignerIdentifier::SubjectKeyIdentifier(ski) => {
            debug!("Signer uses SubjectKeyIdentifier");
            // Convert SKI to hex string for identification
            let ski_hex = hex::encode(ski.0.as_bytes());
            SignerIdentifier {
                issuer: format!("SubjectKeyIdentifier: {ski_hex}"),
                serial_number: ski_hex,
            }
        }
    };

    debug!(
        "Looking for certificate matching issuer='{}', serial='{}'",
        identifier.issuer, identifier.serial_number
    );

    // Find matching certificate
    let certificate = certificates
        .iter()
        .find(|cert| {
            let matches =
                cert.issuer == identifier.issuer && cert.serial_number == identifier.serial_number;
            if !matches {
                trace!(
                    "Certificate mismatch: cert.issuer='{}', cert.serial='{}'",
                    cert.issuer, cert.serial_number
                );
            }
            matches
        })
        .cloned();

    // Extract public key if we have the certificate
    let public_key = certificate
        .as_ref()
        .and_then(|cert| cert.public_key.clone());

    if certificate.is_none() {
        warn!(
            "No certificate found for signer: issuer='{}', serial='{}'",
            identifier.issuer, identifier.serial_number
        );
        debug!("Available certificates: {}", certificates.len());
    } else {
        debug!("Found certificate for signer");
    }

    // Check for signed attributes and encode them if present
    let (has_signed_attributes, signed_attributes_der) = if let Some(signed_attrs) =
        &signer_info.signed_attrs
    {
        debug!("Signer has {} signed attributes", signed_attrs.len());

        // For CMS signature verification, we need to encode the signed attributes
        // as a SET OF (implicit tag [0]) for signature verification
        let mut attr_bytes = Vec::new();

        // We need to re-encode as SET instead of implicit [0]
        // First collect all attributes
        let mut encoded_attrs = Vec::new();
        for attr in signed_attrs.iter() {
            encoded_attrs.push(
                attr.to_der()
                    .map_err(|e| Error::Asn1Error(format!("Failed to encode attribute: {e}")))?,
            );
        }

        // Sort for SET encoding (DER canonical)
        encoded_attrs.sort();

        // Manually build SET OF
        attr_bytes.push(0x31); // SET tag

        // Calculate length
        let content_len: usize = encoded_attrs.iter().map(std::vec::Vec::len).sum();
        if content_len < 128 {
            #[allow(clippy::cast_possible_truncation)]
            {
                attr_bytes.push(content_len as u8);
            }
        } else {
            // Long form
            let len_bytes = content_len.to_be_bytes();
            let len_bytes = &len_bytes[len_bytes.iter().position(|&b| b != 0).unwrap_or(0)..];
            #[allow(clippy::cast_possible_truncation)]
            {
                attr_bytes.push(0x80 | len_bytes.len() as u8);
            }
            attr_bytes.extend_from_slice(len_bytes);
        }

        // Add all attributes
        for attr in encoded_attrs {
            attr_bytes.extend_from_slice(&attr);
        }

        (true, Some(attr_bytes))
    } else {
        debug!("Signer has no signed attributes - signature is directly over content");
        (false, None)
    };

    Ok(SignerDetails {
        identifier,
        digest_algorithm: oid_to_algorithm_name(&signer_info.digest_alg.oid),
        signature_algorithm: oid_to_algorithm_name(&signer_info.signature_algorithm.oid),
        signature: signer_info.signature.as_bytes().to_vec(),
        certificate,
        public_key,
        has_signed_attributes,
        signed_attributes_der,
    })
}

/// Extract details from a certificate
fn extract_certificate_details(cert: &Certificate) -> Result<CertificateDetails> {
    let tbs = &cert.tbs_certificate;

    // Extract public key info
    // Convert SPKI to owned type for compatibility
    let spki_der = tbs
        .subject_public_key_info
        .to_der()
        .map_err(|e| Error::Asn1Error(format!("Failed to encode SPKI: {e}")))?;
    let spki_ref = SubjectPublicKeyInfoRef::from_der(&spki_der)
        .map_err(|e| Error::Asn1Error(format!("Failed to parse SPKI: {e}")))?;

    let public_key = extract_public_key_info(&spki_ref);

    Ok(CertificateDetails {
        subject: tbs.subject.to_string(),
        issuer: tbs.issuer.to_string(),
        serial_number: hex::encode(tbs.serial_number.as_bytes()),
        public_key: Some(public_key),
    })
}

/// Extract public key information from `SubjectPublicKeyInfo`
fn extract_public_key_info(spki: &SubjectPublicKeyInfoRef<'_>) -> PublicKeyInfo {
    let algorithm = oid_to_algorithm_name(&spki.algorithm.oid);
    let key_bytes = spki.subject_public_key.raw_bytes().to_vec();

    // Try to determine key size
    let key_size = match algorithm.as_str() {
        "RSA" => {
            // Try to decode as RSA public key to get modulus size
            if let Ok(rsa_key) = RsaPublicKey::from_pkcs1_der(spki.subject_public_key.raw_bytes()) {
                // Estimate key size from modulus length
                rsa_key.size() * 8
            } else {
                // Fallback: estimate from key length
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

/// Convert OID to human-readable algorithm name
fn oid_to_algorithm_name(oid: &der::asn1::ObjectIdentifier) -> String {
    match oid.to_string().as_str() {
        // Digest algorithms
        "2.16.840.1.101.3.4.2.1" => "SHA-256".to_string(),
        "2.16.840.1.101.3.4.2.2" => "SHA-384".to_string(),
        "2.16.840.1.101.3.4.2.3" => "SHA-512".to_string(),
        "1.3.14.3.2.26" => "SHA-1".to_string(),

        // Signature algorithms
        "1.2.840.113549.1.1.11" => "RSA with SHA-256".to_string(),
        "1.2.840.113549.1.1.12" => "RSA with SHA-384".to_string(),
        "1.2.840.113549.1.1.13" => "RSA with SHA-512".to_string(),
        "1.2.840.113549.1.1.5" => "RSA with SHA-1".to_string(),
        "1.2.840.113549.1.1.1" => "RSA".to_string(),

        // ECDSA
        "1.2.840.10045.4.3.2" => "ECDSA with SHA-256".to_string(),
        "1.2.840.10045.4.3.3" => "ECDSA with SHA-384".to_string(),
        "1.2.840.10045.4.3.4" => "ECDSA with SHA-512".to_string(),

        _ => format!("OID: {oid}"),
    }
}

/// Verify a signature using the extracted public key
///
/// For CMS signatures with signed attributes, pass the DER-encoded attributes
/// as `signed_data`. For direct signatures, pass the original content.
///
/// # Errors
/// Returns an error if the public key algorithm is unsupported or signature verification fails.
pub fn verify_with_public_key(
    public_key: &PublicKeyInfo,
    signed_data: &[u8],
    signature: &[u8],
    digest_algorithm: &str,
) -> Result<bool> {
    match public_key.algorithm.as_str() {
        algo if algo == "RSA" || algo.starts_with("RSA with") => {
            verify_rsa_signature(public_key, signed_data, signature, digest_algorithm)
        }
        _ => Err(Error::Asn1Error(format!(
            "Unsupported algorithm for verification: {}",
            public_key.algorithm
        ))),
    }
}

/// Verify RSA signature
fn verify_rsa_signature(
    public_key: &PublicKeyInfo,
    signed_data: &[u8],
    signature: &[u8],
    digest_algorithm: &str,
) -> Result<bool> {
    // Parse the public key from DER format
    // The key_bytes contain the SubjectPublicKeyInfo, we need to extract the actual RSA key
    let rsa_key = if let Ok(key) = RsaPublicKey::from_pkcs1_der(&public_key.key_bytes) {
        key
    } else {
        // Try parsing as SubjectPublicKeyInfo
        let spki = x509_cert::spki::SubjectPublicKeyInfoOwned::from_der(&public_key.key_bytes)
            .map_err(|e| Error::Asn1Error(format!("Failed to parse SubjectPublicKeyInfo: {e}")))?;

        RsaPublicKey::from_pkcs1_der(spki.subject_public_key.raw_bytes())
            .map_err(|e| Error::Asn1Error(format!("Failed to decode RSA key from SPKI: {e}")))?
    };

    // Create the appropriate verifying key based on the digest algorithm
    let result = match digest_algorithm {
        "SHA-256" => {
            let verifying_key = rsa::pkcs1v15::VerifyingKey::<Sha256>::new(rsa_key);
            let signature_obj = rsa::pkcs1v15::Signature::try_from(signature)
                .map_err(|e| Error::Asn1Error(format!("Invalid signature format: {e}")))?;
            verifying_key.verify(signed_data, &signature_obj).is_ok()
        }
        "SHA-384" => {
            let verifying_key = rsa::pkcs1v15::VerifyingKey::<Sha384>::new(rsa_key);
            let signature_obj = rsa::pkcs1v15::Signature::try_from(signature)
                .map_err(|e| Error::Asn1Error(format!("Invalid signature format: {e}")))?;
            verifying_key.verify(signed_data, &signature_obj).is_ok()
        }
        "SHA-512" => {
            let verifying_key = rsa::pkcs1v15::VerifyingKey::<Sha512>::new(rsa_key);
            let signature_obj = rsa::pkcs1v15::Signature::try_from(signature)
                .map_err(|e| Error::Asn1Error(format!("Invalid signature format: {e}")))?;
            verifying_key.verify(signed_data, &signature_obj).is_ok()
        }
        _ => {
            return Err(Error::Asn1Error(format!(
                "Unsupported digest algorithm: {digest_algorithm}"
            )));
        }
    };

    debug!(
        "RSA signature verification with {}: {}",
        digest_algorithm,
        if result { "SUCCESS" } else { "FAILED" }
    );

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_oid_to_algorithm_name() {
        use der::asn1::ObjectIdentifier;

        let sha256_oid = ObjectIdentifier::new("2.16.840.1.101.3.4.2.1").unwrap();
        assert_eq!(oid_to_algorithm_name(&sha256_oid), "SHA-256");

        let rsa_sha256_oid = ObjectIdentifier::new("1.2.840.113549.1.1.11").unwrap();
        assert_eq!(oid_to_algorithm_name(&rsa_sha256_oid), "RSA with SHA-256");
    }
}
