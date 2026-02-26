//! Suffix array-based binary diff algorithm for ZBSDIFF1 patch creation.
//!
//! Implements the bsdiff algorithm (Colin Percival) using the `divsufsort` crate
//! for suffix array construction. Produces control/diff/extra buffers that are
//! fed into `build_patch_internal()` for zlib compression and header assembly.
//!
//! The algorithm finds longest matches anywhere in the old data via binary search
//! on the suffix array, then uses a greedy scan with forward/backward extension
//! and overlap resolution to produce near-optimal patches.

use crate::zbsdiff::utils::ControlEntry;

/// Result of the diff algorithm: three separate buffers for ZBSDIFF1 assembly.
pub struct DiffResult {
    pub control: Vec<ControlEntry>,
    pub diff_data: Vec<u8>,
    pub extra_data: Vec<u8>,
}

/// Count matching prefix bytes between two slices.
fn matchlen(a: &[u8], b: &[u8]) -> usize {
    a.iter().zip(b.iter()).take_while(|(x, y)| x == y).count()
}

/// Binary search on the suffix array for the longest match of `new_data` in `old`.
///
/// Returns `(position_in_old, match_length)`.
///
/// The search narrows a range `[st, en)` in the suffix array using standard
/// binary search, comparing `new_data` against the suffix at the midpoint.
/// At each step it also records the match length at both boundaries and the
/// midpoint, keeping track of the longest match seen.
fn search(sa: &[i32], old: &[u8], new_data: &[u8]) -> (usize, usize) {
    if sa.is_empty() || new_data.is_empty() {
        return (0, 0);
    }

    let mut st: usize = 0;
    let mut en: usize = sa.len() - 1;

    // Binary search
    while en - st > 1 {
        let pivot = st + (en - st) / 2;
        let pivot_pos = sa[pivot] as usize;
        let pivot_len = matchlen(&old[pivot_pos..], new_data);

        if pivot_len == new_data.len()
            || (pivot_pos + pivot_len < old.len()
                && old[pivot_pos + pivot_len] < new_data[pivot_len])
        {
            st = pivot;
        } else {
            en = pivot;
        }
    }

    // Recompute after search converged
    let start_pos = sa[st] as usize;
    let end_pos = sa[en] as usize;
    let start_len = matchlen(&old[start_pos..], new_data);
    let end_len = matchlen(&old[end_pos..], new_data);

    if start_len > end_len {
        (start_pos, start_len)
    } else {
        (end_pos, end_len)
    }
}

