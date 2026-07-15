#!/usr/bin/env python3
"""Generate verify_mirror_sizes.sh build list from cascette-py build database.

Usage:
    cd ~/Repos/github.com/wowemulation-dev/cascette-py
    uv run cascette builds export -f json /tmp/all_wow_builds.json -p wow
    uv run cascette builds export -f json /tmp/all_wow_anniversary_builds.json -p wow_anniversary
    uv run cascette builds export -f json /tmp/all_wow_classic_builds.json -p wow_classic
    uv run cascette builds export -f json /tmp/all_wow_classic_era_builds.json -p wow_classic_era
    uv run cascette builds export -f json /tmp/all_wow_classic_titan_builds.json -p wow_classic_titan
    uv run tools/generate_verify_builds.py > /tmp/verify_builds.txt

Or export all products at once and merge:
    cd ~/Repos/github.com/wowemulation-dev/cascette-py
    for p in wow wow_anniversary wow_classic wow_classic_era wow_classic_titan; do
        uv run cascette builds export -f json "/tmp/builds_${p}.json" -p "$p" 2>/dev/null
    done
    uv run tools/generate_verify_builds.py /tmp/builds_wow.json /tmp/builds_wow_anniversary.json \
        /tmp/builds_wow_classic.json /tmp/builds_wow_classic_era.json \
        /tmp/builds_wow_classic_titan.json
"""

import json
import sys

PRODUCTS = [
    "wow",
    "wow_anniversary",
    "wow_classic",
    "wow_classic_era",
    "wow_classic_titan",
]

files = (
    sys.argv[1:]
    if len(sys.argv) > 1
    else [
        "/tmp/builds_wow.json",
        "/tmp/builds_wow_anniversary.json",
        "/tmp/builds_wow_classic.json",
        "/tmp/builds_wow_classic_era.json",
        "/tmp/builds_wow_classic_titan.json",
    ]
)

builds = {p: [] for p in PRODUCTS}

for path in files:
    with open(path) as f:
        for entry in json.load(f):
            product = entry["product"]
            if product not in builds:
                continue
            bc = entry.get("build_config", "")
            cc = entry.get("cdn_config", "")
            version = entry.get("version", "")
            if bc and cc:
                builds[product].append((version, bc, cc))

# Output header
total = sum(len(v) for v in builds.values())
counts = ", ".join(f"{p} ({len(builds[p])})" for p in PRODUCTS)
print(f"# Build list: {total} builds across {counts}")
print("# Data source: cascette-py wago_builds.db (wago.tools + BlizzTrack)")
print("#")
print("# To verify only a specific product, comment out the other sections.")
print("# To verify a single version, comment out all but that build's line.")
print("# Re-generate from cascette-py database:")
print("#   cd ~/Repos/github.com/wowemulation-dev/cascette-py")
print(
    "#   for p in wow wow_anniversary wow_classic wow_classic_era wow_classic_titan; do"
)
print('#       uv run cascette builds export -f json "/tmp/builds_${p}.json" -p "$p"')
print("#   done")
print("#   uv run tools/generate_verify_builds.py /tmp/builds_*.json")
print()

for product in PRODUCTS:
    blist = builds[product]
    print(f"# {product} ({len(blist)} builds)")
    print()
    for version, bc, cc in blist:
        print(f"# {product} {version} bc={bc} cc={cc}")
        print(f'verify_build "{product}" "{version}" "{bc}" "{cc}"')
    print()
