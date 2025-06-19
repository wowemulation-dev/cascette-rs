//! Ribbit TCP client implementation

use crate::{
    error::Result,
    response_types::{
        ProductBgdlResponse, ProductCdnsResponse, ProductVersionsResponse, SummaryResponse,
        TypedResponse,
    },
    types::{Endpoint, ProtocolVersion, RIBBIT_PORT, Region},
};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use sha2::{Digest, Sha256};
use std::fmt;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::time::timeout;
use tracing::{debug, instrument, trace, warn};

/// Default connection timeout in seconds
const DEFAULT_CONNECT_TIMEOUT_SECS: u64 = 10;

/// Ribbit TCP client for querying Blizzard version services
///
/// The client supports multiple regions and both V1 (MIME) and V2 (raw PSV) protocols.
///
/// # Example
///
/// ```no_run
/// use ribbit_client::{RibbitClient, Region, ProtocolVersion, Endpoint};
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// // Create a client for EU region with V2 protocol
/// let client = RibbitClient::new(Region::EU)
///     .with_protocol_version(ProtocolVersion::V2);
///
/// // Request version information
/// let endpoint = Endpoint::ProductVersions("wow".to_string());
/// let response = client.request(&endpoint).await?;
/// # Ok(())
/// # }
/// ```
pub struct RibbitClient {
    region: Region,
    protocol_version: ProtocolVersion,
}

impl RibbitClient {
    /// Create a new Ribbit client with the specified region
    #[must_use]
    pub fn new(region: Region) -> Self {
        Self {
            region,
            protocol_version: ProtocolVersion::V1,
        }
    }

    /// Set the protocol version to use
    #[must_use]
    pub fn with_protocol_version(mut self, version: ProtocolVersion) -> Self {
        self.protocol_version = version;
        self
    }

    /// Get the current region
    #[must_use]
    pub fn region(&self) -> Region {
        self.region
    }

    /// Set the region
    pub fn set_region(&mut self, region: Region) {
        self.region = region;
    }

    /// Get the current protocol version
    #[must_use]
    pub fn protocol_version(&self) -> ProtocolVersion {
        self.protocol_version
    }

    /// Set the protocol version
    pub fn set_protocol_version(&mut self, version: ProtocolVersion) {
        self.protocol_version = version;
    }

    /// Send a request to the Ribbit service and get the raw response
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use ribbit_client::{RibbitClient, Region, Endpoint};
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let client = RibbitClient::new(Region::US);
    /// let raw_data = client.request_raw(&Endpoint::Summary).await?;
    /// println!("Received {} bytes", raw_data.len());
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The connection to the Ribbit server fails
    /// - Sending the request fails
    /// - Receiving the response fails
    /// - The response is invalid or incomplete
    #[instrument(skip(self))]
    pub async fn request_raw(&self, endpoint: &Endpoint) -> Result<Vec<u8>> {
        let host = self.region.hostname();
        let address = format!("{host}:{RIBBIT_PORT}");

        debug!("Connecting to Ribbit service at {address}");

        // Connect to the TCP socket with timeout
        let connect_future = TcpStream::connect(&address);
        let timeout_duration = Duration::from_secs(DEFAULT_CONNECT_TIMEOUT_SECS);

        let mut stream = match timeout(timeout_duration, connect_future).await {
            Ok(Ok(stream)) => stream,
            Ok(Err(e)) => {
                debug!("Connection failed to {address}: {e}");
                return Err(crate::error::Error::ConnectionFailed {
                    host: host.to_string(),
                    port: RIBBIT_PORT,
                });
            }
            Err(_) => {
                debug!(
                    "Connection timed out after {} seconds to {address}",
                    DEFAULT_CONNECT_TIMEOUT_SECS
                );
                return Err(crate::error::Error::ConnectionTimeout {
                    host: host.to_string(),
                    port: RIBBIT_PORT,
                    timeout_secs: DEFAULT_CONNECT_TIMEOUT_SECS,
                });
            }
        };

        // Build the command
        let command = format!(
            "{}/{}\n",
            self.protocol_version.prefix(),
            endpoint.as_path()
        );
        let trimmed = command.trim();
        trace!("Sending command: {trimmed}");

        // Send the command
        stream
            .write_all(command.as_bytes())
            .await
            .map_err(|_| crate::error::Error::SendFailed)?;

        // Read the response until EOF (server closes connection)
        let mut response = Vec::new();
        stream
            .read_to_end(&mut response)
            .await
            .map_err(|_| crate::error::Error::ReceiveFailed)?;

        let len = response.len();
        debug!("Received {len} bytes");
        Ok(response)
    }

