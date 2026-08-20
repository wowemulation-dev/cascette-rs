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
#

set -uo pipefail

MIRROR="${MIRROR:-/run/media/danielsreichenbach/NGDP/mirrors/cdn.blizzard.com}"
CDN_URL="${CDN_URL:-https://tactic.wowemu.dev}"
OUTDIR="${OUTDIR:-$(cd "$(dirname "$0")/.." && pwd)/paths}"
MISSING_LOG="${MISSING_LOG:-$(dirname "$0")/missing.log}"

# Parallel workers per build. Override via env (JOBS=16 ./mirror-tact-from-cdn.sh)
# or as the first positional argument (./mirror-tact-from-cdn.sh 16).
JOBS="${JOBS:-${1:-4}}"

fetch_one() {
  local path="$1"
  [[ -z "$path" || "$path" == \#* ]] && return 0

  local dest="$MIRROR/$path"

  if [[ -f "$dest" ]]; then
    return 0
  fi

  echo "$CDN_URL/$path"
  local http_code
  http_code=$(curl -sL --create-dirs -o "$dest" -w '%{http_code}' "$CDN_URL/$path")
  if [[ "$http_code" != 200 ]]; then
    rm -f "$dest"
    echo "SKIP ($http_code): $CDN_URL/$path"
    echo "$http_code $path" >> "$MISSING_LOG"
  fi
}

export -f fetch_one
export MIRROR CDN_URL MISSING_LOG

for pathfile in "$OUTDIR"/wow_classic*_*.*.txt; do
  echo "==> Processing $(basename "$pathfile") with $JOBS workers"
  # Strip comments/blank lines up front so xargs only ever sees real paths.
  grep -v '^[[:space:]]*\(#\|$\)' "$pathfile" \
    | xargs -d '\n' -P "$JOBS" -I {} bash -c 'fetch_one "$@"' _ {}
done
