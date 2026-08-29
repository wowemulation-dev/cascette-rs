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

    // CDN hosts: space-separated hostnames. Built from `config.endpoints`
    // (the endpoints actually used during install), deduplicated.
    //
    // Two filtering modes:
    //
    // 1. Override mode (CASCETTE_AGENT_CDN_HOSTS is set, e.g. integration
    //    tests against a local mirror): keep ONLY the override host(s).
    //    The client's update-check polls these endpoints, so they must be
    //    the same mirror the agent used; otherwise the client polls real
    //    Blizzard CDNs which serve newer builds and trigger an update
    //    dialog. The override mirror is expected to be seeded with
    //    versions/cdns BPSV files matching the installed build.
    //
    // 2. No override (production): keep official Blizzard CDN hosts only.
    //    Community mirrors (wago.tools, arctium.tools, wow.tools) are
    //    filtered because they typically serve the *latest* build of every
    //    product, again triggering the update dialog. Agent.exe's output
    //    contains only Blizzard hosts as well.
    fn is_blizzard_host(host: &str) -> bool {
        host.ends_with(".blizzard.com")
            || host.ends_with(".akamaihd.net")
            || host.ends_with(".cloudn.co.kr")
            || host.ends_with(".netease.com")
    }
    let override_hosts: Option<Vec<String>> =
        std::env::var("CASCETTE_AGENT_CDN_HOSTS").ok().map(|raw| {
            raw.split(|c: char| c.is_whitespace() || c == ',')
                .filter(|s| !s.is_empty())
                .map(|s| {
                    // Strip optional scheme and trailing path so we end up
                    // with just `host[:port]`, matching how endpoints are
                    // stored.
                    let s = s
                        .strip_prefix("https://")
                        .or_else(|| s.strip_prefix("http://"))
                        .unwrap_or(s);
                    s.split('/').next().unwrap_or(s).to_string()
                })
                .collect()
        });
    let mut cdn_hosts: Vec<String> = Vec::new();
    for ep in &config.endpoints {
        let keep = match &override_hosts {
            Some(overrides) => overrides.iter().any(|h| h == &ep.host),
            None => is_blizzard_host(&ep.host),
        };
        if !keep {
            continue;
        }
        if !cdn_hosts.iter().any(|h| h == &ep.host) {
            cdn_hosts.push(ep.host.clone());
        }
    }
    // If override mode listed a host that didn't show up in endpoints (e.g.
    // a future-resolved host), fall back to the raw override list so we
    // never write an empty CDN Hosts column.
    if override_hosts.is_some() && cdn_hosts.is_empty() {
        if let Some(overrides) = override_hosts {
            cdn_hosts = overrides;
        }
    }
    let cdn_hosts_str = cdn_hosts.join(" ");

    // CDN servers: HTTP + HTTPS URL per unique host, matching Agent.exe
    // output shape. Local-loopback hosts (localhost, 127.*, ::1) get only
    // an HTTP entry because the local mirror in our integration tests
    // doesn't terminate TLS — a stray https:// URL would trip the client
    // when it falls back to that scheme.
    fn is_loopback(host: &str) -> bool {
        host.starts_with("localhost") || host.starts_with("127.") || host.starts_with("[::1]")
    }
    let cdn_servers_str = cdn_hosts
        .iter()
        .flat_map(|host| {
            if is_loopback(host) {
                vec![format!("http://{host}/?maxhosts=4")]
            } else {
                vec![
                    format!("http://{host}/?maxhosts=4"),
                    format!("https://{host}/?maxhosts=4&fallback=1"),
                ]
            }
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
        // IM Size: empty in Agent.exe output, not 0. The field is reserved
        // for the install manifest size in legacy installs but Agent leaves
        // it blank for current builds.
        BpsvValue::Empty,
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
