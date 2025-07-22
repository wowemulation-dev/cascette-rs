//! # [Community listfile][0] fetcher
//!
//! How this works:
//!
//! 1. Fetch the latest GitHub releases of `wowdev/wow-listfile` from GitHub,
//!    with API caching.
//!
//! 2. If we have a new release, download that.
//!
//! [0]: https://github.com/wowdev/wow-listfile/

use ngdp_cache::{generic::GenericCache, get_cache_dir};
use octorust::{Client, auth::Credentials, http_cache::HttpCache};
use reqwest::redirect::Policy;
use tokio::fs::File;

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

const OWNER: &str = "wowdev";
const REPO: &str = "wow-listfile";
const RELEASE_FILE: &str = "community-listfile.csv";
const USER_AGENT: &str = concat!(env!("CARGO_PKG_NAME"), "/", env!("CARGO_PKG_VERSION"));

/// Get an [octorust][] [Client][] instance.
pub fn get_octorust_client(credentials: Option<Credentials>) -> Result<Client> {
    let cache = if crate::cached_client::is_caching_enabled() {
        let cache_root = get_cache_dir()?;
        <dyn HttpCache>::in_dir(&cache_root)
    } else {
        <dyn HttpCache>::noop()
    };

    let github = Client::custom(
        USER_AGENT,
        credentials,
        reqwest::Client::builder().build()?.into(),
        cache,
    );

    Ok(github)
}

/// Find the latest release asset ID
pub async fn get_latest_asset_id(github: &Client) -> Result<Option<i64>> {
    let release = github.repos().get_latest_release(OWNER, REPO).await?;

    for asset in &release.body.assets {
        let name = asset.name.to_ascii_lowercase();
        if name == RELEASE_FILE {
            return Ok(Some(asset.id));
        }
    }

    Ok(None)
}

/// Download a release, or fetch it from cache.
pub async fn download_release(asset_id: i64) -> Result<File> {
    let cache = GenericCache::with_subdirectory("listfile").await?;
    let hash = asset_id.to_string();

    if let Some(file) = cache.read_object("community-listfile", &hash).await? {
        return Ok(file);
    }

    let asset_url =
        format!("https://api.github.com/repos/{OWNER}/{REPO}/releases/assets/{asset_id}");

    let client = reqwest::Client::builder()
        .redirect(Policy::default())
        .build()?;

    let response = client
        .get(asset_url)
        .header("Accept", "application/octet-stream")
        .header("User-Agent", USER_AGENT)
        .send()
        .await?;

    let file = cache.write_response("community-listfile", &hash, response).await?;
    
    Ok(file)
}
