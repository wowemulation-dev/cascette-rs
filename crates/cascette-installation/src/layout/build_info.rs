//! `.build.info` BPSV writer.
//!
//! The `.build.info` file sits at the installation root and tells the
//! Battle.net client which build is installed. It uses the BPSV (Bar
//! Pipe Separated Values) format with a 14-column schema.

use cascette_formats::bpsv::{BpsvBuilder, BpsvField, BpsvType, BpsvValue};

use crate::config::InstallConfig;
use crate::error::InstallationResult;
use crate::pipeline::manifests::BuildManifests;

/// Write `.build.info` to the installation root.
///
/// Column ordering matches Blizzard Agent output (14 columns).
/// The file contains a single row for the active installation.
pub async fn write_build_info(
    config: &InstallConfig,
    manifests: &BuildManifests,
) -> InstallationResult<()> {
    let path = config.install_path.join(".build.info");

    let mut builder = BpsvBuilder::new();

    // Schema: 14 fields in Blizzard Agent column order.
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

    // Install key: encoding key of the install manifest from the build config.
    // This is the hash used to locate the install manifest in the encoding table,
    // NOT the content key of any individual entry within the manifest.
    let install_key_bytes = manifests
        .build_config
        .install()
        .first()
        .and_then(|bi| bi.encoding_key.as_ref())
        .map(|k| hex::decode(k).unwrap_or_default())
        .unwrap_or_default();

    // CDN hosts: space-separated hostnames.
    // Exclude local/override endpoints (localhost, 127.*) — these are for the
    // agent's download pipeline, not for the client's update checker. Writing
    // reachable local hosts causes the client to find newer builds and show
    // an update dialog instead of the login screen.
    let cdn_hosts: Vec<String> = config
        .endpoints
        .iter()
        .filter(|ep| {
            !ep.host.starts_with("localhost")
                && !ep.host.starts_with("127.")
                && !ep.host.starts_with("[::1]")
        })
        .map(|ep| ep.host.clone())
        .collect();
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

    // Tags: multi-configuration format matching Blizzard Agent output.
    let tags = config.tag_query.build_info_tags();

    // Version: prefer client-version, fall back to parsing build-name.
    // Older build configs (pre-1.14) lack client-version but have build-name
    // like "WOW-31650patch1.13.2_Retail" → "1.13.2.31650".
    let version = manifests
        .build_config
        .client_version()
        .map(ToString::to_string)
        .or_else(|| version_from_build_name(manifests.build_config.build_name()?))
        .unwrap_or_default();

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

/// Extract version string from `build-name` field.
///
/// Format: `{PRODUCT}-{build_id}patch{version}[_suffix]`
/// Example: `WOW-31650patch1.13.2_Retail` → `1.13.2.31650`
fn version_from_build_name(build_name: &str) -> Option<String> {
    let patch_idx = build_name.find("patch")?;
    let before_patch = &build_name[..patch_idx];
    let after_patch = &build_name[patch_idx + 5..];

    // Build ID is the digits immediately before "patch", after the last '-'.
    let build_id = before_patch.rsplit('-').next()?;
    if build_id.is_empty() || !build_id.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }

    // Version is digits and dots after "patch", before any '_' suffix.
    let version = after_patch.split('_').next()?;
    if version.is_empty() {
        return None;
    }

    Some(format!("{version}.{build_id}"))
}
