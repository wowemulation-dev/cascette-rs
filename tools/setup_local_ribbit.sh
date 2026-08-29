#!/usr/bin/env bash
# Fetch versions/cdns BPSV from Arctium's archive for a historical build,
# rewrite host fields to a local server, and place them under the local
# mirror tree where the range HTTP server can serve them.
#
# Result layout (under <mirror_root>):
#   tpr/<product_path>/versions
#   tpr/<product_path>/cdns
#
# Both files use the canonical Ribbit v2 HTTP path that the wow client and
# wow-patcher's --version-url / --cdns-url expect:
#   http://<host>/<product_path>/versions
#   http://<host>/<product_path>/cdns
#
# The cdns file is rewritten so every Hosts entry is the local host and
# every Servers URL is http://<host>. This keeps the smoke-tested client
# fully offline against the local mirror.
#
# Usage:
#   tools/setup_local_ribbit.sh <region> <product_family> <build> \
#                               <mirror_root> [<host>] [<product_path>]
#
# Example for wow_classic 1.13.2.31650 against localhost:8000:
#   tools/setup_local_ribbit.sh EU wow 31650 \
#     /run/media/$USER/NGDP/mirrors/cdn.blizzard.com \
#     localhost:8000 tpr/wow

set -euo pipefail

if [[ $# -lt 4 ]]; then
  echo "usage: $0 <region> <product_family> <build> <mirror_root> [<host>] [<product_path>]" >&2
  echo "  region          e.g. EU, US, KR" >&2
  echo "  product_family  e.g. wow, wowt, wow_classic_era" >&2
  echo "  build           the build id (e.g. 31650)" >&2
  echo "  mirror_root     local CDN mirror root (e.g. /run/media/USER/NGDP/mirrors/cdn.blizzard.com)" >&2
  echo "  host            host:port for rewritten URLs (default: localhost:8000)" >&2
  echo "  product_path    CDN path used in the mirror tree (default: tpr/<product_family>)" >&2
  exit 2
fi

region="$1"
product_family="$2"
build="$3"
mirror_root="$4"
host="${5:-localhost:8000}"
product_path="${6:-tpr/${product_family}}"

src_versions="http://ngdp.arctium.io/${region}/${product_family}/${build}/versions"
src_cdns="http://ngdp.arctium.io/${region}/${product_family}/${build}/cdns"

dst_dir="${mirror_root}/${product_path}"
mkdir -p "${dst_dir}"

echo "fetching ${src_versions}"
curl -fsSL "${src_versions}" -o "${dst_dir}/versions"

echo "fetching ${src_cdns}"
curl -fsSL "${src_cdns}" > "${dst_dir}/cdns.orig"

# Rewrite the cdns file so Hosts and Servers point at the local host.
# The format is BPSV with header "Name|Path|Hosts|Servers|ConfigPath".
# Each data row keeps Name, Path, and ConfigPath; Hosts becomes the local
# host and Servers becomes http://<host> with no query parameters.
awk -F'|' -v OFS='|' -v host="${host}" '
  /^#/    { print; next }
  NR == 1 { print; next }     # header row, keep as-is
  NF >= 5 {
    $3 = host
    $4 = "http://" host
    print
  }
' "${dst_dir}/cdns.orig" > "${dst_dir}/cdns"
rm -f "${dst_dir}/cdns.orig"

echo
echo "wrote:"
echo "  ${dst_dir}/versions"
echo "  ${dst_dir}/cdns"
echo
echo "Verify by running the range HTTP server and curling:"
echo "  curl -s http://${host}/${product_path}/versions"
echo "  curl -s http://${host}/${product_path}/cdns"
