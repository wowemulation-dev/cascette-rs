//! Certificate command handlers

use crate::{CertFormat, CertsCommands, OutputFormat, cached_client};
use ribbit_client::Endpoint;
use serde_json::json;
use std::str::FromStr;

/// Handle certificate commands
pub async fn handle(
    cmd: CertsCommands,
    output_format: OutputFormat,
) -> Result<(), Box<dyn std::error::Error>> {
    match cmd {
        CertsCommands::Download {
            ski,
            output,
            region,
            cert_format,
            details,
        } => download(ski, output, region, cert_format, details, output_format).await,
    }
}

/// Download a certificate by SKI/hash
async fn download(
    ski: String,
    output: Option<std::path::PathBuf>,
    region: String,
    cert_format: CertFormat,
    show_details: bool,
    output_format: OutputFormat,
) -> Result<(), Box<dyn std::error::Error>> {
    // Parse region
    let region = ribbit_client::Region::from_str(&region)?;

    // Create cached client
    let client = cached_client::create_client(region).await?;

    // Request the certificate
    let endpoint = Endpoint::Cert(ski.clone());
    let response = client.request(&endpoint).await?;

    // Extract the certificate data
    let cert_data = response
        .as_text()
        .ok_or("No certificate data in response")?;

    // Handle JSON output format specially
    match output_format {
        OutputFormat::Json | OutputFormat::JsonPretty => {
            // For JSON output, always include both certificate and details
            let mut json_output = json!({
                "ski": ski,
                "certificate": cert_data,
            });

            // Add details if requested
            if show_details {
                if let Ok(cert_info) = extract_certificate_info(cert_data) {
                    json_output["details"] = json!(cert_info);
                }
            }

            // Write to file or stdout
            if let Some(output_path) = output {
                let json_string = if matches!(output_format, OutputFormat::JsonPretty) {
                    serde_json::to_string_pretty(&json_output)?
                } else {
                    serde_json::to_string(&json_output)?
                };
                std::fs::write(&output_path, json_string)?;
                tracing::info!("Certificate written to: {}", output_path.display());
            } else if matches!(output_format, OutputFormat::JsonPretty) {
                println!("{}", serde_json::to_string_pretty(&json_output)?);
            } else {
                println!("{}", serde_json::to_string(&json_output)?);
            }
        }
        _ => {
            // For non-JSON output formats

            // Show details if requested (text format only)
            if show_details {
                if let Ok(cert_info) = extract_certificate_info(cert_data) {
                    use crate::output::{OutputStyle, format_header, format_key_value};
                    let style = OutputStyle::new();

                    println!("{}", format_header("Certificate Details", &style));
                    println!(
                        "{}",
                        format_key_value("Subject Key Identifier", &ski, &style)
                    );
                    println!(
                        "{}",
                        format_key_value("Subject", &cert_info.subject, &style)
                    );
                    println!("{}", format_key_value("Issuer", &cert_info.issuer, &style));
                    println!(
                        "{}",
                        format_key_value("Not Before", &cert_info.not_before, &style)
                    );
                    println!(
                        "{}",
                        format_key_value("Not After", &cert_info.not_after, &style)
                    );
                    println!(
                        "{}",
                        format_key_value("Serial Number", &cert_info.serial_number, &style)
                    );
                    if !cert_info.subject_alt_names.is_empty() {
                        println!("\nSubject Alternative Names:");
                        for san in &cert_info.subject_alt_names {
                            println!("  - {}", san);
                        }
                    }
                    println!();
                }
            }

            // Handle output format conversion
            let output_data = match cert_format {
                CertFormat::Pem => cert_data.as_bytes().to_vec(),
                CertFormat::Der => {
                    // Convert PEM to DER
                    convert_pem_to_der(cert_data)?
                }
            };

            // Write to output
            if let Some(output_path) = output {
                std::fs::write(&output_path, &output_data)?;
                tracing::info!("Certificate written to: {}", output_path.display());
            } else {
                // Write to stdout
                if cert_format == CertFormat::Pem {
                    print!("{}", cert_data);
                } else {
                    // For DER format, write binary to stdout
                    use std::io::Write;
                    std::io::stdout().write_all(&output_data)?;
                }
            }
        }
    }

    Ok(())
}

/// Convert PEM certificate to DER format
fn convert_pem_to_der(pem_data: &str) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    // Extract base64 content from PEM
    let mut base64_content = String::new();
    let mut in_cert = false;

    for line in pem_data.lines() {
        if line.contains("BEGIN CERTIFICATE") {
            in_cert = true;
            continue;
        }
        if line.contains("END CERTIFICATE") {
            break;
        }
        if in_cert {
            base64_content.push_str(line.trim());
        }
    }

    if base64_content.is_empty() {
        return Err("No certificate content found in PEM data".into());
    }

    // Decode base64 to DER
    use base64::{Engine as _, engine::general_purpose::STANDARD};
    Ok(STANDARD.decode(&base64_content)?)
}

/// Certificate information for JSON output
#[derive(serde::Serialize)]
struct CertificateInfo {
    subject: String,
    issuer: String,
    serial_number: String,
    not_before: String,
    not_after: String,
    subject_alt_names: Vec<String>,
}

/// Extract certificate information from PEM data
fn extract_certificate_info(pem_data: &str) -> Result<CertificateInfo, Box<dyn std::error::Error>> {
    // Convert PEM to DER
    let der_data = convert_pem_to_der(pem_data)?;

    // Parse certificate
    use der::Decode;
    use x509_cert::Certificate;
    let cert = Certificate::from_der(&der_data)?;

    // Extract information
    let subject = cert.tbs_certificate.subject.to_string();
    let issuer = cert.tbs_certificate.issuer.to_string();
    let serial_number = format!("{}", cert.tbs_certificate.serial_number);
    let not_before = cert.tbs_certificate.validity.not_before.to_string();
    let not_after = cert.tbs_certificate.validity.not_after.to_string();

    // Extract SANs if present
    let mut subject_alt_names = Vec::new();
    if let Some(extensions) = &cert.tbs_certificate.extensions {
        for ext in extensions {
            // Check for Subject Alternative Name extension (OID 2.5.29.17)
            if ext.extn_id.to_string() == "2.5.29.17" {
                // For now, just note that SANs are present
                // Full parsing would require more complex ASN.1 handling
                subject_alt_names.push("(Subject Alternative Names present)".to_string());
            }
        }
    }

    Ok(CertificateInfo {
        subject,
        issuer,
        serial_number,
        not_before,
        not_after,
        subject_alt_names,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_convert_pem_to_der() {
        let pem = "-----BEGIN CERTIFICATE-----\n\
                   MIIBkTCB+wIJAKHHIG...\n\
                   -----END CERTIFICATE-----";

        // This should not panic, even if base64 is invalid
        let _ = convert_pem_to_der(pem);
    }
}