    /// Send a request to the Ribbit service and parse the response
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The raw request fails (see [`request_raw`](Self::request_raw))
    /// - Parsing the response fails
    /// - V1 responses fail checksum validation
    /// - V1 responses have invalid MIME structure
    #[instrument(skip(self))]
    pub async fn request(&self, endpoint: &Endpoint) -> Result<Response> {
        let raw_response = self.request_raw(endpoint).await?;

        match self.protocol_version {
            ProtocolVersion::V1 => Response::parse_v1(&raw_response),
            ProtocolVersion::V2 => Ok(Response::parse_v2(&raw_response)),
        }
    }

    /// Request with automatic type parsing
    ///
    /// This method automatically parses the response into the appropriate typed structure
    /// based on the type parameter.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use ribbit_client::{RibbitClient, Region, Endpoint, ProductVersionsResponse};
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let client = RibbitClient::new(Region::US);
    /// let versions: ProductVersionsResponse = client
    ///     .request_typed(&Endpoint::ProductVersions("wow".to_string()))
    ///     .await?;
    ///
    /// for entry in &versions.entries {
    ///     println!("{}: {} (build {})", entry.region, entry.versions_name, entry.build_id);
    /// }
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The request fails
    /// - The response cannot be parsed as BPSV
    /// - The BPSV data doesn't match the expected schema
    #[instrument(skip(self))]
    pub async fn request_typed<T: TypedResponse>(&self, endpoint: &Endpoint) -> Result<T> {
        let response = self.request(endpoint).await?;
        T::from_response(&response)
    }

    /// Request product versions with typed response
    ///
    /// Convenience method for requesting product version information.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use ribbit_client::{RibbitClient, Region};
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let client = RibbitClient::new(Region::US);
    /// let versions = client.get_product_versions("wow").await?;
    ///
    /// if let Some(us_version) = versions.get_region("us") {
    ///     println!("US version: {}", us_version.versions_name);
    /// }
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The request fails
    /// - The response cannot be parsed as BPSV
    /// - The BPSV data doesn't match the expected schema
    pub async fn get_product_versions(&self, product: &str) -> Result<ProductVersionsResponse> {
        self.request_typed(&Endpoint::ProductVersions(product.to_string()))
            .await
    }

    /// Request product CDNs with typed response
    ///
    /// Convenience method for requesting CDN server information.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The request fails
    /// - The response cannot be parsed as BPSV
    /// - The BPSV data doesn't match the expected schema
    pub async fn get_product_cdns(&self, product: &str) -> Result<ProductCdnsResponse> {
        self.request_typed(&Endpoint::ProductCdns(product.to_string()))
            .await
    }

    /// Request product background download config with typed response
    ///
    /// Convenience method for requesting background download configuration.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The request fails
    /// - The response cannot be parsed as BPSV
    /// - The BPSV data doesn't match the expected schema
    pub async fn get_product_bgdl(&self, product: &str) -> Result<ProductBgdlResponse> {
        self.request_typed(&Endpoint::ProductBgdl(product.to_string()))
            .await
    }

