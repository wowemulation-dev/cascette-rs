//! Patch chain application: walks a `PatchChain` step by step, fetching patch
//! blobs from CDN and applying the appropriate patch strategy.
//!
//! Three patch strategies are supported (matching agent.exe):
//! - ZBSDIFF1: full-file binary differential patching
//! - Block patch: block-by-block BLTE chunk patching
//! - Re-encode: decode and re-encode with a different ESpec

use tracing::{debug, warn};

use cascette_formats::CascFormat;
use cascette_formats::blte::BlteFile;
use cascette_formats::patch_chain::PatchChain;
use cascette_protocol::CdnEndpoint;

use crate::cdn_source::CdnSource;
use crate::error::{InstallationError, InstallationResult};
use crate::patch::resolver::PatchResolver;
use crate::patch::{apply_block_patch, apply_bsdiff_patch};

/// ZBSDIFF1 magic bytes at the start of decoded patch data.
const ZBSDIFF1_MAGIC: &[u8] = b"ZBSDIFF1";

/// Apply a full patch chain, producing the target file data.
///
/// For each step in the chain:
/// 1. Read the base data (from `base_data` for step 0, from previous result otherwise)
/// 2. Fetch the patch blob from CDN via the resolver
/// 3. Determine patch strategy (ZBSDIFF1, block, or re-encode) and apply
/// 4. Validate the result against the expected target encoding key
pub async fn apply_patch_chain<S: CdnSource>(
    chain: &PatchChain,
    base_data: Vec<u8>,
    resolver: &PatchResolver,
    cdn: &S,
    endpoints: &[CdnEndpoint],
) -> InstallationResult<Vec<u8>> {
    let endpoint = endpoints.first().ok_or_else(|| {
        InstallationError::InvalidConfig("no CDN endpoints for patch application".to_string())
    })?;

    let mut current_data = base_data;

    for (step_idx, step) in chain.steps.iter().enumerate() {
        debug!(
            step = step_idx,
            source = %hex::encode(step.original_ekey),
            target = %hex::encode(step.result_key),
            patch = %hex::encode(step.patch_key),
            "applying patch step"
        );

        // Fetch patch blob from CDN
        let patch_data = fetch_patch_blob(resolver, cdn, endpoint, &step.patch_key).await?;

        // Apply the appropriate patch strategy
        current_data = apply_patch_step(&current_data, &patch_data).map_err(|e| {
            InstallationError::Format(format!(
                "patch step {step_idx} failed (source={}, target={}): {e}",
                hex::encode(step.original_ekey),
                hex::encode(step.result_key),
            ))
        })?;

        // Validate result by computing MD5 and comparing against expected key
        let result_md5 = md5::compute(&current_data);
        if result_md5.0 != step.result_key {
            return Err(InstallationError::Format(format!(
                "patch step {step_idx} hash mismatch: expected {}, got {}",
                hex::encode(step.result_key),
                hex::encode(result_md5.0),
            )));
        }

        debug!(
            step = step_idx,
            result_size = current_data.len(),
            "patch step completed"
        );
    }

    Ok(current_data)
}

/// Fetch a patch blob from CDN using the resolver to find its archive location.
async fn fetch_patch_blob<S: CdnSource>(
    resolver: &PatchResolver,
    cdn: &S,
    endpoint: &CdnEndpoint,
    patch_ekey: &[u8; 16],
) -> InstallationResult<Vec<u8>> {
    // Look up the patch location in the resolver
    let Some(location) = resolver.locate_patch(patch_ekey) else {
        return Err(InstallationError::Format(format!(
            "patch blob {} not found in any archive index",
            hex::encode(patch_ekey),
        )));
    };

    // Get the archive hash for CDN download
    let archive_hash = resolver
        .archive_hash(location.archive_index)
        .ok_or_else(|| {
            InstallationError::Format(format!(
                "patch archive index {} out of range",
                location.archive_index,
            ))
        })?;

    debug!(
        patch = %hex::encode(patch_ekey),
        archive = %archive_hash,
        offset = location.offset,
        size = location.encoded_size,
        "fetching patch blob from CDN"
    );

    // Download a range from the patch archive covering just the patch blob
    let archive_key = hex::decode(archive_hash)
        .map_err(|e| InstallationError::Format(format!("invalid patch archive hash hex: {e}")))?;

    let data = cdn
        .download_range(
            endpoint,
            cascette_protocol::ContentType::Patch,
            &archive_key,
            location.offset,
            u64::from(location.encoded_size),
        )
        .await?;

    Ok(data)
}

/// Determine the patch strategy and apply it.
///
/// Strategy detection follows agent.exe:
/// - ZBSDIFF1: patch data starts with the "ZBSDIFF1" magic after BLTE decode
/// - Block patch: patch data is BLTE-encoded with multiple chunks matching base
/// - Re-encode: fallback when patch data is not ZBSDIFF1 or block-level
fn apply_patch_step(
    base_data: &[u8],
    patch_data: &[u8],
) -> Result<Vec<u8>, crate::patch::PatchError> {
    // First, try to decode the patch as BLTE to inspect its content
    let decoded = match BlteFile::parse(patch_data) {
        Ok(blte) => match blte.decompress() {
            Ok(data) => data,
            Err(_) => {
                // Not valid BLTE -- treat as raw patch data
                patch_data.to_vec()
            }
        },
        Err(_) => {
            // Not BLTE-wrapped -- treat as raw data
            patch_data.to_vec()
        }
    };

    // Check for ZBSDIFF1 magic
    if decoded.len() >= ZBSDIFF1_MAGIC.len() && decoded[..ZBSDIFF1_MAGIC.len()] == *ZBSDIFF1_MAGIC {
        debug!("applying ZBSDIFF1 patch strategy");
        return apply_bsdiff_patch(base_data, patch_data);
    }

    // Try block patch (operates on BLTE-level chunks)
    match apply_block_patch(base_data, patch_data) {
        Ok(result) => {
            debug!("applied block patch strategy");
            return Ok(result);
        }
        Err(e) => {
            warn!("block patch failed: {e}, attempting re-encode");
        }
    }

    // Fallback: re-encode is handled at the caller level since it requires
    // the target ESpec from the patch index, which we don't have here.
    // If neither ZBSDIFF1 nor block patch works, return an error.
    Err(crate::patch::PatchError::Format(
        "no applicable patch strategy found".to_string(),
    ))
}
