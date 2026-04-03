//! `.build.info` BPSV writer.
//!
//! The `.build.info` file sits at the installation root and tells the
//! Battle.net client which build is installed. It uses the BPSV (Bar
//! Pipe Separated Values) format with a 15-column schema.

use cascette_formats::bpsv::{BpsvBuilder, BpsvField, BpsvType, BpsvValue};

use crate::config::InstallConfig;
use crate::error::InstallationResult;
use crate::pipeline::manifests::BuildManifests;

/// Write `.build.info` to the installation root.
///
/// Column ordering matches agent.exe output. The file contains a single row
/// for the active installation.
pub async fn write_build_info(
    config: &InstallConfig,
    manifests: &BuildManifests,
) -> InstallationResult<()> {
    let path = config.install_path.join(".build.info");

    let mut builder = BpsvBuilder::new();

    // Schema: 15 fields in agent.exe column order
    builder
        .add_field(BpsvField::new("Branch", BpsvType::String(0)))
        .add_field(BpsvField::new("Active", BpsvType::Dec(1)))
        .add_field(BpsvField::new("Build Key", BpsvType::Hex(16)))
        .add_field(BpsvField::new("CDN Key", BpsvType::Hex(16)))
        .add_field(BpsvField::new("Install Key", BpsvType::Hex(16)))
        .add_field(BpsvField::new("IM Size", BpsvType::Dec(4)))
        .add_field(BpsvField::new("CDN Path", BpsvType::String(0)))
        .add_field(BpsvField::new("CDN Hosts", BpsvType::String(0)))
        .add_field(BpsvField::new("CDN Servers", BpsvType::String(0)))
        .add_field(BpsvField::new("Tags", BpsvType::String(0)))
        .add_field(BpsvField::new("Armadillo", BpsvType::String(0)))
        .add_field(BpsvField::new("Last Activated", BpsvType::String(0)))
        .add_field(BpsvField::new("Version", BpsvType::String(0)))
        .add_field(BpsvField::new("KeyRing", BpsvType::Hex(16)))
        .add_field(BpsvField::new("Product", BpsvType::String(0)));

    let build_key_bytes = config
        .build_config
        .as_ref()
        .map(|h| hex::decode(h).unwrap_or_default())
        .unwrap_or_default();

    let cdn_key_bytes = config
        .cdn_config
        .as_ref()
        .map(|h| hex::decode(h).unwrap_or_default())
        .unwrap_or_default();

    // Install key from the first install manifest entry's content key
    let install_key_bytes = manifests
        .install
        .entries
        .first()
        .map(|e| e.content_key.as_bytes().to_vec())
        .unwrap_or_default();

    // CDN hosts: space-separated hostnames
    let cdn_hosts: Vec<String> = config.endpoints.iter().map(|ep| ep.host.clone()).collect();
    let cdn_hosts_str = cdn_hosts.join(" ");

    // CDN servers: each host expanded to HTTP + HTTPS URLs
    let cdn_servers_str = cdn_hosts
        .iter()
        .flat_map(|host| {
            vec![
                format!("http://{host}/?maxhosts=4"),
                format!("https://{host}/?maxhosts=4&fallback=1"),
            ]
        })
        .collect::<Vec<_>>()
        .join(" ");

    // Tags: colon-separated
    let tags = config.platform_tags.join(":");

    // Version from build config
    let version = manifests
        .build_config
        .client_version()
        .unwrap_or("")
        .to_string();

    let row_result = builder.add_row(vec![
        BpsvValue::String(config.region.clone()),
        BpsvValue::Dec(1), // Active = 1
        BpsvValue::Hex(build_key_bytes),
        BpsvValue::Hex(cdn_key_bytes),
        BpsvValue::Hex(install_key_bytes),
        BpsvValue::Dec(0), // IM Size
        BpsvValue::String(config.cdn_path.clone()),
        BpsvValue::String(cdn_hosts_str),
        BpsvValue::String(cdn_servers_str),
        BpsvValue::String(tags),
        BpsvValue::String(String::new()), // Armadillo
        BpsvValue::String(String::new()), // Last Activated
        BpsvValue::String(version),
        BpsvValue::Hex(vec![]), // KeyRing
        BpsvValue::String(config.product.clone()),
    ]);

    if let Err(e) = row_result {
        return Err(crate::error::InstallationError::Bpsv(e));
    }

    let document = builder.build();
    let content = cascette_formats::bpsv::format(&document);
    tokio::fs::write(&path, content.as_bytes()).await?;

    Ok(())
}
