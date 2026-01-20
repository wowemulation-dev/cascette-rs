//! V1 MIME types and data structures
//!
//! This module defines types specific to V1 MIME protocol handling,
//! including PKCS#7 signature information and verification results.

use std::fmt;

/// V1 MIME parsing and verification result
#[derive(Debug, Clone)]
pub struct V1MimeResponse {
    /// The raw response bytes
    pub raw: Vec<u8>,
    /// The extracted data content (PSV format)
    pub data: String,
    /// Signature information if present
    pub signature_info: Option<SignatureInfo>,
    /// Checksum from epilogue if present
    pub checksum: Option<String>,
}

/// Information about a PKCS#7 signature
#[derive(Debug, Clone)]
pub struct SignatureInfo {
    /// Signature format (e.g., "PKCS#7", "CMS")
    pub format: String,
    /// Size of the signature in bytes
    pub size: usize,
    /// Digest algorithm used
    pub digest_algorithm: String,
    /// Signature algorithm used
    pub signature_algorithm: String,
    /// Number of signers
    pub signer_count: usize,
    /// Number of certificates in the signature
    pub certificate_count: usize,
    /// All certificates found in the signature
    pub certificates: Vec<CertificateInfo>,
    /// Signature verification result
    pub verification: SignatureVerification,
}

/// Certificate information extracted from PKCS#7 signature
#[derive(Debug, Clone)]
pub struct CertificateInfo {
    /// Certificate subject distinguished name
    pub subject: String,
    /// Certificate issuer distinguished name
    pub issuer: String,
    /// Certificate serial number (hex string)
    pub serial_number: String,
    /// Public key information
    pub public_key: Option<PublicKeyInfo>,
    /// Subject Key Identifier (hex string) if present
    pub subject_key_identifier: Option<String>,
}

/// Public key information
#[derive(Debug, Clone)]
pub struct PublicKeyInfo {
    /// Algorithm (e.g., "RSA", "ECDSA")
    pub algorithm: String,
    /// Key size in bits
    pub key_size: usize,
    /// DER-encoded public key bytes
    pub key_bytes: Vec<u8>,
}

/// Signature verification result
#[derive(Debug, Clone)]
pub struct SignatureVerification {
    /// Whether the signature is valid
    pub is_valid: bool,
    /// Verification details or error message
    pub message: String,
    /// Whether certificate chain was verified
    pub certificate_chain_valid: bool,
    /// Additional verification details
    pub details: Vec<String>,
}

/// Signer information from PKCS#7 signature
#[derive(Debug, Clone)]
pub struct SignerInfo {
    /// Signer identifier
    pub identifier: SignerIdentifier,
    /// Digest algorithm used by this signer
    pub digest_algorithm: String,
    /// Signature algorithm used by this signer
    pub signature_algorithm: String,
    /// The signature bytes
    pub signature: Vec<u8>,
    /// Whether this signer has signed attributes
    pub has_signed_attributes: bool,
    /// The certificate used by this signer (if available)
    pub certificate: Option<CertificateInfo>,
}

/// Signer identifier from PKCS#7
#[derive(Debug, Clone)]
pub enum SignerIdentifier {
    /// Issuer and serial number
    IssuerAndSerial {
        issuer: String,
        serial_number: String,
    },
    /// Subject Key Identifier
    SubjectKeyIdentifier(String),
}

impl fmt::Display for SignerIdentifier {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::IssuerAndSerial {
                issuer,
                serial_number,
            } => {
                write!(f, "Issuer: {issuer}, Serial: {serial_number}")
            }
            Self::SubjectKeyIdentifier(ski) => {
                write!(f, "SubjectKeyIdentifier: {ski}")
            }
        }
    }
}

impl Default for SignatureVerification {
    fn default() -> Self {
        Self {
            is_valid: false,
            message: "Not verified".to_string(),
            certificate_chain_valid: false,
            details: Vec::new(),
        }
    }
}

impl SignatureVerification {
    /// Create a successful verification result
    pub fn success(message: String) -> Self {
        Self {
            is_valid: true,
            message,
            certificate_chain_valid: false, // Will be updated separately
            details: Vec::new(),
        }
    }

    /// Create a failed verification result
    pub fn failure(message: String) -> Self {
        Self {
            is_valid: false,
            message,
            certificate_chain_valid: false,
            details: Vec::new(),
        }
    }

    /// Add verification detail
    pub fn add_detail(&mut self, detail: String) {
        self.details.push(detail);
    }

    /// Set certificate chain validation status
    pub fn set_certificate_chain_valid(&mut self, valid: bool) {
        self.certificate_chain_valid = valid;
    }
}
