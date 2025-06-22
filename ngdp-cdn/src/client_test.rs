//! Tests for CDN client helper methods

#[cfg(test)]
mod tests {
    use crate::CdnClient;

    #[test]
    fn test_build_config_path() {
        let _client = CdnClient::new().unwrap();

        // BuildConfig should append /config to the path
        let url = CdnClient::build_url(
            "cdn.example.com",
            "tpr/wow/config",
            "abcd1234567890abcdef1234567890ab",
        )
        .unwrap();

        assert_eq!(
            url,
            "http://cdn.example.com/tpr/wow/config/ab/cd/abcd1234567890abcdef1234567890ab"
        );
    }

    #[test]
    fn test_product_config_path() {
        let _client = CdnClient::new().unwrap();

        // ProductConfig uses config_path directly
        let url = CdnClient::build_url(
            "cdn.example.com",
            "tpr/configs/data",
            "1234567890abcdef1234567890abcdef",
        )
        .unwrap();

        assert_eq!(
            url,
            "http://cdn.example.com/tpr/configs/data/12/34/1234567890abcdef1234567890abcdef"
        );
    }

    #[test]
    fn test_data_path() {
        let _client = CdnClient::new().unwrap();

        // Data files should use /data suffix
        let url = CdnClient::build_url(
            "cdn.example.com",
            "tpr/wow/data",
            "fedcba9876543210fedcba9876543210",
        )
        .unwrap();

        assert_eq!(
            url,
            "http://cdn.example.com/tpr/wow/data/fe/dc/fedcba9876543210fedcba9876543210"
        );
    }

    #[test]
    fn test_path_trimming() {
        // Test that trailing slashes are handled correctly
        let _client = CdnClient::new().unwrap();

        let url1 = CdnClient::build_url(
            "cdn.example.com",
            "tpr/wow/",
            "abcd1234567890abcdef1234567890ab",
        )
        .unwrap();

        let url2 = CdnClient::build_url(
            "cdn.example.com",
            "tpr/wow",
            "abcd1234567890abcdef1234567890ab",
        )
        .unwrap();

        assert_eq!(url1, url2);
    }

    #[test]
    fn test_invalid_hash() {
        // Too short
        let result = CdnClient::build_url("cdn.example.com", "tpr/wow/config", "ab");
        assert!(result.is_err());

        // Non-hex characters
        let result = CdnClient::build_url("cdn.example.com", "tpr/wow/config", "invalid");
        assert!(result.is_err());

        // Non-hex characters with valid length
        let result = CdnClient::build_url("cdn.example.com", "tpr/wow/config", "zzzz1234567890ab");
        assert!(result.is_err());

        // Empty hash
        let result = CdnClient::build_url("cdn.example.com", "tpr/wow/config", "");
        assert!(result.is_err());
    }

    #[test]
    fn test_general_url_building() {
        // Test basic URL building
        let url = CdnClient::build_url(
            "blzddist1-a.akamaihd.net",
            "tpr/wow",
            "2e9c1e3b5f5a0c9d9e8f1234567890ab",
        )
        .unwrap();

        assert_eq!(
            url,
            "http://blzddist1-a.akamaihd.net/tpr/wow/2e/9c/2e9c1e3b5f5a0c9d9e8f1234567890ab"
        );
    }
}
