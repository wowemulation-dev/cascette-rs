#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
//! Tests for CDN mirror configuration and endpoint ordering.
//!
//! The mirror ordering logic directly maps to how tools like TACTSharp and
//! wow.export select CDN hosts: official endpoints first for current builds,
//! community mirrors first for historical builds that may have fallen off
//! official CDN.

use cascette_installation::mirror::{COMMUNITY_MIRRORS, MirrorConfig};
use cascette_protocol::CdnEndpoint;

fn make_official_endpoint(host: &str) -> CdnEndpoint {
    CdnEndpoint {
        host: host.to_string(),
        path: "tpr/wow".to_string(),
        product_path: None,
        scheme: Some("https".to_string()),
        is_fallback: false,
        strict: false,
        max_hosts: None,
    }
}

// --- COMMUNITY_MIRRORS constant ---

#[test]
fn community_mirrors_are_non_empty() {
    assert!(
        !COMMUNITY_MIRRORS.is_empty(),
        "Must have at least one community mirror"
    );
}

#[test]
fn community_mirrors_known_hosts_present() {
    // These three mirrors are documented in the project and used by TACTSharp tests
    assert!(
        COMMUNITY_MIRRORS.contains(&"cdn.arctium.tools"),
        "cdn.arctium.tools must be a community mirror"
    );
    assert!(
        COMMUNITY_MIRRORS.contains(&"casc.wago.tools"),
        "casc.wago.tools must be a community mirror"
    );
    assert!(
        COMMUNITY_MIRRORS.contains(&"archive.wow.tools"),
        "archive.wow.tools must be a community mirror"
    );
}

#[test]
fn community_mirrors_are_valid_hostnames() {
    for mirror in COMMUNITY_MIRRORS {
        assert!(!mirror.is_empty(), "Mirror hostname must not be empty");
        assert!(
            !mirror.contains("://"),
            "Mirror must be a hostname only, not a URL: {mirror}"
        );
        assert!(
            mirror.contains('.'),
            "Mirror hostname must contain a dot: {mirror}"
        );
    }
}

// --- Current build: official endpoints first ---

#[test]
fn current_build_official_endpoints_come_first() {
    let official = vec![
        make_official_endpoint("blzddist1-a.akamaihd.net"),
        make_official_endpoint("level3.blizzard.com"),
    ];
    let config = MirrorConfig {
        official: official.clone(),
        use_community_mirrors: true,
        is_historic: false,
    };

    let endpoints = config.build_endpoint_list("tpr/wow");

    // Official endpoints must appear first
    assert_eq!(endpoints.len(), official.len() + COMMUNITY_MIRRORS.len());
    for (i, ep) in official.iter().enumerate() {
        assert_eq!(
            endpoints[i].host, ep.host,
            "Official endpoint {i} must come before community mirrors"
        );
    }
}

#[test]
fn current_build_community_mirrors_are_fallbacks() {
    let config = MirrorConfig {
        official: vec![make_official_endpoint("blzddist1-a.akamaihd.net")],
        use_community_mirrors: true,
        is_historic: false,
    };

    let endpoints = config.build_endpoint_list("tpr/wow");

    // Community mirrors in the tail must be marked as fallback
    let community_start = 1; // After 1 official endpoint
    for ep in &endpoints[community_start..] {
        assert!(
            ep.is_fallback,
            "Community mirror {} must be marked as fallback",
            ep.host
        );
    }
}

#[test]
fn current_build_official_endpoints_not_fallback() {
    let config = MirrorConfig {
        official: vec![
            make_official_endpoint("blzddist1-a.akamaihd.net"),
            make_official_endpoint("level3.blizzard.com"),
        ],
        use_community_mirrors: true,
        is_historic: false,
    };

    let endpoints = config.build_endpoint_list("tpr/wow");

    // The first two endpoints are official and must NOT be fallbacks
    assert!(
        !endpoints[0].is_fallback,
        "Official endpoint must not be a fallback"
    );
    assert!(
        !endpoints[1].is_fallback,
        "Official endpoint must not be a fallback"
    );
}

// --- Historical build: community mirrors first ---