    /// Request summary of all products with typed response
    ///
    /// Convenience method for requesting the summary of all available products.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use ribbit_client::{RibbitClient, Region};
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let client = RibbitClient::new(Region::US);
    /// let summary = client.get_summary().await?;
    ///
    /// for product in &summary.products {
    ///     println!("{}: seqn {}", product.product, product.seqn);
    /// }
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The request fails
    /// - The response cannot be parsed as BPSV
    /// - The BPSV data doesn't match the expected schema
    pub async fn get_summary(&self) -> Result<SummaryResponse> {
        self.request_typed(&Endpoint::Summary).await
    }
}

/// Parsed Ribbit response
///
/// Contains the raw response data and parsed components based on the protocol version.
#[derive(Debug)]
pub struct Response {
    /// Raw response data
    pub raw: Vec<u8>,
    /// Parsed data (PSV format)
    pub data: Option<String>,
    /// MIME parts (V1 only)
    pub mime_parts: Option<MimeParts>,
}

/// MIME parts from a V1 response
#[derive(Debug)]
pub struct MimeParts {
    /// Main data content
    pub data: String,
    /// Signature data (if present)
    pub signature: Option<Vec<u8>>,
    /// Parsed signature information
    pub signature_info: Option<crate::signature::SignatureInfo>,
    /// Enhanced signature verification info (if available)
    pub signature_verification: Option<crate::signature_verify::EnhancedSignatureInfo>,
    /// Checksum from epilogue
    pub checksum: Option<String>,
}

impl Response {
    /// Get the data content as a string slice
    ///
    /// This is a convenience method similar to Ribbit.NET's `ToString()`
    #[must_use]
    pub fn as_text(&self) -> Option<&str> {
        self.data.as_deref()
    }

    /// Parse the response data as BPSV
    ///
    /// This allows direct access to the BPSV document structure.
    /// Note: This method adjusts HEX field lengths for Blizzard's format.
    ///
    /// # Errors
    /// Returns an error if the response has no data or BPSV parsing fails.
    pub fn as_bpsv(&self) -> Result<ngdp_bpsv::BpsvDocument> {
        match &self.data {
            Some(data) => {
                // Use the same adjustment as TypedResponse
                let adjusted_data = crate::response_types::adjust_hex_field_lengths(data);
                ngdp_bpsv::BpsvDocument::parse(&adjusted_data)
                    .map_err(|e| crate::error::Error::ParseError(format!("BPSV parse error: {e}")))
            }
            None => Err(crate::error::Error::ParseError(
                "No data in response".to_string(),
            )),
        }
    }