/// Run the bsdiff diff algorithm on old and new data.
///
/// Produces control entries, diff bytes, and extra bytes as three separate
/// buffers suitable for ZBSDIFF1 assembly via `build_patch_internal()`.
//
// The clippy::suspicious_operation_groupings lint fires on comparisons like
// `old[old_idx] == new[scsc]` suggesting `old[old_idx] == new[old_idx]` instead.
// This is a false positive: old_idx and scsc/scan are intentionally different
// indices into different arrays (tracking the old-file offset drift vs the
// current scan position in new data).
#[allow(clippy::suspicious_operation_groupings)]
pub fn compute_diff(old: &[u8], new: &[u8]) -> DiffResult {
    let old_size = old.len();
    let new_size = new.len();

    // Build suffix array on old data
    let sa = if old_size > 0 {
        let mut sa = vec![0i32; old_size];
        divsufsort::sort_in_place(old, &mut sa);
        sa
    } else {
        Vec::new()
    };

    let mut control = Vec::new();
    let mut diff_data = Vec::new();
    let mut extra_data = Vec::new();

    let mut scan: usize = 0;
    let mut len: usize = 0;
    let mut pos: usize = 0;
    let mut lastscan: usize = 0;
    let mut lastpos: usize = 0;
    let mut lastoffset: i64 = 0;

    while scan < new_size {
        let mut oldscore: usize = 0;
        scan += len;
        let mut scsc = scan;

        while scan < new_size {
            // Find longest match of new[scan..] in old
            let (match_pos, match_len) = search(&sa, old, &new[scan..]);
            pos = match_pos;
            len = match_len;

            // Count how many bytes at current position match via previous offset drift
            while scsc < scan + len {
                let old_idx = (scsc as i64 + lastoffset) as usize;
                if old_idx < old_size && old[old_idx] == new[scsc] {
                    oldscore += 1;
                }
                scsc += 1;
            }

            // Accept match if it beats drift by 8+ bytes, or if drift explains
            // everything (in which case keep scanning)
            if (len == oldscore && len != 0) || len > oldscore + 8 {
                break;
            }

            // Undo the byte we're about to skip past
            let old_idx = (scan as i64 + lastoffset) as usize;
            if old_idx < old_size && old[old_idx] == new[scan] {
                oldscore -= 1;
            }

            scan += 1;
        }

        // Emit a control entry if the match is genuinely better than drift,
        // or we've reached the end of new data
        if len != oldscore || scan == new_size {
            // Forward extension: find optimal lenf from lastscan
            let mut s: i64 = 0;
            let mut sf: i64 = 0;
            let mut lenf: usize = 0;
            {
                let mut i: usize = 0;
                while lastscan + i < scan && lastpos + i < old_size {
                    if old[lastpos + i] == new[lastscan + i] {
                        s += 1;
                    }
                    i += 1;
                    if s * 2 - i as i64 > sf * 2 - lenf as i64 {
                        sf = s;
                        lenf = i;
                    }
                }
            }

            // Backward extension: find optimal lenb from current match
            let mut lenb: usize = 0;
            if scan < new_size {
                let mut s: i64 = 0;
                let mut sb: i64 = 0;
                let mut i: usize = 1;
                while scan >= lastscan + i && pos >= i {
                    if old[pos - i] == new[scan - i] {
                        s += 1;
                    }
                    if s * 2 - i as i64 > sb * 2 - lenb as i64 {
                        sb = s;
                        lenb = i;
                    }
                    i += 1;
                }
            }

            // Overlap resolution
            if lastscan + lenf > scan - lenb {
                let overlap = (lastscan + lenf) - (scan - lenb);
                let mut s: i64 = 0;
                let mut ss: i64 = 0;
                let mut lens: usize = 0;
                for i in 0..overlap {
                    if new[lastscan + lenf - overlap + i] == old[lastpos + lenf - overlap + i] {
                        s += 1;
                    }
                    if new[scan - lenb + i] == old[pos - lenb + i] {
                        s -= 1;
                    }
                    if s > ss {
                        ss = s;
                        lens = i + 1;
                    }
                }
                lenf += lens;
                lenf -= overlap;
                lenb -= lens;
            }

            // Collect diff bytes: new[lastscan..lastscan+lenf] - old[lastpos..lastpos+lenf]
            for i in 0..lenf {
                diff_data.push(new[lastscan + i].wrapping_sub(old[lastpos + i]));
            }

            // Collect extra bytes: new[lastscan+lenf..scan-lenb]
            let extra_start = lastscan + lenf;
            let extra_end = scan - lenb;
            if extra_end > extra_start {
                extra_data.extend_from_slice(&new[extra_start..extra_end]);
            }

            // Emit control entry
            let diff_size = lenf as i64;
            let extra_size = (extra_end - extra_start) as i64;
            let seek_offset = (pos as i64 - lenb as i64) - (lastpos as i64 + lenf as i64);

            control.push(ControlEntry::new(diff_size, extra_size, seek_offset));

            // Advance state
            lastscan = scan - lenb;
            lastpos = pos - lenb;
            lastoffset = pos as i64 - scan as i64;
        }
    }

    DiffResult {
        control,
        diff_data,
        extra_data,
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_matchlen_basic() {
        assert_eq!(matchlen(b"hello", b"hello"), 5);
        assert_eq!(matchlen(b"hello", b"help"), 3);
        assert_eq!(matchlen(b"hello", b"world"), 0);
        assert_eq!(matchlen(b"", b"hello"), 0);
        assert_eq!(matchlen(b"hello", b""), 0);
    }

    #[test]
    fn test_search_empty() {
        let (_pos, len) = search(&[], b"", b"hello");
        assert_eq!(len, 0);
    }

    #[test]
    fn test_search_finds_match() {
        let old = b"the quick brown fox";
        let mut sa = vec![0i32; old.len()];
        divsufsort::sort_in_place(old, &mut sa);

        let (pos, len) = search(&sa, old, b"brown");
        assert_eq!(len, 5);
        assert_eq!(&old[pos..pos + 5], b"brown");
    }

    #[test]
    fn test_compute_diff_identical() {
        let data = b"identical data here";
        let result = compute_diff(data, data);

        // For identical data, diff should be all zeros and no extra
        assert!(result.extra_data.is_empty());
        assert!(result.diff_data.iter().all(|&b| b == 0));
    }

    #[test]
    fn test_compute_diff_empty_old() {
        let result = compute_diff(b"", b"new data");

        // Everything goes into extra
        assert_eq!(result.extra_data, b"new data");
        assert!(result.diff_data.is_empty());
    }

    #[test]
    fn test_compute_diff_empty_new() {
        let result = compute_diff(b"old data", b"");

        // No output at all
        assert!(result.control.is_empty());
        assert!(result.diff_data.is_empty());
        assert!(result.extra_data.is_empty());
    }

    #[test]
    fn test_compute_diff_both_empty() {
        let result = compute_diff(b"", b"");
        assert!(result.control.is_empty());
        assert!(result.diff_data.is_empty());
        assert!(result.extra_data.is_empty());
    }
}
