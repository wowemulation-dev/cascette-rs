//! Tests for CMS/PKCS#7 parser

#[cfg(test)]
mod tests {
    use ribbit_client::cms_parser::{CmsSignatureInfo, parse_cms_signature};

    #[test]
    fn test_parse_empty_signature() {
        let result = parse_cms_signature(&[]);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_invalid_signature() {
        let invalid_data = b"Not a valid CMS signature";
        let result = parse_cms_signature(invalid_data);
        assert!(result.is_err());
    }

    #[test]
    fn test_signature_info_structure() {
        // This test verifies the structure is accessible
        let _ = CmsSignatureInfo {
            signed_data: ribbit_client::cms_parser::SignedDataInfo {
                version: 1,
                digest_algorithms: vec!["SHA-256".to_string()],
                is_detached: true,
            },
            signers: vec![],
            certificates: vec![],
            raw_signed_data: vec![],
        };
    }
}