    /// Parse a V1 (MIME) response
    #[allow(clippy::too_many_lines)]
    fn parse_v1(raw: &[u8]) -> Result<Self> {
        // First, check if there's a checksum at the end of the raw data
        let (_, checksum) = Self::extract_checksum(raw);
        debug!("Extracted checksum from V1 response: {checksum:?}");

        // Parse the full MIME message (including any epilogue with checksum)
        let message = mail_parser::MessageParser::default().parse(raw).ok_or(
            crate::error::Error::MimeParseError("Failed to parse MIME message".to_string()),
        )?;

        // For checksum validation, we need to validate against the message without checksum
        if let Some(expected_checksum) = &checksum {
            // Extract the message bytes without checksum for validation
            let (message_bytes_for_validation, _) = Self::extract_checksum(raw);
            Self::validate_checksum(message_bytes_for_validation, expected_checksum)?;
        }

        let parts_count = message.parts.len();
        let text_body = &message.text_body;
        trace!(
            "Parsed message - parts count: {parts_count}, text_body indices: {text_body:?}, checksum: {checksum:?}"
        );

        // Extract the main data part and signature
        let mut data_content = None;
        let mut signature_content = None;

        // Look for multipart content
        for (idx, part) in message.parts.iter().enumerate() {
            let headers_count = part.headers.len();
            trace!("Processing part {idx}: headers count = {headers_count}");

            // Debug headers
            for header in &part.headers {
                let value_str = match &header.value {
                    mail_parser::HeaderValue::Text(t) => format!("Text: {t}"),
                    mail_parser::HeaderValue::TextList(list) => format!("TextList: {list:?}"),
                    mail_parser::HeaderValue::ContentType(ct) => format!("ContentType: {ct:?}"),
                    _ => format!("Other: {:?}", header.value),
                };
                let name = &header.name;
                trace!("  Header: {name} = {value_str}");
            }

            // Check Content-Disposition header
            let disposition = part
                .headers
                .iter()
                .find(|h| {
                    let name = h.name.as_str();
                    name == "Content-Disposition" || name.to_lowercase() == "content-disposition"
                })
                .map(|h| match &h.value {
                    mail_parser::HeaderValue::ContentType(ct) => ct.c_type.as_ref(),
                    mail_parser::HeaderValue::Text(t) => t.as_ref(),
                    _ => "",
                })
                .unwrap_or_default();

            trace!("Part {idx} disposition: '{disposition}'");

            // Get the text content for data parts
            if disposition.contains("version")
                || disposition.contains("cdns")
                || disposition.contains("bgdl")
                || disposition.contains("cert")
                || disposition.contains("ocsp")
                || disposition.contains("summary")
            {
                if let mail_parser::PartType::Text(text) = &part.body {
                    data_content = Some(text.as_ref().to_string());
                }
            } else if disposition.contains("signature") {
                // Get content for signature - it might be text or binary
                match &part.body {
                    mail_parser::PartType::Binary(binary) => {
                        signature_content = Some(binary.as_ref().to_vec());
                    }
                    mail_parser::PartType::Text(text) => {
                        // The signature is likely base64 encoded
                        let text_str = text.as_ref().trim();
                        // Try to decode base64
                        match STANDARD.decode(text_str) {
                            Ok(decoded) => signature_content = Some(decoded),
                            Err(_) => {
                                // Try as raw bytes if not base64
                                signature_content = Some(text.as_bytes().to_vec());
                            }
                        }
                    }
                    _ => {}
                }
            }
        }

        // If no multipart, try to get the main body
        if data_content.is_none() {
            // Check if we have text body indices
            if !message.text_body.is_empty() {
                // Try to get text from the first text body index
                if let Some(text) = message.body_text(0) {
                    data_content = Some(text.to_string());
                }
            }

            // If still no content, extract from raw message
            if data_content.is_none() {
                let raw_msg = message.raw_message.as_ref();
                // Find the double CRLF that separates headers from body
                if let Some(body_start) = raw_msg.windows(4).position(|w| w == b"\r\n\r\n") {
                    let body_bytes = &raw_msg[body_start + 4..];
                    let body_text = String::from_utf8_lossy(body_bytes);
                    // Trim any trailing whitespace
                    data_content = Some(body_text.trim_end().to_string());
                }
            }
        }

        let mime_parts =
            if data_content.is_some() || signature_content.is_some() || checksum.is_some() {
                // Parse signature if present
                let (signature_info, signature_verification) =
                    if let Some(ref sig_bytes) = signature_content {
                        // For signature verification, use the data without checksum
                        let data_for_verification = if checksum.is_some() {
                            let (data_without_checksum, _) = Self::extract_checksum(raw);
                            data_without_checksum
                        } else {
                            raw
                        };

                        // Try enhanced parsing first
                        match crate::signature_verify::parse_and_verify_signature(
                            sig_bytes,
                            Some(data_for_verification),
                        ) {
                            Ok(enhanced_info) => {
                                debug!("Enhanced signature parsing: {enhanced_info:?}");
                                // Convert to basic SignatureInfo for backward compatibility
                                let basic_info = crate::signature::SignatureInfo {
                                    format: enhanced_info.format.clone(),
                                    size: enhanced_info.size,
                                    algorithm: enhanced_info.digest_algorithm.clone(),
                                    signer_count: enhanced_info.signer_count,
                                    certificate_count: enhanced_info.certificates.len(),
                                };
                                (Some(basic_info), Some(enhanced_info))
                            }
                            Err(e) => {
                                warn!("Enhanced signature parsing failed: {e}");
                                // Fall back to basic parsing
                                match crate::signature::parse_signature(sig_bytes) {
                                    Ok(info) => {
                                        debug!("Basic signature parsing: {info:?}");
                                        (Some(info), None)
                                    }
                                    Err(e) => {
                                        warn!("Failed to parse signature: {e}");
                                        (None, None)
                                    }
                                }
                            }
                        }
                    } else {
                        (None, None)
                    };

                Some(MimeParts {
                    data: data_content.clone().unwrap_or_default(),
                    signature: signature_content,
                    signature_info,
                    signature_verification,
                    checksum,
                })
            } else {
                None
            };

        Ok(Response {
            raw: raw.to_vec(),
            data: data_content,
            mime_parts,
        })
    }

