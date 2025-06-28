//! Debug signed attributes in CMS signatures
//!
//! This helps understand what exact data is being signed.

use cms::content_info::ContentInfo;
use cms::signed_data::SignedData;
use der::{Decode, Encode};
use ribbit_client::{Endpoint, ProtocolVersion, Region, RibbitClient};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = RibbitClient::new(Region::US).with_protocol_version(ProtocolVersion::V1);

    println!("=== Debugging CMS Signed Attributes ===\n");

    let endpoint = Endpoint::ProductVersions("wow".to_string());
    let response = client.request(&endpoint).await?;

    if let Some(mime_parts) = &response.mime_parts {
        if let Some(sig_bytes) = &mime_parts.signature {
            // Parse the CMS structure directly
            let content_info = ContentInfo::from_der(sig_bytes)?;

            // Extract SignedData
            let signed_data_bytes = content_info.content.to_der()?;
            let signed_data = SignedData::from_der(&signed_data_bytes)?;

            println!("SignedData info:");
            println!(
                "  Detached: {}",
                signed_data.encap_content_info.econtent.is_none()
            );
            println!(
                "  Content type: {:?}",
                signed_data.encap_content_info.econtent_type
            );
            println!("  Signers: {}", signed_data.signer_infos.0.len());

            for (i, signer) in signed_data.signer_infos.0.iter().enumerate() {
                println!("\nSigner #{i}:");

                // Check for signed attributes
                if let Some(signed_attrs) = &signer.signed_attrs {
                    println!("  Has signed attributes: {} attributes", signed_attrs.len());

                    // In CMS, when signed attributes are present, the signature is over
                    // the DER encoding of the signed attributes, not the original content
                    for (j, attr) in signed_attrs.iter().enumerate() {
                        println!("  Attribute #{}: OID = {}", j, attr.oid);

                        // Common attribute OIDs:
                        // 1.2.840.113549.1.9.3 = contentType
                        // 1.2.840.113549.1.9.4 = messageDigest
                        // 1.2.840.113549.1.9.5 = signingTime

                        match attr.oid.to_string().as_str() {
                            "1.2.840.113549.1.9.3" => println!("    -> Content Type attribute"),
                            "1.2.840.113549.1.9.4" => {
                                println!("    -> Message Digest attribute");
                                // This contains the hash of the actual content
                            }
                            "1.2.840.113549.1.9.5" => println!("    -> Signing Time attribute"),
                            _ => println!("    -> Unknown attribute"),
                        }
                    }
                } else {
                    println!("  No signed attributes");
                    println!("  -> Signature is directly over the content");
                }

                println!("  Digest algorithm: {:?}", signer.digest_alg);
                println!("  Signature algorithm: {:?}", signer.signature_algorithm);
                println!(
                    "  Signature size: {} bytes",
                    signer.signature.as_bytes().len()
                );
            }
        }
    }

    Ok(())
}