#[test]
fn historic_build_community_mirrors_come_first() {
    let official = vec![make_official_endpoint("blzddist1-a.akamaihd.net")];
    let config = MirrorConfig {
        official,
        use_community_mirrors: true,
        is_historic: true,
    };

    let endpoints = config.build_endpoint_list("tpr/wow");

    // Community mirrors must appear first for historical builds
    let community_count = COMMUNITY_MIRRORS.len();
    for (i, mirror_host) in COMMUNITY_MIRRORS.iter().enumerate() {
        assert_eq!(
            endpoints[i].host, *mirror_host,
            "Community mirror {mirror_host} must be at position {i} for historic builds"
        );
    }

    // Official endpoint comes after community mirrors
    assert_eq!(
        endpoints[community_count].host, "blzddist1-a.akamaihd.net",
        "Official endpoint must come after community mirrors for historic builds"
    );
}

// --- No community mirrors ---

#[test]
fn no_community_mirrors_only_official() {
    let official = vec![
        make_official_endpoint("blzddist1-a.akamaihd.net"),
        make_official_endpoint("level3.blizzard.com"),
    ];
    let config = MirrorConfig {
        official: official.clone(),
        use_community_mirrors: false,
        is_historic: false,
    };

    let endpoints = config.build_endpoint_list("tpr/wow");

    assert_eq!(
        endpoints.len(),
        official.len(),
        "Without community mirrors, only official endpoints"
    );
    for (i, ep) in official.iter().enumerate() {
        assert_eq!(endpoints[i].host, ep.host);
        assert!(!endpoints[i].is_fallback);
    }
}

#[test]
fn no_community_mirrors_historic_still_only_official() {
    let official = vec![make_official_endpoint("blzddist1-a.akamaihd.net")];
    let config = MirrorConfig {
        official,
        use_community_mirrors: false,
        is_historic: true,
    };

    let endpoints = config.build_endpoint_list("tpr/wow");
    assert_eq!(endpoints.len(), 1);
    assert_eq!(endpoints[0].host, "blzddist1-a.akamaihd.net");
}

// --- CDN path propagation ---

#[test]
fn community_mirrors_use_provided_cdn_path() {
    let config = MirrorConfig {
        official: vec![],
        use_community_mirrors: true,
        is_historic: false,
    };

    let cdn_path = "tpr/wow_classic";
    let endpoints = config.build_endpoint_list(cdn_path);

    for ep in &endpoints {
        assert_eq!(
            ep.path, cdn_path,
            "Community mirror must use the provided cdn_path"
        );
    }
}

#[test]
fn community_mirrors_use_https_scheme() {
    let config = MirrorConfig {
        official: vec![],
        use_community_mirrors: true,
        is_historic: false,
    };

    let endpoints = config.build_endpoint_list("tpr/wow");

    for ep in &endpoints {
        assert_eq!(
            ep.scheme.as_deref(),
            Some("https"),
            "Community mirrors must use HTTPS: {}",
            ep.host
        );
    }
}

// --- Total endpoint count ---

#[test]
fn total_endpoint_count_current_build() {
    let n_official = 3;
    let official: Vec<CdnEndpoint> = (0..n_official)
        .map(|i| make_official_endpoint(&format!("official{i}.example.com")))
        .collect();

    let config = MirrorConfig {
        official,
        use_community_mirrors: true,
        is_historic: false,
    };

    let endpoints = config.build_endpoint_list("tpr/wow");
    assert_eq!(
        endpoints.len(),
        n_official + COMMUNITY_MIRRORS.len(),
        "Total = official + community mirrors"
    );
}

#[test]
fn empty_official_with_mirrors_gives_mirror_count() {
    let config = MirrorConfig {
        official: vec![],
        use_community_mirrors: true,
        is_historic: false,
    };

    let endpoints = config.build_endpoint_list("tpr/wow");
    assert_eq!(endpoints.len(), COMMUNITY_MIRRORS.len());
}

// --- Backoff constants ---

#[test]
fn backoff_server_error_heavier_than_auth_range() {
    use cascette_installation::mirror::backoff;

    const {
        assert!(
            backoff::SERVER_ERROR_MULTIPLIER > backoff::AUTH_RANGE_MULTIPLIER,
            "Server errors (5xx) must penalize more heavily than auth/range errors"
        );
    }
}