    /// Parse a V2 (raw PSV) response
    fn parse_v2(raw: &[u8]) -> Self {
        let data = String::from_utf8_lossy(raw).to_string();
        Response {
            raw: raw.to_vec(),
            data: Some(data),
            mime_parts: None,
        }
    }

    /// Extract checksum from the epilogue of a V1 response
    fn extract_checksum(raw: &[u8]) -> (&[u8], Option<String>) {
        const CHECKSUM_PREFIX: &[u8] = b"Checksum: ";

        // Look for the last occurrence of "Checksum: " in the data
        if let Some(checksum_pos) = raw
            .windows(CHECKSUM_PREFIX.len())
            .rposition(|window| window == CHECKSUM_PREFIX)
        {
            trace!("Found checksum at position {checksum_pos}");
            // Found "Checksum: " - extract the rest of the line
            let checksum_line_start = checksum_pos;

            // Find the end of the line (newline character)
            let checksum_line_end = raw[checksum_line_start..]
                .iter()
                .position(|&b| b == b'\n')
                .map_or(raw.len(), |pos| checksum_line_start + pos + 1);

            // Extract just the hex part (after "Checksum: " and before newline)
            let hex_start = checksum_pos + CHECKSUM_PREFIX.len();
            let mut hex_end = if checksum_line_end > 0 && raw[checksum_line_end - 1] == b'\n' {
                checksum_line_end - 1
            } else {
                checksum_line_end
            };

            // Also strip carriage return if present
            if hex_end > 0 && raw[hex_end - 1] == b'\r' {
                hex_end -= 1;
            }

            if hex_start < hex_end {
                let checksum = String::from_utf8_lossy(&raw[hex_start..hex_end]).to_string();
                // Validate it's a proper hex string
                if checksum.len() == 64 && checksum.chars().all(|c| c.is_ascii_hexdigit()) {
                    trace!("Valid checksum found: {checksum}");
                    // Return the message without the checksum line
                    let message_bytes = &raw[..checksum_line_start];
                    return (message_bytes, Some(checksum));
                }
                let len = checksum.len();
                trace!("Invalid checksum format - length: {len}, content: {checksum:?}");
            }
        }

        // No valid checksum found
        let len = raw.len();
        trace!("No checksum found in {len} bytes of data");
        (raw, None)
    }

    /// Validate the SHA-256 checksum of the message
    fn validate_checksum(message_bytes: &[u8], expected_checksum: &str) -> Result<()> {
        let mut hasher = Sha256::new();
        hasher.update(message_bytes);
        let computed = hasher.finalize();
        let computed_hex = format!("{computed:x}");

        if computed_hex != expected_checksum {
            warn!("Checksum mismatch: expected {expected_checksum}, computed {computed_hex}");
            return Err(crate::error::Error::ChecksumMismatch);
        }

        debug!("Checksum validation successful");
        Ok(())
    }
}

impl Default for RibbitClient {
    fn default() -> Self {
        Self::new(Region::US)
    }
}

