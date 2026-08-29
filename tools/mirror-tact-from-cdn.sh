#!/usr/bin/env bash
#
# Mirror TACT CDN files listed in generated path lists to the local mirror.
# Reads path lists produced by generate_wow_paths.sh and downloads each
# missing file from the CDN URL into the local mirror root.
#
# Usage:
#   ./tools/mirror-tact-from-cdn.sh [JOBS]
#
# Environment:
#   MIRROR     Local NGDP mirror root (default: /run/media/danielsreichenbach/NGDP/mirrors/cdn.blizzard.com)
#   CDN_URL    CDN base URL to fetch from (default: https://tactic.wowemu.dev)
#   OUTDIR     Directory containing generated path lists (default: repo paths/)
#   JOBS       Parallel workers per build (default: 4, or first positional arg)
#   MISSING_LOG  Log file for failed downloads (default: tools/missing.log)
#   DOWNLOADED_LOG  Log file for successfully downloaded files (default: tools/downloaded.log)
#   SKIPPED_LOG  Log file for files already present (default: tools/skipped.log)
#
# HTTPS is used (not rsync/SSH) so the script works against any publicly
# reachable CDN URL and can be shared. Downloads go to .part files and are
# resumed (curl -C -) when interrupted; they are renamed to their final
# name only after a complete (200/206) response.

set -uo pipefail

MIRROR="${MIRROR:-/run/media/danielsreichenbach/NGDP/mirrors/cdn.blizzard.com}"
CDN_URL="${CDN_URL:-https://tactic.wowemu.dev}"
OUTDIR="${OUTDIR:-$(cd "$(dirname "$0")/.." && pwd)/paths}"
MISSING_LOG="${MISSING_LOG:-$(dirname "$0")/missing.log}"
DOWNLOADED_LOG="${DOWNLOADED_LOG:-$(dirname "$0")/downloaded.log}"
SKIPPED_LOG="${SKIPPED_LOG:-$(dirname "$0")/skipped.log}"

# Parallel workers per build. Override via env (JOBS=16 ./mirror-tact-from-cdn.sh)
# or as the first positional argument (./mirror-tact-from-cdn.sh 16).
JOBS="${JOBS:-${1:-4}}"

fetch_one() {
	local path="$1"
	[[ -z "$path" ]] && return 0
	local dest="$MIRROR/$path"
	local part="$dest.part"

	# Download to a .part file and rename on success. This makes interrupted
	# transfers resumable (curl -C - sends a Range request and appends) and
	# prevents a truncated file from being mistaken for a complete one.
	# 206 = resumed partial content, 200 = full download.
	local http_code
	http_code=$(curl -sL -C - --create-dirs -o "$part" -w '%{http_code}' "$CDN_URL/$path")
	if [[ "$http_code" == 200 || "$http_code" == 206 ]]; then
		mv "$part" "$dest"
		echo "OK: $path"
		echo "$path" >>"$DOWNLOADED_LOG"
	else
		rm -f "$part"
		echo "SKIP ($http_code): $CDN_URL/$path"
		echo "$http_code $path" >>"$MISSING_LOG"
	fi
}
export -f fetch_one
export MIRROR CDN_URL MISSING_LOG DOWNLOADED_LOG SKIPPED_LOG

for pathfile in "$OUTDIR"/wow*_6.*.*.txt; do
	echo "==> Processing $(basename "$pathfile") with $JOBS workers"

	# Strip comments/blank lines up front so xargs only ever sees real paths.
	grep -v '^[[:space:]]*\(#\|$\)' "$pathfile" >/tmp/_mirror_paths.$$ || true

	# Fast single-process pass: check which files are missing.
	# All existence checks run in this shell — no per-path subprocess overhead.
	missing_count=0
	skipped_count=0
	while IFS= read -r path; do
		if [[ -f "$MIRROR/$path" ]]; then
			echo "$path" >>"$SKIPPED_LOG"
			((skipped_count++)) || true
		else
			echo "$path" >>/tmp/_mirror_missing.$$
			((missing_count++)) || true
		fi
	done </tmp/_mirror_paths.$$

	echo "  Skipped: $skipped_count, Missing: $missing_count"

	# Only spawn curl workers for the paths that actually need downloading.
	if [[ -f /tmp/_mirror_missing.$$ ]]; then
		xargs -d '\n' -P "$JOBS" -n 1 bash -c 'fetch_one "$@"' _ </tmp/_mirror_missing.$$
	fi

	rm -f /tmp/_mirror_paths.$$ /tmp/_mirror_missing.$$
done