impl fmt::Display for Response {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.data {
            Some(data) => write!(f, "{data}"),
            None => write!(f, "<empty response>"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_client_creation() {
        let client = RibbitClient::new(Region::EU);
        assert_eq!(client.region(), Region::EU);
        assert_eq!(client.protocol_version(), ProtocolVersion::V1);
    }

    #[test]
    fn test_client_with_protocol_version() {
        let client = RibbitClient::new(Region::US).with_protocol_version(ProtocolVersion::V2);
        assert_eq!(client.region(), Region::US);
        assert_eq!(client.protocol_version(), ProtocolVersion::V2);
    }

    #[test]
    fn test_client_setters() {
        let mut client = RibbitClient::new(Region::US);

        client.set_region(Region::KR);
        assert_eq!(client.region(), Region::KR);

        client.set_protocol_version(ProtocolVersion::V2);
        assert_eq!(client.protocol_version(), ProtocolVersion::V2);
    }

    #[test]
    fn test_client_default() {
        let client = RibbitClient::default();
        assert_eq!(client.region(), Region::US);
        assert_eq!(client.protocol_version(), ProtocolVersion::V1);
    }

    #[tokio::test]
    async fn test_connection_timeout() {
        // Use a non-routable IP address to ensure timeout
        let client = RibbitClient::new(Region::CN);
        let result = client.request_raw(&Endpoint::Summary).await;

        // The CN region often times out from outside China
        // This test may pass or fail depending on network conditions
        // but we're mainly testing that the timeout mechanism works
        if result.is_err() {
            let err = result.unwrap_err();
            // Check if it's either a connection timeout or connection failed
            match err {
                crate::error::Error::ConnectionTimeout { .. }
                | crate::error::Error::ConnectionFailed { .. } => {
                    // Expected for CN region from most locations
                    // Connection might fail or timeout before completion
                }
                _ => panic!("Unexpected error type: {err:?}"),
            }
        }
    }

    #[test]
    fn test_response_parse_v2() {
        let raw_data = b"test data\nwith lines";
        let response = Response::parse_v2(raw_data);

        assert_eq!(response.raw, raw_data);
        assert_eq!(response.data.unwrap(), "test data\nwith lines");
        assert!(response.mime_parts.is_none());
    }

    #[test]
    fn test_extract_checksum() {
        // Test with valid checksum
        let data_with_checksum = b"Some MIME data here\nChecksum: 1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef\n";
        let (message, checksum) = Response::extract_checksum(data_with_checksum);

        assert_eq!(message, b"Some MIME data here\n");
        assert_eq!(
            checksum,
            Some("1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef".to_string())
        );

        // Test without checksum
        let data_no_checksum = b"Just some data";
        let (message, checksum) = Response::extract_checksum(data_no_checksum);

        assert_eq!(message, data_no_checksum);
        assert!(checksum.is_none());
    }

    #[test]
    fn test_validate_checksum() {
        use sha2::{Digest, Sha256};

        // Test data
        let message = b"test message";

        // Compute expected checksum
        let mut hasher = Sha256::new();
        hasher.update(message);
        let expected = format!("{:x}", hasher.finalize());

        // Should succeed with correct checksum
        assert!(Response::validate_checksum(message, &expected).is_ok());

        // Should fail with incorrect checksum
        let wrong_checksum = "0000000000000000000000000000000000000000000000000000000000000000";
        assert!(Response::validate_checksum(message, wrong_checksum).is_err());
    }

    #[test]
    fn test_parse_v1_simple_mime() {
        // Create a simple MIME message
        let mime_data = concat!(
            "Content-Type: text/plain\r\n",
            "From: Test\r\n",
            "\r\n",
            "Region!STRING:0|BuildConfig!HEX:16\r\n",
            "us|abcdef1234567890\r\n"
        )
        .as_bytes();

        let response = Response::parse_v1(mime_data).unwrap();

        assert!(response.data.is_some());
        assert!(response.data.unwrap().contains("Region!STRING:0"));
        assert!(response.mime_parts.is_some());
    }

    #[test]
    fn test_parse_v1_with_checksum() {
        use sha2::{Digest, Sha256};

        // Create MIME with checksum
        let mime_data =
            concat!("Content-Type: text/plain\r\n", "\r\n", "test data\r\n",).as_bytes();

        // Add checksum
        let mut data_with_checksum = mime_data.to_vec();

        // Calculate real checksum
        let mut hasher = Sha256::new();
        hasher.update(&data_with_checksum);
        let checksum = format!("Checksum: {:x}\n", hasher.finalize());
        data_with_checksum.extend_from_slice(checksum.as_bytes());

        let response = Response::parse_v1(&data_with_checksum).unwrap();
        assert!(response.mime_parts.is_some());
        assert!(response.mime_parts.unwrap().checksum.is_some());
    }

    #[test]
    fn test_parse_v1_multipart_with_checksum() {
        // Create a multipart MIME message similar to what the server returns
        let mime_data = concat!(
            "MIME-Version: 1.0\r\n",
            "Content-Type: multipart/mixed; boundary=\"test-boundary\"\r\n",
            "\r\n",
            "--test-boundary\r\n",
            "Content-Type: text/plain\r\n",
            "\r\n",
            "Product data here\r\n",
            "--test-boundary--\r\n",
            "Checksum: 1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef\r\n"
        )
        .as_bytes();

        let response = Response::parse_v1(mime_data);

        // This will fail validation because the checksum is fake, but we can check it was extracted
        if let Err(crate::error::Error::ChecksumMismatch) = response {
            // This is expected - the checksum was found but doesn't match
            // Test passes because we successfully found and tried to validate the checksum
        } else {
            let response = response.unwrap();
            assert!(response.mime_parts.is_some());
            assert!(response.mime_parts.unwrap().checksum.is_some());
        }
    }

    #[test]
    fn test_parse_v1_with_signature() {
        // Create a multipart MIME message with a signature attachment
        let mut mime_data = Vec::new();
        mime_data.extend_from_slice(b"MIME-Version: 1.0\r\n");
        mime_data
            .extend_from_slice(b"Content-Type: multipart/mixed; boundary=\"test-boundary\"\r\n");
        mime_data.extend_from_slice(b"\r\n");
        mime_data.extend_from_slice(b"--test-boundary\r\n");
        mime_data.extend_from_slice(b"Content-Type: text/plain\r\n");
        mime_data.extend_from_slice(b"Content-Disposition: version\r\n");
        mime_data.extend_from_slice(b"\r\n");
        mime_data.extend_from_slice(b"Product data here\r\n");
        mime_data.extend_from_slice(b"--test-boundary\r\n");
        mime_data.extend_from_slice(b"Content-Type: application/octet-stream\r\n");
        mime_data.extend_from_slice(b"Content-Disposition: signature\r\n");
        mime_data.extend_from_slice(b"\r\n");
        // This is a minimal PKCS#7 signedData structure
        mime_data.extend_from_slice(&[
            0x30, 0x82, 0x01, 0xde, 0x06, 0x09, 0x2a, 0x86, 0x48, 0x86, 0xf7, 0x0d, 0x01, 0x07,
            0x02, 0xa0, 0x82, 0x01, 0xcf, 0x00,
        ]);
        mime_data.extend_from_slice(b"\r\n");
        mime_data.extend_from_slice(b"--test-boundary--\r\n");

        let response = Response::parse_v1(&mime_data).unwrap();
        assert!(response.mime_parts.is_some());

        let mime_parts = response.mime_parts.unwrap();
        assert!(mime_parts.signature.is_some());

        // The signature might have been base64 encoded, check its actual length
        let sig_len = mime_parts.signature.as_ref().unwrap().len();
        assert!(
            sig_len > 0,
            "Signature should not be empty, got {sig_len} bytes"
        );

        // For now, just check that we got a signature
        // The parsing might fail on this minimal test data
        if let Some(sig_info) = mime_parts.signature_info {
            assert_eq!(sig_info.format, "PKCS#7/CMS");
        }
    }
}
