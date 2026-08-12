#!/usr/bin/env bash
#
# Generate --paths file lists for every WoW build against archive.wow.tools.
# Each output file contains one relative CDN path per line for every file
# tracked by that build (configs, manifests, archives, indices). Use with
# wget or curl (prepend your CDN base URL) to create a full mirror.
#
# Data source: cascette-py wago_builds.db (wago.tools + BlizzTrack).
# Generated: 2026-05-28
# Builds: wow, wow_classic, wow_classic_era, wow_classic_titan, wow_anniversary
#
# Output files are written atomically: build_file_tree writes to a temp file,
# which is moved into place only on success. Partial output from failed runs
# is removed. Existing files are skipped, so the script is safe to re-run
# after failures without re-fetching completed builds.
#
# Usage:
#   cargo build --release --example build_file_tree -p cascette-protocol
#   ./tools/generate_wow_paths.sh
#
# Environment:
#   BFT     Path to build_file_tree binary (default: ./target/release/examples/build_file_tree)
#   OUTDIR  Output directory for path lists (default: ./paths)
#
set -uo pipefail

BFT="${BFT:-./target/release/examples/build_file_tree}"
CDN="https://level3.blizzard.com"
CDN_PATH="tpr/wow"
OUTDIR="${OUTDIR:-./paths}"
TOTAL=0
SKIPPED=0
OK=0
FAIL=0

if [[ ! -x "$BFT" ]]; then
	echo "ERROR: build_file_tree not found at $BFT" >&2
	echo "Run: cargo build --release --example build_file_tree -p cascette-protocol" >&2
	exit 1
fi

mkdir -p "$OUTDIR"

# Atomic write: output goes to a temp file, moved into place only on success.
# Existing files are skipped (resume-safe). Partial files from failed runs
# are removed so they are retried next time.
run_build() {
	((TOTAL++))
	local outfile="$1"
	shift
	if [[ -f "$outfile" ]]; then
		((SKIPPED++))
		return 0
	fi
	local tmp
	tmp="$(mktemp "${outfile}.XXXXXX")"
	if "$@" >"$tmp" 2>/dev/null; then
		sed -i "s|${CDN}/||g" "$tmp"
		mv "$tmp" "$outfile"
		((OK++))
	else
		rm -f "$tmp"
		((FAIL++))
		echo "FAIL: $outfile" >&2
	fi
}

echo "Generating path lists into $OUTDIR ..." >&2

# wow 6.0.2.19033 bc=e9a6c927158d7e1444bfd1d5a57c7556 cc=b79ca6e8dd8ee742d3b51a058f1a29ca
run_build "$OUTDIR/wow_6.0.2.19033_b79ca6e8.txt" "$BFT" wow e9a6c927158d7e1444bfd1d5a57c7556 b79ca6e8dd8ee742d3b51a058f1a29ca "$CDN" "$CDN_PATH" --paths
# wow 6.0.2.19034 bc=b052c744ab97188cc23b0097f349f39c cc=b79ca6e8dd8ee742d3b51a058f1a29ca
run_build "$OUTDIR/wow_6.0.2.19034_b79ca6e8.txt" "$BFT" wow b052c744ab97188cc23b0097f349f39c b79ca6e8dd8ee742d3b51a058f1a29ca "$CDN" "$CDN_PATH" --paths
# wow 6.0.3.19085 bc=7cf8150fe72edf90bc68e5978dc951e9 cc=d683173b7b20c195b0167a28e6d9c666
run_build "$OUTDIR/wow_6.0.3.19085_d683173b.txt" "$BFT" wow 7cf8150fe72edf90bc68e5978dc951e9 d683173b7b20c195b0167a28e6d9c666 "$CDN" "$CDN_PATH" --paths
# wow 6.0.3.19102 bc=71dfb479f8890771ce834edf860c844f cc=cef61518113c115d1b52e8e4e0e011a0
run_build "$OUTDIR/wow_6.0.3.19102_cef61518.txt" "$BFT" wow 71dfb479f8890771ce834edf860c844f cef61518113c115d1b52e8e4e0e011a0 "$CDN" "$CDN_PATH" --paths
# wow 6.0.3.19103 bc=5911704e68e9031efddfabef036f1963 cc=d683173b7b20c195b0167a28e6d9c666
run_build "$OUTDIR/wow_6.0.3.19103_d683173b.txt" "$BFT" wow 5911704e68e9031efddfabef036f1963 d683173b7b20c195b0167a28e6d9c666 "$CDN" "$CDN_PATH" --paths
# wow 6.0.3.19116 bc=5ed06d5cfbec0ce4ba38f54cff1ab1f0 cc=51ad435e9199c897775b1c45a7aa880c
run_build "$OUTDIR/wow_6.0.3.19116_51ad435e.txt" "$BFT" wow 5ed06d5cfbec0ce4ba38f54cff1ab1f0 51ad435e9199c897775b1c45a7aa880c "$CDN" "$CDN_PATH" --paths
# wow 6.0.3.19200 bc=874c58841ef8b7fe9fa8fea023c8911a cc=f94ebf519ecade953e56ca63942b5f4c
run_build "$OUTDIR/wow_6.0.3.19200_f94ebf51.txt" "$BFT" wow 874c58841ef8b7fe9fa8fea023c8911a f94ebf519ecade953e56ca63942b5f4c "$CDN" "$CDN_PATH" --paths
# wow 6.0.3.19206 bc=00a1bf6ee6179ba90f0c5a86de3e0f18 cc=ea45754ea48dd89de460de565d5c782c
run_build "$OUTDIR/wow_6.0.3.19206_ea45754e.txt" "$BFT" wow 00a1bf6ee6179ba90f0c5a86de3e0f18 ea45754ea48dd89de460de565d5c782c "$CDN" "$CDN_PATH" --paths
# wow 6.0.3.19227 bc=df95b16e0d71c9a2c416cdbd3021e86c cc=9cc93ee72586a46d5aa87116f52e6892
run_build "$OUTDIR/wow_6.0.3.19227_9cc93ee7.txt" "$BFT" wow df95b16e0d71c9a2c416cdbd3021e86c 9cc93ee72586a46d5aa87116f52e6892 "$CDN" "$CDN_PATH" --paths
# wow 6.0.3.19243 bc=68cf030b5498765cfc2fd4e4d273f50c cc=ea45754ea48dd89de460de565d5c782c
run_build "$OUTDIR/wow_6.0.3.19243_ea45754e.txt" "$BFT" wow 68cf030b5498765cfc2fd4e4d273f50c ea45754ea48dd89de460de565d5c782c "$CDN" "$CDN_PATH" --paths
# wow 6.0.3.19342 bc=9aea7f1d7b4a4a2e4f1a9d2b0b144e9e cc=9cc93ee72586a46d5aa87116f52e6892
run_build "$OUTDIR/wow_6.0.3.19342_9cc93ee7.txt" "$BFT" wow 9aea7f1d7b4a4a2e4f1a9d2b0b144e9e 9cc93ee72586a46d5aa87116f52e6892 "$CDN" "$CDN_PATH" --paths
# wow 6.1.0.19678 bc=5aacafa310062316a2d1a6f45119690c cc=fbdd59f6caecc3e394da00511850a31d
run_build "$OUTDIR/wow_6.1.0.19678_fbdd59f6.txt" "$BFT" wow 5aacafa310062316a2d1a6f45119690c fbdd59f6caecc3e394da00511850a31d "$CDN" "$CDN_PATH" --paths
# wow 6.1.0.19701 bc=852d04dd6fcf8e993a8319a51f4403e3 cc=9094c3e07226a695cf434b97c1109da0
run_build "$OUTDIR/wow_6.1.0.19701_9094c3e0.txt" "$BFT" wow 852d04dd6fcf8e993a8319a51f4403e3 9094c3e07226a695cf434b97c1109da0 "$CDN" "$CDN_PATH" --paths
# wow 6.1.0.19702 bc=51bc569b6da2b00792fb14b1a777a512 cc=37910d1af1dd75970e3748d9f7fea9aa
run_build "$OUTDIR/wow_6.1.0.19702_37910d1a.txt" "$BFT" wow 51bc569b6da2b00792fb14b1a777a512 37910d1af1dd75970e3748d9f7fea9aa "$CDN" "$CDN_PATH" --paths
# wow 6.1.2.19802 bc=61f8c82956b47dc628690e96a7730e6d cc=eddcaecf0c067f5c9370063e00f02ecc
run_build "$OUTDIR/wow_6.1.2.19802_eddcaecf.txt" "$BFT" wow 61f8c82956b47dc628690e96a7730e6d eddcaecf0c067f5c9370063e00f02ecc "$CDN" "$CDN_PATH" --paths
# wow 6.1.2.19831 bc=b9d016a4cfa6c5addad4cbd69c3c9f2d cc=0686264f6efd5edd7fa8dd61ff7156e6
run_build "$OUTDIR/wow_6.1.2.19831_0686264f.txt" "$BFT" wow b9d016a4cfa6c5addad4cbd69c3c9f2d 0686264f6efd5edd7fa8dd61ff7156e6 "$CDN" "$CDN_PATH" --paths
# wow 6.1.2.19865 bc=c8e88fbc7aded52876069bcf33f28af9 cc=ae7a03a3475bae8072f8047d71ff289b
run_build "$OUTDIR/wow_6.1.2.19865_ae7a03a3.txt" "$BFT" wow c8e88fbc7aded52876069bcf33f28af9 ae7a03a3475bae8072f8047d71ff289b "$CDN" "$CDN_PATH" --paths
# wow 6.2.0.20173 bc=778d28f24890ccb7cd157c426c685414 cc=e0652b3c12c16104b0f63856a2fa7d12
run_build "$OUTDIR/wow_6.2.0.20173_e0652b3c.txt" "$BFT" wow 778d28f24890ccb7cd157c426c685414 e0652b3c12c16104b0f63856a2fa7d12 "$CDN" "$CDN_PATH" --paths
# wow 6.2.0.20182 bc=0ad1f56e776f7af2a722aa140ffe683e cc=4546da1c08fce7b4ee547459ecc40f1c
run_build "$OUTDIR/wow_6.2.0.20182_4546da1c.txt" "$BFT" wow 0ad1f56e776f7af2a722aa140ffe683e 4546da1c08fce7b4ee547459ecc40f1c "$CDN" "$CDN_PATH" --paths
# wow 6.2.0.20201 bc=ee785062415be47284129ea901942f78 cc=1faa7d404db3d86d2362bd228190bb6f
run_build "$OUTDIR/wow_6.2.0.20201_1faa7d40.txt" "$BFT" wow ee785062415be47284129ea901942f78 1faa7d404db3d86d2362bd228190bb6f "$CDN" "$CDN_PATH" --paths
# wow 6.2.0.20216 bc=0f0ad8cef5c83c73639fe6d3cb9fe910 cc=1faa7d404db3d86d2362bd228190bb6f
run_build "$OUTDIR/wow_6.2.0.20216_1faa7d40.txt" "$BFT" wow 0f0ad8cef5c83c73639fe6d3cb9fe910 1faa7d404db3d86d2362bd228190bb6f "$CDN" "$CDN_PATH" --paths
# wow 6.2.0.20253 bc=a38ad0a0609d670c2d2adc6628ea0dea cc=1faa7d404db3d86d2362bd228190bb6f
run_build "$OUTDIR/wow_6.2.0.20253_1faa7d40.txt" "$BFT" wow a38ad0a0609d670c2d2adc6628ea0dea 1faa7d404db3d86d2362bd228190bb6f "$CDN" "$CDN_PATH" --paths
# wow 6.2.0.20338 bc=4ebe38762e3805125488840e2e5f836d cc=1faa7d404db3d86d2362bd228190bb6f
run_build "$OUTDIR/wow_6.2.0.20338_1faa7d40.txt" "$BFT" wow 4ebe38762e3805125488840e2e5f836d 1faa7d404db3d86d2362bd228190bb6f "$CDN" "$CDN_PATH" --paths
# wow 6.2.2.20444 bc=0a6f07f48525c4203cb2fdbf6a7d7e9a cc=1faa7d404db3d86d2362bd228190bb6f
run_build "$OUTDIR/wow_6.2.2.20444_1faa7d40.txt" "$BFT" wow 0a6f07f48525c4203cb2fdbf6a7d7e9a 1faa7d404db3d86d2362bd228190bb6f "$CDN" "$CDN_PATH" --paths
# wow 6.2.2.20490 bc=6d3cbd5bfcadff181f33996a9b8df234 cc=1faa7d404db3d86d2362bd228190bb6f
run_build "$OUTDIR/wow_6.2.2.20490_1faa7d40.txt" "$BFT" wow 6d3cbd5bfcadff181f33996a9b8df234 1faa7d404db3d86d2362bd228190bb6f "$CDN" "$CDN_PATH" --paths
# wow 6.2.2.20574 bc=30607a99122cb01122a8a95643980444 cc=1faa7d404db3d86d2362bd228190bb6f
run_build "$OUTDIR/wow_6.2.2.20574_1faa7d40.txt" "$BFT" wow 30607a99122cb01122a8a95643980444 1faa7d404db3d86d2362bd228190bb6f "$CDN" "$CDN_PATH" --paths
# wow 6.2.3.20726 bc=f753912cd7925af825023131aa6a8353 cc=1faa7d404db3d86d2362bd228190bb6f
run_build "$OUTDIR/wow_6.2.3.20726_1faa7d40.txt" "$BFT" wow f753912cd7925af825023131aa6a8353 1faa7d404db3d86d2362bd228190bb6f "$CDN" "$CDN_PATH" --paths
# wow 6.2.3.20779 bc=00647afeb4fe9bfc6b961a0a532a4baa cc=1faa7d404db3d86d2362bd228190bb6f
run_build "$OUTDIR/wow_6.2.3.20779_1faa7d40.txt" "$BFT" wow 00647afeb4fe9bfc6b961a0a532a4baa 1faa7d404db3d86d2362bd228190bb6f "$CDN" "$CDN_PATH" --paths
# wow 6.2.3.20886 bc=dcff4b96b0461d42f3af5aa56304f2e1 cc=eee453d9035886d03a1308de041de88c
run_build "$OUTDIR/wow_6.2.3.20886_eee453d9.txt" "$BFT" wow dcff4b96b0461d42f3af5aa56304f2e1 eee453d9035886d03a1308de041de88c "$CDN" "$CDN_PATH" --paths
# wow 6.2.4.21345 bc=59fe87e7f2f34db95df473b2730efdae cc=eee453d9035886d03a1308de041de88c
run_build "$OUTDIR/wow_6.2.4.21345_eee453d9.txt" "$BFT" wow 59fe87e7f2f34db95df473b2730efdae eee453d9035886d03a1308de041de88c "$CDN" "$CDN_PATH" --paths
# wow 6.2.4.21348 bc=1ffc1fe7ed243c6be4739515c05535a2 cc=eee453d9035886d03a1308de041de88c
run_build "$OUTDIR/wow_6.2.4.21348_eee453d9.txt" "$BFT" wow 1ffc1fe7ed243c6be4739515c05535a2 eee453d9035886d03a1308de041de88c "$CDN" "$CDN_PATH" --paths
# wow 6.2.4.21355 bc=c63ecf5240444ef963a3e6e304f5446b cc=eee453d9035886d03a1308de041de88c
run_build "$OUTDIR/wow_6.2.4.21355_eee453d9.txt" "$BFT" wow c63ecf5240444ef963a3e6e304f5446b eee453d9035886d03a1308de041de88c "$CDN" "$CDN_PATH" --paths
# wow 6.2.4.21463 bc=b10b33224a2ee6058a3c75e099a9d78e cc=eee453d9035886d03a1308de041de88c
run_build "$OUTDIR/wow_6.2.4.21463_eee453d9.txt" "$BFT" wow b10b33224a2ee6058a3c75e099a9d78e eee453d9035886d03a1308de041de88c "$CDN" "$CDN_PATH" --paths
# wow 6.2.4.21676 bc=3a71686af003f38683c6eece4a6c2818 cc=eee453d9035886d03a1308de041de88c
run_build "$OUTDIR/wow_6.2.4.21676_eee453d9.txt" "$BFT" wow 3a71686af003f38683c6eece4a6c2818 eee453d9035886d03a1308de041de88c "$CDN" "$CDN_PATH" --paths
# wow 6.2.4.21742 bc=451679f16b266632976beb8d5c922d9f cc=eee453d9035886d03a1308de041de88c
run_build "$OUTDIR/wow_6.2.4.21742_eee453d9.txt" "$BFT" wow 451679f16b266632976beb8d5c922d9f eee453d9035886d03a1308de041de88c "$CDN" "$CDN_PATH" --paths

# wow 7.0.3.22248 bc=fc2db925b6468d17e8cc619ac25230a6 cc=5acd586a6aa49bb8c7c25f43069b50fe
run_build "$OUTDIR/wow_7.0.3.22248_5acd586a.txt" "$BFT" wow fc2db925b6468d17e8cc619ac25230a6 5acd586a6aa49bb8c7c25f43069b50fe "$CDN" "$CDN_PATH" --paths
# wow 7.0.3.22267 bc=8bea0908e0bd2519758481a1eefd70f0 cc=e0c21b79f1eb66e6d494b3f6b8e33ee7
run_build "$OUTDIR/wow_7.0.3.22267_e0c21b79.txt" "$BFT" wow 8bea0908e0bd2519758481a1eefd70f0 e0c21b79f1eb66e6d494b3f6b8e33ee7 "$CDN" "$CDN_PATH" --paths
# wow 7.0.3.22280 bc=188765318814113b8e80161515ad2fb5 cc=f9def04ed70108c54d18d05eff6b868a
run_build "$OUTDIR/wow_7.0.3.22280_f9def04e.txt" "$BFT" wow 188765318814113b8e80161515ad2fb5 f9def04ed70108c54d18d05eff6b868a "$CDN" "$CDN_PATH" --paths
# wow 7.0.3.22289 bc=55d5805a04d318bcd69a2aeed66fd29e cc=ea99a8165ae83764ab7c42a4d883da0f
run_build "$OUTDIR/wow_7.0.3.22289_ea99a816.txt" "$BFT" wow 55d5805a04d318bcd69a2aeed66fd29e ea99a8165ae83764ab7c42a4d883da0f "$CDN" "$CDN_PATH" --paths
# wow 7.0.3.22293 bc=46860318595610e57bc41142ee83c7d3 cc=62f2b9a23266bc35767fea353c5da4f6
run_build "$OUTDIR/wow_7.0.3.22293_62f2b9a2.txt" "$BFT" wow 46860318595610e57bc41142ee83c7d3 62f2b9a23266bc35767fea353c5da4f6 "$CDN" "$CDN_PATH" --paths
# wow 7.0.3.22345 bc=6e71d73f65cfbd49e42d3ed891c6e506 cc=1adb42788898133c827b0bdc61768900
run_build "$OUTDIR/wow_7.0.3.22345_1adb4278.txt" "$BFT" wow 6e71d73f65cfbd49e42d3ed891c6e506 1adb42788898133c827b0bdc61768900 "$CDN" "$CDN_PATH" --paths
# wow 7.0.3.22396 bc=d1f67616224f593156afc032f9085407 cc=b42beb529e5a0609db662dac770021c5
run_build "$OUTDIR/wow_7.0.3.22396_b42beb52.txt" "$BFT" wow d1f67616224f593156afc032f9085407 b42beb529e5a0609db662dac770021c5 "$CDN" "$CDN_PATH" --paths
# wow 7.0.3.22410 bc=789d8df6fd27d692d42e3a8392991cb9 cc=48c4af7b2f31b38f5cd2491d04939897
run_build "$OUTDIR/wow_7.0.3.22410_48c4af7b.txt" "$BFT" wow 789d8df6fd27d692d42e3a8392991cb9 48c4af7b2f31b38f5cd2491d04939897 "$CDN" "$CDN_PATH" --paths
# wow 7.0.3.22423 bc=93b8696d69a33b0de385f48120e07139 cc=4cbc83e551f4708a28d0a6da49249f64
run_build "$OUTDIR/wow_7.0.3.22423_4cbc83e5.txt" "$BFT" wow 93b8696d69a33b0de385f48120e07139 4cbc83e551f4708a28d0a6da49249f64 "$CDN" "$CDN_PATH" --paths
# wow 7.0.3.22498 bc=a6e8ce222e983a501c95636bbf0eba89 cc=33d1049125d1d88ee963d31c69a5f441
run_build "$OUTDIR/wow_7.0.3.22498_33d10491.txt" "$BFT" wow a6e8ce222e983a501c95636bbf0eba89 33d1049125d1d88ee963d31c69a5f441 "$CDN" "$CDN_PATH" --paths
# wow 7.0.3.22522 bc=b320416468aa8ce2638bc82345125ffc cc=72c378aa75b99343ef30a5283b62a8c3
run_build "$OUTDIR/wow_7.0.3.22522_72c378aa.txt" "$BFT" wow b320416468aa8ce2638bc82345125ffc 72c378aa75b99343ef30a5283b62a8c3 "$CDN" "$CDN_PATH" --paths
# wow 7.0.3.22566 bc=d4fcb55bc5db6b6dd2a967c5755e3bc3 cc=0271f9b8df0f65c86a0cd8e9d39978fb
run_build "$OUTDIR/wow_7.0.3.22566_0271f9b8.txt" "$BFT" wow d4fcb55bc5db6b6dd2a967c5755e3bc3 0271f9b8df0f65c86a0cd8e9d39978fb "$CDN" "$CDN_PATH" --paths
# wow 7.0.3.22594 bc=dc681a0ad63b213e57399faa5255bcd4 cc=e883f2c6a341485264f3eba4efb95a74
run_build "$OUTDIR/wow_7.0.3.22594_e883f2c6.txt" "$BFT" wow dc681a0ad63b213e57399faa5255bcd4 e883f2c6a341485264f3eba4efb95a74 "$CDN" "$CDN_PATH" --paths
# wow 7.0.3.22624 bc=f98a70219c024267cbe36365ca379a59 cc=7ab44e688fe50ca7a2a30244d957bd5c
run_build "$OUTDIR/wow_7.0.3.22624_7ab44e68.txt" "$BFT" wow f98a70219c024267cbe36365ca379a59 7ab44e688fe50ca7a2a30244d957bd5c "$CDN" "$CDN_PATH" --paths
# wow 7.0.3.22747 bc=44ad1361bbae9294cb2be83055318d6c cc=9cc76cbebe92ef4153ddb3e60571a62e
run_build "$OUTDIR/wow_7.0.3.22747_9cc76cbe.txt" "$BFT" wow 44ad1361bbae9294cb2be83055318d6c 9cc76cbebe92ef4153ddb3e60571a62e "$CDN" "$CDN_PATH" --paths
# wow 7.0.3.22810 bc=5aceaf7900be8d43f102fefe1cb60111 cc=9a717d72871609ada96b7ffdbadf8c09
run_build "$OUTDIR/wow_7.0.3.22810_9a717d72.txt" "$BFT" wow 5aceaf7900be8d43f102fefe1cb60111 9a717d72871609ada96b7ffdbadf8c09 "$CDN" "$CDN_PATH" --paths
# wow 7.1.0.22900 bc=ef83b590f8e6905a95d60346fd92d31d cc=53efecb88fd2c88415b9585a693058b2
run_build "$OUTDIR/wow_7.1.0.22900_53efecb8.txt" "$BFT" wow ef83b590f8e6905a95d60346fd92d31d 53efecb88fd2c88415b9585a693058b2 "$CDN" "$CDN_PATH" --paths
# wow 7.1.0.22908 bc=53e9ec572f318ed416a8e59714248abc cc=5c25595138b93452b570fa56ced4e5c2
run_build "$OUTDIR/wow_7.1.0.22908_5c255951.txt" "$BFT" wow 53e9ec572f318ed416a8e59714248abc 5c25595138b93452b570fa56ced4e5c2 "$CDN" "$CDN_PATH" --paths
# wow 7.1.0.22950 bc=4acd096e08b5d875ad98f5b33d41d4d5 cc=66315c41d97a0f5819964408297c6add
run_build "$OUTDIR/wow_7.1.0.22950_66315c41.txt" "$BFT" wow 4acd096e08b5d875ad98f5b33d41d4d5 66315c41d97a0f5819964408297c6add "$CDN" "$CDN_PATH" --paths
# wow 7.1.0.22989 bc=55ac761ccdae20634e2c8f3aeeea4f30 cc=2d2989efc87509c23337f61761a7b5c7
run_build "$OUTDIR/wow_7.1.0.22989_2d2989ef.txt" "$BFT" wow 55ac761ccdae20634e2c8f3aeeea4f30 2d2989efc87509c23337f61761a7b5c7 "$CDN" "$CDN_PATH" --paths
# wow 7.1.0.22995 bc=f1cfadb4527a32c022dccf2c6b3de495 cc=612e20285cce622d100fc701d7e9a8ff
run_build "$OUTDIR/wow_7.1.0.22995_612e2028.txt" "$BFT" wow f1cfadb4527a32c022dccf2c6b3de495 612e20285cce622d100fc701d7e9a8ff "$CDN" "$CDN_PATH" --paths
# wow 7.1.0.22996 bc=8e374bc49d4f3314a2a4497b065441e3 cc=04d06a1670cae89133addffef95d5e52
run_build "$OUTDIR/wow_7.1.0.22996_04d06a16.txt" "$BFT" wow 8e374bc49d4f3314a2a4497b065441e3 04d06a1670cae89133addffef95d5e52 "$CDN" "$CDN_PATH" --paths
# wow 7.1.0.23171 bc=61c1c20d8631eb54641d542f7fb84017 cc=db1f96efccfc9eb936403a14f561279e
run_build "$OUTDIR/wow_7.1.0.23171_db1f96ef.txt" "$BFT" wow 61c1c20d8631eb54641d542f7fb84017 db1f96efccfc9eb936403a14f561279e "$CDN" "$CDN_PATH" --paths
# wow 7.1.0.23222 bc=92a12949ec77db660d851c9f9084fdca cc=3c404e6ed3fa4b605e0974df0ee4fe17
run_build "$OUTDIR/wow_7.1.0.23222_3c404e6e.txt" "$BFT" wow 92a12949ec77db660d851c9f9084fdca 3c404e6ed3fa4b605e0974df0ee4fe17 "$CDN" "$CDN_PATH" --paths
# wow 7.1.5.23360 bc=de91bb8569a324d6663e4c1109816579 cc=e492771dc20684298ddffe23d2a60b5c
run_build "$OUTDIR/wow_7.1.5.23360_e492771d.txt" "$BFT" wow de91bb8569a324d6663e4c1109816579 e492771dc20684298ddffe23d2a60b5c "$CDN" "$CDN_PATH" --paths
# wow 7.1.5.23420 bc=7edb74ac61e856673deb1cbf6af263f8 cc=bd7b4fc5865118ed8ce8411450b42f3b
run_build "$OUTDIR/wow_7.1.5.23420_bd7b4fc5.txt" "$BFT" wow 7edb74ac61e856673deb1cbf6af263f8 bd7b4fc5865118ed8ce8411450b42f3b "$CDN" "$CDN_PATH" --paths
# wow 7.2.0.23835 bc=4fb88f2756db7737aaca2870a938fc7d cc=1fc3e4d55c3bfd81ac67645cc5926cb3
run_build "$OUTDIR/wow_7.2.0.23835_1fc3e4d5.txt" "$BFT" wow 4fb88f2756db7737aaca2870a938fc7d 1fc3e4d55c3bfd81ac67645cc5926cb3 "$CDN" "$CDN_PATH" --paths
# wow 7.2.0.23846 bc=c7a65ee7ac6679f34213a8959fab6a28 cc=f56bc5f379a9fcc658f5adc4dc4d50fe
run_build "$OUTDIR/wow_7.2.0.23846_f56bc5f3.txt" "$BFT" wow c7a65ee7ac6679f34213a8959fab6a28 f56bc5f379a9fcc658f5adc4dc4d50fe "$CDN" "$CDN_PATH" --paths
# wow 7.2.0.23852 bc=b546f985ba4688502e3a8145fca33675 cc=36523e793240c599a9fa1c683c89bc92
run_build "$OUTDIR/wow_7.2.0.23852_36523e79.txt" "$BFT" wow b546f985ba4688502e3a8145fca33675 36523e793240c599a9fa1c683c89bc92 "$CDN" "$CDN_PATH" --paths
# wow 7.2.0.23857 bc=8fd6c7b4652bd1c660402a3595e155b8 cc=6f1db3cd829296280a601309b451aafe
run_build "$OUTDIR/wow_7.2.0.23857_6f1db3cd.txt" "$BFT" wow 8fd6c7b4652bd1c660402a3595e155b8 6f1db3cd829296280a601309b451aafe "$CDN" "$CDN_PATH" --paths
# wow 7.2.0.23877 bc=b72e32a6dfdff0153bcaccd42cbe5b87 cc=dd3a477426555064f59fd95f4f4378ae
run_build "$OUTDIR/wow_7.2.0.23877_dd3a4774.txt" "$BFT" wow b72e32a6dfdff0153bcaccd42cbe5b87 dd3a477426555064f59fd95f4f4378ae "$CDN" "$CDN_PATH" --paths
# wow 7.2.0.23905 bc=0a41231c9716fb8548ba9d7b6f15a170 cc=2937460d1c932fa6cb36c648b8e2561d
run_build "$OUTDIR/wow_7.2.0.23905_2937460d.txt" "$BFT" wow 0a41231c9716fb8548ba9d7b6f15a170 2937460d1c932fa6cb36c648b8e2561d "$CDN" "$CDN_PATH" --paths
# wow 7.2.0.23911 bc=fc58e8bfcd2496d9dd00ce68067bd121 cc=e82a5ee4ee684631b2747d48c7ec309b
run_build "$OUTDIR/wow_7.2.0.23911_e82a5ee4.txt" "$BFT" wow fc58e8bfcd2496d9dd00ce68067bd121 e82a5ee4ee684631b2747d48c7ec309b "$CDN" "$CDN_PATH" --paths
# wow 7.2.0.23937 bc=e717b0cddf8e3688ae3149ad70808cf0 cc=2fcd94f43438f00ed351b14cb27a9b03
run_build "$OUTDIR/wow_7.2.0.23937_2fcd94f4.txt" "$BFT" wow e717b0cddf8e3688ae3149ad70808cf0 2fcd94f43438f00ed351b14cb27a9b03 "$CDN" "$CDN_PATH" --paths
# wow 7.2.0.24015 bc=920a8d7d1cae85d1545727757408228b cc=fd0a0fb9336a5670b213e2731f3b2e1e
run_build "$OUTDIR/wow_7.2.0.24015_fd0a0fb9.txt" "$BFT" wow 920a8d7d1cae85d1545727757408228b fd0a0fb9336a5670b213e2731f3b2e1e "$CDN" "$CDN_PATH" --paths
# wow 7.2.5.24330 bc=5f2fc18e9b4f08371fa6222d33e41169 cc=e6507283896acc41a12cd22f9aa9ed9a
run_build "$OUTDIR/wow_7.2.5.24330_e6507283.txt" "$BFT" wow 5f2fc18e9b4f08371fa6222d33e41169 e6507283896acc41a12cd22f9aa9ed9a "$CDN" "$CDN_PATH" --paths
# wow 7.2.5.24367 bc=d6dac68c6d2ab1a385a41c552ce38cf7 cc=13bfb297ab58586cf0fb915dfe91d20f
run_build "$OUTDIR/wow_7.2.5.24367_13bfb297.txt" "$BFT" wow d6dac68c6d2ab1a385a41c552ce38cf7 13bfb297ab58586cf0fb915dfe91d20f "$CDN" "$CDN_PATH" --paths
# wow 7.2.5.24415 bc=c0fb29eec6312fab2774f8642fb98028 cc=be013f01d14f07e9cc52ca3418093b6a
run_build "$OUTDIR/wow_7.2.5.24415_be013f01.txt" "$BFT" wow c0fb29eec6312fab2774f8642fb98028 be013f01d14f07e9cc52ca3418093b6a "$CDN" "$CDN_PATH" --paths
# wow 7.2.5.24430 bc=fbaca4f832224e32a1a3911473608854 cc=3545fbdaa36ef07d8835738cf4fd7e62
run_build "$OUTDIR/wow_7.2.5.24430_3545fbda.txt" "$BFT" wow fbaca4f832224e32a1a3911473608854 3545fbdaa36ef07d8835738cf4fd7e62 "$CDN" "$CDN_PATH" --paths
# wow 7.2.5.24461 bc=af3ee7e5183dac138040c3bb0a44b2f6 cc=f97c97f746e5df82a1b27ab550f2ffda
run_build "$OUTDIR/wow_7.2.5.24461_f97c97f7.txt" "$BFT" wow af3ee7e5183dac138040c3bb0a44b2f6 f97c97f746e5df82a1b27ab550f2ffda "$CDN" "$CDN_PATH" --paths
# wow 7.2.5.24742 bc=2082f3c37a0900985a6691a9d5ca819b cc=978e4801880e363f7fb8e6d2f304b39e
run_build "$OUTDIR/wow_7.2.5.24742_978e4801.txt" "$BFT" wow 2082f3c37a0900985a6691a9d5ca819b 978e4801880e363f7fb8e6d2f304b39e "$CDN" "$CDN_PATH" --paths
# wow 7.3.0.24920 bc=0c226a22f600a086edb2383788c42da5 cc=a489bbcef837fdff6a79e034338e883f
run_build "$OUTDIR/wow_7.3.0.24920_a489bbce.txt" "$BFT" wow 0c226a22f600a086edb2383788c42da5 a489bbcef837fdff6a79e034338e883f "$CDN" "$CDN_PATH" --paths
# wow 7.3.0.24931 bc=847111420f29e4e605bee2104ec0c5c7 cc=4ced57126e35805a8d24983ee1dd0ce5
run_build "$OUTDIR/wow_7.3.0.24931_4ced5712.txt" "$BFT" wow 847111420f29e4e605bee2104ec0c5c7 4ced57126e35805a8d24983ee1dd0ce5 "$CDN" "$CDN_PATH" --paths
# wow 7.3.0.24956 bc=26fd9d5c600b9405fb06f90d3bc6a7c0 cc=631157c82275de5cd6517f18b91d8b49
run_build "$OUTDIR/wow_7.3.0.24956_631157c8.txt" "$BFT" wow 26fd9d5c600b9405fb06f90d3bc6a7c0 631157c82275de5cd6517f18b91d8b49 "$CDN" "$CDN_PATH" --paths
# wow 7.3.0.24970 bc=d24b6f675c9a1dcfb98d86bd89768900 cc=6d7e36f5e74c53f204804ee057fe5a84
run_build "$OUTDIR/wow_7.3.0.24970_6d7e36f5.txt" "$BFT" wow d24b6f675c9a1dcfb98d86bd89768900 6d7e36f5e74c53f204804ee057fe5a84 "$CDN" "$CDN_PATH" --paths
# wow 7.3.0.24974 bc=2238ab9c57b672457a2fa6fe2107b388 cc=423364147752a596911aa1de2ff1f6a4
run_build "$OUTDIR/wow_7.3.0.24974_42336414.txt" "$BFT" wow 2238ab9c57b672457a2fa6fe2107b388 423364147752a596911aa1de2ff1f6a4 "$CDN" "$CDN_PATH" --paths
# wow 7.3.0.25021 bc=77bd0581f174dcaf4e1eedba6cb494ea cc=db50b78f803617b0335fa4edf22a97b2
run_build "$OUTDIR/wow_7.3.0.25021_db50b78f.txt" "$BFT" wow 77bd0581f174dcaf4e1eedba6cb494ea db50b78f803617b0335fa4edf22a97b2 "$CDN" "$CDN_PATH" --paths
# wow 7.3.0.25195 bc=76428aef3e9506074baaaf04529fd05c cc=d948434cc0079b7f543de1ace19aee1b
run_build "$OUTDIR/wow_7.3.0.25195_d948434c.txt" "$BFT" wow 76428aef3e9506074baaaf04529fd05c d948434cc0079b7f543de1ace19aee1b "$CDN" "$CDN_PATH" --paths
# wow 7.3.2.25326 bc=bd2ecaa37161829978c30659269bbd3c cc=24571681b4c4766082e41cef05f532d1
run_build "$OUTDIR/wow_7.3.2.25326_24571681.txt" "$BFT" wow bd2ecaa37161829978c30659269bbd3c 24571681b4c4766082e41cef05f532d1 "$CDN" "$CDN_PATH" --paths
# wow 7.3.2.25383 bc=9af48e10cc8066587aa2004c47a0d4f7 cc=c343ed36eb3616c4b7b682f904601675
run_build "$OUTDIR/wow_7.3.2.25383_c343ed36.txt" "$BFT" wow 9af48e10cc8066587aa2004c47a0d4f7 c343ed36eb3616c4b7b682f904601675 "$CDN" "$CDN_PATH" --paths
# wow 7.3.2.25442 bc=af288dfb3c421a3ba1d4b6f05beda16b cc=24d805f8397761280e1bb126ca957b49
run_build "$OUTDIR/wow_7.3.2.25442_24d805f8.txt" "$BFT" wow af288dfb3c421a3ba1d4b6f05beda16b 24d805f8397761280e1bb126ca957b49 "$CDN" "$CDN_PATH" --paths
# wow 7.3.2.25455 bc=cf67fb8df72246820d7f4a04b9dacd01 cc=4e12707a0c295bf3962c8495e5334c51
run_build "$OUTDIR/wow_7.3.2.25455_4e12707a.txt" "$BFT" wow cf67fb8df72246820d7f4a04b9dacd01 4e12707a0c295bf3962c8495e5334c51 "$CDN" "$CDN_PATH" --paths
# wow 7.3.2.25477 bc=2f7f76194c170c07363f37839edb793c cc=2fe770b0359be5891f6145fac5f29e3b
run_build "$OUTDIR/wow_7.3.2.25477_2fe770b0.txt" "$BFT" wow 2f7f76194c170c07363f37839edb793c 2fe770b0359be5891f6145fac5f29e3b "$CDN" "$CDN_PATH" --paths
# wow 7.3.2.25480 bc=9c5e9c5e7eb91f6cf4865ded1f0a2214 cc=6624935ed1d932d881117a624ba60ce3
run_build "$OUTDIR/wow_7.3.2.25480_6624935e.txt" "$BFT" wow 9c5e9c5e7eb91f6cf4865ded1f0a2214 6624935ed1d932d881117a624ba60ce3 "$CDN" "$CDN_PATH" --paths
# wow 7.3.2.25497 bc=343baba0086b95764c5cd552c69ed582 cc=a1608a8c140942ecb346856b69ea986e
run_build "$OUTDIR/wow_7.3.2.25497_a1608a8c.txt" "$BFT" wow 343baba0086b95764c5cd552c69ed582 a1608a8c140942ecb346856b69ea986e "$CDN" "$CDN_PATH" --paths
# wow 7.3.2.25516 bc=d4c47cc7a711502628d58c565fcc6320 cc=7d6048c338de1a8bf78602b1bf179531
run_build "$OUTDIR/wow_7.3.2.25516_7d6048c3.txt" "$BFT" wow d4c47cc7a711502628d58c565fcc6320 7d6048c338de1a8bf78602b1bf179531 "$CDN" "$CDN_PATH" --paths
# wow 7.3.2.25549 bc=0dcd27adeb73b039302160d07f6c3402 cc=3cf0b3120dd69d0d754e6d025f2d150f
run_build "$OUTDIR/wow_7.3.2.25549_3cf0b312.txt" "$BFT" wow 0dcd27adeb73b039302160d07f6c3402 3cf0b3120dd69d0d754e6d025f2d150f "$CDN" "$CDN_PATH" --paths
# wow 7.3.5.25848 bc=5ba8fbf1e654bcd4c36e28b80984adb7 cc=ddb19d7bb629a51c0224837d39bf4ea8
run_build "$OUTDIR/wow_7.3.5.25848_ddb19d7b.txt" "$BFT" wow 5ba8fbf1e654bcd4c36e28b80984adb7 ddb19d7bb629a51c0224837d39bf4ea8 "$CDN" "$CDN_PATH" --paths
# wow 7.3.5.25860 bc=15b175164993640f5d85486753d1b803 cc=93d70c7cef4f8b4dc1f488a17be413bd
run_build "$OUTDIR/wow_7.3.5.25860_93d70c7c.txt" "$BFT" wow 15b175164993640f5d85486753d1b803 93d70c7cef4f8b4dc1f488a17be413bd "$CDN" "$CDN_PATH" --paths
# wow 7.3.5.25864 bc=701434da3e787bb93826d142b7b6d11b cc=2b2a51e30295d250726480fdb3196cb9
run_build "$OUTDIR/wow_7.3.5.25864_2b2a51e3.txt" "$BFT" wow 701434da3e787bb93826d142b7b6d11b 2b2a51e30295d250726480fdb3196cb9 "$CDN" "$CDN_PATH" --paths
# wow 7.3.5.25875 bc=b4906073030a87426549019be85ca48c cc=7483079edbc3dc49407ececea3734ba3
run_build "$OUTDIR/wow_7.3.5.25875_7483079e.txt" "$BFT" wow b4906073030a87426549019be85ca48c 7483079edbc3dc49407ececea3734ba3 "$CDN" "$CDN_PATH" --paths
# wow 7.3.5.25881 bc=4986c32b571a378bea05aca8fbb8606d cc=7483079edbc3dc49407ececea3734ba3
run_build "$OUTDIR/wow_7.3.5.25881_7483079e.txt" "$BFT" wow 4986c32b571a378bea05aca8fbb8606d 7483079edbc3dc49407ececea3734ba3 "$CDN" "$CDN_PATH" --paths
# wow 7.3.5.25901 bc=b78fcad283cbd6b9f1943749e6062555 cc=9be09f130fc69821e43fb2d67338ae27
run_build "$OUTDIR/wow_7.3.5.25901_9be09f13.txt" "$BFT" wow b78fcad283cbd6b9f1943749e6062555 9be09f130fc69821e43fb2d67338ae27 "$CDN" "$CDN_PATH" --paths
# wow 7.3.5.25928 bc=6df817ea0b1bbfac586d397f602a3a55 cc=6a47ebaffa6e2c30b590aa948e8978b3
run_build "$OUTDIR/wow_7.3.5.25928_6a47ebaf.txt" "$BFT" wow 6df817ea0b1bbfac586d397f602a3a55 6a47ebaffa6e2c30b590aa948e8978b3 "$CDN" "$CDN_PATH" --paths
# wow 7.3.5.25937 bc=cb455466d60a7ae7ef80615f942282f4 cc=9ebd791ee80ed2e6956c84a1a8ffd532
run_build "$OUTDIR/wow_7.3.5.25937_9ebd791e.txt" "$BFT" wow cb455466d60a7ae7ef80615f942282f4 9ebd791ee80ed2e6956c84a1a8ffd532 "$CDN" "$CDN_PATH" --paths
# wow 7.3.5.25944 bc=b89d98802d929d7bacab5bc3517cfe85 cc=cb52862b44e10de6180e0ca0a6b9961e
run_build "$OUTDIR/wow_7.3.5.25944_cb52862b.txt" "$BFT" wow b89d98802d929d7bacab5bc3517cfe85 cb52862b44e10de6180e0ca0a6b9961e "$CDN" "$CDN_PATH" --paths
# wow 7.3.5.25946 bc=46ea3f1cd58173bf87ab6abdda61f46a cc=db0cdc2146145b6df331b0edb1667192
run_build "$OUTDIR/wow_7.3.5.25946_db0cdc21.txt" "$BFT" wow 46ea3f1cd58173bf87ab6abdda61f46a db0cdc2146145b6df331b0edb1667192 "$CDN" "$CDN_PATH" --paths
# wow 7.3.5.25950 bc=336fec4604c35eaa4c8e51816687b6e6 cc=726da8ca90a261715e8b3950596c8e44
run_build "$OUTDIR/wow_7.3.5.25950_726da8ca.txt" "$BFT" wow 336fec4604c35eaa4c8e51816687b6e6 726da8ca90a261715e8b3950596c8e44 "$CDN" "$CDN_PATH" --paths
# wow 7.3.5.25961 bc=d15c2dce80730c683e298b021a76af51 cc=5679d80d3c0bb9535279cee6a7c625c9
run_build "$OUTDIR/wow_7.3.5.25961_5679d80d.txt" "$BFT" wow d15c2dce80730c683e298b021a76af51 5679d80d3c0bb9535279cee6a7c625c9 "$CDN" "$CDN_PATH" --paths
# wow 7.3.5.25996 bc=c23a2f39506457c036d70547b805a3e7 cc=e7a17048eaf5d62036567e581449dc68
run_build "$OUTDIR/wow_7.3.5.25996_e7a17048.txt" "$BFT" wow c23a2f39506457c036d70547b805a3e7 e7a17048eaf5d62036567e581449dc68 "$CDN" "$CDN_PATH" --paths
# wow 7.3.5.26124 bc=3ef5c6be011f86827ccd3da2048732c3 cc=74f7ebe453e840066c40c054b880922e
run_build "$OUTDIR/wow_7.3.5.26124_74f7ebe4.txt" "$BFT" wow 3ef5c6be011f86827ccd3da2048732c3 74f7ebe453e840066c40c054b880922e "$CDN" "$CDN_PATH" --paths
# wow 7.3.5.26365 bc=69b0ffdd142c4aac45d9fbc416e7231c cc=b33848229e0c6caba5b85cc3db7a3b58
run_build "$OUTDIR/wow_7.3.5.26365_b3384822.txt" "$BFT" wow 69b0ffdd142c4aac45d9fbc416e7231c b33848229e0c6caba5b85cc3db7a3b58 "$CDN" "$CDN_PATH" --paths
# wow 7.3.5.26654 bc=21475b203e1a73938383dbfd2657b272 cc=23012416753696c8c1021906f9072cec
run_build "$OUTDIR/wow_7.3.5.26654_23012416.txt" "$BFT" wow 21475b203e1a73938383dbfd2657b272 23012416753696c8c1021906f9072cec "$CDN" "$CDN_PATH" --paths
# wow 7.3.5.26755 bc=6cbc90da645e300498c1774a364ea391 cc=a53c2c5b8034121e9bb626b8add01574
run_build "$OUTDIR/wow_7.3.5.26755_a53c2c5b.txt" "$BFT" wow 6cbc90da645e300498c1774a364ea391 a53c2c5b8034121e9bb626b8add01574 "$CDN" "$CDN_PATH" --paths
# wow 7.3.5.26822 bc=294fe09eae155958e7c4679703461c39 cc=14e1f67c3de144b28be3e45fd1df819e
run_build "$OUTDIR/wow_7.3.5.26822_14e1f67c.txt" "$BFT" wow 294fe09eae155958e7c4679703461c39 14e1f67c3de144b28be3e45fd1df819e "$CDN" "$CDN_PATH" --paths
# wow 7.3.5.26899 bc=39d9e071cd0443305b534d40a9778a74 cc=904cbe7ad0516a69dc34c264fa21530b
run_build "$OUTDIR/wow_7.3.5.26899_904cbe7a.txt" "$BFT" wow 39d9e071cd0443305b534d40a9778a74 904cbe7ad0516a69dc34c264fa21530b "$CDN" "$CDN_PATH" --paths
# wow 7.3.5.26972 bc=3b0517b51edbe0b96f6ac5ea7eaaed38 cc=da4896ce91922122bc0a2371ee114423
run_build "$OUTDIR/wow_7.3.5.26972_da4896ce.txt" "$BFT" wow 3b0517b51edbe0b96f6ac5ea7eaaed38 da4896ce91922122bc0a2371ee114423 "$CDN" "$CDN_PATH" --paths

# wow 8.0.1.26926 bc=e8f3d86ab313c4bdf5cf4f5daea32477 cc=1964f5dfcb945c1b31acabedeaf13d09
run_build "$OUTDIR/wow_8.0.1.26926_1964f5df.txt" "$BFT" wow e8f3d86ab313c4bdf5cf4f5daea32477 1964f5dfcb945c1b31acabedeaf13d09 "$CDN" "$CDN_PATH" --paths
# wow 8.0.1.27026 bc=535866b6e12d03b5983b5c33b5140582 cc=1de9bfb0fd7fdcf1083010a39fd8c538
run_build "$OUTDIR/wow_8.0.1.27026_1de9bfb0.txt" "$BFT" wow 535866b6e12d03b5983b5c33b5140582 1de9bfb0fd7fdcf1083010a39fd8c538 "$CDN" "$CDN_PATH" --paths
# wow 8.0.1.27101 bc=926defae155eff5ead45bd220a6e795d cc=64807bbe25e10b94729dd8d709a56d4e
run_build "$OUTDIR/wow_8.0.1.27101_64807bbe.txt" "$BFT" wow 926defae155eff5ead45bd220a6e795d 64807bbe25e10b94729dd8d709a56d4e "$CDN" "$CDN_PATH" --paths
# wow 8.0.1.27144 bc=878ee8fa771d50905a1e860a2a761c48 cc=23179ec9cc7f9b9c16e3efab8131c85f
run_build "$OUTDIR/wow_8.0.1.27144_23179ec9.txt" "$BFT" wow 878ee8fa771d50905a1e860a2a761c48 23179ec9cc7f9b9c16e3efab8131c85f "$CDN" "$CDN_PATH" --paths
# wow 8.0.1.27165 bc=be115ab9abb29934b695a0f719c08cd5 cc=a33a2dfc3ce501781dda0d4ef2c4c077
run_build "$OUTDIR/wow_8.0.1.27165_a33a2dfc.txt" "$BFT" wow be115ab9abb29934b695a0f719c08cd5 a33a2dfc3ce501781dda0d4ef2c4c077 "$CDN" "$CDN_PATH" --paths
# wow 8.0.1.27178 bc=bc4704ff59092aec31b2cf8e9267875e cc=695842923131f554492923994c7d6795
run_build "$OUTDIR/wow_8.0.1.27178_69584292.txt" "$BFT" wow bc4704ff59092aec31b2cf8e9267875e 695842923131f554492923994c7d6795 "$CDN" "$CDN_PATH" --paths
# wow 8.0.1.27219 bc=7cde34b5e9929706fb42650c13fc3460 cc=6169fef5bb011eabeb07f587e8021e87
run_build "$OUTDIR/wow_8.0.1.27219_6169fef5.txt" "$BFT" wow 7cde34b5e9929706fb42650c13fc3460 6169fef5bb011eabeb07f587e8021e87 "$CDN" "$CDN_PATH" --paths
# wow 8.0.1.27291 bc=4eb3986466ec004ffa1755642b375a87 cc=fb445ca0526699c61a92830ab894a985
run_build "$OUTDIR/wow_8.0.1.27291_fb445ca0.txt" "$BFT" wow 4eb3986466ec004ffa1755642b375a87 fb445ca0526699c61a92830ab894a985 "$CDN" "$CDN_PATH" --paths
# wow 8.0.1.27326 bc=f98c14b78c838c41a147190d26a85677 cc=e7cbd67bcbf5576044cbe7653cb698e4
run_build "$OUTDIR/wow_8.0.1.27326_e7cbd67b.txt" "$BFT" wow f98c14b78c838c41a147190d26a85677 e7cbd67bcbf5576044cbe7653cb698e4 "$CDN" "$CDN_PATH" --paths
# wow 8.0.1.27353 bc=48c5dc53334d78149b2ec735e69c4c5b cc=08b346d869586a3fdbde95f58c39b3f7
run_build "$OUTDIR/wow_8.0.1.27353_08b346d8.txt" "$BFT" wow 48c5dc53334d78149b2ec735e69c4c5b 08b346d869586a3fdbde95f58c39b3f7 "$CDN" "$CDN_PATH" --paths
# wow 8.0.1.27355 bc=cc45cd10e8ec35b5cc30a39f965f6a6c cc=adad70c3ea0f5b8b4ee173ef43ef9bc1
run_build "$OUTDIR/wow_8.0.1.27355_adad70c3.txt" "$BFT" wow cc45cd10e8ec35b5cc30a39f965f6a6c adad70c3ea0f5b8b4ee173ef43ef9bc1 "$CDN" "$CDN_PATH" --paths
# wow 8.0.1.27356 bc=278cecdfa71085238809b168bba94b73 cc=3a3d53394dfe315df8c4b3cf72108acf
run_build "$OUTDIR/wow_8.0.1.27356_3a3d5339.txt" "$BFT" wow 278cecdfa71085238809b168bba94b73 3a3d53394dfe315df8c4b3cf72108acf "$CDN" "$CDN_PATH" --paths
# wow 8.0.1.27366 bc=abbcd2ddb3199eeaecf45594e6ef7cdb cc=d44a99259c667ffa386b0ed781864301
run_build "$OUTDIR/wow_8.0.1.27366_d44a9925.txt" "$BFT" wow abbcd2ddb3199eeaecf45594e6ef7cdb d44a99259c667ffa386b0ed781864301 "$CDN" "$CDN_PATH" --paths
# wow 8.0.1.27377 bc=5227139c806caca0a153a6fb1f37b364 cc=7c6fd0de33e2563a4e425113afd40121
run_build "$OUTDIR/wow_8.0.1.27377_7c6fd0de.txt" "$BFT" wow 5227139c806caca0a153a6fb1f37b364 7c6fd0de33e2563a4e425113afd40121 "$CDN" "$CDN_PATH" --paths
# wow 8.0.1.27404 bc=6c39970302e3130855a41c7b95cdcb8d cc=e722253d34adff160332e3071e5c25f3
run_build "$OUTDIR/wow_8.0.1.27404_e722253d.txt" "$BFT" wow 6c39970302e3130855a41c7b95cdcb8d e722253d34adff160332e3071e5c25f3 "$CDN" "$CDN_PATH" --paths
# wow 8.0.1.27481 bc=f7e68fd6611317050be908301b944855 cc=529986134eb480bad97dddd9fe1226e8
run_build "$OUTDIR/wow_8.0.1.27481_52998613.txt" "$BFT" wow f7e68fd6611317050be908301b944855 529986134eb480bad97dddd9fe1226e8 "$CDN" "$CDN_PATH" --paths
# wow 8.0.1.27547 bc=60fa3d3492bffb41c02cd338b7604149 cc=0777e1453ad9168ac5312828dd75d990
run_build "$OUTDIR/wow_8.0.1.27547_0777e145.txt" "$BFT" wow 60fa3d3492bffb41c02cd338b7604149 0777e1453ad9168ac5312828dd75d990 "$CDN" "$CDN_PATH" --paths
# wow 8.0.1.27602 bc=149c4a1a3e8b0475d6ca1465dd239590 cc=88286ad271d599df75bc767472f5ea9d
run_build "$OUTDIR/wow_8.0.1.27602_88286ad2.txt" "$BFT" wow 149c4a1a3e8b0475d6ca1465dd239590 88286ad271d599df75bc767472f5ea9d "$CDN" "$CDN_PATH" --paths
# wow 8.0.1.27791 bc=44afdd64f250ce12341b3553e6742319 cc=d77973c206ae0f8fcc50402d14ed0c91
run_build "$OUTDIR/wow_8.0.1.27791_d77973c2.txt" "$BFT" wow 44afdd64f250ce12341b3553e6742319 d77973c206ae0f8fcc50402d14ed0c91 "$CDN" "$CDN_PATH" --paths
# wow 8.0.1.27843 bc=62df0fecacf5ad5bcb8c670a7a2a5a5d cc=4eaaf63ff8151038001f71b39ddd1921
run_build "$OUTDIR/wow_8.0.1.27843_4eaaf63f.txt" "$BFT" wow 62df0fecacf5ad5bcb8c670a7a2a5a5d 4eaaf63ff8151038001f71b39ddd1921 "$CDN" "$CDN_PATH" --paths
# wow 8.0.1.27980 bc=9e057f62b76854841790a935ebe5ece2 cc=da1addb1f5b41eb5f6bfee72f84c1867
run_build "$OUTDIR/wow_8.0.1.27980_da1addb1.txt" "$BFT" wow 9e057f62b76854841790a935ebe5ece2 da1addb1f5b41eb5f6bfee72f84c1867 "$CDN" "$CDN_PATH" --paths
# wow 8.0.1.28153 bc=c19c4111371a31321f0161466c32094f cc=602c4c0c42e414a3c9a2b6978a2ff694
run_build "$OUTDIR/wow_8.0.1.28153_602c4c0c.txt" "$BFT" wow c19c4111371a31321f0161466c32094f 602c4c0c42e414a3c9a2b6978a2ff694 "$CDN" "$CDN_PATH" --paths
# wow 8.1.0.28657 bc=aeecaf500559724e3f1e42511e80e007 cc=ddee861376d1272c6344cdb737ab10fb
run_build "$OUTDIR/wow_8.1.0.28657_ddee8613.txt" "$BFT" wow aeecaf500559724e3f1e42511e80e007 ddee861376d1272c6344cdb737ab10fb "$CDN" "$CDN_PATH" --paths
# wow 8.1.0.28724 bc=2e166369ce9005c2e45e6bfb7f539df4 cc=aa2bb98dab115460c9eac79f1290a380
run_build "$OUTDIR/wow_8.1.0.28724_aa2bb98d.txt" "$BFT" wow 2e166369ce9005c2e45e6bfb7f539df4 aa2bb98dab115460c9eac79f1290a380 "$CDN" "$CDN_PATH" --paths
# wow 8.1.0.28768 bc=e5dfcf67120d72307d92e77c5430791e cc=cc532758174e978f29931e3460875964
run_build "$OUTDIR/wow_8.1.0.28768_cc532758.txt" "$BFT" wow e5dfcf67120d72307d92e77c5430791e cc532758174e978f29931e3460875964 "$CDN" "$CDN_PATH" --paths
# wow 8.1.0.28807 bc=afa23c26c61a579852a1aac950b2ae99 cc=29e2d8476bfdf49f3bf7fa01cd17ddfc
run_build "$OUTDIR/wow_8.1.0.28807_29e2d847.txt" "$BFT" wow afa23c26c61a579852a1aac950b2ae99 29e2d8476bfdf49f3bf7fa01cd17ddfc "$CDN" "$CDN_PATH" --paths
# wow 8.1.0.28822 bc=63cdb738cfc92551eec77795fe4a9246 cc=e38cb5cfce079537a00b34bc47b26015
run_build "$OUTDIR/wow_8.1.0.28822_e38cb5cf.txt" "$BFT" wow 63cdb738cfc92551eec77795fe4a9246 e38cb5cfce079537a00b34bc47b26015 "$CDN" "$CDN_PATH" --paths
# wow 8.1.0.28833 bc=1a0b0cbe3ed11143a44804dc8009b564 cc=d1f253b54f939ae242c91a7f52435b5e
run_build "$OUTDIR/wow_8.1.0.28833_d1f253b5.txt" "$BFT" wow 1a0b0cbe3ed11143a44804dc8009b564 d1f253b54f939ae242c91a7f52435b5e "$CDN" "$CDN_PATH" --paths
# wow 8.1.0.29088 bc=42fc236a621319f62a2ff8cc92d6e06e cc=9245e2346e85fae5218ba7c6e6b4a1ef
run_build "$OUTDIR/wow_8.1.0.29088_9245e234.txt" "$BFT" wow 42fc236a621319f62a2ff8cc92d6e06e 9245e2346e85fae5218ba7c6e6b4a1ef "$CDN" "$CDN_PATH" --paths
# wow 8.1.0.29139 bc=2745a193f63643cf3ca4575855e1d972 cc=90f19816a9bb754227835dc55936d6a3
run_build "$OUTDIR/wow_8.1.0.29139_90f19816.txt" "$BFT" wow 2745a193f63643cf3ca4575855e1d972 90f19816a9bb754227835dc55936d6a3 "$CDN" "$CDN_PATH" --paths
# wow 8.1.0.29235 bc=04ecdd83ef4b408c3a96ca14db25202c cc=8f2dd15139b9437c3ab197285c47dfc9
run_build "$OUTDIR/wow_8.1.0.29235_8f2dd151.txt" "$BFT" wow 04ecdd83ef4b408c3a96ca14db25202c 8f2dd15139b9437c3ab197285c47dfc9 "$CDN" "$CDN_PATH" --paths
# wow 8.1.0.29285 bc=7064f4e56e2f688f7d9cb270728256a9 cc=7ed096e1370c171c81cf767fb7dc56c8
run_build "$OUTDIR/wow_8.1.0.29285_7ed096e1.txt" "$BFT" wow 7064f4e56e2f688f7d9cb270728256a9 7ed096e1370c171c81cf767fb7dc56c8 "$CDN" "$CDN_PATH" --paths
# wow 8.1.0.29297 bc=7d9631ffed095aa3b750c3053694d6be cc=8bd59fa5d8c12aa5a643a06fb06e5485
run_build "$OUTDIR/wow_8.1.0.29297_8bd59fa5.txt" "$BFT" wow 7d9631ffed095aa3b750c3053694d6be 8bd59fa5d8c12aa5a643a06fb06e5485 "$CDN" "$CDN_PATH" --paths
# wow 8.1.0.29482 bc=27fc0317b1a4871f33aa6580df44789b cc=e53ec3d6dd962e548eba3a3388c0535e
run_build "$OUTDIR/wow_8.1.0.29482_e53ec3d6.txt" "$BFT" wow 27fc0317b1a4871f33aa6580df44789b e53ec3d6dd962e548eba3a3388c0535e "$CDN" "$CDN_PATH" --paths
# wow 8.1.0.29600 bc=a5430d7a5a890eeb986105aa31fbf444 cc=50cce8b8a984a0aca7f7d11dd85754eb
run_build "$OUTDIR/wow_8.1.0.29600_50cce8b8.txt" "$BFT" wow a5430d7a5a890eeb986105aa31fbf444 50cce8b8a984a0aca7f7d11dd85754eb "$CDN" "$CDN_PATH" --paths
# wow 8.1.0.29621 bc=d9edbbdf677de04cb57b0f33a99760c4 cc=aae21aea9ea596241555470c65793148
run_build "$OUTDIR/wow_8.1.0.29621_aae21aea.txt" "$BFT" wow d9edbbdf677de04cb57b0f33a99760c4 aae21aea9ea596241555470c65793148 "$CDN" "$CDN_PATH" --paths
# wow 8.1.5.29620 bc=4648e13a653fac27fdc8e896a501e6e9 cc=aae21aea9ea596241555470c65793148
run_build "$OUTDIR/wow_8.1.5.29620_aae21aea.txt" "$BFT" wow 4648e13a653fac27fdc8e896a501e6e9 aae21aea9ea596241555470c65793148 "$CDN" "$CDN_PATH" --paths
# wow 8.1.5.29683 bc=1570f95c5849b4a5a917846d3970c9dc cc=f82a63009d76bc5c194e7858173d2c91
run_build "$OUTDIR/wow_8.1.5.29683_f82a6300.txt" "$BFT" wow 1570f95c5849b4a5a917846d3970c9dc f82a63009d76bc5c194e7858173d2c91 "$CDN" "$CDN_PATH" --paths
# wow 8.1.5.29701 bc=287ca102d210e2aef7c96050f3d8406b cc=72a6fcdbea60bcb1ef285a099846a129
run_build "$OUTDIR/wow_8.1.5.29701_72a6fcdb.txt" "$BFT" wow 287ca102d210e2aef7c96050f3d8406b 72a6fcdbea60bcb1ef285a099846a129 "$CDN" "$CDN_PATH" --paths
# wow 8.1.5.29704 bc=9b51144fa18121db9b58f9408369c3b8 cc=741de78a9171ca34af847f5f6726ad49
run_build "$OUTDIR/wow_8.1.5.29704_741de78a.txt" "$BFT" wow 9b51144fa18121db9b58f9408369c3b8 741de78a9171ca34af847f5f6726ad49 "$CDN" "$CDN_PATH" --paths
# wow 8.1.5.29705 bc=dba969aefc9cc24b43df80aba54f5871 cc=b423ec9cf4e08209f58c18d05bc43d5b
run_build "$OUTDIR/wow_8.1.5.29705_b423ec9c.txt" "$BFT" wow dba969aefc9cc24b43df80aba54f5871 b423ec9cf4e08209f58c18d05bc43d5b "$CDN" "$CDN_PATH" --paths
# wow 8.1.5.29718 bc=e32dca77ab0103a71667c37c29380227 cc=d04df0bf4cf903f545539fd5c92bd111
run_build "$OUTDIR/wow_8.1.5.29718_d04df0bf.txt" "$BFT" wow e32dca77ab0103a71667c37c29380227 d04df0bf4cf903f545539fd5c92bd111 "$CDN" "$CDN_PATH" --paths
# wow 8.1.5.29732 bc=1b11396573f1898b2d89898213d1e5bd cc=46110cbed586bef99977bb74e47c8365
run_build "$OUTDIR/wow_8.1.5.29732_46110cbe.txt" "$BFT" wow 1b11396573f1898b2d89898213d1e5bd 46110cbed586bef99977bb74e47c8365 "$CDN" "$CDN_PATH" --paths
# wow 8.1.5.29737 bc=8667695bde97cb9f5deec61cbd4ae77c cc=f3dc86606de0f074c493c67b050748f8
run_build "$OUTDIR/wow_8.1.5.29737_f3dc8660.txt" "$BFT" wow 8667695bde97cb9f5deec61cbd4ae77c f3dc86606de0f074c493c67b050748f8 "$CDN" "$CDN_PATH" --paths
# wow 8.1.5.29814 bc=f62d4775fee1faeb2c1d51d1f27c2e08 cc=919dc17ab0cc60b3c3ef7e0c895009c4
run_build "$OUTDIR/wow_8.1.5.29814_919dc17a.txt" "$BFT" wow f62d4775fee1faeb2c1d51d1f27c2e08 919dc17ab0cc60b3c3ef7e0c895009c4 "$CDN" "$CDN_PATH" --paths
# wow 8.1.5.29869 bc=e35d2e374590e49a4fa2edbe127549c0 cc=e7edf1845cd1cf43e4b66648512839c5
run_build "$OUTDIR/wow_8.1.5.29869_e7edf184.txt" "$BFT" wow e35d2e374590e49a4fa2edbe127549c0 e7edf1845cd1cf43e4b66648512839c5 "$CDN" "$CDN_PATH" --paths
# wow 8.1.5.29896 bc=dc189818e35427b78b18ec9b1ec1d14e cc=fa9fffcb878bfe3b5be69fad24e19a6a
run_build "$OUTDIR/wow_8.1.5.29896_fa9fffcb.txt" "$BFT" wow dc189818e35427b78b18ec9b1ec1d14e fa9fffcb878bfe3b5be69fad24e19a6a "$CDN" "$CDN_PATH" --paths
# wow 8.1.5.29981 bc=82ecbc808eebb35dd30cb230caabe917 cc=6517dcef8b0fa6d6cf6456bac3b73569
run_build "$OUTDIR/wow_8.1.5.29981_6517dcef.txt" "$BFT" wow 82ecbc808eebb35dd30cb230caabe917 6517dcef8b0fa6d6cf6456bac3b73569 "$CDN" "$CDN_PATH" --paths
# wow 8.1.5.30477 bc=2f4731b2dca36dd7c0455dc2c6eb52ed cc=091c528624683d9110893869e7004c2a
run_build "$OUTDIR/wow_8.1.5.30477_091c5286.txt" "$BFT" wow 2f4731b2dca36dd7c0455dc2c6eb52ed 091c528624683d9110893869e7004c2a "$CDN" "$CDN_PATH" --paths
# wow 8.1.5.30706 bc=d3192b18729bb6f00c595fd37097c19f cc=d4bd153ea2b84bcdd3188e9e3c1bf4e3
run_build "$OUTDIR/wow_8.1.5.30706_d4bd153e.txt" "$BFT" wow d3192b18729bb6f00c595fd37097c19f d4bd153ea2b84bcdd3188e9e3c1bf4e3 "$CDN" "$CDN_PATH" --paths
# wow 8.2.0.30827 bc=ac48361e51271f8c26b00de87d3fdc3e cc=3d014cd9e5940b029109685aee932149
run_build "$OUTDIR/wow_8.2.0.30827_3d014cd9.txt" "$BFT" wow ac48361e51271f8c26b00de87d3fdc3e 3d014cd9e5940b029109685aee932149 "$CDN" "$CDN_PATH" --paths
# wow 8.2.0.30898 bc=5dd4de607c23f0ee4d44cf409e3831a6 cc=4f1ea6bebc45326f78a6207478b099ca
run_build "$OUTDIR/wow_8.2.0.30898_4f1ea6be.txt" "$BFT" wow 5dd4de607c23f0ee4d44cf409e3831a6 4f1ea6bebc45326f78a6207478b099ca "$CDN" "$CDN_PATH" --paths
# wow 8.2.0.30918 bc=dd063c6d8a364ddd194b0e7483361738 cc=6af2fc9eaddc17642ccea77123a95eb7
run_build "$OUTDIR/wow_8.2.0.30918_6af2fc9e.txt" "$BFT" wow dd063c6d8a364ddd194b0e7483361738 6af2fc9eaddc17642ccea77123a95eb7 "$CDN" "$CDN_PATH" --paths
# wow 8.2.0.30920 bc=afc3900f74c007fb082427908ae345f9 cc=d8bae9543e39b3a9ee0961c8517151cc
run_build "$OUTDIR/wow_8.2.0.30920_d8bae954.txt" "$BFT" wow afc3900f74c007fb082427908ae345f9 d8bae9543e39b3a9ee0961c8517151cc "$CDN" "$CDN_PATH" --paths
# wow 8.2.0.30948 bc=be72420aa1d5c68accd19ac484b2f129 cc=54b6f3078ff50e55aaac5538e1f941ad
run_build "$OUTDIR/wow_8.2.0.30948_54b6f307.txt" "$BFT" wow be72420aa1d5c68accd19ac484b2f129 54b6f3078ff50e55aaac5538e1f941ad "$CDN" "$CDN_PATH" --paths
# wow 8.2.0.30993 bc=285a4da90592e8783d2ae1730008ade5 cc=08f82baac130ecb370006978ff29a7f9
run_build "$OUTDIR/wow_8.2.0.30993_08f82baa.txt" "$BFT" wow 285a4da90592e8783d2ae1730008ade5 08f82baac130ecb370006978ff29a7f9 "$CDN" "$CDN_PATH" --paths
# wow 8.2.0.31229 bc=224ae969295d1c7fe3aef736f904467e cc=2f0865df925face2bca90703fd344b84
run_build "$OUTDIR/wow_8.2.0.31229_2f0865df.txt" "$BFT" wow 224ae969295d1c7fe3aef736f904467e 2f0865df925face2bca90703fd344b84 "$CDN" "$CDN_PATH" --paths
# wow 8.2.0.31429 bc=64ac3abe0f7af37f350279683e0acf02 cc=f5c1234c3fb47bac6fa7c6d76468ff7c
run_build "$OUTDIR/wow_8.2.0.31429_f5c1234c.txt" "$BFT" wow 64ac3abe0f7af37f350279683e0acf02 f5c1234c3fb47bac6fa7c6d76468ff7c "$CDN" "$CDN_PATH" --paths
# wow 8.2.0.31478 bc=dbf21656d4e6691daee053017923d3fb cc=d83622e109ad40bf23146dc23a179d24
run_build "$OUTDIR/wow_8.2.0.31478_d83622e1.txt" "$BFT" wow dbf21656d4e6691daee053017923d3fb d83622e109ad40bf23146dc23a179d24 "$CDN" "$CDN_PATH" --paths
# wow 8.2.5.31884 bc=475843993f5752b0477c1516470df00b cc=986c09eeef66e18d275b997c2746b5b2
run_build "$OUTDIR/wow_8.2.5.31884_986c09ee.txt" "$BFT" wow 475843993f5752b0477c1516470df00b 986c09eeef66e18d275b997c2746b5b2 "$CDN" "$CDN_PATH" --paths
# wow 8.2.5.31921 bc=4e6a514bebf77a3d84e902c516e485fb cc=b87dc4b41225031d4f9986781ede263a
run_build "$OUTDIR/wow_8.2.5.31921_b87dc4b4.txt" "$BFT" wow 4e6a514bebf77a3d84e902c516e485fb b87dc4b41225031d4f9986781ede263a "$CDN" "$CDN_PATH" --paths
# wow 8.2.5.31958 bc=2ec07b6222175127f47b11daa8edda7f cc=78aceeb0c40266a6cf1129c4f632ce0b
run_build "$OUTDIR/wow_8.2.5.31958_78aceeb0.txt" "$BFT" wow 2ec07b6222175127f47b11daa8edda7f 78aceeb0c40266a6cf1129c4f632ce0b "$CDN" "$CDN_PATH" --paths
# wow 8.2.5.31960 bc=d3d228aaa42d97b8781d06085ee2263d cc=c4ce06090f704b39888322009b3b58e7
run_build "$OUTDIR/wow_8.2.5.31960_c4ce0609.txt" "$BFT" wow d3d228aaa42d97b8781d06085ee2263d c4ce06090f704b39888322009b3b58e7 "$CDN" "$CDN_PATH" --paths
# wow 8.2.5.31961 bc=eef87df8db000775795496904cb1fc87 cc=e51259759a16043816f31ac50fd07b9a
run_build "$OUTDIR/wow_8.2.5.31961_e5125975.txt" "$BFT" wow eef87df8db000775795496904cb1fc87 e51259759a16043816f31ac50fd07b9a "$CDN" "$CDN_PATH" --paths
# wow 8.2.5.31984 bc=ebd292a06d6a87b7d276ba3bf752a82d cc=9a5ae0928328d24e18546189584dceb8
run_build "$OUTDIR/wow_8.2.5.31984_9a5ae092.txt" "$BFT" wow ebd292a06d6a87b7d276ba3bf752a82d 9a5ae0928328d24e18546189584dceb8 "$CDN" "$CDN_PATH" --paths
# wow 8.2.5.32028 bc=00ca6f8311fbfe40240f3b9d1c4905ad cc=3bacac180343eb5d07eb43cdbcf4c944
run_build "$OUTDIR/wow_8.2.5.32028_3bacac18.txt" "$BFT" wow 00ca6f8311fbfe40240f3b9d1c4905ad 3bacac180343eb5d07eb43cdbcf4c944 "$CDN" "$CDN_PATH" --paths
# wow 8.2.5.32144 bc=875944f051d9bde55103ef66445d6874 cc=1b0967ddb13817e25acb2fa1fe3a4f09
run_build "$OUTDIR/wow_8.2.5.32144_1b0967dd.txt" "$BFT" wow 875944f051d9bde55103ef66445d6874 1b0967ddb13817e25acb2fa1fe3a4f09 "$CDN" "$CDN_PATH" --paths
# wow 8.2.5.32185 bc=89057934ea457e908ca5dc2e8c1dbb72 cc=facf07e03852465b53cd02cf2090df65
run_build "$OUTDIR/wow_8.2.5.32185_facf07e0.txt" "$BFT" wow 89057934ea457e908ca5dc2e8c1dbb72 facf07e03852465b53cd02cf2090df65 "$CDN" "$CDN_PATH" --paths
# wow 8.2.5.32265 bc=dc235cb00c0a67d4621e1afff89f3677 cc=3e9069e70f6d49792013fe5058663a8a
run_build "$OUTDIR/wow_8.2.5.32265_3e9069e7.txt" "$BFT" wow dc235cb00c0a67d4621e1afff89f3677 3e9069e70f6d49792013fe5058663a8a "$CDN" "$CDN_PATH" --paths
# wow 8.2.5.32294 bc=a9cfa3c369bc5a353cdf3fe5ee02b527 cc=3aeb2006732047b3a75740d8e6f07f45
run_build "$OUTDIR/wow_8.2.5.32294_3aeb2006.txt" "$BFT" wow a9cfa3c369bc5a353cdf3fe5ee02b527 3aeb2006732047b3a75740d8e6f07f45 "$CDN" "$CDN_PATH" --paths
# wow 8.2.5.32305 bc=49299eae4e3a195953764bb4adb3c91f cc=706873facbe8f450914bb60a2f1b48bf
run_build "$OUTDIR/wow_8.2.5.32305_706873fa.txt" "$BFT" wow 49299eae4e3a195953764bb4adb3c91f 706873facbe8f450914bb60a2f1b48bf "$CDN" "$CDN_PATH" --paths
# wow 8.2.5.32494 bc=24df27eb6a47daf8b6154c0610c2ac08 cc=63ee5c89b43add43795b6149f1a6d2e0
run_build "$OUTDIR/wow_8.2.5.32494_63ee5c89.txt" "$BFT" wow 24df27eb6a47daf8b6154c0610c2ac08 63ee5c89b43add43795b6149f1a6d2e0 "$CDN" "$CDN_PATH" --paths
# wow 8.2.5.32580 bc=c9ed758b00c35c2594a0f8b8093e2cfb cc=20daef134f4e8cc06976127e2ed45279
run_build "$OUTDIR/wow_8.2.5.32580_20daef13.txt" "$BFT" wow c9ed758b00c35c2594a0f8b8093e2cfb 20daef134f4e8cc06976127e2ed45279 "$CDN" "$CDN_PATH" --paths
# wow 8.2.5.32638 bc=e4afe4b956bb4ed38af33753830783e2 cc=6a2d246b9ab4459dc8052a2b3b0a60c0
run_build "$OUTDIR/wow_8.2.5.32638_6a2d246b.txt" "$BFT" wow e4afe4b956bb4ed38af33753830783e2 6a2d246b9ab4459dc8052a2b3b0a60c0 "$CDN" "$CDN_PATH" --paths
# wow 8.2.5.32722 bc=5a709aa1aa55595f2bd00cc80d38c94d cc=679634c87fb3afda886b04cf003f80d9
run_build "$OUTDIR/wow_8.2.5.32722_679634c8.txt" "$BFT" wow 5a709aa1aa55595f2bd00cc80d38c94d 679634c87fb3afda886b04cf003f80d9 "$CDN" "$CDN_PATH" --paths
# wow 8.2.5.32750 bc=b1bb4de2fb624c4c3fb323f5ed1d0824 cc=efc95c64488ab6dda10a7f57eca91f19
run_build "$OUTDIR/wow_8.2.5.32750_efc95c64.txt" "$BFT" wow b1bb4de2fb624c4c3fb323f5ed1d0824 efc95c64488ab6dda10a7f57eca91f19 "$CDN" "$CDN_PATH" --paths
# wow 8.2.5.32978 bc=624d0d23aac71d2fac9e0bf04a3e7f97 cc=28a7210c08f1944dc9d582bb836527c2
run_build "$OUTDIR/wow_8.2.5.32978_28a7210c.txt" "$BFT" wow 624d0d23aac71d2fac9e0bf04a3e7f97 28a7210c08f1944dc9d582bb836527c2 "$CDN" "$CDN_PATH" --paths
# wow 8.3.0.33051 bc=35ceef54762dcd9cf38c69803097bd1d cc=2e1eb158cd91e8e7baddb72463cbc943
run_build "$OUTDIR/wow_8.3.0.33051_2e1eb158.txt" "$BFT" wow 35ceef54762dcd9cf38c69803097bd1d 2e1eb158cd91e8e7baddb72463cbc943 "$CDN" "$CDN_PATH" --paths
# wow 8.3.0.33062 bc=9ecaf7ff897ef0e9a2db9ef08860db72 cc=b51fdb5fb341de2b035e96a7ec158cab
run_build "$OUTDIR/wow_8.3.0.33062_b51fdb5f.txt" "$BFT" wow 9ecaf7ff897ef0e9a2db9ef08860db72 b51fdb5fb341de2b035e96a7ec158cab "$CDN" "$CDN_PATH" --paths
# wow 8.3.0.33073 bc=31903113f4aaeebb88f5940fa1cf49bd cc=46d1fc75cd7be6a3f7b3302f6b7b12de
run_build "$OUTDIR/wow_8.3.0.33073_46d1fc75.txt" "$BFT" wow 31903113f4aaeebb88f5940fa1cf49bd 46d1fc75cd7be6a3f7b3302f6b7b12de "$CDN" "$CDN_PATH" --paths
# wow 8.3.0.33084 bc=be2812b7a0a3e57620ba83862236aeba cc=609dfc88451c5d11ca1b81f62f69353d
run_build "$OUTDIR/wow_8.3.0.33084_609dfc88.txt" "$BFT" wow be2812b7a0a3e57620ba83862236aeba 609dfc88451c5d11ca1b81f62f69353d "$CDN" "$CDN_PATH" --paths
# wow 8.3.0.33095 bc=7bd457f442b82f52603e11ce59c96869 cc=a7b56749ff261be74f9bc95128dcc44b
run_build "$OUTDIR/wow_8.3.0.33095_a7b56749.txt" "$BFT" wow 7bd457f442b82f52603e11ce59c96869 a7b56749ff261be74f9bc95128dcc44b "$CDN" "$CDN_PATH" --paths
# wow 8.3.0.33115 bc=9c530cb62632e3257d6063408b47b206 cc=460cf56b2933db6d8ca51d820c01cc45
run_build "$OUTDIR/wow_8.3.0.33115_460cf56b.txt" "$BFT" wow 9c530cb62632e3257d6063408b47b206 460cf56b2933db6d8ca51d820c01cc45 "$CDN" "$CDN_PATH" --paths
# wow 8.3.0.33169 bc=de1a99894a9668274fbba5422e4cc189 cc=95985f6317a93c0448232d664d117878
run_build "$OUTDIR/wow_8.3.0.33169_95985f63.txt" "$BFT" wow de1a99894a9668274fbba5422e4cc189 95985f6317a93c0448232d664d117878 "$CDN" "$CDN_PATH" --paths
# wow 8.3.0.33237 bc=58f902be3e36d90b1f6f93c17eb4de08 cc=4254e58562cd944caa49d0c93a241437
run_build "$OUTDIR/wow_8.3.0.33237_4254e585.txt" "$BFT" wow 58f902be3e36d90b1f6f93c17eb4de08 4254e58562cd944caa49d0c93a241437 "$CDN" "$CDN_PATH" --paths
# wow 8.3.0.33369 bc=3a2cb80017792235b94dc6e43424fbc8 cc=5f086f2db5fb2aa59d00206e85223e1a
run_build "$OUTDIR/wow_8.3.0.33369_5f086f2d.txt" "$BFT" wow 3a2cb80017792235b94dc6e43424fbc8 5f086f2db5fb2aa59d00206e85223e1a "$CDN" "$CDN_PATH" --paths
# wow 8.3.0.33528 bc=5cb1d8c1c43c8b03fa5376d3717d39b1 cc=6c6a4f5dd6636a8fb5942b52279e2bdb
run_build "$OUTDIR/wow_8.3.0.33528_6c6a4f5d.txt" "$BFT" wow 5cb1d8c1c43c8b03fa5376d3717d39b1 6c6a4f5dd6636a8fb5942b52279e2bdb "$CDN" "$CDN_PATH" --paths
# wow 8.3.0.33724 bc=929f36a8de620da440bcbb1c9aca452d cc=b35fb2521d53b547e450c9635b98f60d
run_build "$OUTDIR/wow_8.3.0.33724_b35fb252.txt" "$BFT" wow 929f36a8de620da440bcbb1c9aca452d b35fb2521d53b547e450c9635b98f60d "$CDN" "$CDN_PATH" --paths
# wow 8.3.0.33775 bc=34a5e46c0f36710f4e3babd7e76ca54e cc=208fcf8845d5a4b2431ef7bfa37733c7
run_build "$OUTDIR/wow_8.3.0.33775_208fcf88.txt" "$BFT" wow 34a5e46c0f36710f4e3babd7e76ca54e 208fcf8845d5a4b2431ef7bfa37733c7 "$CDN" "$CDN_PATH" --paths
# wow 8.3.0.33941 bc=1df68ff795cf37aea46d468854ebfde4 cc=e3ee09e8f58f57584b124a3bea61374f
run_build "$OUTDIR/wow_8.3.0.33941_e3ee09e8.txt" "$BFT" wow 1df68ff795cf37aea46d468854ebfde4 e3ee09e8f58f57584b124a3bea61374f "$CDN" "$CDN_PATH" --paths
# wow 8.3.0.34220 bc=95dae93e61edc6d2c5f02415400c4675 cc=5187cdfd6fee12b4a0d53003e8249635
run_build "$OUTDIR/wow_8.3.0.34220_5187cdfd.txt" "$BFT" wow 95dae93e61edc6d2c5f02415400c4675 5187cdfd6fee12b4a0d53003e8249635 "$CDN" "$CDN_PATH" --paths
# wow 8.3.0.34601 bc=c9b894215532452535a2fb0130e1869a cc=a9cff2e39633dbfe1885d3aa6c2805c5
run_build "$OUTDIR/wow_8.3.0.34601_a9cff2e3.txt" "$BFT" wow c9b894215532452535a2fb0130e1869a a9cff2e39633dbfe1885d3aa6c2805c5 "$CDN" "$CDN_PATH" --paths
# wow 8.3.0.34769 bc=578305b3540a7544c8a9f96f05b8ee2e cc=0e1fea84f8e29102852b3cfcd8c84047
run_build "$OUTDIR/wow_8.3.0.34769_0e1fea84.txt" "$BFT" wow 578305b3540a7544c8a9f96f05b8ee2e 0e1fea84f8e29102852b3cfcd8c84047 "$CDN" "$CDN_PATH" --paths
# wow 8.3.0.34963 bc=8daa95ebfcd50c1b5792428ff8ff7667 cc=8c3b2d29f6746551e930930d1359531b
run_build "$OUTDIR/wow_8.3.0.34963_8c3b2d29.txt" "$BFT" wow 8daa95ebfcd50c1b5792428ff8ff7667 8c3b2d29f6746551e930930d1359531b "$CDN" "$CDN_PATH" --paths
# wow 8.3.7.35249 bc=75ebb6141906135c30ae14cb18b2eb3c cc=2ae3aa84c542179deac484fc920cf97e
run_build "$OUTDIR/wow_8.3.7.35249_2ae3aa84.txt" "$BFT" wow 75ebb6141906135c30ae14cb18b2eb3c 2ae3aa84c542179deac484fc920cf97e "$CDN" "$CDN_PATH" --paths
# wow 8.3.7.35284 bc=7153081bed5f2ec0fbbc0893c3fb1252 cc=5b01395839bcc65e5b0e328f8cc1c687
run_build "$OUTDIR/wow_8.3.7.35284_5b013958.txt" "$BFT" wow 7153081bed5f2ec0fbbc0893c3fb1252 5b01395839bcc65e5b0e328f8cc1c687 "$CDN" "$CDN_PATH" --paths
# wow 8.3.7.35435 bc=202649bd70de66ee4c7af699ecf1830f cc=05291e8d0afb8515f0675bd4803077fe
run_build "$OUTDIR/wow_8.3.7.35435_05291e8d.txt" "$BFT" wow 202649bd70de66ee4c7af699ecf1830f 05291e8d0afb8515f0675bd4803077fe "$CDN" "$CDN_PATH" --paths
# wow 8.3.7.35662 bc=1328495d77b6e431601bfd916c1a1ab8 cc=675b08548f5f33339ea13d9aa5e0c84d
run_build "$OUTDIR/wow_8.3.7.35662_675b0854.txt" "$BFT" wow 1328495d77b6e431601bfd916c1a1ab8 675b08548f5f33339ea13d9aa5e0c84d "$CDN" "$CDN_PATH" --paths

# wow 9.0.1.35917 bc=f331a1d7c6ba0cd692843c5cbb0a7e25 cc=e2a4a79453651733468c5ba3591f154c
run_build "$OUTDIR/wow_9.0.1.35917_e2a4a794.txt" "$BFT" wow f331a1d7c6ba0cd692843c5cbb0a7e25 e2a4a79453651733468c5ba3591f154c "$CDN" "$CDN_PATH" --paths
# wow 9.0.1.36216 bc=c62a890663bccfd7d66aae6523634a66 cc=8a7a8a3ec758b9ff1263478bf184bbe9
run_build "$OUTDIR/wow_9.0.1.36216_8a7a8a3e.txt" "$BFT" wow c62a890663bccfd7d66aae6523634a66 8a7a8a3ec758b9ff1263478bf184bbe9 "$CDN" "$CDN_PATH" --paths
# wow 9.0.1.36228 bc=71f714d400bab61c87414f7cbe67e701 cc=6af788e2709ec5c1d69ef6d54f96a16e
run_build "$OUTDIR/wow_9.0.1.36228_6af788e2.txt" "$BFT" wow 71f714d400bab61c87414f7cbe67e701 6af788e2709ec5c1d69ef6d54f96a16e "$CDN" "$CDN_PATH" --paths
# wow 9.0.1.36230 bc=4da3c186b7fcd909a14453a3ffc677fe cc=e6841b25b771c26395ab98d27b63b9c6
run_build "$OUTDIR/wow_9.0.1.36230_e6841b25.txt" "$BFT" wow 4da3c186b7fcd909a14453a3ffc677fe e6841b25b771c26395ab98d27b63b9c6 "$CDN" "$CDN_PATH" --paths
# wow 9.0.1.36247 bc=45b5d9046bea1a88786947201c0f593b cc=a7fb467668bec16fa05f39e80826834a
run_build "$OUTDIR/wow_9.0.1.36247_a7fb4676.txt" "$BFT" wow 45b5d9046bea1a88786947201c0f593b a7fb467668bec16fa05f39e80826834a "$CDN" "$CDN_PATH" --paths
# wow 9.0.1.36272 bc=35c9785aa81cfc894c01b771814c7768 cc=c571d3a560a8b923e5dd2378b32bc3b2
run_build "$OUTDIR/wow_9.0.1.36272_c571d3a5.txt" "$BFT" wow 35c9785aa81cfc894c01b771814c7768 c571d3a560a8b923e5dd2378b32bc3b2 "$CDN" "$CDN_PATH" --paths
# wow 9.0.1.36322 bc=d45684591c5088f8cf8ccb109be7aba9 cc=ed0958af48445d2ce4b61bbf86f85388
run_build "$OUTDIR/wow_9.0.1.36322_ed0958af.txt" "$BFT" wow d45684591c5088f8cf8ccb109be7aba9 ed0958af48445d2ce4b61bbf86f85388 "$CDN" "$CDN_PATH" --paths
# wow 9.0.1.36372 bc=005c821975bb8e5bd127c92ad464b168 cc=81807d5e72a45dfeebaaebe81dcfd35b
run_build "$OUTDIR/wow_9.0.1.36372_81807d5e.txt" "$BFT" wow 005c821975bb8e5bd127c92ad464b168 81807d5e72a45dfeebaaebe81dcfd35b "$CDN" "$CDN_PATH" --paths
# wow 9.0.1.36492 bc=182e7cb818234be6120464bc7cf1f112 cc=52b2abb4c232bd7af0f4f74eb6d27887
run_build "$OUTDIR/wow_9.0.1.36492_52b2abb4.txt" "$BFT" wow 182e7cb818234be6120464bc7cf1f112 52b2abb4c232bd7af0f4f74eb6d27887 "$CDN" "$CDN_PATH" --paths
# wow 9.0.1.36577 bc=4abea570b3f8599544883de096e84197 cc=cc0c5f916e4b59d76b58c3075ff4fecb
run_build "$OUTDIR/wow_9.0.1.36577_cc0c5f91.txt" "$BFT" wow 4abea570b3f8599544883de096e84197 cc0c5f916e4b59d76b58c3075ff4fecb "$CDN" "$CDN_PATH" --paths
# wow 9.0.2.36639 bc=7c0a4ea85a4b6ada632793dbb0628d6e cc=477a930b69bb93a93e2c2e148ad9880b
run_build "$OUTDIR/wow_9.0.2.36639_477a930b.txt" "$BFT" wow 7c0a4ea85a4b6ada632793dbb0628d6e 477a930b69bb93a93e2c2e148ad9880b "$CDN" "$CDN_PATH" --paths
# wow 9.0.2.36665 bc=37d420249d3dd1af0dc66a161ad4c5ce cc=c9eb01f3d3b7ed3cfe00acc8c5c92463
run_build "$OUTDIR/wow_9.0.2.36665_c9eb01f3.txt" "$BFT" wow 37d420249d3dd1af0dc66a161ad4c5ce c9eb01f3d3b7ed3cfe00acc8c5c92463 "$CDN" "$CDN_PATH" --paths
# wow 9.0.2.36671 bc=09d8aefad48ca4a72d20d59c1f76003a cc=015c49025cd9dc486d6d041e96960ab2
run_build "$OUTDIR/wow_9.0.2.36671_015c4902.txt" "$BFT" wow 09d8aefad48ca4a72d20d59c1f76003a 015c49025cd9dc486d6d041e96960ab2 "$CDN" "$CDN_PATH" --paths
# wow 9.0.2.36710 bc=c210afb548925eef660107b82d94d529 cc=552699cb7fc94572870e18996ac21d23
run_build "$OUTDIR/wow_9.0.2.36710_552699cb.txt" "$BFT" wow c210afb548925eef660107b82d94d529 552699cb7fc94572870e18996ac21d23 "$CDN" "$CDN_PATH" --paths
# wow 9.0.2.36734 bc=a99053c01db2ceb2cd2ffb792e2ed278 cc=5ba612a5d283db01cd858fc1d1bf2613
run_build "$OUTDIR/wow_9.0.2.36734_5ba612a5.txt" "$BFT" wow a99053c01db2ceb2cd2ffb792e2ed278 5ba612a5d283db01cd858fc1d1bf2613 "$CDN" "$CDN_PATH" --paths
# wow 9.0.2.36751 bc=8cd7d00dac95bd91713fb01be94a6e41 cc=9df534b702dce9ee9a257b4c0e0f04a5
run_build "$OUTDIR/wow_9.0.2.36751_9df534b7.txt" "$BFT" wow 8cd7d00dac95bd91713fb01be94a6e41 9df534b702dce9ee9a257b4c0e0f04a5 "$CDN" "$CDN_PATH" --paths
# wow 9.0.2.36753 bc=63ddc2a35bbaed2cdef476fa8ce97051 cc=9a7e80e6caf140c518b55f846165a3a0
run_build "$OUTDIR/wow_9.0.2.36753_9a7e80e6.txt" "$BFT" wow 63ddc2a35bbaed2cdef476fa8ce97051 9a7e80e6caf140c518b55f846165a3a0 "$CDN" "$CDN_PATH" --paths
# wow 9.0.2.36839 bc=7869e3460f47a6ad705b2cf4be56b81a cc=1fbdbeba125de258322dc285a00b15dd
run_build "$OUTDIR/wow_9.0.2.36839_1fbdbeba.txt" "$BFT" wow 7869e3460f47a6ad705b2cf4be56b81a 1fbdbeba125de258322dc285a00b15dd "$CDN" "$CDN_PATH" --paths
# wow 9.0.2.36949 bc=26b9d3a8ae53bff24ad2eb922ae13561 cc=b911667b69d62aa1e9928e21483ed5e6
run_build "$OUTDIR/wow_9.0.2.36949_b911667b.txt" "$BFT" wow 26b9d3a8ae53bff24ad2eb922ae13561 b911667b69d62aa1e9928e21483ed5e6 "$CDN" "$CDN_PATH" --paths
# wow 9.0.2.37106 bc=26291f284f42494375d511d1fc120216 cc=29f1a801eab72312f681fae3e4b39371
run_build "$OUTDIR/wow_9.0.2.37106_29f1a801.txt" "$BFT" wow 26291f284f42494375d511d1fc120216 29f1a801eab72312f681fae3e4b39371 "$CDN" "$CDN_PATH" --paths
# wow 9.0.2.37142 bc=35cb23772396d75a7edc6bed51e7a34e cc=3ecf17456c83f507946cdce17c3e81c6
run_build "$OUTDIR/wow_9.0.2.37142_3ecf1745.txt" "$BFT" wow 35cb23772396d75a7edc6bed51e7a34e 3ecf17456c83f507946cdce17c3e81c6 "$CDN" "$CDN_PATH" --paths
# wow 9.0.2.37176 bc=c8915151a362f626777a2ff5ccefd4b6 cc=8fcd4040379cd4549bd61c83dc8ce1b4
run_build "$OUTDIR/wow_9.0.2.37176_8fcd4040.txt" "$BFT" wow c8915151a362f626777a2ff5ccefd4b6 8fcd4040379cd4549bd61c83dc8ce1b4 "$CDN" "$CDN_PATH" --paths
# wow 9.0.2.37415 bc=6d2a283098c3bdacc9ac93a4176c3093 cc=e6d1fb70f8481a7cbccecfd177488299
run_build "$OUTDIR/wow_9.0.2.37415_e6d1fb70.txt" "$BFT" wow 6d2a283098c3bdacc9ac93a4176c3093 e6d1fb70f8481a7cbccecfd177488299 "$CDN" "$CDN_PATH" --paths
# wow 9.0.2.37474 bc=cbd02f4dc3aa31d72fc89307242d51b5 cc=81286acb9b1dcb100ee57cd37831635d
run_build "$OUTDIR/wow_9.0.2.37474_81286acb.txt" "$BFT" wow cbd02f4dc3aa31d72fc89307242d51b5 81286acb9b1dcb100ee57cd37831635d "$CDN" "$CDN_PATH" --paths
# wow 9.0.5.37862 bc=187b6d796398516f6a1caee810e7fc11 cc=492caba0d6eb809081a16d2d99377bda
run_build "$OUTDIR/wow_9.0.5.37862_492caba0.txt" "$BFT" wow 187b6d796398516f6a1caee810e7fc11 492caba0d6eb809081a16d2d99377bda "$CDN" "$CDN_PATH" --paths
# wow 9.0.5.37864 bc=2f78adbca3f3a0453667aacdd62ab4a3 cc=492caba0d6eb809081a16d2d99377bda
run_build "$OUTDIR/wow_9.0.5.37864_492caba0.txt" "$BFT" wow 2f78adbca3f3a0453667aacdd62ab4a3 492caba0d6eb809081a16d2d99377bda "$CDN" "$CDN_PATH" --paths
# wow 9.0.5.37893 bc=3031732a94f03a21cc2b5201ab79b231 cc=8ad7db4a29cc7c1a0460a8f8cb1ac12e
run_build "$OUTDIR/wow_9.0.5.37893_8ad7db4a.txt" "$BFT" wow 3031732a94f03a21cc2b5201ab79b231 8ad7db4a29cc7c1a0460a8f8cb1ac12e "$CDN" "$CDN_PATH" --paths
# wow 9.0.5.37899 bc=33ed97ec5432f6b5d1df42178b5e79d2 cc=dea316914410013690c63b9c135a7829
run_build "$OUTDIR/wow_9.0.5.37899_dea31691.txt" "$BFT" wow 33ed97ec5432f6b5d1df42178b5e79d2 dea316914410013690c63b9c135a7829 "$CDN" "$CDN_PATH" --paths
# wow 9.0.5.37988 bc=92303932aca95fc61ae3967b46f37bb9 cc=6b85ddba2d4a931503681772932ad1bd
run_build "$OUTDIR/wow_9.0.5.37988_6b85ddba.txt" "$BFT" wow 92303932aca95fc61ae3967b46f37bb9 6b85ddba2d4a931503681772932ad1bd "$CDN" "$CDN_PATH" --paths
# wow 9.0.5.38134 bc=6d51243da2b456520dc83cf217743807 cc=13427ee8353df86861a6aa3cddcb09e9
run_build "$OUTDIR/wow_9.0.5.38134_13427ee8.txt" "$BFT" wow 6d51243da2b456520dc83cf217743807 13427ee8353df86861a6aa3cddcb09e9 "$CDN" "$CDN_PATH" --paths
# wow 9.0.5.38556 bc=09565dcf5b6ab5fb1e126b9369afa79e cc=9cf97a7504d69ef3e25a843244d9efcd
run_build "$OUTDIR/wow_9.0.5.38556_9cf97a75.txt" "$BFT" wow 09565dcf5b6ab5fb1e126b9369afa79e 9cf97a7504d69ef3e25a843244d9efcd "$CDN" "$CDN_PATH" --paths
# wow 9.1.0.39121 bc=47962cb5453acd8c2972b5efa6631c81 cc=ec358efd3c0f4869e3b118abc55b9202
run_build "$OUTDIR/wow_9.1.0.39121_ec358efd.txt" "$BFT" wow 47962cb5453acd8c2972b5efa6631c81 ec358efd3c0f4869e3b118abc55b9202 "$CDN" "$CDN_PATH" --paths
# wow 9.1.0.39185 bc=de333bf447cda8810a72e8d0be599f5f cc=551db3c2c5799051b000944d720df81f
run_build "$OUTDIR/wow_9.1.0.39185_551db3c2.txt" "$BFT" wow de333bf447cda8810a72e8d0be599f5f 551db3c2c5799051b000944d720df81f "$CDN" "$CDN_PATH" --paths
# wow 9.1.0.39226 bc=f230fba70e8d33e8e243c1eae59ed88d cc=e5c78f60cd2171fbe572e790e8ab3d12
run_build "$OUTDIR/wow_9.1.0.39226_e5c78f60.txt" "$BFT" wow f230fba70e8d33e8e243c1eae59ed88d e5c78f60cd2171fbe572e790e8ab3d12 "$CDN" "$CDN_PATH" --paths
# wow 9.1.0.39229 bc=317ccb3cfe8af9667a519cb7cbfeeb1e cc=d2fbc6be0dc4adaaddfb427b5895913e
run_build "$OUTDIR/wow_9.1.0.39229_d2fbc6be.txt" "$BFT" wow 317ccb3cfe8af9667a519cb7cbfeeb1e d2fbc6be0dc4adaaddfb427b5895913e "$CDN" "$CDN_PATH" --paths
# wow 9.1.0.39262 bc=abfacc575ab884e16db9a65808bf2f51 cc=8ceacd859c0f39a52bf0cde041211d3d
run_build "$OUTDIR/wow_9.1.0.39262_8ceacd85.txt" "$BFT" wow abfacc575ab884e16db9a65808bf2f51 8ceacd859c0f39a52bf0cde041211d3d "$CDN" "$CDN_PATH" --paths
# wow 9.1.0.39282 bc=bb7ad005d38c7fab2940ed655eca7330 cc=7c79e4c141d69c71df1ac27bd61ef35e
run_build "$OUTDIR/wow_9.1.0.39282_7c79e4c1.txt" "$BFT" wow bb7ad005d38c7fab2940ed655eca7330 7c79e4c141d69c71df1ac27bd61ef35e "$CDN" "$CDN_PATH" --paths
# wow 9.1.0.39289 bc=f046942ab9d2aef1a8befea350c8c465 cc=4fed1ad2c499a7ad08183497234e47a1
run_build "$OUTDIR/wow_9.1.0.39289_4fed1ad2.txt" "$BFT" wow f046942ab9d2aef1a8befea350c8c465 4fed1ad2c499a7ad08183497234e47a1 "$CDN" "$CDN_PATH" --paths
# wow 9.1.0.39291 bc=fb8b31543a1de0199fc07b06048e3977 cc=4ac15cdf789f32620e61b70941efa04b
run_build "$OUTDIR/wow_9.1.0.39291_4ac15cdf.txt" "$BFT" wow fb8b31543a1de0199fc07b06048e3977 4ac15cdf789f32620e61b70941efa04b "$CDN" "$CDN_PATH" --paths
# wow 9.1.0.39318 bc=10f0435533a9de45c4c1009859638696 cc=ff496b8cb601d67c342d392ac5117133
run_build "$OUTDIR/wow_9.1.0.39318_ff496b8c.txt" "$BFT" wow 10f0435533a9de45c4c1009859638696 ff496b8cb601d67c342d392ac5117133 "$CDN" "$CDN_PATH" --paths
# wow 9.1.0.39335 bc=6e00acd78b86b9d272d35e79560f89c2 cc=c1894f974de24934ada6ed0cbb574233
run_build "$OUTDIR/wow_9.1.0.39335_c1894f97.txt" "$BFT" wow 6e00acd78b86b9d272d35e79560f89c2 c1894f974de24934ada6ed0cbb574233 "$CDN" "$CDN_PATH" --paths
# wow 9.1.0.39427 bc=1e8b6964cb424d23908c1609a7ed1a1d cc=be8f0cd5d6a9499b8e49f5da9d67f5d4
run_build "$OUTDIR/wow_9.1.0.39427_be8f0cd5.txt" "$BFT" wow 1e8b6964cb424d23908c1609a7ed1a1d be8f0cd5d6a9499b8e49f5da9d67f5d4 "$CDN" "$CDN_PATH" --paths
# wow 9.1.0.39497 bc=e725f7f3a41efa055fcc4d8696377f70 cc=cbeeb758ef691b30706a8791539a339a
run_build "$OUTDIR/wow_9.1.0.39497_cbeeb758.txt" "$BFT" wow e725f7f3a41efa055fcc4d8696377f70 cbeeb758ef691b30706a8791539a339a "$CDN" "$CDN_PATH" --paths
# wow 9.1.0.39498 bc=eb0aab3b882d5be05a63449c931c34c7 cc=4d4b9f14b1fe04f9f6ab8bf7a9979bd6
run_build "$OUTDIR/wow_9.1.0.39498_4d4b9f14.txt" "$BFT" wow eb0aab3b882d5be05a63449c931c34c7 4d4b9f14b1fe04f9f6ab8bf7a9979bd6 "$CDN" "$CDN_PATH" --paths
# wow 9.1.0.39584 bc=4e3cb793dc21d3bbe1bbbc4d35a84ac7 cc=d08c0081da40ebf960f4f1db4465bbaf
run_build "$OUTDIR/wow_9.1.0.39584_d08c0081.txt" "$BFT" wow 4e3cb793dc21d3bbe1bbbc4d35a84ac7 d08c0081da40ebf960f4f1db4465bbaf "$CDN" "$CDN_PATH" --paths
# wow 9.1.0.39617 bc=317f536aea903d05a2a0986b25861d08 cc=4b5491d4eef8590ea1b97ea3df4d1c82
run_build "$OUTDIR/wow_9.1.0.39617_4b5491d4.txt" "$BFT" wow 317f536aea903d05a2a0986b25861d08 4b5491d4eef8590ea1b97ea3df4d1c82 "$CDN" "$CDN_PATH" --paths
# wow 9.1.0.39653 bc=8b16414f86e1c65d90551bd4495ae6d3 cc=333851f5e72e2bf526e1af1ecc64b3fa
run_build "$OUTDIR/wow_9.1.0.39653_333851f5.txt" "$BFT" wow 8b16414f86e1c65d90551bd4495ae6d3 333851f5e72e2bf526e1af1ecc64b3fa "$CDN" "$CDN_PATH" --paths
# wow 9.1.0.39804 bc=5172b7b6b7a17875f5329fd1480fdc46 cc=9c85515e748c7941d772db6677bb759c
run_build "$OUTDIR/wow_9.1.0.39804_9c85515e.txt" "$BFT" wow 5172b7b6b7a17875f5329fd1480fdc46 9c85515e748c7941d772db6677bb759c "$CDN" "$CDN_PATH" --paths
# wow 9.1.0.40000 bc=cdb08c11631a525fab9c367d37ceb716 cc=3d7c398e6fe5ae76e209710917906de6
run_build "$OUTDIR/wow_9.1.0.40000_3d7c398e.txt" "$BFT" wow cdb08c11631a525fab9c367d37ceb716 3d7c398e6fe5ae76e209710917906de6 "$CDN" "$CDN_PATH" --paths
# wow 9.1.0.40120 bc=67bf2c620eb551d8653489ebf89b0db9 cc=ed66b5648d835148354a9e29fa0e7ba2
run_build "$OUTDIR/wow_9.1.0.40120_ed66b564.txt" "$BFT" wow 67bf2c620eb551d8653489ebf89b0db9 ed66b5648d835148354a9e29fa0e7ba2 "$CDN" "$CDN_PATH" --paths
# wow 9.1.0.40443 bc=6edf5ac52818048d4f336d0777640de9 cc=0dfefdf04bf02f87705d2076d7e27e4b
run_build "$OUTDIR/wow_9.1.0.40443_0dfefdf0.txt" "$BFT" wow 6edf5ac52818048d4f336d0777640de9 0dfefdf04bf02f87705d2076d7e27e4b "$CDN" "$CDN_PATH" --paths
# wow 9.1.0.40593 bc=704283a9f240461042abd7ac056208af cc=5857563ff3292e357698b8b3c6e8b7aa
run_build "$OUTDIR/wow_9.1.0.40593_5857563f.txt" "$BFT" wow 704283a9f240461042abd7ac056208af 5857563ff3292e357698b8b3c6e8b7aa "$CDN" "$CDN_PATH" --paths
# wow 9.1.0.40725 bc=7bda39158f8f3ee4d69f2e392b4e4bed cc=1f42437a58ff9fed8db9cf94455d49d4
run_build "$OUTDIR/wow_9.1.0.40725_1f42437a.txt" "$BFT" wow 7bda39158f8f3ee4d69f2e392b4e4bed 1f42437a58ff9fed8db9cf94455d49d4 "$CDN" "$CDN_PATH" --paths
# wow 9.1.5.40772 bc=f08345cf98e48d3b80f8ba9478f1111d cc=44f270eab8178fc828239043276aadd4
run_build "$OUTDIR/wow_9.1.5.40772_44f270ea.txt" "$BFT" wow f08345cf98e48d3b80f8ba9478f1111d 44f270eab8178fc828239043276aadd4 "$CDN" "$CDN_PATH" --paths
# wow 9.1.5.40871 bc=fabeafef91bcc2be5ce36f297fa21def cc=d7fca91e1a3a3fd15f0c92d667bf8106
run_build "$OUTDIR/wow_9.1.5.40871_d7fca91e.txt" "$BFT" wow fabeafef91bcc2be5ce36f297fa21def d7fca91e1a3a3fd15f0c92d667bf8106 "$CDN" "$CDN_PATH" --paths
# wow 9.1.5.40906 bc=378e324d4b10ca32f93e465f96808c91 cc=e980ae2a788385bf08e44b7cb6620d1f
run_build "$OUTDIR/wow_9.1.5.40906_e980ae2a.txt" "$BFT" wow 378e324d4b10ca32f93e465f96808c91 e980ae2a788385bf08e44b7cb6620d1f "$CDN" "$CDN_PATH" --paths
# wow 9.1.5.40944 bc=7f8fa6b96b24ece0474437fd41415092 cc=b8ccd7950400e0e23c8a8397b2c1075b
run_build "$OUTDIR/wow_9.1.5.40944_b8ccd795.txt" "$BFT" wow 7f8fa6b96b24ece0474437fd41415092 b8ccd7950400e0e23c8a8397b2c1075b "$CDN" "$CDN_PATH" --paths
# wow 9.1.5.40966 bc=780c312bebf3e93ba8d0f53bd42f2c49 cc=258023262c03de49ba2f2b8dc0c4c172
run_build "$OUTDIR/wow_9.1.5.40966_25802326.txt" "$BFT" wow 780c312bebf3e93ba8d0f53bd42f2c49 258023262c03de49ba2f2b8dc0c4c172 "$CDN" "$CDN_PATH" --paths
# wow 9.1.5.41031 bc=093fd834e82b21ee7b7e9654a4e429c8 cc=ed08ff95fe4130d591160b919f4320a3
run_build "$OUTDIR/wow_9.1.5.41031_ed08ff95.txt" "$BFT" wow 093fd834e82b21ee7b7e9654a4e429c8 ed08ff95fe4130d591160b919f4320a3 "$CDN" "$CDN_PATH" --paths
# wow 9.1.5.41079 bc=b94e0ec7dd9df19f671c09b8dd055b24 cc=92f8a283aaa73faa6463a3fdaa007d58
run_build "$OUTDIR/wow_9.1.5.41079_92f8a283.txt" "$BFT" wow b94e0ec7dd9df19f671c09b8dd055b24 92f8a283aaa73faa6463a3fdaa007d58 "$CDN" "$CDN_PATH" --paths
# wow 9.1.5.41288 bc=c43ab6d5904cede76c4e56f4ff4cd386 cc=8d995b430b4a36cfbf0766ac7f865e8f
run_build "$OUTDIR/wow_9.1.5.41288_8d995b43.txt" "$BFT" wow c43ab6d5904cede76c4e56f4ff4cd386 8d995b430b4a36cfbf0766ac7f865e8f "$CDN" "$CDN_PATH" --paths
# wow 9.1.5.41323 bc=9a74820c5bb5a77fc82d34855c664b32 cc=b445ccf9fc4dbe4d41b333c41e222827
run_build "$OUTDIR/wow_9.1.5.41323_b445ccf9.txt" "$BFT" wow 9a74820c5bb5a77fc82d34855c664b32 b445ccf9fc4dbe4d41b333c41e222827 "$CDN" "$CDN_PATH" --paths
# wow 9.1.5.41359 bc=5019c5dd07d133d2f8793d2130176c1b cc=884022a20efe7d3700288ccf3f14dca8
run_build "$OUTDIR/wow_9.1.5.41359_884022a2.txt" "$BFT" wow 5019c5dd07d133d2f8793d2130176c1b 884022a20efe7d3700288ccf3f14dca8 "$CDN" "$CDN_PATH" --paths
# wow 9.1.5.41488 bc=d546a9eb7ada55776c68f1877ccd1e79 cc=143d8b8210ef04c9de74a23af073de89
run_build "$OUTDIR/wow_9.1.5.41488_143d8b82.txt" "$BFT" wow d546a9eb7ada55776c68f1877ccd1e79 143d8b8210ef04c9de74a23af073de89 "$CDN" "$CDN_PATH" --paths
# wow 9.1.5.41793 bc=58afb41b46fd768ac6e394ea311c2935 cc=4c70b9597d0faaa309833bda8cf6315f
run_build "$OUTDIR/wow_9.1.5.41793_4c70b959.txt" "$BFT" wow 58afb41b46fd768ac6e394ea311c2935 4c70b9597d0faaa309833bda8cf6315f "$CDN" "$CDN_PATH" --paths
# wow 9.1.5.42010 bc=f34754af19831b97356c9b3542fc96e9 cc=7d9303440e7074c562f9544ce48009e7
run_build "$OUTDIR/wow_9.1.5.42010_7d930344.txt" "$BFT" wow f34754af19831b97356c9b3542fc96e9 7d9303440e7074c562f9544ce48009e7 "$CDN" "$CDN_PATH" --paths
# wow 9.2.0.42423 bc=ecb5b78a8e4882849a2ba790bd0c7dfe cc=c2c5c110b9e12800bf96a66b40911bbc
run_build "$OUTDIR/wow_9.2.0.42423_c2c5c110.txt" "$BFT" wow ecb5b78a8e4882849a2ba790bd0c7dfe c2c5c110b9e12800bf96a66b40911bbc "$CDN" "$CDN_PATH" --paths
# wow 9.2.0.42488 bc=1baeb8b59c558f759c6b0c9c4de2d656 cc=ab549269536c59ad18d47c7a37ce4040
run_build "$OUTDIR/wow_9.2.0.42488_ab549269.txt" "$BFT" wow 1baeb8b59c558f759c6b0c9c4de2d656 ab549269536c59ad18d47c7a37ce4040 "$CDN" "$CDN_PATH" --paths
# wow 9.2.0.42521 bc=ec756ae9da93f76aaaa3081e2347ce9e cc=df9dac22ef113f28e5f97a81a6c7beba
run_build "$OUTDIR/wow_9.2.0.42521_df9dac22.txt" "$BFT" wow ec756ae9da93f76aaaa3081e2347ce9e df9dac22ef113f28e5f97a81a6c7beba "$CDN" "$CDN_PATH" --paths
# wow 9.2.0.42538 bc=3d39f1055370cb7535a4343bc9adfc85 cc=d18a321ef3077ed1c8b595a8cbd56fab
run_build "$OUTDIR/wow_9.2.0.42538_d18a321e.txt" "$BFT" wow 3d39f1055370cb7535a4343bc9adfc85 d18a321ef3077ed1c8b595a8cbd56fab "$CDN" "$CDN_PATH" --paths
# wow 9.2.0.42560 bc=e1a10cdd4aea5d343561aa354b296ac0 cc=e0eb9ab4167e749d22dff05f8f1def61
run_build "$OUTDIR/wow_9.2.0.42560_e0eb9ab4.txt" "$BFT" wow e1a10cdd4aea5d343561aa354b296ac0 e0eb9ab4167e749d22dff05f8f1def61 "$CDN" "$CDN_PATH" --paths
# wow 9.2.0.42614 bc=86295f51d17e93c42911bed4071dc024 cc=a1c80c92bb8268134f2a482748f1dec4
run_build "$OUTDIR/wow_9.2.0.42614_a1c80c92.txt" "$BFT" wow 86295f51d17e93c42911bed4071dc024 a1c80c92bb8268134f2a482748f1dec4 "$CDN" "$CDN_PATH" --paths
# wow 9.2.0.42698 bc=20b96889ed43f7d251f06aec326eed44 cc=25910291e11e74651eeaeb15499d9ca5
run_build "$OUTDIR/wow_9.2.0.42698_25910291.txt" "$BFT" wow 20b96889ed43f7d251f06aec326eed44 25910291e11e74651eeaeb15499d9ca5 "$CDN" "$CDN_PATH" --paths
# wow 9.2.0.42825 bc=88b78016e09f952912ba60af866c1443 cc=4687aa0b13549bab7860f89bd8588300
run_build "$OUTDIR/wow_9.2.0.42825_4687aa0b.txt" "$BFT" wow 88b78016e09f952912ba60af866c1443 4687aa0b13549bab7860f89bd8588300 "$CDN" "$CDN_PATH" --paths
# wow 9.2.0.42852 bc=1fb2ec649688ecabc110d3d85cbd99e7 cc=4bcdbcb1ce7deb4904f18ae280ba6917
run_build "$OUTDIR/wow_9.2.0.42852_4bcdbcb1.txt" "$BFT" wow 1fb2ec649688ecabc110d3d85cbd99e7 4bcdbcb1ce7deb4904f18ae280ba6917 "$CDN" "$CDN_PATH" --paths
# wow 9.2.0.42937 bc=84ba46985ca9d2dd044c676ccd1c8746 cc=7bda6bfcd90a87aacd971f8abe184ec4
run_build "$OUTDIR/wow_9.2.0.42937_7bda6bfc.txt" "$BFT" wow 84ba46985ca9d2dd044c676ccd1c8746 7bda6bfcd90a87aacd971f8abe184ec4 "$CDN" "$CDN_PATH" --paths
# wow 9.2.0.42979 bc=73e2f635646079ffd8e5f3294503bea0 cc=c8859e7baf58af3bf9f7abb539df4c1e
run_build "$OUTDIR/wow_9.2.0.42979_c8859e7b.txt" "$BFT" wow 73e2f635646079ffd8e5f3294503bea0 c8859e7baf58af3bf9f7abb539df4c1e "$CDN" "$CDN_PATH" --paths
# wow 9.2.0.43114 bc=3b75696af65b63be2e863f64157bad8e cc=4ce300f63da16392db29f2eb527f31a2
run_build "$OUTDIR/wow_9.2.0.43114_4ce300f6.txt" "$BFT" wow 3b75696af65b63be2e863f64157bad8e 4ce300f63da16392db29f2eb527f31a2 "$CDN" "$CDN_PATH" --paths
# wow 9.2.0.43206 bc=a3e5fee50a9fd2d3f0284434459979f0 cc=f65eec596482613f84b80c4143e87bf5
run_build "$OUTDIR/wow_9.2.0.43206_f65eec59.txt" "$BFT" wow a3e5fee50a9fd2d3f0284434459979f0 f65eec596482613f84b80c4143e87bf5 "$CDN" "$CDN_PATH" --paths
# wow 9.2.0.43340 bc=5dec0ca7e32f6f3a4e098c0e6251dcc1 cc=cccdebac6fb27c2ff565c8701ffe8e67
run_build "$OUTDIR/wow_9.2.0.43340_cccdebac.txt" "$BFT" wow 5dec0ca7e32f6f3a4e098c0e6251dcc1 cccdebac6fb27c2ff565c8701ffe8e67 "$CDN" "$CDN_PATH" --paths
# wow 9.2.0.43345 bc=840f07ac37ef59bc097a077b257957ef cc=8717213ee8235fa299499e80eecc16c5
run_build "$OUTDIR/wow_9.2.0.43345_8717213e.txt" "$BFT" wow 840f07ac37ef59bc097a077b257957ef 8717213ee8235fa299499e80eecc16c5 "$CDN" "$CDN_PATH" --paths
# wow 9.2.5.43903 bc=0afcda313aac2f6e39f5ba2e53be8e37 cc=5ac20a4d5589ce3c0289ad85bdf7d8c7
run_build "$OUTDIR/wow_9.2.5.43903_5ac20a4d.txt" "$BFT" wow 0afcda313aac2f6e39f5ba2e53be8e37 5ac20a4d5589ce3c0289ad85bdf7d8c7 "$CDN" "$CDN_PATH" --paths
# wow 9.2.5.43971 bc=403ee02ca891c633bcbce294ddd61a55 cc=ed19ee5ca7173ccee27491f27585d8e1
run_build "$OUTDIR/wow_9.2.5.43971_ed19ee5c.txt" "$BFT" wow 403ee02ca891c633bcbce294ddd61a55 ed19ee5ca7173ccee27491f27585d8e1 "$CDN" "$CDN_PATH" --paths
# wow 9.2.5.44015 bc=b067f520d6698ae0e8bffecc030862d5 cc=80f82512832593e96f53e1748a74a376
run_build "$OUTDIR/wow_9.2.5.44015_80f82512.txt" "$BFT" wow b067f520d6698ae0e8bffecc030862d5 80f82512832593e96f53e1748a74a376 "$CDN" "$CDN_PATH" --paths
# wow 9.2.5.44061 bc=e99d24560a677810d0eb5e4ee035dc1a cc=749a058ffd6a351d216e0c7171f5487d
run_build "$OUTDIR/wow_9.2.5.44061_749a058f.txt" "$BFT" wow e99d24560a677810d0eb5e4ee035dc1a 749a058ffd6a351d216e0c7171f5487d "$CDN" "$CDN_PATH" --paths
# wow 9.2.5.44127 bc=39c173a69b7fa33d7e16aefe46cb3c71 cc=7fad9d116bfaff319e81f9aca0a71601
run_build "$OUTDIR/wow_9.2.5.44127_7fad9d11.txt" "$BFT" wow 39c173a69b7fa33d7e16aefe46cb3c71 7fad9d116bfaff319e81f9aca0a71601 "$CDN" "$CDN_PATH" --paths
# wow 9.2.5.44232 bc=d56389932dbca4b7cf2e10e93bbfa32f cc=44cd8ed708a04422b1db0031e6b57170
run_build "$OUTDIR/wow_9.2.5.44232_44cd8ed7.txt" "$BFT" wow d56389932dbca4b7cf2e10e93bbfa32f 44cd8ed708a04422b1db0031e6b57170 "$CDN" "$CDN_PATH" --paths
# wow 9.2.5.44325 bc=100f75df2ea2be37bc04a3a79ed2747f cc=405798eb65dbb717c37c276a8f6aec4f
run_build "$OUTDIR/wow_9.2.5.44325_405798eb.txt" "$BFT" wow 100f75df2ea2be37bc04a3a79ed2747f 405798eb65dbb717c37c276a8f6aec4f "$CDN" "$CDN_PATH" --paths
# wow 9.2.5.44730 bc=7b2d28612380b07f7b6eee48ddf16ca4 cc=629023178dbfa2e6e631f9d7b6851012
run_build "$OUTDIR/wow_9.2.5.44730_62902317.txt" "$BFT" wow 7b2d28612380b07f7b6eee48ddf16ca4 629023178dbfa2e6e631f9d7b6851012 "$CDN" "$CDN_PATH" --paths
# wow 9.2.5.44908 bc=b94534751f5adde3bd6840cde43b4197 cc=fc7a34244d828ae07b64075b89fea356
run_build "$OUTDIR/wow_9.2.5.44908_fc7a3424.txt" "$BFT" wow b94534751f5adde3bd6840cde43b4197 fc7a34244d828ae07b64075b89fea356 "$CDN" "$CDN_PATH" --paths
# wow 9.2.7.45114 bc=f2ce3b5bcd343f097d5f9cbea7dc9237 cc=9d7aff907ca57b2d2de96c3c74dd085f
run_build "$OUTDIR/wow_9.2.7.45114_9d7aff90.txt" "$BFT" wow f2ce3b5bcd343f097d5f9cbea7dc9237 9d7aff907ca57b2d2de96c3c74dd085f "$CDN" "$CDN_PATH" --paths
# wow 9.2.7.45161 bc=29408d05ef7d13d37cdc0e78c1af6fbe cc=0c6771232f825d0fafec682e0f400d5e
run_build "$OUTDIR/wow_9.2.7.45161_0c677123.txt" "$BFT" wow 29408d05ef7d13d37cdc0e78c1af6fbe 0c6771232f825d0fafec682e0f400d5e "$CDN" "$CDN_PATH" --paths
# wow 9.2.7.45338 bc=938f71f7d8254622725933a115aac2b5 cc=cfafa43d4629f513b70ad86369dd0958
run_build "$OUTDIR/wow_9.2.7.45338_cfafa43d.txt" "$BFT" wow 938f71f7d8254622725933a115aac2b5 cfafa43d4629f513b70ad86369dd0958 "$CDN" "$CDN_PATH" --paths
# wow 9.2.7.45745 bc=43b2762b8e4a57c4771a5cf9a1d99661 cc=8be9cf988078dd923677d222be5dfe38
run_build "$OUTDIR/wow_9.2.7.45745_8be9cf98.txt" "$BFT" wow 43b2762b8e4a57c4771a5cf9a1d99661 8be9cf988078dd923677d222be5dfe38 "$CDN" "$CDN_PATH" --paths
# wow 12.0.7.68887 bc=0fcf22030198cca211997a998743ba7f cc=5eeeb7a664e41a88349215461af353bb
run_build "$OUTDIR/wow_12.0.7.68887_0fcf2203.txt" "$BFT" wow 0fcf22030198cca211997a998743ba7f 5eeeb7a664e41a88349215461af353bb "$CDN" "$CDN_PATH" --paths
# wow 12.0.7.68974 bc=96db6554c1ba271b52390175d50589f3 cc=9a824cce21b48ebf0b11367ae32d1597
run_build "$OUTDIR/wow_12.0.7.68974_96db6554.txt" "$BFT" wow 96db6554c1ba271b52390175d50589f3 9a824cce21b48ebf0b11367ae32d1597 "$CDN" "$CDN_PATH" --paths
# wow 12.1.0.69214 bc=742fe96462414a88e1b1c35f5d09e2c5 cc=58c5b154cd4305b529d77f8cabe995a0
run_build "$OUTDIR/wow_12.1.0.69214_742fe964.txt" "$BFT" wow 742fe96462414a88e1b1c35f5d09e2c5 58c5b154cd4305b529d77f8cabe995a0 "$CDN" "$CDN_PATH" --paths
# wow 12.1.0.69273 bc=8fc12df3f3746ac3760a7edcd4cf0ec4 cc=9a824cce21b48ebf0b11367ae32d1597
run_build "$OUTDIR/wow_12.1.0.69273_8fc12df3.txt" "$BFT" wow 8fc12df3f3746ac3760a7edcd4cf0ec4 9a824cce21b48ebf0b11367ae32d1597 "$CDN" "$CDN_PATH" --paths

# wow 10.0.0.46181 bc=8cc219f402853e62a659185cf57de96a cc=4668c189ee585b7ac16ae35bb90a781b
run_build "$OUTDIR/wow_10.0.0.46181_4668c189.txt" "$BFT" wow 8cc219f402853e62a659185cf57de96a 4668c189ee585b7ac16ae35bb90a781b "$CDN" "$CDN_PATH" --paths
# wow 10.0.0.46293 bc=a41cc30b58bfbf2e6f02d60ddd82ec22 cc=10d4f546a1890a60dfdbf4ba09422e13
run_build "$OUTDIR/wow_10.0.0.46293_10d4f546.txt" "$BFT" wow a41cc30b58bfbf2e6f02d60ddd82ec22 10d4f546a1890a60dfdbf4ba09422e13 "$CDN" "$CDN_PATH" --paths
# wow 10.0.0.46313 bc=a98bc63156e62945604ec3260ae30f0d cc=bd54b458762f2dd342762eb2b81ce24d
run_build "$OUTDIR/wow_10.0.0.46313_bd54b458.txt" "$BFT" wow a98bc63156e62945604ec3260ae30f0d bd54b458762f2dd342762eb2b81ce24d "$CDN" "$CDN_PATH" --paths
# wow 10.0.0.46340 bc=d650830003ab126f64d0588b0e6dcad6 cc=cb7eaf80e5d1531a7f6009678871b16e
run_build "$OUTDIR/wow_10.0.0.46340_cb7eaf80.txt" "$BFT" wow d650830003ab126f64d0588b0e6dcad6 cb7eaf80e5d1531a7f6009678871b16e "$CDN" "$CDN_PATH" --paths
# wow 10.0.0.46366 bc=cde5309417f422cc72ab683043c8f5eb cc=d29401eefa34a0e88e1ccd1d12b45756
run_build "$OUTDIR/wow_10.0.0.46366_d29401ee.txt" "$BFT" wow cde5309417f422cc72ab683043c8f5eb d29401eefa34a0e88e1ccd1d12b45756 "$CDN" "$CDN_PATH" --paths
# wow 10.0.0.46455 bc=83059516d43056c6f9c023c1128f3f19 cc=8b1d85c40e732fbf5cf7bd91eeb1056f
run_build "$OUTDIR/wow_10.0.0.46455_8b1d85c4.txt" "$BFT" wow 83059516d43056c6f9c023c1128f3f19 8b1d85c40e732fbf5cf7bd91eeb1056f "$CDN" "$CDN_PATH" --paths
# wow 10.0.0.46547 bc=67edd2447d4557dbbe59050f41499900 cc=33d7f30f65d14c24b7efb0c00487933c
run_build "$OUTDIR/wow_10.0.0.46547_33d7f30f.txt" "$BFT" wow 67edd2447d4557dbbe59050f41499900 33d7f30f65d14c24b7efb0c00487933c "$CDN" "$CDN_PATH" --paths
# wow 10.0.0.46549 bc=82a14b9ae2acbbd5881f441e3dc49c94 cc=c2e69e8a7a37a96bfefb0237261cf1a4
run_build "$OUTDIR/wow_10.0.0.46549_c2e69e8a.txt" "$BFT" wow 82a14b9ae2acbbd5881f441e3dc49c94 c2e69e8a7a37a96bfefb0237261cf1a4 "$CDN" "$CDN_PATH" --paths
# wow 10.0.0.46597 bc=e5e125cafc4129c8742ac23f884414c0 cc=f77c0a513d350c62e4f3194db216ccf1
run_build "$OUTDIR/wow_10.0.0.46597_f77c0a51.txt" "$BFT" wow e5e125cafc4129c8742ac23f884414c0 f77c0a513d350c62e4f3194db216ccf1 "$CDN" "$CDN_PATH" --paths
# wow 10.0.2.46479 bc=21c64ec13643e9a117c87b12cfeb5572 cc=66c8ea17b1c956f050ab0191842a58ce
run_build "$OUTDIR/wow_10.0.2.46479_66c8ea17.txt" "$BFT" wow 21c64ec13643e9a117c87b12cfeb5572 66c8ea17b1c956f050ab0191842a58ce "$CDN" "$CDN_PATH" --paths
# wow 10.0.2.46658 bc=8601a7423b61856e9680073c27e17512 cc=0216c94c7b21712d5217b164b9fe540b
run_build "$OUTDIR/wow_10.0.2.46658_0216c94c.txt" "$BFT" wow 8601a7423b61856e9680073c27e17512 0216c94c7b21712d5217b164b9fe540b "$CDN" "$CDN_PATH" --paths
# wow 10.0.2.46689 bc=5ae4e902fbb5deec333370caea8ee268 cc=33fa09616d00d3de708d7c3f9347f5e2
run_build "$OUTDIR/wow_10.0.2.46689_33fa0961.txt" "$BFT" wow 5ae4e902fbb5deec333370caea8ee268 33fa09616d00d3de708d7c3f9347f5e2 "$CDN" "$CDN_PATH" --paths
# wow 10.0.2.46702 bc=9a3387ebc064f348a531246978fa4806 cc=39fb5d972a17bb21b2e3c82fbbe8650b
run_build "$OUTDIR/wow_10.0.2.46702_39fb5d97.txt" "$BFT" wow 9a3387ebc064f348a531246978fa4806 39fb5d972a17bb21b2e3c82fbbe8650b "$CDN" "$CDN_PATH" --paths
# wow 10.0.2.46741 bc=e4838f62d190d3c5b887b9643fb26be9 cc=fc626903c3b522771da2cfff603aee2a
run_build "$OUTDIR/wow_10.0.2.46741_fc626903.txt" "$BFT" wow e4838f62d190d3c5b887b9643fb26be9 fc626903c3b522771da2cfff603aee2a "$CDN" "$CDN_PATH" --paths
# wow 10.0.2.46801 bc=13e13a23bf7184fca97ef0ec61a8263f cc=6b594d02a1a72c894932309c2092bedd
run_build "$OUTDIR/wow_10.0.2.46801_6b594d02.txt" "$BFT" wow 13e13a23bf7184fca97ef0ec61a8263f 6b594d02a1a72c894932309c2092bedd "$CDN" "$CDN_PATH" --paths
# wow 10.0.2.46879 bc=2a43c52913a87842204e6a02aca1d407 cc=e12e7493bda5fd743e0c21d85bf525a9
run_build "$OUTDIR/wow_10.0.2.46879_e12e7493.txt" "$BFT" wow 2a43c52913a87842204e6a02aca1d407 e12e7493bda5fd743e0c21d85bf525a9 "$CDN" "$CDN_PATH" --paths
# wow 10.0.2.46924 bc=65c3e32c0b590d4287f84833d228c73c cc=442ac8e2cd689fd54983ba7e2af28961
run_build "$OUTDIR/wow_10.0.2.46924_442ac8e2.txt" "$BFT" wow 65c3e32c0b590d4287f84833d228c73c 442ac8e2cd689fd54983ba7e2af28961 "$CDN" "$CDN_PATH" --paths
# wow 10.0.2.46999 bc=ddf90db9d5c1f23bf9d05faba021b503 cc=ab486c9fc6a5deb9ec794eac41aa0114
run_build "$OUTDIR/wow_10.0.2.46999_ab486c9f.txt" "$BFT" wow ddf90db9d5c1f23bf9d05faba021b503 ab486c9fc6a5deb9ec794eac41aa0114 "$CDN" "$CDN_PATH" --paths
# wow 10.0.2.47067 bc=b7c50fb47a8b97c129dee4d1c2172f4d cc=847204ef4900004fbc3d7fac80494e9d
run_build "$OUTDIR/wow_10.0.2.47067_847204ef.txt" "$BFT" wow b7c50fb47a8b97c129dee4d1c2172f4d 847204ef4900004fbc3d7fac80494e9d "$CDN" "$CDN_PATH" --paths
# wow 10.0.2.47120 bc=2fdc4e9192dfe70158027350dde82626 cc=970abc47b80fc73038bfae41ba1df705
run_build "$OUTDIR/wow_10.0.2.47120_970abc47.txt" "$BFT" wow 2fdc4e9192dfe70158027350dde82626 970abc47b80fc73038bfae41ba1df705 "$CDN" "$CDN_PATH" --paths
# wow 10.0.2.47187 bc=8e6575f77aba9de840ad9ae7afed29a5 cc=78f58fb36f3ec730105a343a8cf9e230
run_build "$OUTDIR/wow_10.0.2.47187_78f58fb3.txt" "$BFT" wow 8e6575f77aba9de840ad9ae7afed29a5 78f58fb36f3ec730105a343a8cf9e230 "$CDN" "$CDN_PATH" --paths
# wow 10.0.2.47213 bc=968defa5a1dee9cd63c5603819067a28 cc=92106fe9a8f9b25d139fbf3b925aa2c9
run_build "$OUTDIR/wow_10.0.2.47213_92106fe9.txt" "$BFT" wow 968defa5a1dee9cd63c5603819067a28 92106fe9a8f9b25d139fbf3b925aa2c9 "$CDN" "$CDN_PATH" --paths
# wow 10.0.2.47631 bc=914a955abec6b614e5907a4ee1d7a110 cc=abfa67a0d44050074be13d0184d98690
run_build "$OUTDIR/wow_10.0.2.47631_abfa67a0.txt" "$BFT" wow 914a955abec6b614e5907a4ee1d7a110 abfa67a0d44050074be13d0184d98690 "$CDN" "$CDN_PATH" --paths
# wow 10.0.2.47657 bc=f1bc76dfaf6619e73133b56bfaaec3fd cc=abfa67a0d44050074be13d0184d98690
run_build "$OUTDIR/wow_10.0.2.47657_abfa67a0.txt" "$BFT" wow f1bc76dfaf6619e73133b56bfaaec3fd abfa67a0d44050074be13d0184d98690 "$CDN" "$CDN_PATH" --paths
# wow 10.0.5.47777 bc=1e5226824726199de02869f0256d9fcb cc=fc32fd7ccfc78243faafedcc9e224b27
run_build "$OUTDIR/wow_10.0.5.47777_fc32fd7c.txt" "$BFT" wow 1e5226824726199de02869f0256d9fcb fc32fd7ccfc78243faafedcc9e224b27 "$CDN" "$CDN_PATH" --paths
# wow 10.0.5.47799 bc=12dfddc08777e63e05b57314c33ed420 cc=ef8fb837588e99b6247a00008601db26
run_build "$OUTDIR/wow_10.0.5.47799_ef8fb837.txt" "$BFT" wow 12dfddc08777e63e05b57314c33ed420 ef8fb837588e99b6247a00008601db26 "$CDN" "$CDN_PATH" --paths
# wow 10.0.5.47825 bc=d8eff957d18a3899da26046d0e1c7952 cc=c612c4ad3b33fd50b4c189a47ddc7bda
run_build "$OUTDIR/wow_10.0.5.47825_c612c4ad.txt" "$BFT" wow d8eff957d18a3899da26046d0e1c7952 c612c4ad3b33fd50b4c189a47ddc7bda "$CDN" "$CDN_PATH" --paths
# wow 10.0.5.47849 bc=1eeb361904e5d5257a45710bec85ed6d cc=beda496a8edd4e7f8be8a213b2777ce1
run_build "$OUTDIR/wow_10.0.5.47849_beda496a.txt" "$BFT" wow 1eeb361904e5d5257a45710bec85ed6d beda496a8edd4e7f8be8a213b2777ce1 "$CDN" "$CDN_PATH" --paths
# wow 10.0.5.47871 bc=49793f7d24f64be37093a91d8581ad75 cc=4983b04db36e76bf914df363107a6bf1
run_build "$OUTDIR/wow_10.0.5.47871_4983b04d.txt" "$BFT" wow 49793f7d24f64be37093a91d8581ad75 4983b04db36e76bf914df363107a6bf1 "$CDN" "$CDN_PATH" --paths
# wow 10.0.5.47884 bc=b574cf28b9e138e24500404503c2a607 cc=9e322933f574f4c5d0abd49fdf696898
run_build "$OUTDIR/wow_10.0.5.47884_9e322933.txt" "$BFT" wow b574cf28b9e138e24500404503c2a607 9e322933f574f4c5d0abd49fdf696898 "$CDN" "$CDN_PATH" --paths
# wow 10.0.5.47936 bc=8763073acf54a078bd65fe5bdb4d7855 cc=fbebe5b3a4eb891c1f1ddfa0c9a53067
run_build "$OUTDIR/wow_10.0.5.47936_fbebe5b3.txt" "$BFT" wow 8763073acf54a078bd65fe5bdb4d7855 fbebe5b3a4eb891c1f1ddfa0c9a53067 "$CDN" "$CDN_PATH" --paths
# wow 10.0.5.47967 bc=48cece437793d92cf3c76620a301f8be cc=5b5a23459396738dc9e5b2d93d22d201
run_build "$OUTDIR/wow_10.0.5.47967_5b5a2345.txt" "$BFT" wow 48cece437793d92cf3c76620a301f8be 5b5a23459396738dc9e5b2d93d22d201 "$CDN" "$CDN_PATH" --paths
# wow 10.0.5.48001 bc=d5669bb2f7511a734b6ffcd3752bedec cc=e2733562826e73ab8639dc272bfe6049
run_build "$OUTDIR/wow_10.0.5.48001_e2733562.txt" "$BFT" wow d5669bb2f7511a734b6ffcd3752bedec e2733562826e73ab8639dc272bfe6049 "$CDN" "$CDN_PATH" --paths
# wow 10.0.5.48069 bc=e4d13ccbd171637753cd24d834fa9dca cc=6f5b548cfd473c6ad033bcf9f6d5cbdd
run_build "$OUTDIR/wow_10.0.5.48069_6f5b548c.txt" "$BFT" wow e4d13ccbd171637753cd24d834fa9dca 6f5b548cfd473c6ad033bcf9f6d5cbdd "$CDN" "$CDN_PATH" --paths
# wow 10.0.5.48317 bc=e577645bdad65496e3f84135b244dfed cc=ea03521c23d142d3399fbed731753ce2
run_build "$OUTDIR/wow_10.0.5.48317_ea03521c.txt" "$BFT" wow e577645bdad65496e3f84135b244dfed ea03521c23d142d3399fbed731753ce2 "$CDN" "$CDN_PATH" --paths
# wow 10.0.5.48397 bc=acab5eb3c45b2c218fb9160b40a6c79b cc=e94b1f2b5a9b29035930d0149d4e3052
run_build "$OUTDIR/wow_10.0.5.48397_e94b1f2b.txt" "$BFT" wow acab5eb3c45b2c218fb9160b40a6c79b e94b1f2b5a9b29035930d0149d4e3052 "$CDN" "$CDN_PATH" --paths
# wow 10.0.5.48526 bc=b9ab4274800680659b47e4e67933a38c cc=e3a6836776c8a2036a6cf49dd8601cef
run_build "$OUTDIR/wow_10.0.5.48526_e3a68367.txt" "$BFT" wow b9ab4274800680659b47e4e67933a38c e3a6836776c8a2036a6cf49dd8601cef "$CDN" "$CDN_PATH" --paths
# wow 10.0.7.48676 bc=cb3aad7bf30180e263ccbe3aa362e273 cc=cee2ca2d6bc7356398e0b44fc15ed317
run_build "$OUTDIR/wow_10.0.7.48676_cee2ca2d.txt" "$BFT" wow cb3aad7bf30180e263ccbe3aa362e273 cee2ca2d6bc7356398e0b44fc15ed317 "$CDN" "$CDN_PATH" --paths
# wow 10.0.7.48749 bc=6077e42edefb982e4ac9f225254ab1da cc=c7db8bf36705e4a3483a3f9876789808
run_build "$OUTDIR/wow_10.0.7.48749_c7db8bf3.txt" "$BFT" wow 6077e42edefb982e4ac9f225254ab1da c7db8bf36705e4a3483a3f9876789808 "$CDN" "$CDN_PATH" --paths
# wow 10.0.7.48838 bc=c1884878a0d5b5225c68044a0f1b685c cc=302eecb3ec9e0a076f3bc0c6d65367de
run_build "$OUTDIR/wow_10.0.7.48838_302eecb3.txt" "$BFT" wow c1884878a0d5b5225c68044a0f1b685c 302eecb3ec9e0a076f3bc0c6d65367de "$CDN" "$CDN_PATH" --paths
# wow 10.0.7.48865 bc=f6b55e87a9ac24c46fde097763492544 cc=8efe8962a2201e7d145195b859e6e601
run_build "$OUTDIR/wow_10.0.7.48865_8efe8962.txt" "$BFT" wow f6b55e87a9ac24c46fde097763492544 8efe8962a2201e7d145195b859e6e601 "$CDN" "$CDN_PATH" --paths
# wow 10.0.7.48892 bc=b152947f434cb26e31a128e263457544 cc=357ecda3c1beb737c09d276c3676b059
run_build "$OUTDIR/wow_10.0.7.48892_357ecda3.txt" "$BFT" wow b152947f434cb26e31a128e263457544 357ecda3c1beb737c09d276c3676b059 "$CDN" "$CDN_PATH" --paths
# wow 10.0.7.48966 bc=1bb9334c84586949854974074a2af928 cc=2b03d0ab065764c15897af294737441c
run_build "$OUTDIR/wow_10.0.7.48966_2b03d0ab.txt" "$BFT" wow 1bb9334c84586949854974074a2af928 2b03d0ab065764c15897af294737441c "$CDN" "$CDN_PATH" --paths
# wow 10.0.7.48999 bc=46db5a81da6505de842941fe3555b237 cc=807779c010a9a5730ca3ce2e60929158
run_build "$OUTDIR/wow_10.0.7.48999_807779c0.txt" "$BFT" wow 46db5a81da6505de842941fe3555b237 807779c010a9a5730ca3ce2e60929158 "$CDN" "$CDN_PATH" --paths
# wow 10.0.7.49267 bc=2a18cdc2090826a39aad5804fa056799 cc=18d63336ef6a93b7f5cf5e1e6e3fdd56
run_build "$OUTDIR/wow_10.0.7.49267_18d63336.txt" "$BFT" wow 2a18cdc2090826a39aad5804fa056799 18d63336ef6a93b7f5cf5e1e6e3fdd56 "$CDN" "$CDN_PATH" --paths
# wow 10.0.7.49343 bc=5daa07780ec9d83068d663a61984ce7d cc=dfbf3af977343cca0f1dd6b90ebf399c
run_build "$OUTDIR/wow_10.0.7.49343_dfbf3af9.txt" "$BFT" wow 5daa07780ec9d83068d663a61984ce7d dfbf3af977343cca0f1dd6b90ebf399c "$CDN" "$CDN_PATH" --paths
# wow 10.1.0.49318 bc=8877c8e5a47f56cca03f898a6f6b2f99 cc=b8e457b0f7c12655d12cd686100bcaf9
run_build "$OUTDIR/wow_10.1.0.49318_b8e457b0.txt" "$BFT" wow 8877c8e5a47f56cca03f898a6f6b2f99 b8e457b0f7c12655d12cd686100bcaf9 "$CDN" "$CDN_PATH" --paths
# wow 10.1.0.49407 bc=0b1b642594e4ccdd7dcecef145243d2c cc=58a7017f9128069f2175ae7ee694b53f
run_build "$OUTDIR/wow_10.1.0.49407_58a7017f.txt" "$BFT" wow 0b1b642594e4ccdd7dcecef145243d2c 58a7017f9128069f2175ae7ee694b53f "$CDN" "$CDN_PATH" --paths
# wow 10.1.0.49426 bc=9d32a30ddb352f0f633b2185046281bd cc=b6bce4a1d63e6a77bff6bf8e882bfd99
run_build "$OUTDIR/wow_10.1.0.49426_b6bce4a1.txt" "$BFT" wow 9d32a30ddb352f0f633b2185046281bd b6bce4a1d63e6a77bff6bf8e882bfd99 "$CDN" "$CDN_PATH" --paths
# wow 10.1.0.49444 bc=c0870cbd7cf5bec1a3c1fb3c2c9fae2d cc=5e99c1fad7fd188affd41bf8cb6f5ff5
run_build "$OUTDIR/wow_10.1.0.49444_5e99c1fa.txt" "$BFT" wow c0870cbd7cf5bec1a3c1fb3c2c9fae2d 5e99c1fad7fd188affd41bf8cb6f5ff5 "$CDN" "$CDN_PATH" --paths
# wow 10.1.0.49474 bc=059ba4b33ef69401625a6a035785f036 cc=5f77c1920e24fabab787ce21022faf67
run_build "$OUTDIR/wow_10.1.0.49474_5f77c192.txt" "$BFT" wow 059ba4b33ef69401625a6a035785f036 5f77c1920e24fabab787ce21022faf67 "$CDN" "$CDN_PATH" --paths
# wow 10.1.0.49570 bc=b352d562d44264ba8c1dcc9d7ca7b57c cc=6372270fc51a98f579c50c86bcf9c8b9
run_build "$OUTDIR/wow_10.1.0.49570_6372270f.txt" "$BFT" wow b352d562d44264ba8c1dcc9d7ca7b57c 6372270fc51a98f579c50c86bcf9c8b9 "$CDN" "$CDN_PATH" --paths
# wow 10.1.0.49679 bc=13acc107e1de46278c664039f8e3e7b8 cc=337acf37efe70d77b3598a9f3f0f6623
run_build "$OUTDIR/wow_10.1.0.49679_337acf37.txt" "$BFT" wow 13acc107e1de46278c664039f8e3e7b8 337acf37efe70d77b3598a9f3f0f6623 "$CDN" "$CDN_PATH" --paths
# wow 10.1.0.49741 bc=2c0e5ab868d479641838c94cffb350dc cc=4e380c47417b1d428f5c05fb6706442d
run_build "$OUTDIR/wow_10.1.0.49741_4e380c47.txt" "$BFT" wow 2c0e5ab868d479641838c94cffb350dc 4e380c47417b1d428f5c05fb6706442d "$CDN" "$CDN_PATH" --paths
# wow 10.1.0.49801 bc=76b07cdb98f6604251ae81dc4ebd7f7e cc=c842402337836ea4a2665f2b2a97e859
run_build "$OUTDIR/wow_10.1.0.49801_c8424023.txt" "$BFT" wow 76b07cdb98f6604251ae81dc4ebd7f7e c842402337836ea4a2665f2b2a97e859 "$CDN" "$CDN_PATH" --paths
# wow 10.1.0.49890 bc=31dd8da8f6525059f85f6554df8c142d cc=9e349ace8931d9d86abda067b3d40e6f
run_build "$OUTDIR/wow_10.1.0.49890_9e349ace.txt" "$BFT" wow 31dd8da8f6525059f85f6554df8c142d 9e349ace8931d9d86abda067b3d40e6f "$CDN" "$CDN_PATH" --paths
# wow 10.1.0.50000 bc=26f9ff71801755f7cfc70519c32e41b5 cc=6b0366fa88b0a6fab11dfcd7ec0f7767
run_build "$OUTDIR/wow_10.1.0.50000_6b0366fa.txt" "$BFT" wow 26f9ff71801755f7cfc70519c32e41b5 6b0366fa88b0a6fab11dfcd7ec0f7767 "$CDN" "$CDN_PATH" --paths
# wow 10.1.5.50401 bc=4a5ba7902fc81b482f3f3b8a5e01ad29 cc=b9dc7e110e6337ec437196cd4c5bf089
run_build "$OUTDIR/wow_10.1.5.50401_b9dc7e11.txt" "$BFT" wow 4a5ba7902fc81b482f3f3b8a5e01ad29 b9dc7e110e6337ec437196cd4c5bf089 "$CDN" "$CDN_PATH" --paths
# wow 10.1.5.50438 bc=de837d3b7231f66db95b1b4afd41762b cc=bf4d4baee7ac7e3cf77549d401bc3a2e
run_build "$OUTDIR/wow_10.1.5.50438_bf4d4bae.txt" "$BFT" wow de837d3b7231f66db95b1b4afd41762b bf4d4baee7ac7e3cf77549d401bc3a2e "$CDN" "$CDN_PATH" --paths
# wow 10.1.5.50467 bc=667e68dc4f8d1e590a6c16ad67036034 cc=362f7da0c1f2ad3039550771f1b9ba92
run_build "$OUTDIR/wow_10.1.5.50467_362f7da0.txt" "$BFT" wow 667e68dc4f8d1e590a6c16ad67036034 362f7da0c1f2ad3039550771f1b9ba92 "$CDN" "$CDN_PATH" --paths
# wow 10.1.5.50469 bc=245f6edea2c19dad52543ecf5e60e89b cc=92b35045847dde115441448fc842a0b7
run_build "$OUTDIR/wow_10.1.5.50469_92b35045.txt" "$BFT" wow 245f6edea2c19dad52543ecf5e60e89b 92b35045847dde115441448fc842a0b7 "$CDN" "$CDN_PATH" --paths
# wow 10.1.5.50504 bc=c4ad6c4be30eb30dbddd981dc682901d cc=7d9ded19d0762b3f0a09c0657f466c96
run_build "$OUTDIR/wow_10.1.5.50504_7d9ded19.txt" "$BFT" wow c4ad6c4be30eb30dbddd981dc682901d 7d9ded19d0762b3f0a09c0657f466c96 "$CDN" "$CDN_PATH" --paths
# wow 10.1.5.50585 bc=a4550ba4260d3c56d1d06766917fbb23 cc=94c9cc72756f5f04d8687596338abbed
run_build "$OUTDIR/wow_10.1.5.50585_94c9cc72.txt" "$BFT" wow a4550ba4260d3c56d1d06766917fbb23 94c9cc72756f5f04d8687596338abbed "$CDN" "$CDN_PATH" --paths
# wow 10.1.5.50622 bc=89c082b44675a7a2d076bb78b94b219d cc=084f1ddd4ed2ab0d59360c86e1c66d8a
run_build "$OUTDIR/wow_10.1.5.50622_084f1ddd.txt" "$BFT" wow 89c082b44675a7a2d076bb78b94b219d 084f1ddd4ed2ab0d59360c86e1c66d8a "$CDN" "$CDN_PATH" --paths
# wow 10.1.5.50747 bc=a27f0a8113ddabb1b20607f99eb71eaf cc=5e022a8882da91e4dd69d9cde15a9003
run_build "$OUTDIR/wow_10.1.5.50747_5e022a88.txt" "$BFT" wow a27f0a8113ddabb1b20607f99eb71eaf 5e022a8882da91e4dd69d9cde15a9003 "$CDN" "$CDN_PATH" --paths
# wow 10.1.5.50791 bc=3ce8dab403cdb01ff117c31b3a75f173 cc=a40afc0e77a4f06932befa786eedd2c7
run_build "$OUTDIR/wow_10.1.5.50791_a40afc0e.txt" "$BFT" wow 3ce8dab403cdb01ff117c31b3a75f173 a40afc0e77a4f06932befa786eedd2c7 "$CDN" "$CDN_PATH" --paths
# wow 10.1.5.50793 bc=913225dc9c9a6aab21dbb295e89d127b cc=b62eca61282aed300db20a16498ed662
run_build "$OUTDIR/wow_10.1.5.50793_b62eca61.txt" "$BFT" wow 913225dc9c9a6aab21dbb295e89d127b b62eca61282aed300db20a16498ed662 "$CDN" "$CDN_PATH" --paths
# wow 10.1.5.51130 bc=2a3fca5b2e7043984707ceec34bf23a9 cc=ffd476ba69b46419ee6f5d922447c623
run_build "$OUTDIR/wow_10.1.5.51130_ffd476ba.txt" "$BFT" wow 2a3fca5b2e7043984707ceec34bf23a9 ffd476ba69b46419ee6f5d922447c623 "$CDN" "$CDN_PATH" --paths
# wow 10.1.7.51187 bc=7990745128c99aa70954e857f1aa2129 cc=87025e4c467593a7ecbc71e845d5e874
run_build "$OUTDIR/wow_10.1.7.51187_87025e4c.txt" "$BFT" wow 7990745128c99aa70954e857f1aa2129 87025e4c467593a7ecbc71e845d5e874 "$CDN" "$CDN_PATH" --paths
# wow 10.1.7.51237 bc=a6138ad2103233b5d8ae343f07927f64 cc=bfc467acb3eddfa60e4dba4f0ac07c3d
run_build "$OUTDIR/wow_10.1.7.51237_bfc467ac.txt" "$BFT" wow a6138ad2103233b5d8ae343f07927f64 bfc467acb3eddfa60e4dba4f0ac07c3d "$CDN" "$CDN_PATH" --paths
# wow 10.1.7.51261 bc=e91a91fecfea01afd9f72555a20d76a3 cc=bce97e9e01937a5d8d8df730798c9f77
run_build "$OUTDIR/wow_10.1.7.51261_bce97e9e.txt" "$BFT" wow e91a91fecfea01afd9f72555a20d76a3 bce97e9e01937a5d8d8df730798c9f77 "$CDN" "$CDN_PATH" --paths
# wow 10.1.7.51313 bc=353749be9378aad52443a51d73814341 cc=4c6f9d251aa5c82d3db3356b59dfbee8
run_build "$OUTDIR/wow_10.1.7.51313_4c6f9d25.txt" "$BFT" wow 353749be9378aad52443a51d73814341 4c6f9d251aa5c82d3db3356b59dfbee8 "$CDN" "$CDN_PATH" --paths
# wow 10.1.7.51421 bc=f67896863d0b12ee843da8bd4c9a0841 cc=27ce6a55070eb1e5d3939d20e88a802f
run_build "$OUTDIR/wow_10.1.7.51421_27ce6a55.txt" "$BFT" wow f67896863d0b12ee843da8bd4c9a0841 27ce6a55070eb1e5d3939d20e88a802f "$CDN" "$CDN_PATH" --paths
# wow 10.1.7.51485 bc=0c57646373fe3249c04dd24591c63b01 cc=ad9a4c0fb25692504a9a8cc7d6800c35
run_build "$OUTDIR/wow_10.1.7.51485_ad9a4c0f.txt" "$BFT" wow 0c57646373fe3249c04dd24591c63b01 ad9a4c0fb25692504a9a8cc7d6800c35 "$CDN" "$CDN_PATH" --paths
# wow 10.1.7.51536 bc=1fcc1bf93b3478ee84d8f1447a590372 cc=1297d1ab2cae31c188c29081bd35fbf8
run_build "$OUTDIR/wow_10.1.7.51536_1297d1ab.txt" "$BFT" wow 1fcc1bf93b3478ee84d8f1447a590372 1297d1ab2cae31c188c29081bd35fbf8 "$CDN" "$CDN_PATH" --paths
# wow 10.1.7.51754 bc=ba7b1ebb3bb70b3ed8b58d8b6176851a cc=eda5d67d0bb94879b391b7cc0c1a7743
run_build "$OUTDIR/wow_10.1.7.51754_eda5d67d.txt" "$BFT" wow ba7b1ebb3bb70b3ed8b58d8b6176851a eda5d67d0bb94879b391b7cc0c1a7743 "$CDN" "$CDN_PATH" --paths
# wow 10.1.7.51886 bc=ebf4e18460491d4c4fe80080fad69184 cc=e8bc8e56f69b36f4ff1346c82ae9575e
run_build "$OUTDIR/wow_10.1.7.51886_e8bc8e56.txt" "$BFT" wow ebf4e18460491d4c4fe80080fad69184 e8bc8e56f69b36f4ff1346c82ae9575e "$CDN" "$CDN_PATH" --paths
# wow 10.1.7.51972 bc=226e1cb0e75e44cc1c0abf049caf9880 cc=5433438497f57268cbcbe182b88d8e87
run_build "$OUTDIR/wow_10.1.7.51972_54334384.txt" "$BFT" wow 226e1cb0e75e44cc1c0abf049caf9880 5433438497f57268cbcbe182b88d8e87 "$CDN" "$CDN_PATH" --paths
# wow 10.2.0.52038 bc=d57f3143b145ade0fb3f6a6a3f6beacd cc=093f5285fd8d06df7ec29583b68fc46d
run_build "$OUTDIR/wow_10.2.0.52038_093f5285.txt" "$BFT" wow d57f3143b145ade0fb3f6a6a3f6beacd 093f5285fd8d06df7ec29583b68fc46d "$CDN" "$CDN_PATH" --paths
# wow 10.2.0.52068 bc=88fb232fbfe8b1a66fd01b004c24ca4d cc=0316a85c1cbbc476416705e125188b22
run_build "$OUTDIR/wow_10.2.0.52068_0316a85c.txt" "$BFT" wow 88fb232fbfe8b1a66fd01b004c24ca4d 0316a85c1cbbc476416705e125188b22 "$CDN" "$CDN_PATH" --paths
# wow 10.2.0.52095 bc=746d0b4932530024b468af432ee60375 cc=4258b494dd8f18c9aca1d06e7600c7cb
run_build "$OUTDIR/wow_10.2.0.52095_4258b494.txt" "$BFT" wow 746d0b4932530024b468af432ee60375 4258b494dd8f18c9aca1d06e7600c7cb "$CDN" "$CDN_PATH" --paths
# wow 10.2.0.52106 bc=0b0be0a6ae8dfd7d00e67f7f189a9a09 cc=5efe37fa48bbc5002d3980a9c62f26ac
run_build "$OUTDIR/wow_10.2.0.52106_5efe37fa.txt" "$BFT" wow 0b0be0a6ae8dfd7d00e67f7f189a9a09 5efe37fa48bbc5002d3980a9c62f26ac "$CDN" "$CDN_PATH" --paths
# wow 10.2.0.52129 bc=40e8b6f4833bb28c8b993d0ddbe91a56 cc=9ae1a2e2dcc7dba10cdd4ffa3510d43d
run_build "$OUTDIR/wow_10.2.0.52129_9ae1a2e2.txt" "$BFT" wow 40e8b6f4833bb28c8b993d0ddbe91a56 9ae1a2e2dcc7dba10cdd4ffa3510d43d "$CDN" "$CDN_PATH" --paths
# wow 10.2.0.52148 bc=187853d05e17381f14f6de3714bb64f2 cc=13342ef057d7fb3bd5ac673bfb555548
run_build "$OUTDIR/wow_10.2.0.52148_13342ef0.txt" "$BFT" wow 187853d05e17381f14f6de3714bb64f2 13342ef057d7fb3bd5ac673bfb555548 "$CDN" "$CDN_PATH" --paths
# wow 10.2.0.52188 bc=2b43d605f508b250f40f1c952ec2f296 cc=f304ca2d88bce90f0aea1430986f6628
run_build "$OUTDIR/wow_10.2.0.52188_f304ca2d.txt" "$BFT" wow 2b43d605f508b250f40f1c952ec2f296 f304ca2d88bce90f0aea1430986f6628 "$CDN" "$CDN_PATH" --paths
# wow 10.2.0.52301 bc=f57f41c58c296998fbd02a8e0822ec41 cc=9a4cf85dee0d2865c739eefe1c9c11cd
run_build "$OUTDIR/wow_10.2.0.52301_9a4cf85d.txt" "$BFT" wow f57f41c58c296998fbd02a8e0822ec41 9a4cf85dee0d2865c739eefe1c9c11cd "$CDN" "$CDN_PATH" --paths
# wow 10.2.0.52393 bc=7d4314a46d9e8a935dfe8e4c2ccf03c3 cc=ee03563d993a759b0b6413e9c3f0f7cf
run_build "$OUTDIR/wow_10.2.0.52393_ee03563d.txt" "$BFT" wow 7d4314a46d9e8a935dfe8e4c2ccf03c3 ee03563d993a759b0b6413e9c3f0f7cf "$CDN" "$CDN_PATH" --paths
# wow 10.2.0.52485 bc=260998a4e79490ede428bda903995028 cc=1459674c4825c913de2c20b5dafcc45c
run_build "$OUTDIR/wow_10.2.0.52485_1459674c.txt" "$BFT" wow 260998a4e79490ede428bda903995028 1459674c4825c913de2c20b5dafcc45c "$CDN" "$CDN_PATH" --paths
# wow 10.2.0.52545 bc=c892c2ac6b430626eb0999c984e68c81 cc=90e5b83606a3ad37d4b41d1735720e09
run_build "$OUTDIR/wow_10.2.0.52545_90e5b836.txt" "$BFT" wow c892c2ac6b430626eb0999c984e68c81 90e5b83606a3ad37d4b41d1735720e09 "$CDN" "$CDN_PATH" --paths
# wow 10.2.0.52607 bc=6b7d607c50ce3455515a031260011ace cc=308352f1db2c5e56c70e18ffe9030c71
run_build "$OUTDIR/wow_10.2.0.52607_308352f1.txt" "$BFT" wow 6b7d607c50ce3455515a031260011ace 308352f1db2c5e56c70e18ffe9030c71 "$CDN" "$CDN_PATH" --paths
# wow 10.2.0.52649 bc=4ff8c9de9a6202738928bc3f40b4d09a cc=a2713b40db972aa0bab91820e0801e7c
run_build "$OUTDIR/wow_10.2.0.52649_a2713b40.txt" "$BFT" wow 4ff8c9de9a6202738928bc3f40b4d09a a2713b40db972aa0bab91820e0801e7c "$CDN" "$CDN_PATH" --paths
# wow 10.2.0.52808 bc=bbce59818936dee292d4b32c69877c6c cc=c4ae5a81f431e333e7e8fac5b299dbc2
run_build "$OUTDIR/wow_10.2.0.52808_c4ae5a81.txt" "$BFT" wow bbce59818936dee292d4b32c69877c6c c4ae5a81f431e333e7e8fac5b299dbc2 "$CDN" "$CDN_PATH" --paths
# wow 10.2.5.52902 bc=21e761759807dcbbf16ceaa66f068096 cc=f9a3d515b3581cf14e739b2c09a93695
run_build "$OUTDIR/wow_10.2.5.52902_f9a3d515.txt" "$BFT" wow 21e761759807dcbbf16ceaa66f068096 f9a3d515b3581cf14e739b2c09a93695 "$CDN" "$CDN_PATH" --paths
# wow 10.2.5.52968 bc=e1e5896239a109f6ed1c1c58aeab69d3 cc=cbe4ac5365b6009a853734d83864e563
run_build "$OUTDIR/wow_10.2.5.52968_cbe4ac53.txt" "$BFT" wow e1e5896239a109f6ed1c1c58aeab69d3 cbe4ac5365b6009a853734d83864e563 "$CDN" "$CDN_PATH" --paths
# wow 10.2.5.52983 bc=497d26f986a5a7afd72065c6126c1cb3 cc=665cbe741b8408f41cdb02d29081c878
run_build "$OUTDIR/wow_10.2.5.52983_665cbe74.txt" "$BFT" wow 497d26f986a5a7afd72065c6126c1cb3 665cbe741b8408f41cdb02d29081c878 "$CDN" "$CDN_PATH" --paths
# wow 10.2.5.53007 bc=2095ade6738106cbfb43ed044013652a cc=1cdb5446d7899393530de0a3950bfcc5
run_build "$OUTDIR/wow_10.2.5.53007_1cdb5446.txt" "$BFT" wow 2095ade6738106cbfb43ed044013652a 1cdb5446d7899393530de0a3950bfcc5 "$CDN" "$CDN_PATH" --paths
# wow 10.2.5.53040 bc=8e2a29c99fa002f43b917ac234a8bcc4 cc=671523ecc01b7447ef14a35f591948a7
run_build "$OUTDIR/wow_10.2.5.53040_671523ec.txt" "$BFT" wow 8e2a29c99fa002f43b917ac234a8bcc4 671523ecc01b7447ef14a35f591948a7 "$CDN" "$CDN_PATH" --paths
# wow 10.2.5.53104 bc=92f9536580e665aafec6876b53ebfae0 cc=639dc8999f4c3dbae892367eb3bedce3
run_build "$OUTDIR/wow_10.2.5.53104_639dc899.txt" "$BFT" wow 92f9536580e665aafec6876b53ebfae0 639dc8999f4c3dbae892367eb3bedce3 "$CDN" "$CDN_PATH" --paths
# wow 10.2.5.53162 bc=09eff11aed021ca0961e986a062eaa0d cc=aa854ee1b99f42279ad9fd12d58ce174
run_build "$OUTDIR/wow_10.2.5.53162_aa854ee1.txt" "$BFT" wow 09eff11aed021ca0961e986a062eaa0d aa854ee1b99f42279ad9fd12d58ce174 "$CDN" "$CDN_PATH" --paths
# wow 10.2.5.53212 bc=cf7a55c788a1e0473dda97c4e286749e cc=6b1d4d1be980e9584d95560b8e10e76f
run_build "$OUTDIR/wow_10.2.5.53212_6b1d4d1b.txt" "$BFT" wow cf7a55c788a1e0473dda97c4e286749e 6b1d4d1be980e9584d95560b8e10e76f "$CDN" "$CDN_PATH" --paths
# wow 10.2.5.53262 bc=a415af04ba13c3604e60edc7fc014f5e cc=26f8d1d58311ada6219c71f721f24ced
run_build "$OUTDIR/wow_10.2.5.53262_26f8d1d5.txt" "$BFT" wow a415af04ba13c3604e60edc7fc014f5e 26f8d1d58311ada6219c71f721f24ced "$CDN" "$CDN_PATH" --paths
# wow 10.2.5.53441 bc=7d8594a162a572cfdcc1577356237913 cc=c6d4324cc3c5e6b64d137970971cb7a4
run_build "$OUTDIR/wow_10.2.5.53441_c6d4324c.txt" "$BFT" wow 7d8594a162a572cfdcc1577356237913 c6d4324cc3c5e6b64d137970971cb7a4 "$CDN" "$CDN_PATH" --paths
# wow 10.2.5.53584 bc=47e9e06f8371afb141e22614a912acc8 cc=1f3344bfab22e2d43d4b20741ceaa9dd
run_build "$OUTDIR/wow_10.2.5.53584_1f3344bf.txt" "$BFT" wow 47e9e06f8371afb141e22614a912acc8 1f3344bfab22e2d43d4b20741ceaa9dd "$CDN" "$CDN_PATH" --paths
# wow 10.2.6.53840 bc=fdca3ff8fbb210f29fbb6aee1335cc09 cc=55d26237ef8e7eace1c9b0bd58df9013
run_build "$OUTDIR/wow_10.2.6.53840_55d26237.txt" "$BFT" wow fdca3ff8fbb210f29fbb6aee1335cc09 55d26237ef8e7eace1c9b0bd58df9013 "$CDN" "$CDN_PATH" --paths
# wow 10.2.6.53877 bc=d689689cb67e98a69c415e92cba9eb6d cc=d2dc7842208c1bc955af98cf7e3a14ab
run_build "$OUTDIR/wow_10.2.6.53877_d2dc7842.txt" "$BFT" wow d689689cb67e98a69c415e92cba9eb6d d2dc7842208c1bc955af98cf7e3a14ab "$CDN" "$CDN_PATH" --paths
# wow 10.2.6.53913 bc=af23e9ab5787bfab6cf679ff7241bfff cc=57bc731056412d5795a28d7341c91f73
run_build "$OUTDIR/wow_10.2.6.53913_57bc7310.txt" "$BFT" wow af23e9ab5787bfab6cf679ff7241bfff 57bc731056412d5795a28d7341c91f73 "$CDN" "$CDN_PATH" --paths
# wow 10.2.6.53989 bc=acb4ae782e4c990ef764592e78bc9e85 cc=84347da0003f8bf8e7c39ee2ec696cca
run_build "$OUTDIR/wow_10.2.6.53989_84347da0.txt" "$BFT" wow acb4ae782e4c990ef764592e78bc9e85 84347da0003f8bf8e7c39ee2ec696cca "$CDN" "$CDN_PATH" --paths
# wow 10.2.6.54070 bc=0cc44a6561645b08b4551665c20271d9 cc=8fbb06198256b49d9769464173a832fa
run_build "$OUTDIR/wow_10.2.6.54070_8fbb0619.txt" "$BFT" wow 0cc44a6561645b08b4551665c20271d9 8fbb06198256b49d9769464173a832fa "$CDN" "$CDN_PATH" --paths
# wow 10.2.6.54205 bc=d182d7d4ba066d98ee62a675a1cc3a9c cc=d365b7da13c26403d561f2c934c7ef0d
run_build "$OUTDIR/wow_10.2.6.54205_d365b7da.txt" "$BFT" wow d182d7d4ba066d98ee62a675a1cc3a9c d365b7da13c26403d561f2c934c7ef0d "$CDN" "$CDN_PATH" --paths
# wow 10.2.6.54358 bc=c258dd0d5fe7de9f254611ee2b042e2b cc=8e908e8dac08b4de7417db68eabfc6d4
run_build "$OUTDIR/wow_10.2.6.54358_8e908e8d.txt" "$BFT" wow c258dd0d5fe7de9f254611ee2b042e2b 8e908e8dac08b4de7417db68eabfc6d4 "$CDN" "$CDN_PATH" --paths
# wow 10.2.6.54499 bc=a65768c230a4db2d5cf975cfa74a8ae7 cc=1d03bef4dd45595566498f3e276f176a
run_build "$OUTDIR/wow_10.2.6.54499_1d03bef4.txt" "$BFT" wow a65768c230a4db2d5cf975cfa74a8ae7 1d03bef4dd45595566498f3e276f176a "$CDN" "$CDN_PATH" --paths
# wow 10.2.7.54577 bc=065594636086a700570a4777ad500f55 cc=48514176e794a3ce93772283367d402b
run_build "$OUTDIR/wow_10.2.7.54577_48514176.txt" "$BFT" wow 065594636086a700570a4777ad500f55 48514176e794a3ce93772283367d402b "$CDN" "$CDN_PATH" --paths
# wow 10.2.7.54601 bc=30ba99322cf44ab18f2a4c37f4331372 cc=eef1bf91a405d63fdb2cc11ac00126b7
run_build "$OUTDIR/wow_10.2.7.54601_eef1bf91.txt" "$BFT" wow 30ba99322cf44ab18f2a4c37f4331372 eef1bf91a405d63fdb2cc11ac00126b7 "$CDN" "$CDN_PATH" --paths
# wow 10.2.7.54604 bc=5da0df54af91ba9f5ab57b1365e4947b cc=e8bcee3ab6369240263857c513cefae7
run_build "$OUTDIR/wow_10.2.7.54604_e8bcee3a.txt" "$BFT" wow 5da0df54af91ba9f5ab57b1365e4947b e8bcee3ab6369240263857c513cefae7 "$CDN" "$CDN_PATH" --paths
# wow 10.2.7.54630 bc=ff05918405a85c4f9a52b396758a6cf0 cc=c0acafef4ec64399f4439fd9efb3c023
run_build "$OUTDIR/wow_10.2.7.54630_c0acafef.txt" "$BFT" wow ff05918405a85c4f9a52b396758a6cf0 c0acafef4ec64399f4439fd9efb3c023 "$CDN" "$CDN_PATH" --paths
# wow 10.2.7.54673 bc=7244695fd85ec726fe50909dead8be04 cc=1963bcb1b836b21fdae6c9b757c0b622
run_build "$OUTDIR/wow_10.2.7.54673_1963bcb1.txt" "$BFT" wow 7244695fd85ec726fe50909dead8be04 1963bcb1b836b21fdae6c9b757c0b622 "$CDN" "$CDN_PATH" --paths
# wow 10.2.7.54717 bc=5dbff9e77101d3695af52d6e5f887992 cc=0fe08f26b3841735a913a728d5adc3f5
run_build "$OUTDIR/wow_10.2.7.54717_0fe08f26.txt" "$BFT" wow 5dbff9e77101d3695af52d6e5f887992 0fe08f26b3841735a913a728d5adc3f5 "$CDN" "$CDN_PATH" --paths
# wow 10.2.7.54736 bc=ae1b66e62b1ccad1b39344f219a4e011 cc=e6be8766a186e7ebe678bbd72f4ce0c7
run_build "$OUTDIR/wow_10.2.7.54736_e6be8766.txt" "$BFT" wow ae1b66e62b1ccad1b39344f219a4e011 e6be8766a186e7ebe678bbd72f4ce0c7 "$CDN" "$CDN_PATH" --paths
# wow 10.2.7.54762 bc=1a01b1e6bbecaa23cb13848c50c10f33 cc=2ac38d9ae035a8ea99bda43cac3df9e6
run_build "$OUTDIR/wow_10.2.7.54762_2ac38d9a.txt" "$BFT" wow 1a01b1e6bbecaa23cb13848c50c10f33 2ac38d9ae035a8ea99bda43cac3df9e6 "$CDN" "$CDN_PATH" --paths
# wow 10.2.7.54847 bc=c3676c7d4bd37514d94cb96d02bef7da cc=9c6f0269786fb5c6b4943ac612179127
run_build "$OUTDIR/wow_10.2.7.54847_9c6f0269.txt" "$BFT" wow c3676c7d4bd37514d94cb96d02bef7da 9c6f0269786fb5c6b4943ac612179127 "$CDN" "$CDN_PATH" --paths
# wow 10.2.7.54904 bc=11bc0edf70595f781eb2efd1c4220aed cc=8b004ac6957c1baf172168e1dea3f684
run_build "$OUTDIR/wow_10.2.7.54904_8b004ac6.txt" "$BFT" wow 11bc0edf70595f781eb2efd1c4220aed 8b004ac6957c1baf172168e1dea3f684 "$CDN" "$CDN_PATH" --paths
# wow 10.2.7.54988 bc=d7882e066f2ecfbd762dc21b68aef689 cc=7cb313ab15371488a2577539b4c6b0ae
run_build "$OUTDIR/wow_10.2.7.54988_7cb313ab.txt" "$BFT" wow d7882e066f2ecfbd762dc21b68aef689 7cb313ab15371488a2577539b4c6b0ae "$CDN" "$CDN_PATH" --paths
# wow 10.2.7.55142 bc=c629e95bffc20ed4bb09abeac90a52f3 cc=2506f68d24cabd5d4fe4fbceaf2a961f
run_build "$OUTDIR/wow_10.2.7.55142_2506f68d.txt" "$BFT" wow c629e95bffc20ed4bb09abeac90a52f3 2506f68d24cabd5d4fe4fbceaf2a961f "$CDN" "$CDN_PATH" --paths
# wow 10.2.7.55165 bc=b6caaf0ed53500bd8008d8535921b4cf cc=98956f781029ec02b5ffd5ebc2eb8f67
run_build "$OUTDIR/wow_10.2.7.55165_98956f78.txt" "$BFT" wow b6caaf0ed53500bd8008d8535921b4cf 98956f781029ec02b5ffd5ebc2eb8f67 "$CDN" "$CDN_PATH" --paths
# wow 10.2.7.55261 bc=e3666f82de301acf400257d9a2fb1945 cc=8a556b424bfc08c80aa90b3703403142
run_build "$OUTDIR/wow_10.2.7.55261_8a556b42.txt" "$BFT" wow e3666f82de301acf400257d9a2fb1945 8a556b424bfc08c80aa90b3703403142 "$CDN" "$CDN_PATH" --paths
# wow 10.2.7.55461 bc=5149c3cdf17c082c42fb843e6b49e2aa cc=a427fc06ac4969ae454854f67537635f
run_build "$OUTDIR/wow_10.2.7.55461_a427fc06.txt" "$BFT" wow 5149c3cdf17c082c42fb843e6b49e2aa a427fc06ac4969ae454854f67537635f "$CDN" "$CDN_PATH" --paths
# wow 10.2.7.55664 bc=379ba71ea4d3c1b6173534226589af85 cc=90948ba6c2446aa2af3fbb41b5a7134a
run_build "$OUTDIR/wow_10.2.7.55664_90948ba6.txt" "$BFT" wow 379ba71ea4d3c1b6173534226589af85 90948ba6c2446aa2af3fbb41b5a7134a "$CDN" "$CDN_PATH" --paths

# wow 11.0.0.55666 bc=1aaafc571d67429f268bcc7b5ef40b43 cc=92eb8c113d40b9a39d45f183cbb4ae5c
run_build "$OUTDIR/wow_11.0.0.55666_92eb8c11.txt" "$BFT" wow 1aaafc571d67429f268bcc7b5ef40b43 92eb8c113d40b9a39d45f183cbb4ae5c "$CDN" "$CDN_PATH" --paths
# wow 11.0.0.55793 bc=702603e4b6dd92d6e3c4b73ee0723028 cc=1880f5901b0e15bd254a4ad9dad24bc4
run_build "$OUTDIR/wow_11.0.0.55793_1880f590.txt" "$BFT" wow 702603e4b6dd92d6e3c4b73ee0723028 1880f5901b0e15bd254a4ad9dad24bc4 "$CDN" "$CDN_PATH" --paths
# wow 11.0.0.55818 bc=3b98b62446b1c7e3e99eab256bad5bef cc=e6242a0de0bbad94d5d5b7d9e9c1b889
run_build "$OUTDIR/wow_11.0.0.55818_e6242a0d.txt" "$BFT" wow 3b98b62446b1c7e3e99eab256bad5bef e6242a0de0bbad94d5d5b7d9e9c1b889 "$CDN" "$CDN_PATH" --paths
# wow 11.0.0.55824 bc=ea870f14529bcc9f403bf774fc9160a1 cc=e959c001d8d6c01eb075204ab1ab9a01
run_build "$OUTDIR/wow_11.0.0.55824_e959c001.txt" "$BFT" wow ea870f14529bcc9f403bf774fc9160a1 e959c001d8d6c01eb075204ab1ab9a01 "$CDN" "$CDN_PATH" --paths
# wow 11.0.0.55846 bc=2dea7a76dde1f630a2b161d0aca7c6ec cc=b928b239010cb4fc4ce701eaad7f6113
run_build "$OUTDIR/wow_11.0.0.55846_b928b239.txt" "$BFT" wow 2dea7a76dde1f630a2b161d0aca7c6ec b928b239010cb4fc4ce701eaad7f6113 "$CDN" "$CDN_PATH" --paths
# wow 11.0.0.55933 bc=6d129616611680fe8a8102b93c38c509 cc=95503e2fa99e959cf93991bade0e0a18
run_build "$OUTDIR/wow_11.0.0.55933_95503e2f.txt" "$BFT" wow 6d129616611680fe8a8102b93c38c509 95503e2fa99e959cf93991bade0e0a18 "$CDN" "$CDN_PATH" --paths
# wow 11.0.0.55939 bc=45b2adc6600622009b384808796c107a cc=5530374a58924c0cb0b0770dbe1cb5e4
run_build "$OUTDIR/wow_11.0.0.55939_5530374a.txt" "$BFT" wow 45b2adc6600622009b384808796c107a 5530374a58924c0cb0b0770dbe1cb5e4 "$CDN" "$CDN_PATH" --paths
# wow 11.0.0.55945 bc=280fd82b059ad0c192698ae6abeba3b3 cc=842cdcf6208f5dae17e56d95a66b0b1f
run_build "$OUTDIR/wow_11.0.0.55945_842cdcf6.txt" "$BFT" wow 280fd82b059ad0c192698ae6abeba3b3 842cdcf6208f5dae17e56d95a66b0b1f "$CDN" "$CDN_PATH" --paths
# wow 11.0.0.55960 bc=14bd2df2e053ccaa2a83d5920202b84f cc=e07f05eb1af4f48a187f61b244775d81
run_build "$OUTDIR/wow_11.0.0.55960_e07f05eb.txt" "$BFT" wow 14bd2df2e053ccaa2a83d5920202b84f e07f05eb1af4f48a187f61b244775d81 "$CDN" "$CDN_PATH" --paths
# wow 11.0.0.56008 bc=7ad793b140905b3d9a871bbcab1f5f87 cc=857a33c600a9493f875c09bf6f40def5
run_build "$OUTDIR/wow_11.0.0.56008_857a33c6.txt" "$BFT" wow 7ad793b140905b3d9a871bbcab1f5f87 857a33c600a9493f875c09bf6f40def5 "$CDN" "$CDN_PATH" --paths
# wow 11.0.2.55959 bc=a222c40d49808ea596b9679ed669dfbb cc=e0a1ed23f195af731c7aa2e7b39b47e6
run_build "$OUTDIR/wow_11.0.2.55959_e0a1ed23.txt" "$BFT" wow a222c40d49808ea596b9679ed669dfbb e0a1ed23f195af731c7aa2e7b39b47e6 "$CDN" "$CDN_PATH" --paths
# wow 11.0.2.56110 bc=85dc91ad96159bcff1ebece21805f10f cc=e66d82597ad897aeba818c40536c6d42
run_build "$OUTDIR/wow_11.0.2.56110_e66d8259.txt" "$BFT" wow 85dc91ad96159bcff1ebece21805f10f e66d82597ad897aeba818c40536c6d42 "$CDN" "$CDN_PATH" --paths
# wow 11.0.2.56162 bc=0d218989d16434971dcb3647e451a4e0 cc=08a1b4ee0adc987a176d82b910d74fd2
run_build "$OUTDIR/wow_11.0.2.56162_08a1b4ee.txt" "$BFT" wow 0d218989d16434971dcb3647e451a4e0 08a1b4ee0adc987a176d82b910d74fd2 "$CDN" "$CDN_PATH" --paths
# wow 11.0.2.56196 bc=fec03bc217eb94a0a1588dedf4fa204c cc=9057f930a6e1459165d026f185152374
run_build "$OUTDIR/wow_11.0.2.56196_9057f930.txt" "$BFT" wow fec03bc217eb94a0a1588dedf4fa204c 9057f930a6e1459165d026f185152374 "$CDN" "$CDN_PATH" --paths
# wow 11.0.2.56263 bc=6663bd74d1266c58fedf649e4e936e43 cc=f3aeeebbf8227805348e0d46c748d84b
run_build "$OUTDIR/wow_11.0.2.56263_f3aeeebb.txt" "$BFT" wow 6663bd74d1266c58fedf649e4e936e43 f3aeeebbf8227805348e0d46c748d84b "$CDN" "$CDN_PATH" --paths
# wow 11.0.2.56288 bc=08354c917dac27d70caf5e6fe51751b7 cc=c23815ad4771e5a443b064dd86e42e1c
run_build "$OUTDIR/wow_11.0.2.56288_c23815ad.txt" "$BFT" wow 08354c917dac27d70caf5e6fe51751b7 c23815ad4771e5a443b064dd86e42e1c "$CDN" "$CDN_PATH" --paths
# wow 11.0.2.56311 bc=403ecc9cef499d173253eec1ef756ffa cc=583b62aa7f8a4fb0e590e073da4a5d04
run_build "$OUTDIR/wow_11.0.2.56311_583b62aa.txt" "$BFT" wow 403ecc9cef499d173253eec1ef756ffa 583b62aa7f8a4fb0e590e073da4a5d04 "$CDN" "$CDN_PATH" --paths
# wow 11.0.2.56313 bc=0b5b17c14d139452504729d22c3e2cbb cc=b9b6f99151e76c72d093e15191d68f9c
run_build "$OUTDIR/wow_11.0.2.56313_b9b6f991.txt" "$BFT" wow 0b5b17c14d139452504729d22c3e2cbb b9b6f99151e76c72d093e15191d68f9c "$CDN" "$CDN_PATH" --paths
# wow 11.0.2.56380 bc=476309012286b54fcc6c0406d8ac6fdc cc=d8901ab70136327222713dc2eec9ca08
run_build "$OUTDIR/wow_11.0.2.56380_d8901ab7.txt" "$BFT" wow 476309012286b54fcc6c0406d8ac6fdc d8901ab70136327222713dc2eec9ca08 "$CDN" "$CDN_PATH" --paths
# wow 11.0.2.56382 bc=6470ccbaac854d7643d1bb470f7d829b cc=1b0f85987f3b8d348c6b1a1cd2d6bcfa
run_build "$OUTDIR/wow_11.0.2.56382_1b0f8598.txt" "$BFT" wow 6470ccbaac854d7643d1bb470f7d829b 1b0f85987f3b8d348c6b1a1cd2d6bcfa "$CDN" "$CDN_PATH" --paths
# wow 11.0.2.56421 bc=79337c606b37023c39cd90927c06bcf6 cc=8ce35b11223e7de81a510ffb52e04692
run_build "$OUTDIR/wow_11.0.2.56421_8ce35b11.txt" "$BFT" wow 79337c606b37023c39cd90927c06bcf6 8ce35b11223e7de81a510ffb52e04692 "$CDN" "$CDN_PATH" --paths
# wow 11.0.2.56461 bc=4f035247651c379e52a0c942a372f711 cc=a9705b7d44592026286c8dfa8b680dd8
run_build "$OUTDIR/wow_11.0.2.56461_a9705b7d.txt" "$BFT" wow 4f035247651c379e52a0c942a372f711 a9705b7d44592026286c8dfa8b680dd8 "$CDN" "$CDN_PATH" --paths
# wow 11.0.2.56513 bc=70607411eef2604b2bc48fb5797018fd cc=b6e963e99ec06830ec48a13574dba40c
run_build "$OUTDIR/wow_11.0.2.56513_b6e963e9.txt" "$BFT" wow 70607411eef2604b2bc48fb5797018fd b6e963e99ec06830ec48a13574dba40c "$CDN" "$CDN_PATH" --paths
# wow 11.0.2.56625 bc=409d2d25bd75cfb908a6aef5703f3d5b cc=ecf5a589068cb089376d12bc93aea451
run_build "$OUTDIR/wow_11.0.2.56625_ecf5a589.txt" "$BFT" wow 409d2d25bd75cfb908a6aef5703f3d5b ecf5a589068cb089376d12bc93aea451 "$CDN" "$CDN_PATH" --paths
# wow 11.0.2.56647 bc=c1d5f7a5cfd05810fe5e4d2e64b6cd98 cc=8ea3975944cc0d9cfb488ec3b0c406d7
run_build "$OUTDIR/wow_11.0.2.56647_8ea39759.txt" "$BFT" wow c1d5f7a5cfd05810fe5e4d2e64b6cd98 8ea3975944cc0d9cfb488ec3b0c406d7 "$CDN" "$CDN_PATH" --paths
# wow 11.0.2.56819 bc=bbc549c3af3e408e55eb7e4931ace1b7 cc=7e6abdfd4e055849a2756ae44d65e5b3
run_build "$OUTDIR/wow_11.0.2.56819_7e6abdfd.txt" "$BFT" wow bbc549c3af3e408e55eb7e4931ace1b7 7e6abdfd4e055849a2756ae44d65e5b3 "$CDN" "$CDN_PATH" --paths
# wow 11.0.5.57147 bc=02695b8ddad8c86f8a123ddfc691b26c cc=9639475fb8f00c8e3802fb72b9d2d4d3
run_build "$OUTDIR/wow_11.0.5.57147_9639475f.txt" "$BFT" wow 02695b8ddad8c86f8a123ddfc691b26c 9639475fb8f00c8e3802fb72b9d2d4d3 "$CDN" "$CDN_PATH" --paths
# wow 11.0.5.57171 bc=a32f0865f327786cead099a0433d9473 cc=59dee3139492f6a5d9f7b9279a483e8c
run_build "$OUTDIR/wow_11.0.5.57171_59dee313.txt" "$BFT" wow a32f0865f327786cead099a0433d9473 59dee3139492f6a5d9f7b9279a483e8c "$CDN" "$CDN_PATH" --paths
# wow 11.0.5.57212 bc=afb222415432704dab1c5849cfd3e39f cc=1942cd48cf1891fb6479a99789a46b25
run_build "$OUTDIR/wow_11.0.5.57212_1942cd48.txt" "$BFT" wow afb222415432704dab1c5849cfd3e39f 1942cd48cf1891fb6479a99789a46b25 "$CDN" "$CDN_PATH" --paths
# wow 11.0.5.57292 bc=34095fd747ffffc9bdaa42c8317c5efd cc=ddcd715134ff6e8ea6b26cd809f18ff9
run_build "$OUTDIR/wow_11.0.5.57292_ddcd7151.txt" "$BFT" wow 34095fd747ffffc9bdaa42c8317c5efd ddcd715134ff6e8ea6b26cd809f18ff9 "$CDN" "$CDN_PATH" --paths
# wow 11.0.5.57388 bc=08bb65d7bb507e5ea8c94683913ac978 cc=220046cb50c6bef1112bf09ea8ef2aff
run_build "$OUTDIR/wow_11.0.5.57388_220046cb.txt" "$BFT" wow 08bb65d7bb507e5ea8c94683913ac978 220046cb50c6bef1112bf09ea8ef2aff "$CDN" "$CDN_PATH" --paths
# wow 11.0.5.57534 bc=ce6271d444cda54fbf4589105b4ace2f cc=5f9c69f71f7f97107451f95ea7e4350a
run_build "$OUTDIR/wow_11.0.5.57534_5f9c69f7.txt" "$BFT" wow ce6271d444cda54fbf4589105b4ace2f 5f9c69f71f7f97107451f95ea7e4350a "$CDN" "$CDN_PATH" --paths
# wow 11.0.5.57637 bc=c6d5141f5936be1144d162325911b86f cc=38b154749c970aac221ac77c95dc8baa
run_build "$OUTDIR/wow_11.0.5.57637_38b15474.txt" "$BFT" wow c6d5141f5936be1144d162325911b86f 38b154749c970aac221ac77c95dc8baa "$CDN" "$CDN_PATH" --paths
# wow 11.0.5.57689 bc=f16ef9d3cf788b00bc00bda77f09aa14 cc=bbec466cce0be7b3c58b1f923f58f474
run_build "$OUTDIR/wow_11.0.5.57689_bbec466c.txt" "$BFT" wow f16ef9d3cf788b00bc00bda77f09aa14 bbec466cce0be7b3c58b1f923f58f474 "$CDN" "$CDN_PATH" --paths
# wow 11.0.7.58123 bc=41906c3b32a1d4e36279387e2ee855d0 cc=611d759decc5a5c0129be3441f21fb1c
run_build "$OUTDIR/wow_11.0.7.58123_611d759d.txt" "$BFT" wow 41906c3b32a1d4e36279387e2ee855d0 611d759decc5a5c0129be3441f21fb1c "$CDN" "$CDN_PATH" --paths
# wow 11.0.7.58162 bc=f2c152194e7a79bcf5c7af9b88918652 cc=611d759decc5a5c0129be3441f21fb1c
run_build "$OUTDIR/wow_11.0.7.58162_611d759d.txt" "$BFT" wow f2c152194e7a79bcf5c7af9b88918652 611d759decc5a5c0129be3441f21fb1c "$CDN" "$CDN_PATH" --paths
# wow 11.0.7.58187 bc=1e4ba4127ab815160785a70468bd3d1f cc=7b6e4bd83a112fafc407ece3809b0022
run_build "$OUTDIR/wow_11.0.7.58187_7b6e4bd8.txt" "$BFT" wow 1e4ba4127ab815160785a70468bd3d1f 7b6e4bd83a112fafc407ece3809b0022 "$CDN" "$CDN_PATH" --paths
# wow 11.0.7.58238 bc=0c245919b5294f12f4c65238b15f550c cc=cd18191b8928c33bf24b962e9330460f
run_build "$OUTDIR/wow_11.0.7.58238_cd18191b.txt" "$BFT" wow 0c245919b5294f12f4c65238b15f550c cd18191b8928c33bf24b962e9330460f "$CDN" "$CDN_PATH" --paths
# wow 11.0.7.58533 bc=51324cf6c3ee32bea328c76ba6c8ce67 cc=3d5538e3dcb94b09431f9aa76ca6b7c4
run_build "$OUTDIR/wow_11.0.7.58533_3d5538e3.txt" "$BFT" wow 51324cf6c3ee32bea328c76ba6c8ce67 3d5538e3dcb94b09431f9aa76ca6b7c4 "$CDN" "$CDN_PATH" --paths
# wow 11.0.7.58608 bc=645739ee4e372d3ac47291ca1b7fb31a cc=21bbc6554078300da267d282caa5ddb4
run_build "$OUTDIR/wow_11.0.7.58608_21bbc655.txt" "$BFT" wow 645739ee4e372d3ac47291ca1b7fb31a 21bbc6554078300da267d282caa5ddb4 "$CDN" "$CDN_PATH" --paths
# wow 11.0.7.58630 bc=63f2f80bbaabd19b6c945b9a289d5ca4 cc=940e02d634640392d91a077cadb21d23
run_build "$OUTDIR/wow_11.0.7.58630_940e02d6.txt" "$BFT" wow 63f2f80bbaabd19b6c945b9a289d5ca4 940e02d634640392d91a077cadb21d23 "$CDN" "$CDN_PATH" --paths
# wow 11.0.7.58680 bc=4ec06917a304663100367bb6f519f087 cc=692331728e444f8a3c7ff55af61412ff
run_build "$OUTDIR/wow_11.0.7.58680_69233172.txt" "$BFT" wow 4ec06917a304663100367bb6f519f087 692331728e444f8a3c7ff55af61412ff "$CDN" "$CDN_PATH" --paths
# wow 11.0.7.58773 bc=64ad48f078e0ade021139221fedc2803 cc=3737e3fc0318efbf3f894b8628b27744
run_build "$OUTDIR/wow_11.0.7.58773_3737e3fc.txt" "$BFT" wow 64ad48f078e0ade021139221fedc2803 3737e3fc0318efbf3f894b8628b27744 "$CDN" "$CDN_PATH" --paths
# wow 11.0.7.58867 bc=d286a270962f7423a32e276cedc23bea cc=ea311024a7c9624d44276a6be7f2afc1
run_build "$OUTDIR/wow_11.0.7.58867_ea311024.txt" "$BFT" wow d286a270962f7423a32e276cedc23bea ea311024a7c9624d44276a6be7f2afc1 "$CDN" "$CDN_PATH" --paths
# wow 11.0.7.58911 bc=659ab614980cd1c280d6090e0e8815fa cc=0d3d208dcf3f632eee201131f02ea920
run_build "$OUTDIR/wow_11.0.7.58911_0d3d208d.txt" "$BFT" wow 659ab614980cd1c280d6090e0e8815fa 0d3d208dcf3f632eee201131f02ea920 "$CDN" "$CDN_PATH" --paths
# wow 11.0.7.59207 bc=badda550862784d504d385bb1d69eddd cc=66bc8199fb219268f7944978ae0978f0
run_build "$OUTDIR/wow_11.0.7.59207_66bc8199.txt" "$BFT" wow badda550862784d504d385bb1d69eddd 66bc8199fb219268f7944978ae0978f0 "$CDN" "$CDN_PATH" --paths
# wow 11.0.7.59302 bc=e22fce62b0d285607bc6f347e899828a cc=d25cf2508891cddb678b7a1f7f9d1abb
run_build "$OUTDIR/wow_11.0.7.59302_d25cf250.txt" "$BFT" wow e22fce62b0d285607bc6f347e899828a d25cf2508891cddb678b7a1f7f9d1abb "$CDN" "$CDN_PATH" --paths
# wow 11.1.0.59347 bc=9f8ec2df1e2f8d3b8700d28083762398 cc=2b72e01d8f73db0bad24633de300327c
run_build "$OUTDIR/wow_11.1.0.59347_2b72e01d.txt" "$BFT" wow 9f8ec2df1e2f8d3b8700d28083762398 2b72e01d8f73db0bad24633de300327c "$CDN" "$CDN_PATH" --paths
# wow 11.1.0.59425 bc=ba33771b0748613c5cd942f297058d23 cc=b120b7aa0418944dfa5533086cd057c9
run_build "$OUTDIR/wow_11.1.0.59425_b120b7aa.txt" "$BFT" wow ba33771b0748613c5cd942f297058d23 b120b7aa0418944dfa5533086cd057c9 "$CDN" "$CDN_PATH" --paths
# wow 11.1.0.59466 bc=e1cfdbb58e29422f20ed460d82881a22 cc=0d4a4672639a03668e1e5ca789783df5
run_build "$OUTDIR/wow_11.1.0.59466_0d4a4672.txt" "$BFT" wow e1cfdbb58e29422f20ed460d82881a22 0d4a4672639a03668e1e5ca789783df5 "$CDN" "$CDN_PATH" --paths
# wow 11.1.0.59490 bc=cd1e703314f5169b03946d1beab74442 cc=6238f5480a0422c2fbef20194714949c
run_build "$OUTDIR/wow_11.1.0.59490_6238f548.txt" "$BFT" wow cd1e703314f5169b03946d1beab74442 6238f5480a0422c2fbef20194714949c "$CDN" "$CDN_PATH" --paths
# wow 11.1.0.59538 bc=11ef5fe20746d2601cc2f38f1d95dfa5 cc=e83e6828fb4f5aba2f9eec66a4204214
run_build "$OUTDIR/wow_11.1.0.59538_e83e6828.txt" "$BFT" wow 11ef5fe20746d2601cc2f38f1d95dfa5 e83e6828fb4f5aba2f9eec66a4204214 "$CDN" "$CDN_PATH" --paths
# wow 11.1.0.59570 bc=083adc130a2fb9ea9ceaaa7cbd9bb379 cc=76765d1fbc011e988a752fb270ae1b00
run_build "$OUTDIR/wow_11.1.0.59570_76765d1f.txt" "$BFT" wow 083adc130a2fb9ea9ceaaa7cbd9bb379 76765d1fbc011e988a752fb270ae1b00 "$CDN" "$CDN_PATH" --paths
# wow 11.1.0.59679 bc=24d63311ed4c6f9757c6cf72d3ef3cb1 cc=23d7026780a50010d9e0a6c4ea3f03c1
run_build "$OUTDIR/wow_11.1.0.59679_23d70267.txt" "$BFT" wow 24d63311ed4c6f9757c6cf72d3ef3cb1 23d7026780a50010d9e0a6c4ea3f03c1 "$CDN" "$CDN_PATH" --paths
# wow 11.1.0.59888 bc=fedf67300d823c4622e6836956848ebd cc=449de76dd1eff06a7d3f37a3cf1afb5c
run_build "$OUTDIR/wow_11.1.0.59888_449de76d.txt" "$BFT" wow fedf67300d823c4622e6836956848ebd 449de76dd1eff06a7d3f37a3cf1afb5c "$CDN" "$CDN_PATH" --paths
# wow 11.1.0.60037 bc=21ac78f0dc41787c3748ef07504b9a07 cc=f2b441efb65fb2a2a0d421117f436790
run_build "$OUTDIR/wow_11.1.0.60037_f2b441ef.txt" "$BFT" wow 21ac78f0dc41787c3748ef07504b9a07 f2b441efb65fb2a2a0d421117f436790 "$CDN" "$CDN_PATH" --paths
# wow 11.1.0.60189 bc=caaacfc8c90f91e441373ed29c513bed cc=e5071a656e6cda472a78ade2f808454a
run_build "$OUTDIR/wow_11.1.0.60189_e5071a65.txt" "$BFT" wow caaacfc8c90f91e441373ed29c513bed e5071a656e6cda472a78ade2f808454a "$CDN" "$CDN_PATH" --paths
# wow 11.1.0.60228 bc=859ea8c0cc2fedcae2ab3df52b960dee cc=ae5bd003c6b4f439b79b44b6900c5d93
run_build "$OUTDIR/wow_11.1.0.60228_ae5bd003.txt" "$BFT" wow 859ea8c0cc2fedcae2ab3df52b960dee ae5bd003c6b4f439b79b44b6900c5d93 "$CDN" "$CDN_PATH" --paths
# wow 11.1.0.60257 bc=92697a5d9452a8f821741e36796721ce cc=dbc8f5e4ed4f7941b365a1803f53e610
run_build "$OUTDIR/wow_11.1.0.60257_dbc8f5e4.txt" "$BFT" wow 92697a5d9452a8f821741e36796721ce dbc8f5e4ed4f7941b365a1803f53e610 "$CDN" "$CDN_PATH" --paths
# wow 11.1.5.60392 bc=cf8335172263063db28c8efc23a375db cc=5dd44d245d21f63123cb5f20cad68545
run_build "$OUTDIR/wow_11.1.5.60392_5dd44d24.txt" "$BFT" wow cf8335172263063db28c8efc23a375db 5dd44d245d21f63123cb5f20cad68545 "$CDN" "$CDN_PATH" --paths
# wow 11.1.5.60428 bc=7f41fa2ddd4cbacf5f1d2fa4089f0239 cc=8a7d6086297f80e544bef29163858b38
run_build "$OUTDIR/wow_11.1.5.60428_8a7d6086.txt" "$BFT" wow 7f41fa2ddd4cbacf5f1d2fa4089f0239 8a7d6086297f80e544bef29163858b38 "$CDN" "$CDN_PATH" --paths
# wow 11.1.5.60490 bc=2aadccaa17b14c4bdc53a20639150fd8 cc=fdc70733e8dca497c40fdf519dc0212c
run_build "$OUTDIR/wow_11.1.5.60490_fdc70733.txt" "$BFT" wow 2aadccaa17b14c4bdc53a20639150fd8 fdc70733e8dca497c40fdf519dc0212c "$CDN" "$CDN_PATH" --paths
# wow 11.1.5.60568 bc=48b065403fc95edb6f0a6e8321df45fd cc=aff6e13881198ea8d579747a5734c132
run_build "$OUTDIR/wow_11.1.5.60568_aff6e138.txt" "$BFT" wow 48b065403fc95edb6f0a6e8321df45fd aff6e13881198ea8d579747a5734c132 "$CDN" "$CDN_PATH" --paths
# wow 11.1.5.60822 bc=641b776b8c7032aa4fa9d7d9f583e7df cc=5709e81ff5c5d02ffe7a02453ed5f406
run_build "$OUTDIR/wow_11.1.5.60822_5709e81f.txt" "$BFT" wow 641b776b8c7032aa4fa9d7d9f583e7df 5709e81ff5c5d02ffe7a02453ed5f406 "$CDN" "$CDN_PATH" --paths
# wow 11.1.5.61122 bc=1c6c570f54b932d23c4a6b0b99f011fa cc=435d7276b42f7ae41a8e4a00463ccf35
run_build "$OUTDIR/wow_11.1.5.61122_435d7276.txt" "$BFT" wow 1c6c570f54b932d23c4a6b0b99f011fa 435d7276b42f7ae41a8e4a00463ccf35 "$CDN" "$CDN_PATH" --paths
# wow 11.1.5.61188 bc=2d19fe079fe83eed84090a900c563da2 cc=09031e74823f27011150c4b0aca7087c
run_build "$OUTDIR/wow_11.1.5.61188_09031e74.txt" "$BFT" wow 2d19fe079fe83eed84090a900c563da2 09031e74823f27011150c4b0aca7087c "$CDN" "$CDN_PATH" --paths
# wow 11.1.5.61265 bc=dcfc289eea032df214ebba097dc2880d cc=cea70cf029e4ff7e8d4fbf497f87e50e
run_build "$OUTDIR/wow_11.1.5.61265_cea70cf0.txt" "$BFT" wow dcfc289eea032df214ebba097dc2880d cea70cf029e4ff7e8d4fbf497f87e50e "$CDN" "$CDN_PATH" --paths
# wow 11.1.7.61491 bc=be2bb98dc28aee05bbee519393696cdb cc=fac77b9ca52c84ac28ad83a7dbe1c829
run_build "$OUTDIR/wow_11.1.7.61491_fac77b9c.txt" "$BFT" wow be2bb98dc28aee05bbee519393696cdb fac77b9ca52c84ac28ad83a7dbe1c829 "$CDN" "$CDN_PATH" --paths
# wow 11.1.7.61559 bc=e359107662e72559b4e1ab721b157cb0 cc=4407e0df4f21631dde7651927980b945
run_build "$OUTDIR/wow_11.1.7.61559_4407e0df.txt" "$BFT" wow e359107662e72559b4e1ab721b157cb0 4407e0df4f21631dde7651927980b945 "$CDN" "$CDN_PATH" --paths
# wow 11.1.7.61609 bc=89afa50b688ed58685dca894c71b160f cc=0b6c850c408eb9de8c8ecf6ae9afbaa5
run_build "$OUTDIR/wow_11.1.7.61609_0b6c850c.txt" "$BFT" wow 89afa50b688ed58685dca894c71b160f 0b6c850c408eb9de8c8ecf6ae9afbaa5 "$CDN" "$CDN_PATH" --paths
# wow 11.1.7.61965 bc=b76f789ac5338e9341240b4b9c808707 cc=51f6171b8b3f5db9abd06b778df12ec0
run_build "$OUTDIR/wow_11.1.7.61965_51f6171b.txt" "$BFT" wow b76f789ac5338e9341240b4b9c808707 51f6171b8b3f5db9abd06b778df12ec0 "$CDN" "$CDN_PATH" --paths
# wow 11.1.7.61967 bc=bf2689888ce3ac287273acd93158e46b cc=c8940696493179b5c4f9d59cf4fc9a9b
run_build "$OUTDIR/wow_11.1.7.61967_c8940696.txt" "$BFT" wow bf2689888ce3ac287273acd93158e46b c8940696493179b5c4f9d59cf4fc9a9b "$CDN" "$CDN_PATH" --paths
# wow 11.2.0.62213 bc=9d26e3cd7c5225d304dd51510f0b0828 cc=57725bd9af5fc71ea150ce8354e6c7d1
run_build "$OUTDIR/wow_11.2.0.62213_57725bd9.txt" "$BFT" wow 9d26e3cd7c5225d304dd51510f0b0828 57725bd9af5fc71ea150ce8354e6c7d1 "$CDN" "$CDN_PATH" --paths
# wow 11.2.0.62417 bc=c079e2452e0de235718d30a6e21a97de cc=71678975049c2a3bd3ccb642b2fd1773
run_build "$OUTDIR/wow_11.2.0.62417_71678975.txt" "$BFT" wow c079e2452e0de235718d30a6e21a97de 71678975049c2a3bd3ccb642b2fd1773 "$CDN" "$CDN_PATH" --paths
# wow 11.2.0.62438 bc=a638f8401a8fab39671878e0c405e62d cc=706e795b444adf45026f1c91e8677353
run_build "$OUTDIR/wow_11.2.0.62438_706e795b.txt" "$BFT" wow a638f8401a8fab39671878e0c405e62d 706e795b444adf45026f1c91e8677353 "$CDN" "$CDN_PATH" --paths
# wow 11.2.0.62493 bc=b7b342cf87c3828f09fc81828b0d06c0 cc=6edf813917c9047b748127dbbb10e316
run_build "$OUTDIR/wow_11.2.0.62493_6edf8139.txt" "$BFT" wow b7b342cf87c3828f09fc81828b0d06c0 6edf813917c9047b748127dbbb10e316 "$CDN" "$CDN_PATH" --paths
# wow 11.2.0.62610 bc=c10c9c1dfbf56476742e1dd81d531425 cc=ff414f218db022c62fa23388c87200d3
run_build "$OUTDIR/wow_11.2.0.62610_ff414f21.txt" "$BFT" wow c10c9c1dfbf56476742e1dd81d531425 ff414f218db022c62fa23388c87200d3 "$CDN" "$CDN_PATH" --paths
# wow 11.2.0.62680 bc=0e00e5a4365d4df86c5e9460c8c9d539 cc=25ff42f09d57e9af0be5804cb93ede79
run_build "$OUTDIR/wow_11.2.0.62680_25ff42f0.txt" "$BFT" wow 0e00e5a4365d4df86c5e9460c8c9d539 25ff42f09d57e9af0be5804cb93ede79 "$CDN" "$CDN_PATH" --paths
# wow 11.2.0.62706 bc=e534a0f82add7edd22f18b9158309e5b cc=61d4e518e0f6f9d65c386ee47488a2e2
run_build "$OUTDIR/wow_11.2.0.62706_61d4e518.txt" "$BFT" wow e534a0f82add7edd22f18b9158309e5b 61d4e518e0f6f9d65c386ee47488a2e2 "$CDN" "$CDN_PATH" --paths
# wow 11.2.0.62748 bc=25e0add477697e628dbdaf2d2baf827d cc=d629e9cf17f7b464b94cf27ab26dd38a
run_build "$OUTDIR/wow_11.2.0.62748_d629e9cf.txt" "$BFT" wow 25e0add477697e628dbdaf2d2baf827d d629e9cf17f7b464b94cf27ab26dd38a "$CDN" "$CDN_PATH" --paths
# wow 11.2.0.62801 bc=af3ca7e1579f12adbee496d34bb53571 cc=1d0ba1f51990c114004f191ec35ea20e
run_build "$OUTDIR/wow_11.2.0.62801_1d0ba1f5.txt" "$BFT" wow af3ca7e1579f12adbee496d34bb53571 1d0ba1f51990c114004f191ec35ea20e "$CDN" "$CDN_PATH" --paths
# wow 11.2.0.62876 bc=23ec68f42dd74f314118ebb51d312ea4 cc=d478f6bc6cdb4630a6a39d3b91556ed2
run_build "$OUTDIR/wow_11.2.0.62876_d478f6bc.txt" "$BFT" wow 23ec68f42dd74f314118ebb51d312ea4 d478f6bc6cdb4630a6a39d3b91556ed2 "$CDN" "$CDN_PATH" --paths
# wow 11.2.0.62958 bc=5f6cd4ee47cb05014a7e36c7cc643a65 cc=8bea1bcdd3984f541ac42638b1522349
run_build "$OUTDIR/wow_11.2.0.62958_8bea1bcd.txt" "$BFT" wow 5f6cd4ee47cb05014a7e36c7cc643a65 8bea1bcdd3984f541ac42638b1522349 "$CDN" "$CDN_PATH" --paths
# wow 11.2.0.63003 bc=e4d5932b54c3872934f8575ffabd8365 cc=6e09fd503ee1e3aecb6958284ad552a3
run_build "$OUTDIR/wow_11.2.0.63003_6e09fd50.txt" "$BFT" wow e4d5932b54c3872934f8575ffabd8365 6e09fd503ee1e3aecb6958284ad552a3 "$CDN" "$CDN_PATH" --paths
# wow 11.2.0.63163 bc=7099f18a0c858e807e0e156d052cea6d cc=391397d3164e0d13b9752aee3a6a15f3
run_build "$OUTDIR/wow_11.2.0.63163_391397d3.txt" "$BFT" wow 7099f18a0c858e807e0e156d052cea6d 391397d3164e0d13b9752aee3a6a15f3 "$CDN" "$CDN_PATH" --paths
# wow 11.2.0.63305 bc=0a613ab3d004dd2b19c9c62637c9599a cc=64a59ce381589d58e5aa83900a02c719
run_build "$OUTDIR/wow_11.2.0.63305_64a59ce3.txt" "$BFT" wow 0a613ab3d004dd2b19c9c62637c9599a 64a59ce381589d58e5aa83900a02c719 "$CDN" "$CDN_PATH" --paths
# wow 11.2.5.63506 bc=7d6b7e2c2edc75176a6f7deffbe9aeb2 cc=be561ec34f1a11db0c2e085ab1962687
run_build "$OUTDIR/wow_11.2.5.63506_be561ec3.txt" "$BFT" wow 7d6b7e2c2edc75176a6f7deffbe9aeb2 be561ec34f1a11db0c2e085ab1962687 "$CDN" "$CDN_PATH" --paths
# wow 11.2.5.63660 bc=06de4005cf07d3be5e315323af9a15a2 cc=e1393bb3f40735af17a467b75fc7a884
run_build "$OUTDIR/wow_11.2.5.63660_e1393bb3.txt" "$BFT" wow 06de4005cf07d3be5e315323af9a15a2 e1393bb3f40735af17a467b75fc7a884 "$CDN" "$CDN_PATH" --paths
# wow 11.2.5.63704 bc=84de191d3233a2b0a75b5bad9de8e0ea cc=23182fb0c9e8ed760bf98a672927c60e
run_build "$OUTDIR/wow_11.2.5.63704_23182fb0.txt" "$BFT" wow 84de191d3233a2b0a75b5bad9de8e0ea 23182fb0c9e8ed760bf98a672927c60e "$CDN" "$CDN_PATH" --paths
# wow 11.2.5.63796 bc=0bb0882e1354ace94ce3ddd3e825ed9a cc=f67b6166ec3e4d7a34cfe33476243649
run_build "$OUTDIR/wow_11.2.5.63796_f67b6166.txt" "$BFT" wow 0bb0882e1354ace94ce3ddd3e825ed9a f67b6166ec3e4d7a34cfe33476243649 "$CDN" "$CDN_PATH" --paths
# wow 11.2.5.63825 bc=0a5d8437f0cd4c3bd1d8d9c0462474e6 cc=8ab5c3d3bf33abe136cf8c0d6273223b
run_build "$OUTDIR/wow_11.2.5.63825_8ab5c3d3.txt" "$BFT" wow 0a5d8437f0cd4c3bd1d8d9c0462474e6 8ab5c3d3bf33abe136cf8c0d6273223b "$CDN" "$CDN_PATH" --paths
# wow 11.2.5.63834 bc=26d3ebac3dc3f790597a943f911d8e14 cc=da85687f1cd0a99d79eb0cd1969084c0
run_build "$OUTDIR/wow_11.2.5.63834_da85687f.txt" "$BFT" wow 26d3ebac3dc3f790597a943f911d8e14 da85687f1cd0a99d79eb0cd1969084c0 "$CDN" "$CDN_PATH" --paths
# wow 11.2.5.63906 bc=1bf3e29673c3b556e8779a3020fc7dc0 cc=3e3b594599aa6ebba08445631868c2aa
run_build "$OUTDIR/wow_11.2.5.63906_3e3b5945.txt" "$BFT" wow 1bf3e29673c3b556e8779a3020fc7dc0 3e3b594599aa6ebba08445631868c2aa "$CDN" "$CDN_PATH" --paths
# wow 11.2.5.64154 bc=550b30669b0eaec9ba56d7f48da6b659 cc=5eecacb59d5c7ab42952a1fab7e1a942
run_build "$OUTDIR/wow_11.2.5.64154_5eecacb5.txt" "$BFT" wow 550b30669b0eaec9ba56d7f48da6b659 5eecacb59d5c7ab42952a1fab7e1a942 "$CDN" "$CDN_PATH" --paths
# wow 11.2.5.64270 bc=78e92df828de2f1c525ae737fde22a89 cc=7800a9dd03609bbe28e5b5ec31850b47
run_build "$OUTDIR/wow_11.2.5.64270_7800a9dd.txt" "$BFT" wow 78e92df828de2f1c525ae737fde22a89 7800a9dd03609bbe28e5b5ec31850b47 "$CDN" "$CDN_PATH" --paths
# wow 11.2.5.64395 bc=c2276c4b8acf5a88750647e04ff58015 cc=c1e48ba24e823d18241a726d6d25c454
run_build "$OUTDIR/wow_11.2.5.64395_c1e48ba2.txt" "$BFT" wow c2276c4b8acf5a88750647e04ff58015 c1e48ba24e823d18241a726d6d25c454 "$CDN" "$CDN_PATH" --paths
# wow 11.2.5.64484 bc=b52c20bb1af996dbf27a994851c9c46d cc=c3ecc1e87a864955320670a245b3236f
run_build "$OUTDIR/wow_11.2.5.64484_c3ecc1e8.txt" "$BFT" wow b52c20bb1af996dbf27a994851c9c46d c3ecc1e87a864955320670a245b3236f "$CDN" "$CDN_PATH" --paths
# wow 11.2.5.64502 bc=353a77846e7a5d072e954dd81d457099 cc=46e556371a4c108d14cfb7d35435cb29
run_build "$OUTDIR/wow_11.2.5.64502_46e55637.txt" "$BFT" wow 353a77846e7a5d072e954dd81d457099 46e556371a4c108d14cfb7d35435cb29 "$CDN" "$CDN_PATH" --paths
# wow 11.2.7.64632 bc=a2c41236d708cd4d259f425dc2c894ee cc=fbf778d329e1663e024cd3afed93918b
run_build "$OUTDIR/wow_11.2.7.64632_fbf778d3.txt" "$BFT" wow a2c41236d708cd4d259f425dc2c894ee fbf778d329e1663e024cd3afed93918b "$CDN" "$CDN_PATH" --paths
# wow 11.2.7.64704 bc=343b9f61effa952bdd08ee96f7607e24 cc=97acaa425aa821a4f98cf40ff1ca97e4
run_build "$OUTDIR/wow_11.2.7.64704_97acaa42.txt" "$BFT" wow 343b9f61effa952bdd08ee96f7607e24 97acaa425aa821a4f98cf40ff1ca97e4 "$CDN" "$CDN_PATH" --paths
# wow 11.2.7.64725 bc=063d1aecbfa82c722770d1cb327e0479 cc=82b925e2e81c7a40c76691a3b4644f9c
run_build "$OUTDIR/wow_11.2.7.64725_82b925e2.txt" "$BFT" wow 063d1aecbfa82c722770d1cb327e0479 82b925e2e81c7a40c76691a3b4644f9c "$CDN" "$CDN_PATH" --paths
# wow 11.2.7.64743 bc=80513e9590bf74b4e5730caa891b368e cc=d0e1675314482d64cc82dace38656a84
run_build "$OUTDIR/wow_11.2.7.64743_d0e16753.txt" "$BFT" wow 80513e9590bf74b4e5730caa891b368e d0e1675314482d64cc82dace38656a84 "$CDN" "$CDN_PATH" --paths
# wow 11.2.7.64772 bc=a6445a2cae4e113f9ad4c3f6a84f2e1f cc=ce248ac011be198c60ff627d70524b7f
run_build "$OUTDIR/wow_11.2.7.64772_ce248ac0.txt" "$BFT" wow a6445a2cae4e113f9ad4c3f6a84f2e1f ce248ac011be198c60ff627d70524b7f "$CDN" "$CDN_PATH" --paths
# wow 11.2.7.64797 bc=d05b75990e6d05aea7262fca4b394673 cc=d0455ed5eec4ad2cc6a2eff229e2a24b
run_build "$OUTDIR/wow_11.2.7.64797_d0455ed5.txt" "$BFT" wow d05b75990e6d05aea7262fca4b394673 d0455ed5eec4ad2cc6a2eff229e2a24b "$CDN" "$CDN_PATH" --paths
# wow 11.2.7.64877 bc=0bb4c8b23779c3b6992782df172ff691 cc=a8ec3e77b14615973b2a7906c46d6be7
run_build "$OUTDIR/wow_11.2.7.64877_a8ec3e77.txt" "$BFT" wow 0bb4c8b23779c3b6992782df172ff691 a8ec3e77b14615973b2a7906c46d6be7 "$CDN" "$CDN_PATH" --paths
# wow 11.2.7.64978 bc=f2be6c5648065f58634a5d42306ed74d cc=0691991819080f8e4945b6165c5f7f2f
run_build "$OUTDIR/wow_11.2.7.64978_06919918.txt" "$BFT" wow f2be6c5648065f58634a5d42306ed74d 0691991819080f8e4945b6165c5f7f2f "$CDN" "$CDN_PATH" --paths
# wow 11.2.7.65299 bc=68cc240fcca6ab408601e2b3f9279abb cc=b3ea000d5d26fa6615af24a01e181e5f
run_build "$OUTDIR/wow_11.2.7.65299_b3ea000d.txt" "$BFT" wow 68cc240fcca6ab408601e2b3f9279abb b3ea000d5d26fa6615af24a01e181e5f "$CDN" "$CDN_PATH" --paths

# wow 12.0.0.65390 bc=9ffe965fd15902cc5845fd4b2abdd8bb cc=de01e5d341c7381fbb93e12c514c81c3
run_build "$OUTDIR/wow_12.0.0.65390_de01e5d3.txt" "$BFT" wow 9ffe965fd15902cc5845fd4b2abdd8bb de01e5d341c7381fbb93e12c514c81c3 "$CDN" "$CDN_PATH" --paths
# wow 12.0.0.65459 bc=1953e6962b5dd739b09b5e2c4700f14b cc=aa30bf2c25ac4e91ee08d584fc0df81e
run_build "$OUTDIR/wow_12.0.0.65459_aa30bf2c.txt" "$BFT" wow 1953e6962b5dd739b09b5e2c4700f14b aa30bf2c25ac4e91ee08d584fc0df81e "$CDN" "$CDN_PATH" --paths
# wow 12.0.0.65512 bc=4422b88df36a6c62f13adc9d0d14389a cc=ce3e7e595c43d7eecd0f3e3b979cebfa
run_build "$OUTDIR/wow_12.0.0.65512_ce3e7e59.txt" "$BFT" wow 4422b88df36a6c62f13adc9d0d14389a ce3e7e595c43d7eecd0f3e3b979cebfa "$CDN" "$CDN_PATH" --paths
# wow 12.0.0.65535 bc=0d675a21062c7fa1b5621e06afa74e0b cc=c6b0b15c44599af901c19a7b818bfd95
run_build "$OUTDIR/wow_12.0.0.65535_c6b0b15c.txt" "$BFT" wow 0d675a21062c7fa1b5621e06afa74e0b c6b0b15c44599af901c19a7b818bfd95 "$CDN" "$CDN_PATH" --paths
# wow 12.0.0.65560 bc=48362282f7234370b68b7cd777a7ae28 cc=51df87c814483c562788a7bf3e5df3f8
run_build "$OUTDIR/wow_12.0.0.65560_51df87c8.txt" "$BFT" wow 48362282f7234370b68b7cd777a7ae28 51df87c814483c562788a7bf3e5df3f8 "$CDN" "$CDN_PATH" --paths
# wow 12.0.0.65614 bc=b631d1932a607378a89be858945be282 cc=7d932d208d85448a5343f7a15c13aa72
run_build "$OUTDIR/wow_12.0.0.65614_7d932d20.txt" "$BFT" wow b631d1932a607378a89be858945be282 7d932d208d85448a5343f7a15c13aa72 "$CDN" "$CDN_PATH" --paths
# wow 12.0.0.65655 bc=30192472b2519840e5afd356cf59b6d0 cc=024a1f582a69894b66321a7ab5561d39
run_build "$OUTDIR/wow_12.0.0.65655_024a1f58.txt" "$BFT" wow 30192472b2519840e5afd356cf59b6d0 024a1f582a69894b66321a7ab5561d39 "$CDN" "$CDN_PATH" --paths
# wow 12.0.0.65699 bc=16d0c9f87c071164f2257cf042f30397 cc=c4ac957c960c24fe5b910e1dfd60f7cd
run_build "$OUTDIR/wow_12.0.0.65699_c4ac957c.txt" "$BFT" wow 16d0c9f87c071164f2257cf042f30397 c4ac957c960c24fe5b910e1dfd60f7cd "$CDN" "$CDN_PATH" --paths
# wow 12.0.0.65727 bc=a7256684eac3f19b74aa4a7eee41bcab cc=aaf6c86aebc4b04bd0335555d4494a31
run_build "$OUTDIR/wow_12.0.0.65727_aaf6c86a.txt" "$BFT" wow a7256684eac3f19b74aa4a7eee41bcab aaf6c86aebc4b04bd0335555d4494a31 "$CDN" "$CDN_PATH" --paths
# wow 12.0.1.65617 bc=3a8a39573814bc600e5ef7c291222b07 cc=88698d00d03d4fbbdbe3799d7227ca82
run_build "$OUTDIR/wow_12.0.1.65617_88698d00.txt" "$BFT" wow 3a8a39573814bc600e5ef7c291222b07 88698d00d03d4fbbdbe3799d7227ca82 "$CDN" "$CDN_PATH" --paths
# wow 12.0.1.65818 bc=e6cbe68a1fb240c0c54c413b09fd8590 cc=6cd184435249057cf391627d20d3a746
run_build "$OUTDIR/wow_12.0.1.65818_6cd18443.txt" "$BFT" wow e6cbe68a1fb240c0c54c413b09fd8590 6cd184435249057cf391627d20d3a746 "$CDN" "$CDN_PATH" --paths
# wow 12.0.1.65848 bc=ebd1a38a7286943653cd26ba613e0e3f cc=8f418f264e9752a7eb7e7656ff62fa1d
run_build "$OUTDIR/wow_12.0.1.65848_8f418f26.txt" "$BFT" wow ebd1a38a7286943653cd26ba613e0e3f 8f418f264e9752a7eb7e7656ff62fa1d "$CDN" "$CDN_PATH" --paths
# wow 12.0.1.65867 bc=f570e92f2f1e353eef0bdad695c62dc9 cc=b420eb85889fccea1a0fd3f2563c3e89
run_build "$OUTDIR/wow_12.0.1.65867_b420eb85.txt" "$BFT" wow f570e92f2f1e353eef0bdad695c62dc9 b420eb85889fccea1a0fd3f2563c3e89 "$CDN" "$CDN_PATH" --paths
# wow 12.0.1.65893 bc=ef9541e11283ef91b1b8f145dff970aa cc=a83dcb55a7c076f863610bde509282c8
run_build "$OUTDIR/wow_12.0.1.65893_a83dcb55.txt" "$BFT" wow ef9541e11283ef91b1b8f145dff970aa a83dcb55a7c076f863610bde509282c8 "$CDN" "$CDN_PATH" --paths
# wow 12.0.1.65899 bc=6168e9d94ba9129524d874d550224378 cc=030aa188c384555b57fa27b529063545
run_build "$OUTDIR/wow_12.0.1.65899_030aa188.txt" "$BFT" wow 6168e9d94ba9129524d874d550224378 030aa188c384555b57fa27b529063545 "$CDN" "$CDN_PATH" --paths
# wow 12.0.1.65940 bc=93955257abcea10c10837d43bd4a711e cc=532ffaaf1ee3fd016b397ebfce693c95
run_build "$OUTDIR/wow_12.0.1.65940_532ffaaf.txt" "$BFT" wow 93955257abcea10c10837d43bd4a711e 532ffaaf1ee3fd016b397ebfce693c95 "$CDN" "$CDN_PATH" --paths
# wow 12.0.1.66017 bc=e0375500f17df4ec2de44a99640388aa cc=ba594faca2339e0b5fcd41254b45cdfa
run_build "$OUTDIR/wow_12.0.1.66017_ba594fac.txt" "$BFT" wow e0375500f17df4ec2de44a99640388aa ba594faca2339e0b5fcd41254b45cdfa "$CDN" "$CDN_PATH" --paths
# wow 12.0.1.66044 bc=1c41635e1604e945a953439d088dda0b cc=e3159a0a1542de45b75fbd4f0af93bb5
run_build "$OUTDIR/wow_12.0.1.66044_e3159a0a.txt" "$BFT" wow 1c41635e1604e945a953439d088dda0b e3159a0a1542de45b75fbd4f0af93bb5 "$CDN" "$CDN_PATH" --paths
# wow 12.0.1.66066 bc=7b498dd7e196bf4161d631064f617189 cc=76cd7e3ed47b6b0c1395f236c77034fe
run_build "$OUTDIR/wow_12.0.1.66066_76cd7e3e.txt" "$BFT" wow 7b498dd7e196bf4161d631064f617189 76cd7e3ed47b6b0c1395f236c77034fe "$CDN" "$CDN_PATH" --paths
# wow 12.0.1.66102 bc=3d6e9a3613f363f1b0dc1baa0a37492f cc=f040bf4c48712a8c66c2d88cf7e6d93e
run_build "$OUTDIR/wow_12.0.1.66102_f040bf4c.txt" "$BFT" wow 3d6e9a3613f363f1b0dc1baa0a37492f f040bf4c48712a8c66c2d88cf7e6d93e "$CDN" "$CDN_PATH" --paths
# wow 12.0.1.66192 bc=13e1eb56839dfaf734d7fab21b0c8ea4 cc=8d59fb41af19b9bcd8d9d0df2fa447a8
run_build "$OUTDIR/wow_12.0.1.66192_8d59fb41.txt" "$BFT" wow 13e1eb56839dfaf734d7fab21b0c8ea4 8d59fb41af19b9bcd8d9d0df2fa447a8 "$CDN" "$CDN_PATH" --paths
# wow 12.0.1.66198 bc=4db7e319d28f6fefa82292208fa44086 cc=5d7598e3376fbdda7583d1d21ef45df2
run_build "$OUTDIR/wow_12.0.1.66198_5d7598e3.txt" "$BFT" wow 4db7e319d28f6fefa82292208fa44086 5d7598e3376fbdda7583d1d21ef45df2 "$CDN" "$CDN_PATH" --paths
# wow 12.0.1.66220 bc=4aa277f4f65711851b1d2a56c80f0751 cc=d94b91e07fdaee4625ca8355dec0d324
run_build "$OUTDIR/wow_12.0.1.66220_d94b91e0.txt" "$BFT" wow 4aa277f4f65711851b1d2a56c80f0751 d94b91e07fdaee4625ca8355dec0d324 "$CDN" "$CDN_PATH" --paths
# wow 12.0.1.66263 bc=315e251a457f83b8738f3c74546d9e54 cc=f8727114951ff7b7163023ff56f60e61
run_build "$OUTDIR/wow_12.0.1.66263_f8727114.txt" "$BFT" wow 315e251a457f83b8738f3c74546d9e54 f8727114951ff7b7163023ff56f60e61 "$CDN" "$CDN_PATH" --paths
# wow 12.0.1.66337 bc=5872d065d94a7d0eb9ad2e05d2053648 cc=fa97703aee2cfd37296150779ccabe1c
run_build "$OUTDIR/wow_12.0.1.66337_fa97703a.txt" "$BFT" wow 5872d065d94a7d0eb9ad2e05d2053648 fa97703aee2cfd37296150779ccabe1c "$CDN" "$CDN_PATH" --paths
# wow 12.0.1.66384 bc=33d46e02ee37d8abb0121c322d319d09 cc=7571386d39d54b031fa8924a5edbd939
run_build "$OUTDIR/wow_12.0.1.66384_7571386d.txt" "$BFT" wow 33d46e02ee37d8abb0121c322d319d09 7571386d39d54b031fa8924a5edbd939 "$CDN" "$CDN_PATH" --paths
# wow 12.0.1.66431 bc=0a5b570b2247a818253b1b72f7f399b2 cc=42382ad6379ce032b4c65dcadb6fe16d
run_build "$OUTDIR/wow_12.0.1.66431_42382ad6.txt" "$BFT" wow 0a5b570b2247a818253b1b72f7f399b2 42382ad6379ce032b4c65dcadb6fe16d "$CDN" "$CDN_PATH" --paths
# wow 12.0.1.66527 bc=1d21a7dc67b33d856e5194ddd2fd9e8a cc=393542519f9f624b288bed93837d2d21
run_build "$OUTDIR/wow_12.0.1.66527_39354251.txt" "$BFT" wow 1d21a7dc67b33d856e5194ddd2fd9e8a 393542519f9f624b288bed93837d2d21 "$CDN" "$CDN_PATH" --paths
# wow 12.0.1.66562 bc=d12eaf54b29d863c4f9f1ff86ac74bb4 cc=8068856ac15d97cae6a5d453ae148cd0
run_build "$OUTDIR/wow_12.0.1.66562_8068856a.txt" "$BFT" wow d12eaf54b29d863c4f9f1ff86ac74bb4 8068856ac15d97cae6a5d453ae148cd0 "$CDN" "$CDN_PATH" --paths
# wow 12.0.1.66666 bc=305b217be0dcecf44765c800bc9e9ac2 cc=60e20617c70e4078f4d725b1f40df310
run_build "$OUTDIR/wow_12.0.1.66666_60e20617.txt" "$BFT" wow 305b217be0dcecf44765c800bc9e9ac2 60e20617c70e4078f4d725b1f40df310 "$CDN" "$CDN_PATH" --paths
# wow 12.0.1.66709 bc=8d07263c0bb34301871c0d3e676d1315 cc=4f9ba92e21e24154f3290b78bf4d6ca5
run_build "$OUTDIR/wow_12.0.1.66709_4f9ba92e.txt" "$BFT" wow 8d07263c0bb34301871c0d3e676d1315 4f9ba92e21e24154f3290b78bf4d6ca5 "$CDN" "$CDN_PATH" --paths
# wow 12.0.1.66838 bc=19ce05dccc68ac27a74a7777046de5ff cc=65867e1b7915ce900bb24ce02b33debc
run_build "$OUTDIR/wow_12.0.1.66838_65867e1b.txt" "$BFT" wow 19ce05dccc68ac27a74a7777046de5ff 65867e1b7915ce900bb24ce02b33debc "$CDN" "$CDN_PATH" --paths
# wow 12.0.5.66741 bc=2580e922537172acc962fdd24661e255 cc=1f2a48ea2ac2c9d1d177a51cd1dfe5ad
run_build "$OUTDIR/wow_12.0.5.66741_1f2a48ea.txt" "$BFT" wow 2580e922537172acc962fdd24661e255 1f2a48ea2ac2c9d1d177a51cd1dfe5ad "$CDN" "$CDN_PATH" --paths
# wow 12.0.5.67088 bc=6f336b1c534faf581ffa97c70946a9ab cc=e6c8a361351272713482a464cddfabec
run_build "$OUTDIR/wow_12.0.5.67088_e6c8a361.txt" "$BFT" wow 6f336b1c534faf581ffa97c70946a9ab e6c8a361351272713482a464cddfabec "$CDN" "$CDN_PATH" --paths
# wow 12.0.5.67114 bc=4b46353093fa851b29c3caaf308884a8 cc=b4564cd752f89b5381b38c56b0f8409c
run_build "$OUTDIR/wow_12.0.5.67114_b4564cd7.txt" "$BFT" wow 4b46353093fa851b29c3caaf308884a8 b4564cd752f89b5381b38c56b0f8409c "$CDN" "$CDN_PATH" --paths
# wow 12.0.5.67165 bc=02482dc9c788698c83e7ae0e24ab2bb7 cc=3c5ad7aaa7b670c73022dbcca72119b1
run_build "$OUTDIR/wow_12.0.5.67165_3c5ad7aa.txt" "$BFT" wow 02482dc9c788698c83e7ae0e24ab2bb7 3c5ad7aaa7b670c73022dbcca72119b1 "$CDN" "$CDN_PATH" --paths
# wow 12.0.5.67186 bc=c08ceb24c8a8b03c663bdd1645d0eb9a cc=0dd3fed23fec6b2cccaaa8ec85d43cfd
run_build "$OUTDIR/wow_12.0.5.67186_0dd3fed2.txt" "$BFT" wow c08ceb24c8a8b03c663bdd1645d0eb9a 0dd3fed23fec6b2cccaaa8ec85d43cfd "$CDN" "$CDN_PATH" --paths
# wow 12.0.5.67235 bc=cdc468119c963a11871d2a566b711070 cc=c470d880d58d9dbc50b2bf71ffe847bc
run_build "$OUTDIR/wow_12.0.5.67235_c470d880.txt" "$BFT" wow cdc468119c963a11871d2a566b711070 c470d880d58d9dbc50b2bf71ffe847bc "$CDN" "$CDN_PATH" --paths
# wow 12.0.5.67314 bc=9ba3185bcf82bcd82ac6d347e97d888d cc=a92e857bec01edc25a1af437cab380bc
run_build "$OUTDIR/wow_12.0.5.67314_a92e857b.txt" "$BFT" wow 9ba3185bcf82bcd82ac6d347e97d888d a92e857bec01edc25a1af437cab380bc "$CDN" "$CDN_PATH" --paths
# wow 12.0.5.67403 bc=11d3219f80f8ffa6f234a6ea55134686 cc=0d9860f7c81c1a6791a612099ccf30e2
run_build "$OUTDIR/wow_12.0.5.67403_0d9860f7.txt" "$BFT" wow 11d3219f80f8ffa6f234a6ea55134686 0d9860f7c81c1a6791a612099ccf30e2 "$CDN" "$CDN_PATH" --paths
# wow 12.0.5.67451 bc=0aacf4f96df375d90c805554d07b61ff cc=1824e670e59e0c6b67be2f7f2296e840
run_build "$OUTDIR/wow_12.0.5.67451_1824e670.txt" "$BFT" wow 0aacf4f96df375d90c805554d07b61ff 1824e670e59e0c6b67be2f7f2296e840 "$CDN" "$CDN_PATH" --paths
# wow 12.0.5.67602 bc=847a33c58a55d0a52d323d1cc6ee7d72 cc=6a9a636d09c782d5ff7f1d960e043d9f
run_build "$OUTDIR/wow_12.0.5.67602_6a9a636d.txt" "$BFT" wow 847a33c58a55d0a52d323d1cc6ee7d72 6a9a636d09c782d5ff7f1d960e043d9f "$CDN" "$CDN_PATH" --paths
# wow 12.0.5.67823 bc=399d19713d9fe33f5c84e6935a515e2d cc=5041b30215d06a0c5403d6af9a9456df
run_build "$OUTDIR/wow_12.0.5.67823_8610593d.txt" "$BFT" wow 399d19713d9fe33f5c84e6935a515e2d 5041b30215d06a0c5403d6af9a9456df "$CDN" "$CDN_PATH" --paths
# wow 12.0.7.67808 bc=29c0b9858abc172f5a6a29377c8493d8 cc=2374bc9b22181d368eab210a0374bdf0
run_build "$OUTDIR/wow_12.0.7.67808_2374bc9b.txt" "$BFT" wow 29c0b9858abc172f5a6a29377c8493d8 2374bc9b22181d368eab210a0374bdf0 "$CDN" "$CDN_PATH" --paths
# wow 12.0.7.68182 bc=f50c48c3afefa874e474e57d2405dd93 cc=fb665ecbea0fb5665e1ca58b30227472
run_build "$OUTDIR/wow_12.0.7.68182_fb665ecb.txt" "$BFT" wow f50c48c3afefa874e474e57d2405dd93 fb665ecbea0fb5665e1ca58b30227472 "$CDN" "$CDN_PATH" --paths
# wow 12.0.7.68232 bc=a4d8fd2b48cfdc71faed5c403a736c68 cc=75f5efa00f27bd95c698f4024c00cb81
run_build "$OUTDIR/wow_12.0.7.68232_75f5efa0.txt" "$BFT" wow a4d8fd2b48cfdc71faed5c403a736c68 75f5efa00f27bd95c698f4024c00cb81 "$CDN" "$CDN_PATH" --paths
# wow 12.0.7.68235 bc=9d40dfb399a5e38d263d301c1162b6e3 cc=c04aae25c788be2d903d099232776718
run_build "$OUTDIR/wow_12.0.7.68235_c04aae25.txt" "$BFT" wow 9d40dfb399a5e38d263d301c1162b6e3 c04aae25c788be2d903d099232776718 "$CDN" "$CDN_PATH" --paths
# wow 12.0.7.68256 bc=03c2b83ef84786c476ab5d6b5696612d cc=080396f7a57021a4a0f4fbf0371c6de0
run_build "$OUTDIR/wow_12.0.7.68256_a84be43d.txt" "$BFT" wow 03c2b83ef84786c476ab5d6b5696612d 080396f7a57021a4a0f4fbf0371c6de0 "$CDN" "$CDN_PATH" --paths
# wow 12.0.7.68275 bc=7d4236927405a1a168e6571ee2a8e00b cc=fccf4c94de71f6d8543a63801ddbf0f2
run_build "$OUTDIR/wow_12.0.7.68275_42eace50.txt" "$BFT" wow 7d4236927405a1a168e6571ee2a8e00b fccf4c94de71f6d8543a63801ddbf0f2 "$CDN" "$CDN_PATH" --paths
# wow 12.0.7.68367 bc=d39191da08bd9b3e9cceaad833b4d5ba cc=bebc96b5cfff6149becf9069d1fbb859
run_build "$OUTDIR/wow_12.0.7.68367_bebc96b5.txt" "$BFT" wow d39191da08bd9b3e9cceaad833b4d5ba bebc96b5cfff6149becf9069d1fbb859 "$CDN" "$CDN_PATH" --paths
# wow 12.0.7.68453 bc=34a1a445ae41066c7f7a6564892d8bdd cc=1deda1a1fd806f80590fc17ccbf4f852
run_build "$OUTDIR/wow_12.0.7.68453_34a1a445.txt" "$BFT" wow 34a1a445ae41066c7f7a6564892d8bdd 1deda1a1fd806f80590fc17ccbf4f852 "$CDN" "$CDN_PATH" --paths

# wow_anniversary 2.5.5.65340 bc=1272ef5be270d8aadda6f213ee152544 cc=dc6b58108dcf80651311523f370a2035
run_build "$OUTDIR/wow_anniversary_2.5.5.65340_dc6b5810.txt" "$BFT" wow_anniversary 1272ef5be270d8aadda6f213ee152544 dc6b58108dcf80651311523f370a2035 "$CDN" "$CDN_PATH" --paths
# wow_anniversary 2.5.5.65417 bc=2ca42cbb1c7d3bf54cef01828988c96d cc=9616b52e1fd3107e1d4bb77e021a0ebb
run_build "$OUTDIR/wow_anniversary_2.5.5.65417_9616b52e.txt" "$BFT" wow_anniversary 2ca42cbb1c7d3bf54cef01828988c96d 9616b52e1fd3107e1d4bb77e021a0ebb "$CDN" "$CDN_PATH" --paths
# wow_anniversary 2.5.5.65463 bc=35659f0227be70c1315c84605fc41e75 cc=a18a6f1a641b729b718c614c8c104171
run_build "$OUTDIR/wow_anniversary_2.5.5.65463_a18a6f1a.txt" "$BFT" wow_anniversary 35659f0227be70c1315c84605fc41e75 a18a6f1a641b729b718c614c8c104171 "$CDN" "$CDN_PATH" --paths
# wow_anniversary 2.5.5.65534 bc=302378abfb48be1ad15b0170adbcf36d cc=9c33a1e8753cdc8c082dacf43854f8fc
run_build "$OUTDIR/wow_anniversary_2.5.5.65534_9c33a1e8.txt" "$BFT" wow_anniversary 302378abfb48be1ad15b0170adbcf36d 9c33a1e8753cdc8c082dacf43854f8fc "$CDN" "$CDN_PATH" --paths
# wow_anniversary 2.5.5.65676 bc=95ffb4f5ffc1ba662c2b97ce590b5990 cc=92c7c3556100e0d6ef51e2ebd96fd38a
run_build "$OUTDIR/wow_anniversary_2.5.5.65676_92c7c355.txt" "$BFT" wow_anniversary 95ffb4f5ffc1ba662c2b97ce590b5990 92c7c3556100e0d6ef51e2ebd96fd38a "$CDN" "$CDN_PATH" --paths
# wow_anniversary 2.5.5.65795 bc=9e5ce1035523a6ee0b98d08ab0258d1e cc=5337e6d142fb9de87c427e5a3d312586
run_build "$OUTDIR/wow_anniversary_2.5.5.65795_5337e6d1.txt" "$BFT" wow_anniversary 9e5ce1035523a6ee0b98d08ab0258d1e 5337e6d142fb9de87c427e5a3d312586 "$CDN" "$CDN_PATH" --paths
# wow_anniversary 2.5.5.65895 bc=df2221c87fa81a64523f02a0b31d9586 cc=a83dcb55a7c076f863610bde509282c8
run_build "$OUTDIR/wow_anniversary_2.5.5.65895_a83dcb55.txt" "$BFT" wow_anniversary df2221c87fa81a64523f02a0b31d9586 a83dcb55a7c076f863610bde509282c8 "$CDN" "$CDN_PATH" --paths
# wow_anniversary 2.5.5.66150 bc=06a0b51e77e1bd237402a4a94c8a38c3 cc=1c3a2b84f07563d78abbef59284ab96a
run_build "$OUTDIR/wow_anniversary_2.5.5.66150_1c3a2b84.txt" "$BFT" wow_anniversary 06a0b51e77e1bd237402a4a94c8a38c3 1c3a2b84f07563d78abbef59284ab96a "$CDN" "$CDN_PATH" --paths
# wow_anniversary 2.5.5.66265 bc=b52c5f018e2f3b0f2b9bff4088365fc2 cc=7a6d7318838b9ef2504931ab6ec9a999
run_build "$OUTDIR/wow_anniversary_2.5.5.66265_7a6d7318.txt" "$BFT" wow_anniversary b52c5f018e2f3b0f2b9bff4088365fc2 7a6d7318838b9ef2504931ab6ec9a999 "$CDN" "$CDN_PATH" --paths
# wow_anniversary 2.5.5.66383 bc=905e422197af095c4861128bf53fc5ad cc=1be92317d62bc1a2523ebd61e985467f
run_build "$OUTDIR/wow_anniversary_2.5.5.66383_1be92317.txt" "$BFT" wow_anniversary 905e422197af095c4861128bf53fc5ad 1be92317d62bc1a2523ebd61e985467f "$CDN" "$CDN_PATH" --paths
# wow_anniversary 2.5.5.66567 bc=00b7b80098077309d9326bebc904daef cc=e96ac31b3e09478731646839347892e4
run_build "$OUTDIR/wow_anniversary_2.5.5.66567_e96ac31b.txt" "$BFT" wow_anniversary 00b7b80098077309d9326bebc904daef e96ac31b3e09478731646839347892e4 "$CDN" "$CDN_PATH" --paths
# wow_anniversary 2.5.5.66765 bc=146490b888d45ad42f8ac5e2de6b7d3d cc=12f22daaa492104d81be26a49793dbed
run_build "$OUTDIR/wow_anniversary_2.5.5.66765_12f22daa.txt" "$BFT" wow_anniversary 146490b888d45ad42f8ac5e2de6b7d3d 12f22daaa492104d81be26a49793dbed "$CDN" "$CDN_PATH" --paths
# wow_anniversary 2.5.5.67157 bc=a96cd2d6b9135ca5e6f53a2927bb83c4 cc=a4ae293c478d89f00ef637d12a0c2e04
run_build "$OUTDIR/wow_anniversary_2.5.5.67157_a4ae293c.txt" "$BFT" wow_anniversary a96cd2d6b9135ca5e6f53a2927bb83c4 a4ae293c478d89f00ef637d12a0c2e04 "$CDN" "$CDN_PATH" --paths
# wow_anniversary 2.5.5.67511 bc=b57fb5113b62c73c4c9ee8b977b91d18 cc=2d8e5f5a523b73403c961ee50e875d70
run_build "$OUTDIR/wow_anniversary_2.5.5.67511_8610593d.txt" "$BFT" wow_anniversary b57fb5113b62c73c4c9ee8b977b91d18 2d8e5f5a523b73403c961ee50e875d70 "$CDN" "$CDN_PATH" --paths
# wow_anniversary 2.5.5.67852 bc=bc5e6071acd67f7a86a2730a1afe3fd9 cc=938e83ee682e94cc47e7bed94c0a4b55
run_build "$OUTDIR/wow_anniversary_2.5.5.67852_938e83ee.txt" "$BFT" wow_anniversary bc5e6071acd67f7a86a2730a1afe3fd9 938e83ee682e94cc47e7bed94c0a4b55 "$CDN" "$CDN_PATH" --paths
# wow_anniversary 2.5.5.68101 bc=bc3b8e61b6ba9f0686029dca74271889 cc=d3d3c884b11a7f550bca82fd0d5a3e40
run_build "$OUTDIR/wow_anniversary_2.5.5.68101_d3d3c884.txt" "$BFT" wow_anniversary bc3b8e61b6ba9f0686029dca74271889 d3d3c884b11a7f550bca82fd0d5a3e40 "$CDN" "$CDN_PATH" --paths
# wow_anniversary 2.5.6.68502 bc=cedfb91c7860b2f5752e38e13c7d9ea5 cc=1deda1a1fd806f80590fc17ccbf4f852
run_build "$OUTDIR/wow_anniversary_2.5.6.68502_2da464eb.txt" "$BFT" wow_anniversary cedfb91c7860b2f5752e38e13c7d9ea5 1deda1a1fd806f80590fc17ccbf4f852 "$CDN" "$CDN_PATH" --paths
# wow_anniversary 2.5.5.68575 bc=7d418167a40670b76b3b2a02c7cb682f cc=38bfbaeb7f6c2a293def5ab2873b7784
run_build "$OUTDIR/wow_anniversary_2.5.5.68575_7d418167.txt" "$BFT" wow_anniversary 7d418167a40670b76b3b2a02c7cb682f 38bfbaeb7f6c2a293def5ab2873b7784 "$CDN" "$CDN_PATH" --paths
# wow_anniversary 2.5.6.68749 bc=2026cf997f1c3df6c0c4a4749690395e cc=cdd540cf71fba841a296f3c6da69ff30
run_build "$OUTDIR/wow_anniversary_2.5.6.68749_2026cf99.txt" "$BFT" wow_anniversary 2026cf997f1c3df6c0c4a4749690395e cdd540cf71fba841a296f3c6da69ff30 "$CDN" "$CDN_PATH" --paths
# wow_anniversary 2.5.6.68775 bc=48733c01faf635d8220af49ac03c206c cc=b20264e873e1b14626ca639483cd2bcc
run_build "$OUTDIR/wow_anniversary_2.5.6.68775_48733c01.txt" "$BFT" wow_anniversary 48733c01faf635d8220af49ac03c206c b20264e873e1b14626ca639483cd2bcc "$CDN" "$CDN_PATH" --paths
# wow_anniversary 2.5.6.68941 bc=a28e58bce063f7ce3051a07a3588bcf1 cc=d7bdbe77b648d19d4a4449c3d2fe7df8
run_build "$OUTDIR/wow_anniversary_2.5.6.68941_a28e58bc.txt" "$BFT" wow_anniversary a28e58bce063f7ce3051a07a3588bcf1 d7bdbe77b648d19d4a4449c3d2fe7df8 "$CDN" "$CDN_PATH" --paths
# wow_anniversary 2.5.6.69110 bc=c5ade9dade89ac36fb4b2d9fd6c09a9a cc=9a824cce21b48ebf0b11367ae32d1597
run_build "$OUTDIR/wow_anniversary_2.5.6.69110_c5ade9da.txt" "$BFT" wow_anniversary c5ade9dade89ac36fb4b2d9fd6c09a9a 9a824cce21b48ebf0b11367ae32d1597 "$CDN" "$CDN_PATH" --paths

# wow_classic 1.13.0.28211 bc=bf24b9d67a4a9c7cc0ce59d63df459a8 cc=2b5b60cdbcd07c5f88c23385069ead40
run_build "$OUTDIR/wow_classic_1.13.0.28211_2b5b60cd.txt" "$BFT" wow_classic bf24b9d67a4a9c7cc0ce59d63df459a8 2b5b60cdbcd07c5f88c23385069ead40 "$CDN" "$CDN_PATH" --paths
# wow_classic 1.13.0.28377 bc=db00c310c6ba0215be3f386264402d56 cc=1e32d08ef668e70aac36a516bd43dff1
run_build "$OUTDIR/wow_classic_1.13.0.28377_1e32d08e.txt" "$BFT" wow_classic db00c310c6ba0215be3f386264402d56 1e32d08ef668e70aac36a516bd43dff1 "$CDN" "$CDN_PATH" --paths
# wow_classic 1.13.2.30786 bc=c8470ae1807bb4f59c1667a6054e6535 cc=3d014cd9e5940b029109685aee932149
run_build "$OUTDIR/wow_classic_1.13.2.30786_3d014cd9.txt" "$BFT" wow_classic c8470ae1807bb4f59c1667a6054e6535 3d014cd9e5940b029109685aee932149 "$CDN" "$CDN_PATH" --paths
# wow_classic 1.13.2.30862 bc=846836d2a8acae42c4fdc7e4382aec67 cc=ae58dd263f346579705f212feefb82c8
run_build "$OUTDIR/wow_classic_1.13.2.30862_ae58dd26.txt" "$BFT" wow_classic 846836d2a8acae42c4fdc7e4382aec67 ae58dd263f346579705f212feefb82c8 "$CDN" "$CDN_PATH" --paths
# wow_classic 1.13.2.31407 bc=a373db049bb20c9b3e1869cd75e35cca cc=0217092a53536772a44d673ecfeb9c22
run_build "$OUTDIR/wow_classic_1.13.2.31407_0217092a.txt" "$BFT" wow_classic a373db049bb20c9b3e1869cd75e35cca 0217092a53536772a44d673ecfeb9c22 "$CDN" "$CDN_PATH" --paths
# wow_classic 1.13.2.31446 bc=a115a2f86cd841f1468468904d8327b3 cc=3043ebe0073cbd28294935b67e4a5896
run_build "$OUTDIR/wow_classic_1.13.2.31446_3043ebe0.txt" "$BFT" wow_classic a115a2f86cd841f1468468904d8327b3 3043ebe0073cbd28294935b67e4a5896 "$CDN" "$CDN_PATH" --paths
# wow_classic 1.13.2.31650 bc=2c915a9a226a3f35af6c65fcc7b6ca4a cc=c54b41b3195b9482ce0d3c6bf0b86cdb
run_build "$OUTDIR/wow_classic_1.13.2.31650_c54b41b3.txt" "$BFT" wow_classic 2c915a9a226a3f35af6c65fcc7b6ca4a c54b41b3195b9482ce0d3c6bf0b86cdb "$CDN" "$CDN_PATH" --paths
# wow_classic 1.13.2.31687 bc=6839bd436f8ec2e2429ff0725e09b63c cc=ba42282b283c46f43b96dc4c3465f321
run_build "$OUTDIR/wow_classic_1.13.2.31687_ba42282b.txt" "$BFT" wow_classic 6839bd436f8ec2e2429ff0725e09b63c ba42282b283c46f43b96dc4c3465f321 "$CDN" "$CDN_PATH" --paths
# wow_classic 1.13.2.31727 bc=0f9b4efb7e262dac08d8e330ffa32126 cc=a75010533695b4ee8d1fa5c13e948cd1
run_build "$OUTDIR/wow_classic_1.13.2.31727_a7501053.txt" "$BFT" wow_classic 0f9b4efb7e262dac08d8e330ffa32126 a75010533695b4ee8d1fa5c13e948cd1 "$CDN" "$CDN_PATH" --paths
# wow_classic 1.13.2.31830 bc=a46865b6b382b963ffe78b3d77f7cfcb cc=3a209c6bbda59a3ae784db6b79b9adfa
run_build "$OUTDIR/wow_classic_1.13.2.31830_3a209c6b.txt" "$BFT" wow_classic a46865b6b382b963ffe78b3d77f7cfcb 3a209c6bbda59a3ae784db6b79b9adfa "$CDN" "$CDN_PATH" --paths
# wow_classic 1.13.2.31882 bc=8a01b815ea1356e7e031aee9e762f99c cc=70dccbfe816129ebd271f34c6d2c65e3
run_build "$OUTDIR/wow_classic_1.13.2.31882_70dccbfe.txt" "$BFT" wow_classic 8a01b815ea1356e7e031aee9e762f99c 70dccbfe816129ebd271f34c6d2c65e3 "$CDN" "$CDN_PATH" --paths
# wow_classic 1.13.2.32089 bc=3635c5789e7688ccfb1742eb626d07b0 cc=e8a2018ced1a890947262afcb256b658
run_build "$OUTDIR/wow_classic_1.13.2.32089_e8a2018c.txt" "$BFT" wow_classic 3635c5789e7688ccfb1742eb626d07b0 e8a2018ced1a890947262afcb256b658 "$CDN" "$CDN_PATH" --paths
# wow_classic 1.13.2.32421 bc=b5a105b40ba80c786884a80058969d33 cc=e9705588a51b6042e81c1dc5919bf037
run_build "$OUTDIR/wow_classic_1.13.2.32421_e9705588.txt" "$BFT" wow_classic b5a105b40ba80c786884a80058969d33 e9705588a51b6042e81c1dc5919bf037 "$CDN" "$CDN_PATH" --paths
# wow_classic 1.13.2.32600 bc=596c212114208f0f849c6b6e596e6680 cc=bf4672a701f0795b21ad63bf6b98ae0a
run_build "$OUTDIR/wow_classic_1.13.2.32600_bf4672a7.txt" "$BFT" wow_classic 596c212114208f0f849c6b6e596e6680 bf4672a701f0795b21ad63bf6b98ae0a "$CDN" "$CDN_PATH" --paths
# wow_classic 1.13.3.32790 bc=eabc7dd92330e4907bc234899dd0cd4b cc=efc95c64488ab6dda10a7f57eca91f19
run_build "$OUTDIR/wow_classic_1.13.3.32790_efc95c64.txt" "$BFT" wow_classic eabc7dd92330e4907bc234899dd0cd4b efc95c64488ab6dda10a7f57eca91f19 "$CDN" "$CDN_PATH" --paths
# wow_classic 1.13.3.32836 bc=d987e36d3d50c61a55ef4a84bae915ec cc=6dd16e012450b105c22d5270bb2bf3ea
run_build "$OUTDIR/wow_classic_1.13.3.32836_6dd16e01.txt" "$BFT" wow_classic d987e36d3d50c61a55ef4a84bae915ec 6dd16e012450b105c22d5270bb2bf3ea "$CDN" "$CDN_PATH" --paths
# wow_classic 1.13.3.32887 bc=289e100e9c14605242193aa351ef16f1 cc=a1737895dbca2a38c6c2d8cfa1253766
run_build "$OUTDIR/wow_classic_1.13.3.32887_a1737895.txt" "$BFT" wow_classic 289e100e9c14605242193aa351ef16f1 a1737895dbca2a38c6c2d8cfa1253766 "$CDN" "$CDN_PATH" --paths
# wow_classic 1.13.3.33155 bc=48dc748014ed1ab293fffe8c624f1924 cc=45902a4a96c071bbb6f0d117faf112e8
run_build "$OUTDIR/wow_classic_1.13.3.33155_45902a4a.txt" "$BFT" wow_classic 48dc748014ed1ab293fffe8c624f1924 45902a4a96c071bbb6f0d117faf112e8 "$CDN" "$CDN_PATH" --paths
# wow_classic 1.13.3.33302 bc=3bc04fd7c5309c47b9f19fb7caec0efc cc=d80a505ffed27dc34e80f06f3b6163e5
run_build "$OUTDIR/wow_classic_1.13.3.33302_d80a505f.txt" "$BFT" wow_classic 3bc04fd7c5309c47b9f19fb7caec0efc d80a505ffed27dc34e80f06f3b6163e5 "$CDN" "$CDN_PATH" --paths
# wow_classic 1.13.3.33526 bc=5bd9e122809e9566dbd353a781cf7324 cc=bd7286c25b285500610be670014c3e48
run_build "$OUTDIR/wow_classic_1.13.3.33526_bd7286c2.txt" "$BFT" wow_classic 5bd9e122809e9566dbd353a781cf7324 bd7286c25b285500610be670014c3e48 "$CDN" "$CDN_PATH" --paths
# wow_classic 1.13.4.33598 bc=2d5c34af4961eeb72f643a5a0c4d4204 cc=4914d14eba83165faf065dc1d46e684e
run_build "$OUTDIR/wow_classic_1.13.4.33598_4914d14e.txt" "$BFT" wow_classic 2d5c34af4961eeb72f643a5a0c4d4204 4914d14eba83165faf065dc1d46e684e "$CDN" "$CDN_PATH" --paths
# wow_classic 1.13.4.33645 bc=eddb723042b8ee7e866af382ecf8ed5f cc=96385859575c86bc9a90da66300ce691
run_build "$OUTDIR/wow_classic_1.13.4.33645_96385859.txt" "$BFT" wow_classic eddb723042b8ee7e866af382ecf8ed5f 96385859575c86bc9a90da66300ce691 "$CDN" "$CDN_PATH" --paths
# wow_classic 1.13.4.33728 bc=4756dfc0367df50312b250070498e024 cc=b35fb2521d53b547e450c9635b98f60d
run_build "$OUTDIR/wow_classic_1.13.4.33728_b35fb252.txt" "$BFT" wow_classic 4756dfc0367df50312b250070498e024 b35fb2521d53b547e450c9635b98f60d "$CDN" "$CDN_PATH" --paths
# wow_classic 1.13.4.33920 bc=2851a35aa1afa1de57044b34b6000116 cc=e3ee09e8f58f57584b124a3bea61374f
run_build "$OUTDIR/wow_classic_1.13.4.33920_e3ee09e8.txt" "$BFT" wow_classic 2851a35aa1afa1de57044b34b6000116 e3ee09e8f58f57584b124a3bea61374f "$CDN" "$CDN_PATH" --paths
# wow_classic 1.13.4.34219 bc=38cfb8f68cca92d6127f4e27a2324006 cc=5187cdfd6fee12b4a0d53003e8249635
run_build "$OUTDIR/wow_classic_1.13.4.34219_5187cdfd.txt" "$BFT" wow_classic 38cfb8f68cca92d6127f4e27a2324006 5187cdfd6fee12b4a0d53003e8249635 "$CDN" "$CDN_PATH" --paths
# wow_classic 1.13.4.34266 bc=f059cc7b8ef018d03343db634734536d cc=4080392e2318cba8480d50fd2b3545ce
run_build "$OUTDIR/wow_classic_1.13.4.34266_4080392e.txt" "$BFT" wow_classic f059cc7b8ef018d03343db634734536d 4080392e2318cba8480d50fd2b3545ce "$CDN" "$CDN_PATH" --paths
# wow_classic 1.13.4.34600 bc=016b016529281241bf712e3c24bf093a cc=a9cff2e39633dbfe1885d3aa6c2805c5
run_build "$OUTDIR/wow_classic_1.13.4.34600_a9cff2e3.txt" "$BFT" wow_classic 016b016529281241bf712e3c24bf093a a9cff2e39633dbfe1885d3aa6c2805c5 "$CDN" "$CDN_PATH" --paths
# wow_classic 1.13.4.34835 bc=77a2fb029d38b05f6325d47e18183429 cc=9905ae5744462f7d237cc15a1c68d6ef
run_build "$OUTDIR/wow_classic_1.13.4.34835_9905ae57.txt" "$BFT" wow_classic 77a2fb029d38b05f6325d47e18183429 9905ae5744462f7d237cc15a1c68d6ef "$CDN" "$CDN_PATH" --paths
# wow_classic 1.13.5.35000 bc=f8a2a2cdb41bdac6c8c145452ded4615 cc=563871c878ef97fe38f9ae4b882c2416
run_build "$OUTDIR/wow_classic_1.13.5.35000_563871c8.txt" "$BFT" wow_classic f8a2a2cdb41bdac6c8c145452ded4615 563871c878ef97fe38f9ae4b882c2416 "$CDN" "$CDN_PATH" --paths
# wow_classic 1.13.5.35186 bc=1747ffa887e10e50b864776a788b367b cc=b9cfc3fe1f5fd897fe490d9e109adeef
run_build "$OUTDIR/wow_classic_1.13.5.35186_b9cfc3fe.txt" "$BFT" wow_classic 1747ffa887e10e50b864776a788b367b b9cfc3fe1f5fd897fe490d9e109adeef "$CDN" "$CDN_PATH" --paths
# wow_classic 1.13.5.35395 bc=fd5ca49bf27105a6c2d4da5a2315c436 cc=4afb7c5ea93d6783825b4ae606524181
run_build "$OUTDIR/wow_classic_1.13.5.35395_4afb7c5e.txt" "$BFT" wow_classic fd5ca49bf27105a6c2d4da5a2315c436 4afb7c5ea93d6783825b4ae606524181 "$CDN" "$CDN_PATH" --paths
# wow_classic 1.13.5.35663 bc=c34ddcfbe5266498198080d5f6d8aca2 cc=675b08548f5f33339ea13d9aa5e0c84d
run_build "$OUTDIR/wow_classic_1.13.5.35663_675b0854.txt" "$BFT" wow_classic c34ddcfbe5266498198080d5f6d8aca2 675b08548f5f33339ea13d9aa5e0c84d "$CDN" "$CDN_PATH" --paths
# wow_classic 1.13.5.35705 bc=4401cdcd14380e0de4c40f0cc2e371ce cc=2fb6dfeb299c59f211f5b177f22e51a3
run_build "$OUTDIR/wow_classic_1.13.5.35705_2fb6dfeb.txt" "$BFT" wow_classic 4401cdcd14380e0de4c40f0cc2e371ce 2fb6dfeb299c59f211f5b177f22e51a3 "$CDN" "$CDN_PATH" --paths
# wow_classic 1.13.5.35753 bc=c51ade9bbee175fa364400b800cf0471 cc=68827db24917a18eff6fce153848f22f
run_build "$OUTDIR/wow_classic_1.13.5.35753_68827db2.txt" "$BFT" wow_classic c51ade9bbee175fa364400b800cf0471 68827db24917a18eff6fce153848f22f "$CDN" "$CDN_PATH" --paths
# wow_classic 1.13.5.36035 bc=93715636d14629a1d35c84d3975c2452 cc=a109313184f9ad3d36e8e6f9f33f7463
run_build "$OUTDIR/wow_classic_1.13.5.36035_a1093131.txt" "$BFT" wow_classic 93715636d14629a1d35c84d3975c2452 a109313184f9ad3d36e8e6f9f33f7463 "$CDN" "$CDN_PATH" --paths
# wow_classic 1.13.5.36307 bc=8a43830b0e3606268651e61b9a1cf9e4 cc=7c7549619617220b8bd452b90c19e85f
run_build "$OUTDIR/wow_classic_1.13.5.36307_7c754961.txt" "$BFT" wow_classic 8a43830b0e3606268651e61b9a1cf9e4 7c7549619617220b8bd452b90c19e85f "$CDN" "$CDN_PATH" --paths
# wow_classic 1.13.5.36325 bc=705f74266bd6cdd073bcd6999db5a36c cc=a887a76c0ec1db77a64b34ccb75a98f6
run_build "$OUTDIR/wow_classic_1.13.5.36325_a887a76c.txt" "$BFT" wow_classic 705f74266bd6cdd073bcd6999db5a36c a887a76c0ec1db77a64b34ccb75a98f6 "$CDN" "$CDN_PATH" --paths
# wow_classic 1.13.6.36714 bc=6925a54e477975408ae5a154388bc895 cc=ba51606bc8fc3067856b01f27121835d
run_build "$OUTDIR/wow_classic_1.13.6.36714_ba51606b.txt" "$BFT" wow_classic 6925a54e477975408ae5a154388bc895 ba51606bc8fc3067856b01f27121835d "$CDN" "$CDN_PATH" --paths
# wow_classic 1.13.6.36935 bc=9ad6ad5306deb8eed364b64cc628ac98 cc=7786e3be6cfec81537c55d3fc669d371
run_build "$OUTDIR/wow_classic_1.13.6.36935_7786e3be.txt" "$BFT" wow_classic 9ad6ad5306deb8eed364b64cc628ac98 7786e3be6cfec81537c55d3fc669d371 "$CDN" "$CDN_PATH" --paths
# wow_classic 1.13.6.37497 bc=3f54383a77a7c4774335d74b7e8e8f56 cc=af2693f26cd0ead8fc82687a4186b62e
run_build "$OUTDIR/wow_classic_1.13.6.37497_af2693f2.txt" "$BFT" wow_classic 3f54383a77a7c4774335d74b7e8e8f56 af2693f26cd0ead8fc82687a4186b62e "$CDN" "$CDN_PATH" --paths
# wow_classic 1.13.7.38363 bc=e17045a2b2fa5288054e87a4868e58a4 cc=0263db91f386bf94ebdcf955d23b397c
run_build "$OUTDIR/wow_classic_1.13.7.38363_0263db91.txt" "$BFT" wow_classic e17045a2b2fa5288054e87a4868e58a4 0263db91f386bf94ebdcf955d23b397c "$CDN" "$CDN_PATH" --paths
# wow_classic 1.13.7.38386 bc=fea9e027366e9e85a4c954bf3f310899 cc=f0bebd4529d6e42ef2f0add948b9de8d
run_build "$OUTDIR/wow_classic_1.13.7.38386_f0bebd45.txt" "$BFT" wow_classic fea9e027366e9e85a4c954bf3f310899 f0bebd4529d6e42ef2f0add948b9de8d "$CDN" "$CDN_PATH" --paths
# wow_classic 1.13.7.38475 bc=dbd4a8b2733f619e64ec356d461198a5 cc=9c652476248f2eb83cf2be7d79aefacb
run_build "$OUTDIR/wow_classic_1.13.7.38475_9c652476.txt" "$BFT" wow_classic dbd4a8b2733f619e64ec356d461198a5 9c652476248f2eb83cf2be7d79aefacb "$CDN" "$CDN_PATH" --paths
# wow_classic 1.13.7.38631 bc=4ffc9fd8dd2bf6a604313908898aa78c cc=9cf97a7504d69ef3e25a843244d9efcd
run_build "$OUTDIR/wow_classic_1.13.7.38631_9cf97a75.txt" "$BFT" wow_classic 4ffc9fd8dd2bf6a604313908898aa78c 9cf97a7504d69ef3e25a843244d9efcd "$CDN" "$CDN_PATH" --paths

# wow_classic 2.5.1.38644 bc=90271d6f835fb8c3d51da743e5da8deb cc=724436c9aec9beeb91a8cb914bb921a0
run_build "$OUTDIR/wow_classic_2.5.1.38644_724436c9.txt" "$BFT" wow_classic 90271d6f835fb8c3d51da743e5da8deb 724436c9aec9beeb91a8cb914bb921a0 "$CDN" "$CDN_PATH" --paths
# wow_classic 2.5.1.38707 bc=c82d84c9d9fbeecacb57af9047097bbc cc=572649a9eda7c06a42b37858d27fbc0f
run_build "$OUTDIR/wow_classic_2.5.1.38707_572649a9.txt" "$BFT" wow_classic c82d84c9d9fbeecacb57af9047097bbc 572649a9eda7c06a42b37858d27fbc0f "$CDN" "$CDN_PATH" --paths
# wow_classic 2.5.1.38741 bc=e743cf55beffb0d65c5c91f45269ebe3 cc=deec22c4460e2da5d992f7946ee26b6e
run_build "$OUTDIR/wow_classic_2.5.1.38741_deec22c4.txt" "$BFT" wow_classic e743cf55beffb0d65c5c91f45269ebe3 deec22c4460e2da5d992f7946ee26b6e "$CDN" "$CDN_PATH" --paths
# wow_classic 2.5.1.38757 bc=7c65688b4cec96ce0a998d42512a0b42 cc=03c9d6508a5ade0878581e7f40dc355b
run_build "$OUTDIR/wow_classic_2.5.1.38757_03c9d650.txt" "$BFT" wow_classic 7c65688b4cec96ce0a998d42512a0b42 03c9d6508a5ade0878581e7f40dc355b "$CDN" "$CDN_PATH" --paths
# wow_classic 2.5.1.38835 bc=402cedb98d828c98656a55f058007aa9 cc=64f8d1850e94e3de64ef5ff18bfdb86c
run_build "$OUTDIR/wow_classic_2.5.1.38835_64f8d185.txt" "$BFT" wow_classic 402cedb98d828c98656a55f058007aa9 64f8d1850e94e3de64ef5ff18bfdb86c "$CDN" "$CDN_PATH" --paths
# wow_classic 2.5.1.38892 bc=c6ca8bd03db9d3f7484337538fbb74a7 cc=d4b8b6ebee2f9ebfefb748e036ca05d0
run_build "$OUTDIR/wow_classic_2.5.1.38892_d4b8b6eb.txt" "$BFT" wow_classic c6ca8bd03db9d3f7484337538fbb74a7 d4b8b6ebee2f9ebfefb748e036ca05d0 "$CDN" "$CDN_PATH" --paths
# wow_classic 2.5.1.38921 bc=ed58058032894e9b52c430ba4fd8644e cc=e5c88608adf9713525f4e1656781f358
run_build "$OUTDIR/wow_classic_2.5.1.38921_e5c88608.txt" "$BFT" wow_classic ed58058032894e9b52c430ba4fd8644e e5c88608adf9713525f4e1656781f358 "$CDN" "$CDN_PATH" --paths
# wow_classic 2.5.1.38988 bc=c1f9b166a584be66939897dc75a6ab6a cc=fa23f7133af2fc4119a3b09c3087b513
run_build "$OUTDIR/wow_classic_2.5.1.38988_fa23f713.txt" "$BFT" wow_classic c1f9b166a584be66939897dc75a6ab6a fa23f7133af2fc4119a3b09c3087b513 "$CDN" "$CDN_PATH" --paths
# wow_classic 2.5.1.39170 bc=679eb6982267ccd29c79c90af20c1a8b cc=e0998d9603415a7dab6f6e2ed06db2e9
run_build "$OUTDIR/wow_classic_2.5.1.39170_e0998d96.txt" "$BFT" wow_classic 679eb6982267ccd29c79c90af20c1a8b e0998d9603415a7dab6f6e2ed06db2e9 "$CDN" "$CDN_PATH" --paths
# wow_classic 2.5.1.39399 bc=ce7d698c2dc8a080c8e7a909ee325283 cc=1491e29ba9521d1f9ff0497d3ce0044d
run_build "$OUTDIR/wow_classic_2.5.1.39399_1491e29b.txt" "$BFT" wow_classic ce7d698c2dc8a080c8e7a909ee325283 1491e29ba9521d1f9ff0497d3ce0044d "$CDN" "$CDN_PATH" --paths
# wow_classic 2.5.1.39475 bc=6e69ab2116145832f45b38b1d6062787 cc=722842e5ee93d2a68b2f63b67983dac6
run_build "$OUTDIR/wow_classic_2.5.1.39475_722842e5.txt" "$BFT" wow_classic 6e69ab2116145832f45b38b1d6062787 722842e5ee93d2a68b2f63b67983dac6 "$CDN" "$CDN_PATH" --paths
# wow_classic 2.5.1.39603 bc=fc97b4af5374757038e90fa40f9065ef cc=aea89f4edbafc0736170edb663580849
run_build "$OUTDIR/wow_classic_2.5.1.39603_aea89f4e.txt" "$BFT" wow_classic fc97b4af5374757038e90fa40f9065ef aea89f4edbafc0736170edb663580849 "$CDN" "$CDN_PATH" --paths
# wow_classic 2.5.1.39640 bc=251a3cf09d8ced83c124db6b0d68df22 cc=c762995160b102a3e0dcb72caa753542
run_build "$OUTDIR/wow_classic_2.5.1.39640_c7629951.txt" "$BFT" wow_classic 251a3cf09d8ced83c124db6b0d68df22 c762995160b102a3e0dcb72caa753542 "$CDN" "$CDN_PATH" --paths
# wow_classic 2.5.2.39926 bc=ada048d6e10b2eac12aef5a923a0d933 cc=949ed913868f1359c03831a224163498
run_build "$OUTDIR/wow_classic_2.5.2.39926_949ed913.txt" "$BFT" wow_classic ada048d6e10b2eac12aef5a923a0d933 949ed913868f1359c03831a224163498 "$CDN" "$CDN_PATH" --paths
# wow_classic 2.5.2.40011 bc=8f17221edbf666589478eb514a3320cd cc=3d7c398e6fe5ae76e209710917906de6
run_build "$OUTDIR/wow_classic_2.5.2.40011_3d7c398e.txt" "$BFT" wow_classic 8f17221edbf666589478eb514a3320cd 3d7c398e6fe5ae76e209710917906de6 "$CDN" "$CDN_PATH" --paths
# wow_classic 2.5.2.40045 bc=2bcc112c25343d288e492d800084f74e cc=e39e543a7c896e1ea1e60e7d867db3ca
run_build "$OUTDIR/wow_classic_2.5.2.40045_e39e543a.txt" "$BFT" wow_classic 2bcc112c25343d288e492d800084f74e e39e543a7c896e1ea1e60e7d867db3ca "$CDN" "$CDN_PATH" --paths
# wow_classic 2.5.2.40203 bc=cedae01d48a7b3a4f2927cd3b08ba2d8 cc=d478674a248a8e086ab48763b89e21d1
run_build "$OUTDIR/wow_classic_2.5.2.40203_d478674a.txt" "$BFT" wow_classic cedae01d48a7b3a4f2927cd3b08ba2d8 d478674a248a8e086ab48763b89e21d1 "$CDN" "$CDN_PATH" --paths
# wow_classic 2.5.2.40260 bc=9d6841932e88d05fe32fd61d0dfb6704 cc=680aa51c29bda88abcb044081b9991b2
run_build "$OUTDIR/wow_classic_2.5.2.40260_680aa51c.txt" "$BFT" wow_classic 9d6841932e88d05fe32fd61d0dfb6704 680aa51c29bda88abcb044081b9991b2 "$CDN" "$CDN_PATH" --paths
# wow_classic 2.5.2.40422 bc=acc8481ab6f540dfcd80a9443013b886 cc=be79670e7d6a0c8c39185f1340afa5db
run_build "$OUTDIR/wow_classic_2.5.2.40422_be79670e.txt" "$BFT" wow_classic acc8481ab6f540dfcd80a9443013b886 be79670e7d6a0c8c39185f1340afa5db "$CDN" "$CDN_PATH" --paths
# wow_classic 2.5.2.40488 bc=b9ba132c4efb3e1c5f996350cd0e06c6 cc=8286723107c4737cd1f0c50194da68f6
run_build "$OUTDIR/wow_classic_2.5.2.40488_82867231.txt" "$BFT" wow_classic b9ba132c4efb3e1c5f996350cd0e06c6 8286723107c4737cd1f0c50194da68f6 "$CDN" "$CDN_PATH" --paths
# wow_classic 2.5.2.40617 bc=5928b4e88c654823bd6f64fdc6bea422 cc=5857563ff3292e357698b8b3c6e8b7aa
run_build "$OUTDIR/wow_classic_2.5.2.40617_5857563f.txt" "$BFT" wow_classic 5928b4e88c654823bd6f64fdc6bea422 5857563ff3292e357698b8b3c6e8b7aa "$CDN" "$CDN_PATH" --paths
# wow_classic 2.5.2.40892 bc=45f53cdda050261fc3f6a5ee2aa3024d cc=7572f8ba3c63a4e0a574352eae2514a2
run_build "$OUTDIR/wow_classic_2.5.2.40892_7572f8ba.txt" "$BFT" wow_classic 45f53cdda050261fc3f6a5ee2aa3024d 7572f8ba3c63a4e0a574352eae2514a2 "$CDN" "$CDN_PATH" --paths
# wow_classic 2.5.2.41446 bc=2ea7bad9af7ae6efc5a707d2af88f32e cc=5742a739db4e9ba1303de863624d5f00
run_build "$OUTDIR/wow_classic_2.5.2.41446_5742a739.txt" "$BFT" wow_classic 2ea7bad9af7ae6efc5a707d2af88f32e 5742a739db4e9ba1303de863624d5f00 "$CDN" "$CDN_PATH" --paths
# wow_classic 2.5.2.41510 bc=e3a89d90c7ef7cbe8c538cf62da1901f cc=418a6ad82131fbbd12a5da0839bd230a
run_build "$OUTDIR/wow_classic_2.5.2.41510_418a6ad8.txt" "$BFT" wow_classic e3a89d90c7ef7cbe8c538cf62da1901f 418a6ad82131fbbd12a5da0839bd230a "$CDN" "$CDN_PATH" --paths
# wow_classic 2.5.3.41812 bc=446ac0a9ed6cc494088d16f7a5af899d cc=01daa9d765f20eb05e460ff21b15547f
run_build "$OUTDIR/wow_classic_2.5.3.41812_01daa9d7.txt" "$BFT" wow_classic 446ac0a9ed6cc494088d16f7a5af899d 01daa9d765f20eb05e460ff21b15547f "$CDN" "$CDN_PATH" --paths
# wow_classic 2.5.3.42083 bc=48c70b711ce7bd99605a70c9972980e5 cc=bb40f66ed6dc06d1842d17ede769c8cb
run_build "$OUTDIR/wow_classic_2.5.3.42083_bb40f66e.txt" "$BFT" wow_classic 48c70b711ce7bd99605a70c9972980e5 bb40f66ed6dc06d1842d17ede769c8cb "$CDN" "$CDN_PATH" --paths
# wow_classic 2.5.3.42328 bc=0113033a5771dabbe6b8f44916b57dd9 cc=e3e07b9731fd7a5dd17612c4571db34a
run_build "$OUTDIR/wow_classic_2.5.3.42328_e3e07b97.txt" "$BFT" wow_classic 0113033a5771dabbe6b8f44916b57dd9 e3e07b9731fd7a5dd17612c4571db34a "$CDN" "$CDN_PATH" --paths
# wow_classic 2.5.3.42598 bc=50fddf2df6545dd9196698d65693cab9 cc=73ecc8385c2f940c1e181aa86feacc84
run_build "$OUTDIR/wow_classic_2.5.3.42598_73ecc838.txt" "$BFT" wow_classic 50fddf2df6545dd9196698d65693cab9 73ecc8385c2f940c1e181aa86feacc84 "$CDN" "$CDN_PATH" --paths
# wow_classic 2.5.4.42800 bc=5e4ebf4bba2d91cb375f7049f08f49e0 cc=4687aa0b13549bab7860f89bd8588300
run_build "$OUTDIR/wow_classic_2.5.4.42800_4687aa0b.txt" "$BFT" wow_classic 5e4ebf4bba2d91cb375f7049f08f49e0 4687aa0b13549bab7860f89bd8588300 "$CDN" "$CDN_PATH" --paths
# wow_classic 2.5.4.42869 bc=a5bc686959b38cae22fda9407bffc018 cc=67ca3d80b26230688056aab1201bf277
run_build "$OUTDIR/wow_classic_2.5.4.42869_67ca3d80.txt" "$BFT" wow_classic a5bc686959b38cae22fda9407bffc018 67ca3d80b26230688056aab1201bf277 "$CDN" "$CDN_PATH" --paths
# wow_classic 2.5.4.42873 bc=4ab40d711cfb3d34130867be70e7f183 cc=2b459e96d2b183c4dd362821e4bc450b
run_build "$OUTDIR/wow_classic_2.5.4.42873_2b459e96.txt" "$BFT" wow_classic 4ab40d711cfb3d34130867be70e7f183 2b459e96d2b183c4dd362821e4bc450b "$CDN" "$CDN_PATH" --paths
# wow_classic 2.5.4.42917 bc=51910fa59cd53b1fe24c75c955b295ec cc=ab17bdeff4abb1ce76074cf67b5bafea
run_build "$OUTDIR/wow_classic_2.5.4.42917_ab17bdef.txt" "$BFT" wow_classic 51910fa59cd53b1fe24c75c955b295ec ab17bdeff4abb1ce76074cf67b5bafea "$CDN" "$CDN_PATH" --paths
# wow_classic 2.5.4.42940 bc=93de275c18e6cbfcfb3c5d0252a4e344 cc=c8859e7baf58af3bf9f7abb539df4c1e
run_build "$OUTDIR/wow_classic_2.5.4.42940_c8859e7b.txt" "$BFT" wow_classic 93de275c18e6cbfcfb3c5d0252a4e344 c8859e7baf58af3bf9f7abb539df4c1e "$CDN" "$CDN_PATH" --paths
# wow_classic 2.5.4.43400 bc=0b7b2f5c0f0360e8dfa1417be6e5ab91 cc=a2d2e69de54362e51cf4ad375bd98ce3
run_build "$OUTDIR/wow_classic_2.5.4.43400_a2d2e69d.txt" "$BFT" wow_classic 0b7b2f5c0f0360e8dfa1417be6e5ab91 a2d2e69de54362e51cf4ad375bd98ce3 "$CDN" "$CDN_PATH" --paths
# wow_classic 2.5.4.43638 bc=5ef2dd3f9e8c2e3b345bb168ff67e25f cc=693c8b74c62f28e2d6fd46fff94b3a8c
run_build "$OUTDIR/wow_classic_2.5.4.43638_693c8b74.txt" "$BFT" wow_classic 5ef2dd3f9e8c2e3b345bb168ff67e25f 693c8b74c62f28e2d6fd46fff94b3a8c "$CDN" "$CDN_PATH" --paths
# wow_classic 2.5.4.43861 bc=0d45748f4fdce3009fc26a537525b488 cc=1f116ce7b1dd5869e8157adff97a5667
run_build "$OUTDIR/wow_classic_2.5.4.43861_1f116ce7.txt" "$BFT" wow_classic 0d45748f4fdce3009fc26a537525b488 1f116ce7b1dd5869e8157adff97a5667 "$CDN" "$CDN_PATH" --paths
# wow_classic 2.5.4.44036 bc=bf18514fffcd9d69b5be0a186c269c12 cc=bd3f6a2ab20a1bce35c3793a16c7c809
run_build "$OUTDIR/wow_classic_2.5.4.44036_bd3f6a2a.txt" "$BFT" wow_classic bf18514fffcd9d69b5be0a186c269c12 bd3f6a2ab20a1bce35c3793a16c7c809 "$CDN" "$CDN_PATH" --paths
# wow_classic 2.5.4.44171 bc=f97421719a696b5a17f760dc4a53bc5e cc=d48916ffaa8b444a809b1365a878bae9
run_build "$OUTDIR/wow_classic_2.5.4.44171_d48916ff.txt" "$BFT" wow_classic f97421719a696b5a17f760dc4a53bc5e d48916ffaa8b444a809b1365a878bae9 "$CDN" "$CDN_PATH" --paths
# wow_classic 2.5.4.44400 bc=1df86b8224eea3d8642ccc1fdd0e4afd cc=697364f7d3f80f13d382e998169372a4
run_build "$OUTDIR/wow_classic_2.5.4.44400_697364f7.txt" "$BFT" wow_classic 1df86b8224eea3d8642ccc1fdd0e4afd 697364f7d3f80f13d382e998169372a4 "$CDN" "$CDN_PATH" --paths
# wow_classic 2.5.4.44833 bc=6c37f5836b397404c64887f638732142 cc=5325d98983238fc35126a296fa2550d2
run_build "$OUTDIR/wow_classic_2.5.4.44833_5325d989.txt" "$BFT" wow_classic 6c37f5836b397404c64887f638732142 5325d98983238fc35126a296fa2550d2 "$CDN" "$CDN_PATH" --paths

# wow_classic 3.4.0.45327 bc=4c101cabe237a8c9d9ccfcb31b57078a cc=add5da5845864f99b02a6d669f7d284f
run_build "$OUTDIR/wow_classic_3.4.0.45327_add5da58.txt" "$BFT" wow_classic 4c101cabe237a8c9d9ccfcb31b57078a add5da5845864f99b02a6d669f7d284f "$CDN" "$CDN_PATH" --paths
# wow_classic 3.4.0.45435 bc=0f30de7bdb3f5ae9c9e8d06c942cf722 cc=6e09dac6f08fa0ee968a46ba99f6971e
run_build "$OUTDIR/wow_classic_3.4.0.45435_6e09dac6.txt" "$BFT" wow_classic 0f30de7bdb3f5ae9c9e8d06c942cf722 6e09dac6f08fa0ee968a46ba99f6971e "$CDN" "$CDN_PATH" --paths
# wow_classic 3.4.0.45506 bc=7b606ae3edb590718ee76e50ba072524 cc=f3d58fc8003bdc51bc4747699165937b
run_build "$OUTDIR/wow_classic_3.4.0.45506_f3d58fc8.txt" "$BFT" wow_classic 7b606ae3edb590718ee76e50ba072524 f3d58fc8003bdc51bc4747699165937b "$CDN" "$CDN_PATH" --paths
# wow_classic 3.4.0.45572 bc=3bcc4eed202d4e56b9a4ead2ff402913 cc=134fe178c5cb6224d8efdffe25610728
run_build "$OUTDIR/wow_classic_3.4.0.45572_134fe178.txt" "$BFT" wow_classic 3bcc4eed202d4e56b9a4ead2ff402913 134fe178c5cb6224d8efdffe25610728 "$CDN" "$CDN_PATH" --paths
# wow_classic 3.4.0.45613 bc=e9195207b0e3db82b2ed2fc4330c1d39 cc=2e10b923bf4bb1829f54052b0889258a
run_build "$OUTDIR/wow_classic_3.4.0.45613_2e10b923.txt" "$BFT" wow_classic e9195207b0e3db82b2ed2fc4330c1d39 2e10b923bf4bb1829f54052b0889258a "$CDN" "$CDN_PATH" --paths
# wow_classic 3.4.0.45704 bc=afee0f18d9b3a9f20722b3be8bffe996 cc=daa2ca151a4750c011383863a5ffabb3
run_build "$OUTDIR/wow_classic_3.4.0.45704_daa2ca15.txt" "$BFT" wow_classic afee0f18d9b3a9f20722b3be8bffe996 daa2ca151a4750c011383863a5ffabb3 "$CDN" "$CDN_PATH" --paths
# wow_classic 3.4.0.45770 bc=a52ae41cf5dd2ae908aceba88dc63719 cc=45a2fd9a600c6aa65c6d7940c3bfe5b3
run_build "$OUTDIR/wow_classic_3.4.0.45770_45a2fd9a.txt" "$BFT" wow_classic a52ae41cf5dd2ae908aceba88dc63719 45a2fd9a600c6aa65c6d7940c3bfe5b3 "$CDN" "$CDN_PATH" --paths
# wow_classic 3.4.0.45772 bc=0f21c38ceba8940ab36e340d6a426c25 cc=c6ebd647eb8f696ba02959d625e9193e
run_build "$OUTDIR/wow_classic_3.4.0.45772_c6ebd647.txt" "$BFT" wow_classic 0f21c38ceba8940ab36e340d6a426c25 c6ebd647eb8f696ba02959d625e9193e "$CDN" "$CDN_PATH" --paths
# wow_classic 3.4.0.45854 bc=986f449d71bef2e27eab1cb756da9485 cc=b26e3c5fb0ab2e05842d64a3819c3046
run_build "$OUTDIR/wow_classic_3.4.0.45854_b26e3c5f.txt" "$BFT" wow_classic 986f449d71bef2e27eab1cb756da9485 b26e3c5fb0ab2e05842d64a3819c3046 "$CDN" "$CDN_PATH" --paths
# wow_classic 3.4.0.45942 bc=8759a464510048f0489e9fa12c149f48 cc=44834d4510189d882ac08ae2523b944b
run_build "$OUTDIR/wow_classic_3.4.0.45942_44834d45.txt" "$BFT" wow_classic 8759a464510048f0489e9fa12c149f48 44834d4510189d882ac08ae2523b944b "$CDN" "$CDN_PATH" --paths
# wow_classic 3.4.0.46158 bc=cc0e8579280ef793d03f804eee90dd6e cc=cbc4f682e8b3f70d6be519bdd997c334
run_build "$OUTDIR/wow_classic_3.4.0.46158_cbc4f682.txt" "$BFT" wow_classic cc0e8579280ef793d03f804eee90dd6e cbc4f682e8b3f70d6be519bdd997c334 "$CDN" "$CDN_PATH" --paths
# wow_classic 3.4.0.46182 bc=348fc4f8b9036396a2e71a4d2968b274 cc=9a87d61e68fcc0a86bfd5e7f515eec96
run_build "$OUTDIR/wow_classic_3.4.0.46182_9a87d61e.txt" "$BFT" wow_classic 348fc4f8b9036396a2e71a4d2968b274 9a87d61e68fcc0a86bfd5e7f515eec96 "$CDN" "$CDN_PATH" --paths
# wow_classic 3.4.0.46248 bc=61038bc2516536bba0d79e1cf410210e cc=4b33f4b0254c0ce40ac4225fe20267e2
run_build "$OUTDIR/wow_classic_3.4.0.46248_4b33f4b0.txt" "$BFT" wow_classic 61038bc2516536bba0d79e1cf410210e 4b33f4b0254c0ce40ac4225fe20267e2 "$CDN" "$CDN_PATH" --paths
# wow_classic 3.4.0.46368 bc=d47185815ae55b7f7a638ca0929a78db cc=59b5fbed4f9c1c5b9e1947566e3c1edb
run_build "$OUTDIR/wow_classic_3.4.0.46368_59b5fbed.txt" "$BFT" wow_classic d47185815ae55b7f7a638ca0929a78db 59b5fbed4f9c1c5b9e1947566e3c1edb "$CDN" "$CDN_PATH" --paths
# wow_classic 3.4.0.46779 bc=0985695c350c9a4ac280c394bc2f0d37 cc=48410e3d71c37a06869c7645f167c5d2
run_build "$OUTDIR/wow_classic_3.4.0.46779_48410e3d.txt" "$BFT" wow_classic 0985695c350c9a4ac280c394bc2f0d37 48410e3d71c37a06869c7645f167c5d2 "$CDN" "$CDN_PATH" --paths
# wow_classic 3.4.0.46902 bc=916510894a1c7dbed1735344036dfaf0 cc=ffd557aa536f1dd0e8b9d965bff68eae
run_build "$OUTDIR/wow_classic_3.4.0.46902_ffd557aa.txt" "$BFT" wow_classic 916510894a1c7dbed1735344036dfaf0 ffd557aa536f1dd0e8b9d965bff68eae "$CDN" "$CDN_PATH" --paths
# wow_classic 3.4.0.47168 bc=ebe6bd0f586f7830d10fb37afc00c610 cc=78f58fb36f3ec730105a343a8cf9e230
run_build "$OUTDIR/wow_classic_3.4.0.47168_78f58fb3.txt" "$BFT" wow_classic ebe6bd0f586f7830d10fb37afc00c610 78f58fb36f3ec730105a343a8cf9e230 "$CDN" "$CDN_PATH" --paths
# wow_classic 3.4.0.47659 bc=e1d98cc146dfbec5a8430fc2b83c836e cc=abfa67a0d44050074be13d0184d98690
run_build "$OUTDIR/wow_classic_3.4.0.47659_abfa67a0.txt" "$BFT" wow_classic e1d98cc146dfbec5a8430fc2b83c836e abfa67a0d44050074be13d0184d98690 "$CDN" "$CDN_PATH" --paths
# wow_classic 3.4.1.47612 bc=575cca332754f31148657d52c6ff9b04 cc=abfa67a0d44050074be13d0184d98690
run_build "$OUTDIR/wow_classic_3.4.1.47612_abfa67a0.txt" "$BFT" wow_classic 575cca332754f31148657d52c6ff9b04 abfa67a0d44050074be13d0184d98690 "$CDN" "$CDN_PATH" --paths
# wow_classic 3.4.1.47720 bc=3b30bb346b25af52d51d6ed3d48601f2 cc=7e2b3989ef6ff08a7a25736aba82906f
run_build "$OUTDIR/wow_classic_3.4.1.47720_7e2b3989.txt" "$BFT" wow_classic 3b30bb346b25af52d51d6ed3d48601f2 7e2b3989ef6ff08a7a25736aba82906f "$CDN" "$CDN_PATH" --paths
# wow_classic 3.4.1.47800 bc=82ffc4eeef3cf31a554808a9c328713d cc=5205c30b720424ad1007d3fc7c056073
run_build "$OUTDIR/wow_classic_3.4.1.47800_5205c30b.txt" "$BFT" wow_classic 82ffc4eeef3cf31a554808a9c328713d 5205c30b720424ad1007d3fc7c056073 "$CDN" "$CDN_PATH" --paths
# wow_classic 3.4.1.47966 bc=237c264cb5a547cad396a1c067cc1dcf cc=fa134856ec2038df9f79efd25b1b2cde
run_build "$OUTDIR/wow_classic_3.4.1.47966_fa134856.txt" "$BFT" wow_classic 237c264cb5a547cad396a1c067cc1dcf fa134856ec2038df9f79efd25b1b2cde "$CDN" "$CDN_PATH" --paths
# wow_classic 3.4.1.48019 bc=92123063f2e8a1791f4b69056ac77ffc cc=db9bedd87b158160766aa2eace1e5046
run_build "$OUTDIR/wow_classic_3.4.1.48019_db9bedd8.txt" "$BFT" wow_classic 92123063f2e8a1791f4b69056ac77ffc db9bedd87b158160766aa2eace1e5046 "$CDN" "$CDN_PATH" --paths
# wow_classic 3.4.1.48120 bc=1c771ef0b2489471821e4dba85da9d25 cc=6a04fbc0a89cc692f39431c8c0f93404
run_build "$OUTDIR/wow_classic_3.4.1.48120_6a04fbc0.txt" "$BFT" wow_classic 1c771ef0b2489471821e4dba85da9d25 6a04fbc0a89cc692f39431c8c0f93404 "$CDN" "$CDN_PATH" --paths
# wow_classic 3.4.1.48340 bc=a57c0078be5af4493ff8428d84f32d5d cc=29289ea1234e55e6cab4ea011f2f4eef
run_build "$OUTDIR/wow_classic_3.4.1.48340_29289ea1.txt" "$BFT" wow_classic a57c0078be5af4493ff8428d84f32d5d 29289ea1234e55e6cab4ea011f2f4eef "$CDN" "$CDN_PATH" --paths
# wow_classic 3.4.1.48503 bc=5cb9b1527dc350f5687ea49bd56c0bd2 cc=60f1a3413979e1d22f55b3c23f4d6ebd
run_build "$OUTDIR/wow_classic_3.4.1.48503_60f1a341.txt" "$BFT" wow_classic 5cb9b1527dc350f5687ea49bd56c0bd2 60f1a3413979e1d22f55b3c23f4d6ebd "$CDN" "$CDN_PATH" --paths
# wow_classic 3.4.1.48632 bc=e5a4f6341ada2f482a75360649303add cc=cee2ca2d6bc7356398e0b44fc15ed317
run_build "$OUTDIR/wow_classic_3.4.1.48632_cee2ca2d.txt" "$BFT" wow_classic e5a4f6341ada2f482a75360649303add cee2ca2d6bc7356398e0b44fc15ed317 "$CDN" "$CDN_PATH" --paths
# wow_classic 3.4.1.49345 bc=b5941573a8b317c5765f041fb9cfb4cb cc=f41161c161e6c9605831ce031791d209
run_build "$OUTDIR/wow_classic_3.4.1.49345_f41161c1.txt" "$BFT" wow_classic b5941573a8b317c5765f041fb9cfb4cb f41161c161e6c9605831ce031791d209 "$CDN" "$CDN_PATH" --paths
# wow_classic 3.4.1.49822 bc=2e5546bd6d79e5beed2c41629af7da89 cc=54602f540da11282061c862936251408
run_build "$OUTDIR/wow_classic_3.4.1.49822_54602f54.txt" "$BFT" wow_classic 2e5546bd6d79e5beed2c41629af7da89 54602f540da11282061c862936251408 "$CDN" "$CDN_PATH" --paths
# wow_classic 3.4.1.49936 bc=3d30e5e7dcdb99a3f4771187371492d9 cc=cf6819d6c38711c0a799ac0cb32ff299
run_build "$OUTDIR/wow_classic_3.4.1.49936_cf6819d6.txt" "$BFT" wow_classic 3d30e5e7dcdb99a3f4771187371492d9 cf6819d6c38711c0a799ac0cb32ff299 "$CDN" "$CDN_PATH" --paths
# wow_classic 3.4.2.50063 bc=a98a4b71c807548a74baebf423c6dd9b cc=fc6594833a28a6adeae6c2b485fd288d
run_build "$OUTDIR/wow_classic_3.4.2.50063_fc659483.txt" "$BFT" wow_classic a98a4b71c807548a74baebf423c6dd9b fc6594833a28a6adeae6c2b485fd288d "$CDN" "$CDN_PATH" --paths
# wow_classic 3.4.2.50129 bc=2c97f23eb29397582a1991409d606855 cc=206bede1cf9a3e284fae4956f376bc0d
run_build "$OUTDIR/wow_classic_3.4.2.50129_206bede1.txt" "$BFT" wow_classic 2c97f23eb29397582a1991409d606855 206bede1cf9a3e284fae4956f376bc0d "$CDN" "$CDN_PATH" --paths
# wow_classic 3.4.2.50172 bc=fa4272f96f7ee8c15f59d41de4cff3f6 cc=d07f46df5c8175589b1187f4aea4c195
run_build "$OUTDIR/wow_classic_3.4.2.50172_d07f46df.txt" "$BFT" wow_classic fa4272f96f7ee8c15f59d41de4cff3f6 d07f46df5c8175589b1187f4aea4c195 "$CDN" "$CDN_PATH" --paths
# wow_classic 3.4.2.50250 bc=d54f7579ecb89d90424aa55f91ecbe4a cc=53ef93a825bf9e73e9f31b7150f71d53
run_build "$OUTDIR/wow_classic_3.4.2.50250_53ef93a8.txt" "$BFT" wow_classic d54f7579ecb89d90424aa55f91ecbe4a 53ef93a825bf9e73e9f31b7150f71d53 "$CDN" "$CDN_PATH" --paths
# wow_classic 3.4.2.50375 bc=892a7623c49cb8fa9e0b7682a465fec6 cc=91438507f5bc9733689679c9ccf7ea74
run_build "$OUTDIR/wow_classic_3.4.2.50375_91438507.txt" "$BFT" wow_classic 892a7623c49cb8fa9e0b7682a465fec6 91438507f5bc9733689679c9ccf7ea74 "$CDN" "$CDN_PATH" --paths
# wow_classic 3.4.2.50664 bc=fb02a39d11c5943387024fb3270e1f38 cc=e1e679f54713a466ca94c199372b1022
run_build "$OUTDIR/wow_classic_3.4.2.50664_e1e679f5.txt" "$BFT" wow_classic fb02a39d11c5943387024fb3270e1f38 e1e679f54713a466ca94c199372b1022 "$CDN" "$CDN_PATH" --paths
# wow_classic 3.4.3.51666 bc=77bae4596dcfa74f665b2ffe6c756ee3 cc=5327964545ec8ac68a227deb67d1134d
run_build "$OUTDIR/wow_classic_3.4.3.51666_53279645.txt" "$BFT" wow_classic 77bae4596dcfa74f665b2ffe6c756ee3 5327964545ec8ac68a227deb67d1134d "$CDN" "$CDN_PATH" --paths
# wow_classic 3.4.3.51739 bc=2ee6dac60f1a8a8f083b8a38e60c0756 cc=d7554527f3ad3ced93069fff38ac2c83
run_build "$OUTDIR/wow_classic_3.4.3.51739_d7554527.txt" "$BFT" wow_classic 2ee6dac60f1a8a8f083b8a38e60c0756 d7554527f3ad3ced93069fff38ac2c83 "$CDN" "$CDN_PATH" --paths
# wow_classic 3.4.3.51831 bc=13b46c10331da0c3a7f446fcc0bea60f cc=e8bc8e56f69b36f4ff1346c82ae9575e
run_build "$OUTDIR/wow_classic_3.4.3.51831_e8bc8e56.txt" "$BFT" wow_classic 13b46c10331da0c3a7f446fcc0bea60f e8bc8e56f69b36f4ff1346c82ae9575e "$CDN" "$CDN_PATH" --paths
# wow_classic 3.4.3.51943 bc=9f2a748e681bedc545c6f8a04f233485 cc=ca2cc5e0ac17c929683dc7f721c56811
run_build "$OUTDIR/wow_classic_3.4.3.51943_ca2cc5e0.txt" "$BFT" wow_classic 9f2a748e681bedc545c6f8a04f233485 ca2cc5e0ac17c929683dc7f721c56811 "$CDN" "$CDN_PATH" --paths
# wow_classic 3.4.3.52237 bc=268a7d2d4bd28cad7c3779a1f5d0a11d cc=e1a66c607ca11d0d5d7fcc99e9cabe09
run_build "$OUTDIR/wow_classic_3.4.3.52237_e1a66c60.txt" "$BFT" wow_classic 268a7d2d4bd28cad7c3779a1f5d0a11d e1a66c607ca11d0d5d7fcc99e9cabe09 "$CDN" "$CDN_PATH" --paths
# wow_classic 3.4.3.53622 bc=ec668d1319825791341e1e10f217d05b cc=62e1f33234a93167aeb6e9d052fdde86
run_build "$OUTDIR/wow_classic_3.4.3.53622_62e1f332.txt" "$BFT" wow_classic ec668d1319825791341e1e10f217d05b 62e1f33234a93167aeb6e9d052fdde86 "$CDN" "$CDN_PATH" --paths
# wow_classic 3.4.3.53788 bc=ac0b091d8852985a2ce03e416fbf1cf1 cc=1f6b67590d1017e32f75aceff1be0b79
run_build "$OUTDIR/wow_classic_3.4.3.53788_1f6b6759.txt" "$BFT" wow_classic ac0b091d8852985a2ce03e416fbf1cf1 1f6b67590d1017e32f75aceff1be0b79 "$CDN" "$CDN_PATH" --paths
# wow_classic 3.4.3.54261 bc=c91609c69ed2ab39d44039390a1be969 cc=a838aeb3cda2e027e9c96bd9953944b3
run_build "$OUTDIR/wow_classic_3.4.3.54261_a838aeb3.txt" "$BFT" wow_classic c91609c69ed2ab39d44039390a1be969 a838aeb3cda2e027e9c96bd9953944b3 "$CDN" "$CDN_PATH" --paths
# wow_classic 3.4.3.54948 bc=d3275c523c9d7cc4683ec56b555fcc4e cc=8b004ac6957c1baf172168e1dea3f684
run_build "$OUTDIR/wow_classic_3.4.3.54948_8b004ac6.txt" "$BFT" wow_classic d3275c523c9d7cc4683ec56b555fcc4e 8b004ac6957c1baf172168e1dea3f684 "$CDN" "$CDN_PATH" --paths
# wow_classic 3.4.3.54987 bc=a8483c9d636d4214f1ab4b3184e6106d cc=7cb313ab15371488a2577539b4c6b0ae
run_build "$OUTDIR/wow_classic_3.4.3.54987_7cb313ab.txt" "$BFT" wow_classic a8483c9d636d4214f1ab4b3184e6106d 7cb313ab15371488a2577539b4c6b0ae "$CDN" "$CDN_PATH" --paths
# wow_classic 3.4.3.55085 bc=05519141104edc4ad29f591a938ca05e cc=28fc43767d570ee3445da9d12b9399d0
run_build "$OUTDIR/wow_classic_3.4.3.55085_28fc4376.txt" "$BFT" wow_classic 05519141104edc4ad29f591a938ca05e 28fc43767d570ee3445da9d12b9399d0 "$CDN" "$CDN_PATH" --paths
# wow_classic 3.4.3.55095 bc=9855373f17f67bf40f8a0339a6ccb625 cc=8c0993120c5a53f1574beb4c757dc8db
run_build "$OUTDIR/wow_classic_3.4.3.55095_8c099312.txt" "$BFT" wow_classic 9855373f17f67bf40f8a0339a6ccb625 8c0993120c5a53f1574beb4c757dc8db "$CDN" "$CDN_PATH" --paths
# wow_classic 3.4.3.55115 bc=4a85c60f749baaa8c6488a6ebdf8c3a1 cc=bc45b7cac135242addcbda4fafd8c4c5
run_build "$OUTDIR/wow_classic_3.4.3.55115_bc45b7ca.txt" "$BFT" wow_classic 4a85c60f749baaa8c6488a6ebdf8c3a1 bc45b7cac135242addcbda4fafd8c4c5 "$CDN" "$CDN_PATH" --paths
# wow_classic 3.4.3.55136 bc=ba8fd4adb878472e70fed5ee9b064a99 cc=0b8c80bb450c46e234c032c3380f3318
run_build "$OUTDIR/wow_classic_3.4.3.55136_0b8c80bb.txt" "$BFT" wow_classic ba8fd4adb878472e70fed5ee9b064a99 0b8c80bb450c46e234c032c3380f3318 "$CDN" "$CDN_PATH" --paths
# wow_classic 3.4.3.55161 bc=c2b0b36fc3a001ef6fb27e7b1636826d cc=9fc9fd871232161555b2b478875bbb03
run_build "$OUTDIR/wow_classic_3.4.3.55161_9fc9fd87.txt" "$BFT" wow_classic c2b0b36fc3a001ef6fb27e7b1636826d 9fc9fd871232161555b2b478875bbb03 "$CDN" "$CDN_PATH" --paths
# wow_classic 3.4.3.55221 bc=b4df6e2c3d2142e126b10db2d3ec9e0f cc=fc3a988b9922c12fbd5f9663f99ac39a
run_build "$OUTDIR/wow_classic_3.4.3.55221_fc3a988b.txt" "$BFT" wow_classic b4df6e2c3d2142e126b10db2d3ec9e0f fc3a988b9922c12fbd5f9663f99ac39a "$CDN" "$CDN_PATH" --paths
# wow_classic 3.4.3.55286 bc=03cab0201582d1d494452a71bcbb73c4 cc=e0d8c68a349b1e44a2994556ca5942d4
run_build "$OUTDIR/wow_classic_3.4.3.55286_e0d8c68a.txt" "$BFT" wow_classic 03cab0201582d1d494452a71bcbb73c4 e0d8c68a349b1e44a2994556ca5942d4 "$CDN" "$CDN_PATH" --paths
# wow_classic 3.4.3.55325 bc=4a699c77306a5f1fa9449753660f7167 cc=1bba77338ed9f10f56d4e55eebe1cf0f
run_build "$OUTDIR/wow_classic_3.4.3.55325_1bba7733.txt" "$BFT" wow_classic 4a699c77306a5f1fa9449753660f7167 1bba77338ed9f10f56d4e55eebe1cf0f "$CDN" "$CDN_PATH" --paths
# wow_classic 3.4.3.55326 bc=165a1f5a1401d137012bd2f6042fe761 cc=05b41787efd91c39154bf599eaa33112
run_build "$OUTDIR/wow_classic_3.4.3.55326_05b41787.txt" "$BFT" wow_classic 165a1f5a1401d137012bd2f6042fe761 05b41787efd91c39154bf599eaa33112 "$CDN" "$CDN_PATH" --paths
# wow_classic 3.4.3.55392 bc=9c52aa5e3b5c027420c3eb12a1103d86 cc=a52d28472505aa2f6f82757698b110e6
run_build "$OUTDIR/wow_classic_3.4.3.55392_a52d2847.txt" "$BFT" wow_classic 9c52aa5e3b5c027420c3eb12a1103d86 a52d28472505aa2f6f82757698b110e6 "$CDN" "$CDN_PATH" --paths
# wow_classic 3.4.3.55417 bc=56eaddb19b1803632506649016ae1787 cc=77b63a45b6a7f9326ea81e7a9c547a34
run_build "$OUTDIR/wow_classic_3.4.3.55417_77b63a45.txt" "$BFT" wow_classic 56eaddb19b1803632506649016ae1787 77b63a45b6a7f9326ea81e7a9c547a34 "$CDN" "$CDN_PATH" --paths
# wow_classic 3.4.3.55541 bc=3e60fc9f39c6a880292a3e2484735ac0 cc=4aaf8769a73b8ca7512c0d44483f27a2
run_build "$OUTDIR/wow_classic_3.4.3.55541_4aaf8769.txt" "$BFT" wow_classic 3e60fc9f39c6a880292a3e2484735ac0 4aaf8769a73b8ca7512c0d44483f27a2 "$CDN" "$CDN_PATH" --paths
# wow_classic 3.4.3.55586 bc=98b7156ce4d7aa5ef4c3afdb34a87d13 cc=6e78fed2b17638cd29e72e939db3312b
run_build "$OUTDIR/wow_classic_3.4.3.55586_6e78fed2.txt" "$BFT" wow_classic 98b7156ce4d7aa5ef4c3afdb34a87d13 6e78fed2b17638cd29e72e939db3312b "$CDN" "$CDN_PATH" --paths
# wow_classic 3.4.3.55758 bc=ba475a13602008a09ae1f6879eddad97 cc=b316dfdaff19e086895302beb6021b90
run_build "$OUTDIR/wow_classic_3.4.3.55758_b316dfda.txt" "$BFT" wow_classic ba475a13602008a09ae1f6879eddad97 b316dfdaff19e086895302beb6021b90 "$CDN" "$CDN_PATH" --paths
# wow_classic 3.4.3.56011 bc=0b06a1107dd7f843fa81994f87e62da5 cc=0154d26d0a90b8c282f14b11b6e69457
run_build "$OUTDIR/wow_classic_3.4.3.56011_0154d26d.txt" "$BFT" wow_classic 0b06a1107dd7f843fa81994f87e62da5 0154d26d0a90b8c282f14b11b6e69457 "$CDN" "$CDN_PATH" --paths
# wow_classic 3.4.3.56030 bc=9911f607ce0020fc836fa157682b1155 cc=c8504560a39a4841a9cc3484097e8063
run_build "$OUTDIR/wow_classic_3.4.3.56030_c8504560.txt" "$BFT" wow_classic 9911f607ce0020fc836fa157682b1155 c8504560a39a4841a9cc3484097e8063 "$CDN" "$CDN_PATH" --paths
# wow_classic 3.4.3.56262 bc=e091f7641594db7671d2bb767defa400 cc=e2028d87e6a47620d78fb9f08836da03
run_build "$OUTDIR/wow_classic_3.4.3.56262_e2028d87.txt" "$BFT" wow_classic e091f7641594db7671d2bb767defa400 e2028d87e6a47620d78fb9f08836da03 "$CDN" "$CDN_PATH" --paths
# wow_classic 3.4.3.56514 bc=11640ae8c9026c6ce6b366c5fd43a6ec cc=f0111a4a72f285397a8d1504697bcce1
run_build "$OUTDIR/wow_classic_3.4.3.56514_f0111a4a.txt" "$BFT" wow_classic 11640ae8c9026c6ce6b366c5fd43a6ec f0111a4a72f285397a8d1504697bcce1 "$CDN" "$CDN_PATH" --paths
# wow_classic 3.4.3.57027 bc=e6b64c8b4622fce39100c5f8bc5f38b9 cc=9b631514298ab4aa614ab5a2f1faa813
run_build "$OUTDIR/wow_classic_3.4.3.57027_9b631514.txt" "$BFT" wow_classic e6b64c8b4622fce39100c5f8bc5f38b9 9b631514298ab4aa614ab5a2f1faa813 "$CDN" "$CDN_PATH" --paths
# wow_classic 3.4.3.57082 bc=1ae60915b51915aba96fc27a84a6bbc6 cc=28765deedffe5661b369c112506de715
run_build "$OUTDIR/wow_classic_3.4.3.57082_28765dee.txt" "$BFT" wow_classic 1ae60915b51915aba96fc27a84a6bbc6 28765deedffe5661b369c112506de715 "$CDN" "$CDN_PATH" --paths
# wow_classic 3.4.3.57135 bc=720f69bfaba0d811ac0de3c28cd9a18f cc=e48cad646ecd7120a23469d145c8c735
run_build "$OUTDIR/wow_classic_3.4.3.57135_e48cad64.txt" "$BFT" wow_classic 720f69bfaba0d811ac0de3c28cd9a18f e48cad646ecd7120a23469d145c8c735 "$CDN" "$CDN_PATH" --paths
# wow_classic 3.4.3.57242 bc=a21c7382a0c143dc582a310ea508ad2d cc=2b5a7cf4cec194b03f19f05c681858b1
run_build "$OUTDIR/wow_classic_3.4.3.57242_2b5a7cf4.txt" "$BFT" wow_classic a21c7382a0c143dc582a310ea508ad2d 2b5a7cf4cec194b03f19f05c681858b1 "$CDN" "$CDN_PATH" --paths
# wow_classic 3.4.3.57269 bc=d15ffcd6a991a45a3a2138e7b7e3ebb8 cc=bba400d95ca3cbf8a0912ec7c9d8899d
run_build "$OUTDIR/wow_classic_3.4.3.57269_bba400d9.txt" "$BFT" wow_classic d15ffcd6a991a45a3a2138e7b7e3ebb8 bba400d95ca3cbf8a0912ec7c9d8899d "$CDN" "$CDN_PATH" --paths
# wow_classic 3.4.3.57316 bc=48f0688a4726bfe1acd46dbe499849d3 cc=b61ec7e7fa0f49102a76d320cb7c08af
run_build "$OUTDIR/wow_classic_3.4.3.57316_b61ec7e7.txt" "$BFT" wow_classic 48f0688a4726bfe1acd46dbe499849d3 b61ec7e7fa0f49102a76d320cb7c08af "$CDN" "$CDN_PATH" --paths
# wow_classic 3.4.3.57364 bc=fb70b888b7736cfa4a7fb0341f43285f cc=f3ee57fdde2651dac86d6d413a107a08
run_build "$OUTDIR/wow_classic_3.4.3.57364_f3ee57fd.txt" "$BFT" wow_classic fb70b888b7736cfa4a7fb0341f43285f f3ee57fdde2651dac86d6d413a107a08 "$CDN" "$CDN_PATH" --paths
# wow_classic 3.4.3.57635 bc=f29253aab29ce5a7c64fecf5de1aa096 cc=002428f9e39ece9f89351068198c7e27
run_build "$OUTDIR/wow_classic_3.4.3.57635_002428f9.txt" "$BFT" wow_classic f29253aab29ce5a7c64fecf5de1aa096 002428f9e39ece9f89351068198c7e27 "$CDN" "$CDN_PATH" --paths
# wow_classic 3.4.3.57978 bc=fbb976a46b774968d4b5b18723556a0d cc=002428f9e39ece9f89351068198c7e27
run_build "$OUTDIR/wow_classic_3.4.3.57978_002428f9.txt" "$BFT" wow_classic fbb976a46b774968d4b5b18723556a0d 002428f9e39ece9f89351068198c7e27 "$CDN" "$CDN_PATH" --paths
# wow_classic 3.4.3.58528 bc=be9271096129bd7737ec697c01aa07bb cc=cc07601bc6887a1f80f1c95199b1538f
run_build "$OUTDIR/wow_classic_3.4.3.58528_cc07601b.txt" "$BFT" wow_classic be9271096129bd7737ec697c01aa07bb cc07601bc6887a1f80f1c95199b1538f "$CDN" "$CDN_PATH" --paths
# wow_classic 3.4.3.58629 bc=00c4b67bd2306145d494bc4ee2f1f41a cc=a7e7624b96d796fa136fb39c1813d42e
run_build "$OUTDIR/wow_classic_3.4.3.58629_a7e7624b.txt" "$BFT" wow_classic 00c4b67bd2306145d494bc4ee2f1f41a a7e7624b96d796fa136fb39c1813d42e "$CDN" "$CDN_PATH" --paths
# wow_classic 3.4.3.58936 bc=a14ed3bad54f7c50bb2beb472441b1e5 cc=854ffbb60ca31b5cfbfeaaee05657107
run_build "$OUTDIR/wow_classic_3.4.3.58936_854ffbb6.txt" "$BFT" wow_classic a14ed3bad54f7c50bb2beb472441b1e5 854ffbb60ca31b5cfbfeaaee05657107 "$CDN" "$CDN_PATH" --paths
# wow_classic 3.4.4.59817 bc=e7da6ad7b77b03d230733479c00f302d cc=7ccb008a7207ac73abdf9ba1a44a40d4
run_build "$OUTDIR/wow_classic_3.4.4.59817_7ccb008a.txt" "$BFT" wow_classic e7da6ad7b77b03d230733479c00f302d 7ccb008a7207ac73abdf9ba1a44a40d4 "$CDN" "$CDN_PATH" --paths
# wow_classic 3.4.4.59853 bc=0a48380dc7c83f589f222d460dbab72f cc=455f592d000a5b3b39908dece334137c
run_build "$OUTDIR/wow_classic_3.4.4.59853_455f592d.txt" "$BFT" wow_classic 0a48380dc7c83f589f222d460dbab72f 455f592d000a5b3b39908dece334137c "$CDN" "$CDN_PATH" --paths
# wow_classic 3.4.4.59887 bc=86e2535fd479b9bbc9902c8cd8400e56 cc=709b1ab442b6a499d2e3812b273def42
run_build "$OUTDIR/wow_classic_3.4.4.59887_709b1ab4.txt" "$BFT" wow_classic 86e2535fd479b9bbc9902c8cd8400e56 709b1ab442b6a499d2e3812b273def42 "$CDN" "$CDN_PATH" --paths
# wow_classic 3.4.4.60003 bc=113c2c2d9efcc03d3af59f78e8c3908f cc=15a7b32f994db7be64fcbcacedaeab49
run_build "$OUTDIR/wow_classic_3.4.4.60003_15a7b32f.txt" "$BFT" wow_classic 113c2c2d9efcc03d3af59f78e8c3908f 15a7b32f994db7be64fcbcacedaeab49 "$CDN" "$CDN_PATH" --paths
# wow_classic 3.4.4.60063 bc=a78401dfe5e592eef88f29448c19823d cc=87bea68fca70e23f2dd6ec76963d1391
run_build "$OUTDIR/wow_classic_3.4.4.60063_87bea68f.txt" "$BFT" wow_classic a78401dfe5e592eef88f29448c19823d 87bea68fca70e23f2dd6ec76963d1391 "$CDN" "$CDN_PATH" --paths
# wow_classic 3.4.4.60190 bc=708468c9ac72bf0acd97ea5b16e9a161 cc=791ccb6ecae3bd570b4e081ac3f364a6
run_build "$OUTDIR/wow_classic_3.4.4.60190_791ccb6e.txt" "$BFT" wow_classic 708468c9ac72bf0acd97ea5b16e9a161 791ccb6ecae3bd570b4e081ac3f364a6 "$CDN" "$CDN_PATH" --paths
# wow_classic 3.4.4.60273 bc=fd4314a8a1046de49b5b5ae5c1c087a8 cc=7eef4043d28c1cf673fc0aba7533db0b
run_build "$OUTDIR/wow_classic_3.4.4.60273_7eef4043.txt" "$BFT" wow_classic fd4314a8a1046de49b5b5ae5c1c087a8 7eef4043d28c1cf673fc0aba7533db0b "$CDN" "$CDN_PATH" --paths
# wow_classic 3.4.4.60320 bc=136197e683b7439e3a391740f0d3a443 cc=032dd4065745a67875ff3004a346f70e
run_build "$OUTDIR/wow_classic_3.4.4.60320_032dd406.txt" "$BFT" wow_classic 136197e683b7439e3a391740f0d3a443 032dd4065745a67875ff3004a346f70e "$CDN" "$CDN_PATH" --paths
# wow_classic 3.4.4.60430 bc=fc45a2413330f09130d3305a8d76a3c3 cc=c8f0a275cb86ec36a3c677b2fa0d8587
run_build "$OUTDIR/wow_classic_3.4.4.60430_c8f0a275.txt" "$BFT" wow_classic fc45a2413330f09130d3305a8d76a3c3 c8f0a275cb86ec36a3c677b2fa0d8587 "$CDN" "$CDN_PATH" --paths
# wow_classic 3.4.4.60842 bc=f306dfcb7c5e3b3e55852b75eae3ea45 cc=4fbe93c64f298c0bbd081f062e9f3149
run_build "$OUTDIR/wow_classic_3.4.4.60842_4fbe93c6.txt" "$BFT" wow_classic f306dfcb7c5e3b3e55852b75eae3ea45 4fbe93c64f298c0bbd081f062e9f3149 "$CDN" "$CDN_PATH" --paths
# wow_classic 3.4.4.60892 bc=ee3922beedf72ab0cdfaef38a907be22 cc=0efbd0915dc8375778f25b23e960f737
run_build "$OUTDIR/wow_classic_3.4.4.60892_0efbd091.txt" "$BFT" wow_classic ee3922beedf72ab0cdfaef38a907be22 0efbd0915dc8375778f25b23e960f737 "$CDN" "$CDN_PATH" --paths
# wow_classic 3.4.4.61075 bc=69d55b617f60f3ddb26a44e618d3cfe4 cc=435d7276b42f7ae41a8e4a00463ccf35
run_build "$OUTDIR/wow_classic_3.4.4.61075_435d7276.txt" "$BFT" wow_classic 69d55b617f60f3ddb26a44e618d3cfe4 435d7276b42f7ae41a8e4a00463ccf35 "$CDN" "$CDN_PATH" --paths
# wow_classic 3.4.4.61187 bc=202e37c18b58825b2e0f3c86f2e05932 cc=09031e74823f27011150c4b0aca7087c
run_build "$OUTDIR/wow_classic_3.4.4.61187_09031e74.txt" "$BFT" wow_classic 202e37c18b58825b2e0f3c86f2e05932 09031e74823f27011150c4b0aca7087c "$CDN" "$CDN_PATH" --paths
# wow_classic 3.4.4.61256 bc=38921c78d7d589af0c8e5cb7bef60923 cc=cea70cf029e4ff7e8d4fbf497f87e50e
run_build "$OUTDIR/wow_classic_3.4.4.61256_cea70cf0.txt" "$BFT" wow_classic 38921c78d7d589af0c8e5cb7bef60923 cea70cf029e4ff7e8d4fbf497f87e50e "$CDN" "$CDN_PATH" --paths
# wow_classic 3.4.4.61581 bc=c2b611183fb96dfbde88c8f781dc5b4e cc=bbfaf246fe12df7ff11f56de92d241df
run_build "$OUTDIR/wow_classic_3.4.4.61581_bbfaf246.txt" "$BFT" wow_classic c2b611183fb96dfbde88c8f781dc5b4e bbfaf246fe12df7ff11f56de92d241df "$CDN" "$CDN_PATH" --paths
# wow_classic 3.4.5.61815 bc=75a37f0a9d674a22575bfdc38525768d cc=0c1e739890c394080b1b5efffcd26e8c
run_build "$OUTDIR/wow_classic_3.4.5.61815_0c1e7398.txt" "$BFT" wow_classic 75a37f0a9d674a22575bfdc38525768d 0c1e739890c394080b1b5efffcd26e8c "$CDN" "$CDN_PATH" --paths
# wow_classic 3.4.5.61934 bc=e35891bc2aae40582a89c11a36d50ac7 cc=050088263d894534f85cb507c87d7987
run_build "$OUTDIR/wow_classic_3.4.5.61934_05008826.txt" "$BFT" wow_classic e35891bc2aae40582a89c11a36d50ac7 050088263d894534f85cb507c87d7987 "$CDN" "$CDN_PATH" --paths
# wow_classic 3.4.5.61937 bc=a65ccfb13c354838ae51b043151346db cc=d83e2dd1c8c0efccf00c469c251ed015
run_build "$OUTDIR/wow_classic_3.4.5.61937_d83e2dd1.txt" "$BFT" wow_classic a65ccfb13c354838ae51b043151346db d83e2dd1c8c0efccf00c469c251ed015 "$CDN" "$CDN_PATH" --paths
# wow_classic 3.4.5.61996 bc=305f4322ada7d9e0820d8e25904bdeae cc=3dc491e7ea8da915ec35d9b8d3f4707e
run_build "$OUTDIR/wow_classic_3.4.5.61996_3dc491e7.txt" "$BFT" wow_classic 305f4322ada7d9e0820d8e25904bdeae 3dc491e7ea8da915ec35d9b8d3f4707e "$CDN" "$CDN_PATH" --paths
# wow_classic 3.4.5.62072 bc=eacb6e27a7772ee3e9a24b5dc5f5e1c2 cc=8e5d013a487e9058ed22c34a80ec6a98
run_build "$OUTDIR/wow_classic_3.4.5.62072_8e5d013a.txt" "$BFT" wow_classic eacb6e27a7772ee3e9a24b5dc5f5e1c2 8e5d013a487e9058ed22c34a80ec6a98 "$CDN" "$CDN_PATH" --paths
# wow_classic 3.4.5.62256 bc=03dcd03f5729a5e731d3fed6fc546611 cc=9d6b66901b96cfd798c377263fcf7e2f
run_build "$OUTDIR/wow_classic_3.4.5.62256_9d6b6690.txt" "$BFT" wow_classic 03dcd03f5729a5e731d3fed6fc546611 9d6b66901b96cfd798c377263fcf7e2f "$CDN" "$CDN_PATH" --paths
# wow_classic 3.4.5.62386 bc=4e4b6680cfad0ddf8ce98f46513c55ee cc=71678975049c2a3bd3ccb642b2fd1773
run_build "$OUTDIR/wow_classic_3.4.5.62386_71678975.txt" "$BFT" wow_classic 4e4b6680cfad0ddf8ce98f46513c55ee 71678975049c2a3bd3ccb642b2fd1773 "$CDN" "$CDN_PATH" --paths
# wow_classic 3.4.5.62423 bc=7abcbf7e74e8257a8a9d171ef11cf870 cc=9b6acc5709f802858f00ee31927952a7
run_build "$OUTDIR/wow_classic_3.4.5.62423_9b6acc57.txt" "$BFT" wow_classic 7abcbf7e74e8257a8a9d171ef11cf870 9b6acc5709f802858f00ee31927952a7 "$CDN" "$CDN_PATH" --paths
# wow_classic 3.4.5.62544 bc=2d223651bc3a708a6238fd57629d6c7e cc=61d4e518e0f6f9d65c386ee47488a2e2
run_build "$OUTDIR/wow_classic_3.4.5.62544_61d4e518.txt" "$BFT" wow_classic 2d223651bc3a708a6238fd57629d6c7e 61d4e518e0f6f9d65c386ee47488a2e2 "$CDN" "$CDN_PATH" --paths
# wow_classic 3.4.5.62824 bc=2af74869a96709723b5a4fe68fc4ef71 cc=e3555045ae267d5b2ac4e8c7c4871dbb
run_build "$OUTDIR/wow_classic_3.4.5.62824_e3555045.txt" "$BFT" wow_classic 2af74869a96709723b5a4fe68fc4ef71 e3555045ae267d5b2ac4e8c7c4871dbb "$CDN" "$CDN_PATH" --paths
# wow_classic 3.4.5.62916 bc=2d6dad2f0dcac95f67a3e999c86488e2 cc=8bea1bcdd3984f541ac42638b1522349
run_build "$OUTDIR/wow_classic_3.4.5.62916_8bea1bcd.txt" "$BFT" wow_classic 2d6dad2f0dcac95f67a3e999c86488e2 8bea1bcdd3984f541ac42638b1522349 "$CDN" "$CDN_PATH" --paths
# wow_classic 3.4.5.63009 bc=c6a424d958e1d685fdf706c603345608 cc=2d160ed1bc54640e9511415b8c9c0082
run_build "$OUTDIR/wow_classic_3.4.5.63009_2d160ed1.txt" "$BFT" wow_classic c6a424d958e1d685fdf706c603345608 2d160ed1bc54640e9511415b8c9c0082 "$CDN" "$CDN_PATH" --paths
# wow_classic 3.4.5.63309 bc=892a357521332d262899c7aca0a91899 cc=20d8c0c2f193328ec144b3ecac49e574
run_build "$OUTDIR/wow_classic_3.4.5.63309_20d8c0c2.txt" "$BFT" wow_classic 892a357521332d262899c7aca0a91899 20d8c0c2f193328ec144b3ecac49e574 "$CDN" "$CDN_PATH" --paths
# wow_classic 3.4.5.63623 bc=665e7c9293ba3dd85238d5e851e053f5 cc=83747e9588a868f6f72cce14655de2f9
run_build "$OUTDIR/wow_classic_3.4.5.63623_83747e95.txt" "$BFT" wow_classic 665e7c9293ba3dd85238d5e851e053f5 83747e9588a868f6f72cce14655de2f9 "$CDN" "$CDN_PATH" --paths
# wow_classic 3.4.5.63697 bc=06cbc85e28267a3b4fd669f2abe15a32 cc=16219b0cf6797cf1690c8e73d314d5b9
run_build "$OUTDIR/wow_classic_3.4.5.63697_16219b0c.txt" "$BFT" wow_classic 06cbc85e28267a3b4fd669f2abe15a32 16219b0cf6797cf1690c8e73d314d5b9 "$CDN" "$CDN_PATH" --paths

# wow_classic 4.4.0.54481 bc=642eb929f245a188fed354da6961359b cc=aae6362f86cf96f70a75128daab179db
run_build "$OUTDIR/wow_classic_4.4.0.54481_aae6362f.txt" "$BFT" wow_classic 642eb929f245a188fed354da6961359b aae6362f86cf96f70a75128daab179db "$CDN" "$CDN_PATH" --paths
# wow_classic 4.4.0.54500 bc=90ce8f9e90148d5b298ed5b4fb7a8a75 cc=e0b5899e372972ab92fb98f8092e7225
run_build "$OUTDIR/wow_classic_4.4.0.54500_e0b5899e.txt" "$BFT" wow_classic 90ce8f9e90148d5b298ed5b4fb7a8a75 e0b5899e372972ab92fb98f8092e7225 "$CDN" "$CDN_PATH" --paths
# wow_classic 4.4.0.54501 bc=a1531955aa7d37d747427e8c0ffc84f1 cc=5de93106a40ce4d71e6c0fd8b469338c
run_build "$OUTDIR/wow_classic_4.4.0.54501_5de93106.txt" "$BFT" wow_classic a1531955aa7d37d747427e8c0ffc84f1 5de93106a40ce4d71e6c0fd8b469338c "$CDN" "$CDN_PATH" --paths
# wow_classic 4.4.0.54525 bc=08dd7ad4abe5af339d4009342b0f3fbe cc=917eaaea12baceef162f565c423c9077
run_build "$OUTDIR/wow_classic_4.4.0.54525_917eaaea.txt" "$BFT" wow_classic 08dd7ad4abe5af339d4009342b0f3fbe 917eaaea12baceef162f565c423c9077 "$CDN" "$CDN_PATH" --paths
# wow_classic 4.4.0.54558 bc=ce9b844d2e9574fbc61b26aa7724c9ee cc=48514176e794a3ce93772283367d402b
run_build "$OUTDIR/wow_classic_4.4.0.54558_48514176.txt" "$BFT" wow_classic ce9b844d2e9574fbc61b26aa7724c9ee 48514176e794a3ce93772283367d402b "$CDN" "$CDN_PATH" --paths
# wow_classic 4.4.0.54647 bc=85c3b8ae5f6e8a85554bbf1599780546 cc=2e77ae0c05d3d8e91537343cb73f9496
run_build "$OUTDIR/wow_classic_4.4.0.54647_2e77ae0c.txt" "$BFT" wow_classic 85c3b8ae5f6e8a85554bbf1599780546 2e77ae0c05d3d8e91537343cb73f9496 "$CDN" "$CDN_PATH" --paths
# wow_classic 4.4.0.54670 bc=9c345b726011d7ef8b08d02110e62cb5 cc=9bb3e2ead1f11811431fd27bcda925cf
run_build "$OUTDIR/wow_classic_4.4.0.54670_9bb3e2ea.txt" "$BFT" wow_classic 9c345b726011d7ef8b08d02110e62cb5 9bb3e2ead1f11811431fd27bcda925cf "$CDN" "$CDN_PATH" --paths
# wow_classic 4.4.0.54737 bc=06b860252869196bc7af50aebd8b1e83 cc=2ac38d9ae035a8ea99bda43cac3df9e6
run_build "$OUTDIR/wow_classic_4.4.0.54737_2ac38d9a.txt" "$BFT" wow_classic 06b860252869196bc7af50aebd8b1e83 2ac38d9ae035a8ea99bda43cac3df9e6 "$CDN" "$CDN_PATH" --paths
# wow_classic 4.4.0.54851 bc=2fd1be3f3ce7c0df5f7e449e2083d2e0 cc=9c6f0269786fb5c6b4943ac612179127
run_build "$OUTDIR/wow_classic_4.4.0.54851_9c6f0269.txt" "$BFT" wow_classic 2fd1be3f3ce7c0df5f7e449e2083d2e0 9c6f0269786fb5c6b4943ac612179127 "$CDN" "$CDN_PATH" --paths
# wow_classic 4.4.0.55006 bc=486f4330c60f1eb83fe3d6a59d862d2a cc=4e2fd3cb899cd3dc0878f3c790dd226d
run_build "$OUTDIR/wow_classic_4.4.0.55006_4e2fd3cb.txt" "$BFT" wow_classic 486f4330c60f1eb83fe3d6a59d862d2a 4e2fd3cb899cd3dc0878f3c790dd226d "$CDN" "$CDN_PATH" --paths
# wow_classic 4.4.0.55460 bc=90fd9eea10112bd3858d0eba0f5e6f79 cc=9871fac58b4341f26a6e4d3f4c3d9362
run_build "$OUTDIR/wow_classic_4.4.0.55460_9871fac5.txt" "$BFT" wow_classic 90fd9eea10112bd3858d0eba0f5e6f79 9871fac58b4341f26a6e4d3f4c3d9362 "$CDN" "$CDN_PATH" --paths
# wow_classic 4.4.0.55613 bc=3fca2ca5b5b1d2195b09b6ff4181410d cc=1362038d83a2fd77738d50befc33e4f2
run_build "$OUTDIR/wow_classic_4.4.0.55613_1362038d.txt" "$BFT" wow_classic 3fca2ca5b5b1d2195b09b6ff4181410d 1362038d83a2fd77738d50befc33e4f2 "$CDN" "$CDN_PATH" --paths
# wow_classic 4.4.0.55639 bc=4c1f6ffb13af74e7d241af25be771a9b cc=90948ba6c2446aa2af3fbb41b5a7134a
run_build "$OUTDIR/wow_classic_4.4.0.55639_90948ba6.txt" "$BFT" wow_classic 4c1f6ffb13af74e7d241af25be771a9b 90948ba6c2446aa2af3fbb41b5a7134a "$CDN" "$CDN_PATH" --paths
# wow_classic 4.4.0.56014 bc=0636a4b3aa0b9aa69dbb7db17a319395 cc=9dc716386b20bf8f6931b25966cf06cc
run_build "$OUTDIR/wow_classic_4.4.0.56014_9dc71638.txt" "$BFT" wow_classic 0636a4b3aa0b9aa69dbb7db17a319395 9dc716386b20bf8f6931b25966cf06cc "$CDN" "$CDN_PATH" --paths
# wow_classic 4.4.0.56420 bc=ea94eea551fc2d409b1b4f5b2ebf8042 cc=30a850b1730e0384cd390df9dba3a6ac
run_build "$OUTDIR/wow_classic_4.4.0.56420_30a850b1.txt" "$BFT" wow_classic ea94eea551fc2d409b1b4f5b2ebf8042 30a850b1730e0384cd390df9dba3a6ac "$CDN" "$CDN_PATH" --paths
# wow_classic 4.4.0.56489 bc=7944060322116d6dbb71e20c9ec91b6a cc=f0111a4a72f285397a8d1504697bcce1
run_build "$OUTDIR/wow_classic_4.4.0.56489_f0111a4a.txt" "$BFT" wow_classic 7944060322116d6dbb71e20c9ec91b6a f0111a4a72f285397a8d1504697bcce1 "$CDN" "$CDN_PATH" --paths
# wow_classic 4.4.0.56713 bc=3bfcb1c121ecc3fbe0c4588619f0aedf cc=de4deebfb4a8278c1ca21ca53b4fe012
run_build "$OUTDIR/wow_classic_4.4.0.56713_de4deebf.txt" "$BFT" wow_classic 3bfcb1c121ecc3fbe0c4588619f0aedf de4deebfb4a8278c1ca21ca53b4fe012 "$CDN" "$CDN_PATH" --paths
# wow_classic 4.4.0.57244 bc=24c413adf3ec83e373db804b2d3c4eff cc=2b5a7cf4cec194b03f19f05c681858b1
run_build "$OUTDIR/wow_classic_4.4.0.57244_2b5a7cf4.txt" "$BFT" wow_classic 24c413adf3ec83e373db804b2d3c4eff 2b5a7cf4cec194b03f19f05c681858b1 "$CDN" "$CDN_PATH" --paths
# wow_classic 4.4.1.57294 bc=0cb3cd77f38173126dc3b157adb624e2 cc=ddcd715134ff6e8ea6b26cd809f18ff9
run_build "$OUTDIR/wow_classic_4.4.1.57294_ddcd7151.txt" "$BFT" wow_classic 0cb3cd77f38173126dc3b157adb624e2 ddcd715134ff6e8ea6b26cd809f18ff9 "$CDN" "$CDN_PATH" --paths
# wow_classic 4.4.1.57359 bc=93f16b91cfb402e8896ab27af2d257b6 cc=2832fa4e3cf3c1f2dc51d1741d5d920e
run_build "$OUTDIR/wow_classic_4.4.1.57359_2832fa4e.txt" "$BFT" wow_classic 93f16b91cfb402e8896ab27af2d257b6 2832fa4e3cf3c1f2dc51d1741d5d920e "$CDN" "$CDN_PATH" --paths
# wow_classic 4.4.1.57564 bc=efa3c97470bb5a9e0b9012828517ebfe cc=5f9c69f71f7f97107451f95ea7e4350a
run_build "$OUTDIR/wow_classic_4.4.1.57564_5f9c69f7.txt" "$BFT" wow_classic efa3c97470bb5a9e0b9012828517ebfe 5f9c69f71f7f97107451f95ea7e4350a "$CDN" "$CDN_PATH" --paths
# wow_classic 4.4.1.57916 bc=03c9a67e2a37133f076bb9a6a2f83238 cc=d779fe8f2a325f0a943fb45be7f8a819
run_build "$OUTDIR/wow_classic_4.4.1.57916_d779fe8f.txt" "$BFT" wow_classic 03c9a67e2a37133f076bb9a6a2f83238 d779fe8f2a325f0a943fb45be7f8a819 "$CDN" "$CDN_PATH" --paths
# wow_classic 4.4.1.58158 bc=694cee226aaf48ada4ad5024064bfc88 cc=926f5e7e569fbab98ccfda431cb30fe1
run_build "$OUTDIR/wow_classic_4.4.1.58158_926f5e7e.txt" "$BFT" wow_classic 694cee226aaf48ada4ad5024064bfc88 926f5e7e569fbab98ccfda431cb30fe1 "$CDN" "$CDN_PATH" --paths
# wow_classic 4.4.1.58558 bc=45e7030665363d429ded42ca57c04767 cc=573aa01f7470175040eb380bd66886d3
run_build "$OUTDIR/wow_classic_4.4.1.58558_573aa01f.txt" "$BFT" wow_classic 45e7030665363d429ded42ca57c04767 573aa01f7470175040eb380bd66886d3 "$CDN" "$CDN_PATH" --paths
# wow_classic 4.4.1.59069 bc=601c65a25cd15b329bd0b04c87021669 cc=51a9f14dd0e21a22af826749419facca
run_build "$OUTDIR/wow_classic_4.4.1.59069_51a9f14d.txt" "$BFT" wow_classic 601c65a25cd15b329bd0b04c87021669 51a9f14dd0e21a22af826749419facca "$CDN" "$CDN_PATH" --paths
# wow_classic 4.4.2.59185 bc=1d47d025053bb34c370a57aecf21abe4 cc=a058d8e76af60c56a5340af29749dd9c
run_build "$OUTDIR/wow_classic_4.4.2.59185_a058d8e7.txt" "$BFT" wow_classic 1d47d025053bb34c370a57aecf21abe4 a058d8e76af60c56a5340af29749dd9c "$CDN" "$CDN_PATH" --paths
# wow_classic 4.4.2.59297 bc=3865041553eca38bd15161c2e0a1949d cc=8fa00d1c50ac62852f4b66ef473104f1
run_build "$OUTDIR/wow_classic_4.4.2.59297_8fa00d1c.txt" "$BFT" wow_classic 3865041553eca38bd15161c2e0a1949d 8fa00d1c50ac62852f4b66ef473104f1 "$CDN" "$CDN_PATH" --paths
# wow_classic 4.4.2.59346 bc=d504ea77cfcff4526b11dee56b1954d1 cc=2b72e01d8f73db0bad24633de300327c
run_build "$OUTDIR/wow_classic_4.4.2.59346_2b72e01d.txt" "$BFT" wow_classic d504ea77cfcff4526b11dee56b1954d1 2b72e01d8f73db0bad24633de300327c "$CDN" "$CDN_PATH" --paths
# wow_classic 4.4.2.59536 bc=2ac7351e2c5c169036de395e4b3d3300 cc=cb73f11fc0e6e63ddf54b46f40670f61
run_build "$OUTDIR/wow_classic_4.4.2.59536_cb73f11f.txt" "$BFT" wow_classic 2ac7351e2c5c169036de395e4b3d3300 cb73f11fc0e6e63ddf54b46f40670f61 "$CDN" "$CDN_PATH" --paths
# wow_classic 4.4.2.59734 bc=f0e001304fc0d43649af97a53e4f1a3b cc=e471c9cc736f155c5fec94787206a575
run_build "$OUTDIR/wow_classic_4.4.2.59734_e471c9cc.txt" "$BFT" wow_classic f0e001304fc0d43649af97a53e4f1a3b e471c9cc736f155c5fec94787206a575 "$CDN" "$CDN_PATH" --paths
# wow_classic 4.4.2.59962 bc=8a4d82da80395487ece86bd3fab15617 cc=2d25ee7c3a173a4e6b2a9b1728f528b0
run_build "$OUTDIR/wow_classic_4.4.2.59962_2d25ee7c.txt" "$BFT" wow_classic 8a4d82da80395487ece86bd3fab15617 2d25ee7c3a173a4e6b2a9b1728f528b0 "$CDN" "$CDN_PATH" --paths
# wow_classic 4.4.2.60142 bc=c5cda7a89e23b18b805ec3f559d39993 cc=ded1793d038111b4af466f3d8290f0c1
run_build "$OUTDIR/wow_classic_4.4.2.60142_ded1793d.txt" "$BFT" wow_classic c5cda7a89e23b18b805ec3f559d39993 ded1793d038111b4af466f3d8290f0c1 "$CDN" "$CDN_PATH" --paths
# wow_classic 4.4.2.60192 bc=fba9ec18b5ed14f979a3f29ba49d1442 cc=e5071a656e6cda472a78ade2f808454a
run_build "$OUTDIR/wow_classic_4.4.2.60192_e5071a65.txt" "$BFT" wow_classic fba9ec18b5ed14f979a3f29ba49d1442 e5071a656e6cda472a78ade2f808454a "$CDN" "$CDN_PATH" --paths
# wow_classic 4.4.2.60895 bc=dae7ca8bbea21a2730c9d103a4ae9897 cc=70ff04fa50020ceef4dd281f8a9c2527
run_build "$OUTDIR/wow_classic_4.4.2.60895_70ff04fa.txt" "$BFT" wow_classic dae7ca8bbea21a2730c9d103a4ae9897 70ff04fa50020ceef4dd281f8a9c2527 "$CDN" "$CDN_PATH" --paths

# wow_classic 5.5.0.61735 bc=85a10334dfedff4e65aed0f114b52bf0 cc=3c5402fdf1681a20a27a0ce9025c26d0
run_build "$OUTDIR/wow_classic_5.5.0.61735_3c5402fd.txt" "$BFT" wow_classic 85a10334dfedff4e65aed0f114b52bf0 3c5402fdf1681a20a27a0ce9025c26d0 "$CDN" "$CDN_PATH" --paths
# wow_classic 5.5.0.61767 bc=6cb4a7192ee8e23ed6103e5b352fd1e3 cc=f5b0fce50cd47c8ad2ff0a0fafe22315
run_build "$OUTDIR/wow_classic_5.5.0.61767_f5b0fce5.txt" "$BFT" wow_classic 6cb4a7192ee8e23ed6103e5b352fd1e3 f5b0fce50cd47c8ad2ff0a0fafe22315 "$CDN" "$CDN_PATH" --paths
# wow_classic 5.5.0.61798 bc=7881c6cb7c06a2e8da2fc6ac47868c73 cc=bef497013a68e2e5d3f32b6ef36c99f4
run_build "$OUTDIR/wow_classic_5.5.0.61798_bef49701.txt" "$BFT" wow_classic 7881c6cb7c06a2e8da2fc6ac47868c73 bef497013a68e2e5d3f32b6ef36c99f4 "$CDN" "$CDN_PATH" --paths
# wow_classic 5.5.0.61820 bc=b57dd616b9b7bc47ed29d3a420c731ec cc=b29b490f6a1a707320a208d79580b268
run_build "$OUTDIR/wow_classic_5.5.0.61820_b29b490f.txt" "$BFT" wow_classic b57dd616b9b7bc47ed29d3a420c731ec b29b490f6a1a707320a208d79580b268 "$CDN" "$CDN_PATH" --paths
# wow_classic 5.5.0.61879 bc=37ffc418a5ea83e8350c9634806b460a cc=fb10fed6afe3e723c395c5ae384cd1dd
run_build "$OUTDIR/wow_classic_5.5.0.61879_fb10fed6.txt" "$BFT" wow_classic 37ffc418a5ea83e8350c9634806b460a fb10fed6afe3e723c395c5ae384cd1dd "$CDN" "$CDN_PATH" --paths
# wow_classic 5.5.0.61916 bc=195f2a3bd4d771d2d4296e5cba1e0ccd cc=126f5dc6600db277af647c6d4c8f9b75
run_build "$OUTDIR/wow_classic_5.5.0.61916_126f5dc6.txt" "$BFT" wow_classic 195f2a3bd4d771d2d4296e5cba1e0ccd 126f5dc6600db277af647c6d4c8f9b75 "$CDN" "$CDN_PATH" --paths
# wow_classic 5.5.0.62044 bc=494d5d38fb8d98bf91d50fa04ab06c63 cc=676753f9c15efd1ca759b973a824aad4
run_build "$OUTDIR/wow_classic_5.5.0.62044_676753f9.txt" "$BFT" wow_classic 494d5d38fb8d98bf91d50fa04ab06c63 676753f9c15efd1ca759b973a824aad4 "$CDN" "$CDN_PATH" --paths
# wow_classic 5.5.0.62071 bc=52cafaa03849e975ab2d7e7bd0ab863d cc=2ba6426689ea188ba4dfe7419d72ab8f
run_build "$OUTDIR/wow_classic_5.5.0.62071_2ba64266.txt" "$BFT" wow_classic 52cafaa03849e975ab2d7e7bd0ab863d 2ba6426689ea188ba4dfe7419d72ab8f "$CDN" "$CDN_PATH" --paths
# wow_classic 5.5.0.62232 bc=aa380ccee15418dcc9bc03eaa27f64cc cc=ae2c4f19479c96e1e98f90addee340c4
run_build "$OUTDIR/wow_classic_5.5.0.62232_ae2c4f19.txt" "$BFT" wow_classic aa380ccee15418dcc9bc03eaa27f64cc ae2c4f19479c96e1e98f90addee340c4 "$CDN" "$CDN_PATH" --paths
# wow_classic 5.5.0.62258 bc=0a03542084b83d7b18d23141b8570341 cc=dcd388d6eb2c5c361bbcd49dd2850f2b
run_build "$OUTDIR/wow_classic_5.5.0.62258_dcd388d6.txt" "$BFT" wow_classic 0a03542084b83d7b18d23141b8570341 dcd388d6eb2c5c361bbcd49dd2850f2b "$CDN" "$CDN_PATH" --paths
# wow_classic 5.5.0.62422 bc=1b9593402c28bda698226486ca7491c5 cc=2cd9f42a5b9ab87716361a8047f28f8c
run_build "$OUTDIR/wow_classic_5.5.0.62422_2cd9f42a.txt" "$BFT" wow_classic 1b9593402c28bda698226486ca7491c5 2cd9f42a5b9ab87716361a8047f28f8c "$CDN" "$CDN_PATH" --paths
# wow_classic 5.5.0.62518 bc=b89f011ef5f0bb89138f46677fecb3fa cc=660f28e7347371728b9a1d0920fd5085
run_build "$OUTDIR/wow_classic_5.5.0.62518_660f28e7.txt" "$BFT" wow_classic b89f011ef5f0bb89138f46677fecb3fa 660f28e7347371728b9a1d0920fd5085 "$CDN" "$CDN_PATH" --paths
# wow_classic 5.5.0.62655 bc=f53e4574b5ce0bddc1f69cbf5872df07 cc=25ff42f09d57e9af0be5804cb93ede79
run_build "$OUTDIR/wow_classic_5.5.0.62655_25ff42f0.txt" "$BFT" wow_classic f53e4574b5ce0bddc1f69cbf5872df07 25ff42f09d57e9af0be5804cb93ede79 "$CDN" "$CDN_PATH" --paths
# wow_classic 5.5.0.62959 bc=9290a9967248df775414ec03736b7a79 cc=6fd98230eef634e5026f1cb33ea2be90
run_build "$OUTDIR/wow_classic_5.5.0.62959_6fd98230.txt" "$BFT" wow_classic 9290a9967248df775414ec03736b7a79 6fd98230eef634e5026f1cb33ea2be90 "$CDN" "$CDN_PATH" --paths
# wow_classic 5.5.1.63311 bc=9fc8f5472844d8e2159ea8fcce07db72 cc=ed368ee4aff2538cf29bf37104c2bb4a
run_build "$OUTDIR/wow_classic_5.5.1.63311_ed368ee4.txt" "$BFT" wow_classic 9fc8f5472844d8e2159ea8fcce07db72 ed368ee4aff2538cf29bf37104c2bb4a "$CDN" "$CDN_PATH" --paths
# wow_classic 5.5.1.63364 bc=01dabb8a6103df2b11e2fbb3ce54e29b cc=6d7c15794a6bce797fb7ee17a55dd237
run_build "$OUTDIR/wow_classic_5.5.1.63364_6d7c1579.txt" "$BFT" wow_classic 01dabb8a6103df2b11e2fbb3ce54e29b 6d7c15794a6bce797fb7ee17a55dd237 "$CDN" "$CDN_PATH" --paths
# wow_classic 5.5.1.63393 bc=b99bc9f9e5bdca00134238b3df8cbd07 cc=c8d26368b031f5549704f0aad9b3e99d
run_build "$OUTDIR/wow_classic_5.5.1.63393_c8d26368.txt" "$BFT" wow_classic b99bc9f9e5bdca00134238b3df8cbd07 c8d26368b031f5549704f0aad9b3e99d "$CDN" "$CDN_PATH" --paths
# wow_classic 5.5.1.63421 bc=d416bf004e23af5e67d9ac5bb13781e9 cc=8bdb002f4e6fb9154fcd2877c52c798c
run_build "$OUTDIR/wow_classic_5.5.1.63421_8bdb002f.txt" "$BFT" wow_classic d416bf004e23af5e67d9ac5bb13781e9 8bdb002f4e6fb9154fcd2877c52c798c "$CDN" "$CDN_PATH" --paths
# wow_classic 5.5.1.63449 bc=d6c6cbfb3aad60dcdc55de2787a24a16 cc=6a315c8340ddcbb3c9f021208945e7bb
run_build "$OUTDIR/wow_classic_5.5.1.63449_6a315c83.txt" "$BFT" wow_classic d6c6cbfb3aad60dcdc55de2787a24a16 6a315c8340ddcbb3c9f021208945e7bb "$CDN" "$CDN_PATH" --paths
# wow_classic 5.5.1.63538 bc=1710e604951fb33ada1c7af57cb92f8a cc=3926d73bb2786875f78ec9757c64ad96
run_build "$OUTDIR/wow_classic_5.5.1.63538_3926d73b.txt" "$BFT" wow_classic 1710e604951fb33ada1c7af57cb92f8a 3926d73bb2786875f78ec9757c64ad96 "$CDN" "$CDN_PATH" --paths
# wow_classic 5.5.1.63698 bc=73e5553435d05219882566bfe97c0ef8 cc=16219b0cf6797cf1690c8e73d314d5b9
run_build "$OUTDIR/wow_classic_5.5.1.63698_16219b0c.txt" "$BFT" wow_classic 73e5553435d05219882566bfe97c0ef8 16219b0cf6797cf1690c8e73d314d5b9 "$CDN" "$CDN_PATH" --paths
# wow_classic 5.5.2.64068 bc=70454c294245df2df4c823c9321da0b1 cc=355f95c33300a0c8655367735576f640
run_build "$OUTDIR/wow_classic_5.5.2.64068_355f95c3.txt" "$BFT" wow_classic 70454c294245df2df4c823c9321da0b1 355f95c33300a0c8655367735576f640 "$CDN" "$CDN_PATH" --paths
# wow_classic 5.5.2.64133 bc=4fbb551c8959607b19b400fe92791e7f cc=5eecacb59d5c7ab42952a1fab7e1a942
run_build "$OUTDIR/wow_classic_5.5.2.64133_5eecacb5.txt" "$BFT" wow_classic 4fbb551c8959607b19b400fe92791e7f 5eecacb59d5c7ab42952a1fab7e1a942 "$CDN" "$CDN_PATH" --paths
# wow_classic 5.5.2.64271 bc=34c14cfc0ef1ef4b8996072edb8647fe cc=7d5cabbc8e41c4b045cedda74516f471
run_build "$OUTDIR/wow_classic_5.5.2.64271_7d5cabbc.txt" "$BFT" wow_classic 34c14cfc0ef1ef4b8996072edb8647fe 7d5cabbc8e41c4b045cedda74516f471 "$CDN" "$CDN_PATH" --paths
# wow_classic 5.5.2.64481 bc=1310b6457b96420ffb30f8f5a8d11d6d cc=6ff7387d594f38b6e9944413ea91f2b1
run_build "$OUTDIR/wow_classic_5.5.2.64481_6ff7387d.txt" "$BFT" wow_classic 1310b6457b96420ffb30f8f5a8d11d6d 6ff7387d594f38b6e9944413ea91f2b1 "$CDN" "$CDN_PATH" --paths
# wow_classic 5.5.3.64802 bc=f7a098c3ba005e41e5ad75405eba0c6f cc=ce248ac011be198c60ff627d70524b7f
run_build "$OUTDIR/wow_classic_5.5.3.64802_ce248ac0.txt" "$BFT" wow_classic f7a098c3ba005e41e5ad75405eba0c6f ce248ac011be198c60ff627d70524b7f "$CDN" "$CDN_PATH" --paths
# wow_classic 5.5.3.64857 bc=b0718103bd9c550822cd0a7c2c4ca787 cc=45cecd053f8582b04c79da068e212198
run_build "$OUTDIR/wow_classic_5.5.3.64857_45cecd05.txt" "$BFT" wow_classic b0718103bd9c550822cd0a7c2c4ca787 45cecd053f8582b04c79da068e212198 "$CDN" "$CDN_PATH" --paths
# wow_classic 5.5.3.65302 bc=56e52feeaea5310d38419bd35c6f1d49 cc=b3ea000d5d26fa6615af24a01e181e5f
run_build "$OUTDIR/wow_classic_5.5.3.65302_b3ea000d.txt" "$BFT" wow_classic 56e52feeaea5310d38419bd35c6f1d49 b3ea000d5d26fa6615af24a01e181e5f "$CDN" "$CDN_PATH" --paths
# wow_classic 5.5.3.65703 bc=098f44d9cc121ebc3dfd5d141d9bb1a9 cc=c4ac957c960c24fe5b910e1dfd60f7cd
run_build "$OUTDIR/wow_classic_5.5.3.65703_c4ac957c.txt" "$BFT" wow_classic 098f44d9cc121ebc3dfd5d141d9bb1a9 c4ac957c960c24fe5b910e1dfd60f7cd "$CDN" "$CDN_PATH" --paths
# wow_classic 5.5.3.65746 bc=81ce7b4853a56fe44e2a722d247686aa cc=aaf6c86aebc4b04bd0335555d4494a31
run_build "$OUTDIR/wow_classic_5.5.3.65746_aaf6c86a.txt" "$BFT" wow_classic 81ce7b4853a56fe44e2a722d247686aa aaf6c86aebc4b04bd0335555d4494a31 "$CDN" "$CDN_PATH" --paths
# wow_classic 5.5.3.65890 bc=066331608b2108511488c21d535151ae cc=a83dcb55a7c076f863610bde509282c8
run_build "$OUTDIR/wow_classic_5.5.3.65890_a83dcb55.txt" "$BFT" wow_classic 066331608b2108511488c21d535151ae a83dcb55a7c076f863610bde509282c8 "$CDN" "$CDN_PATH" --paths
# wow_classic 5.5.3.65988 bc=8c0bf563261db21d953517aba9564738 cc=d4da0f42963dead186f504a79da17536
run_build "$OUTDIR/wow_classic_5.5.3.65988_d4da0f42.txt" "$BFT" wow_classic 8c0bf563261db21d953517aba9564738 d4da0f42963dead186f504a79da17536 "$CDN" "$CDN_PATH" --paths
# wow_classic 5.5.3.66128 bc=e06c268cd915345618ad05b438d63f01 cc=c4ef4f50d56a2ebf40e8d2d411a362ad
run_build "$OUTDIR/wow_classic_5.5.3.66128_c4ef4f50.txt" "$BFT" wow_classic e06c268cd915345618ad05b438d63f01 c4ef4f50d56a2ebf40e8d2d411a362ad "$CDN" "$CDN_PATH" --paths
# wow_classic 5.5.3.66264 bc=9801cee67fbd99f6af3b696bf81902d4 cc=7a6d7318838b9ef2504931ab6ec9a999
run_build "$OUTDIR/wow_classic_5.5.3.66264_7a6d7318.txt" "$BFT" wow_classic 9801cee67fbd99f6af3b696bf81902d4 7a6d7318838b9ef2504931ab6ec9a999 "$CDN" "$CDN_PATH" --paths
# wow_classic 5.5.3.66290 bc=55f7bad306b57fa7410191d256e6984c cc=6a20f29e0818bcf7f30f0255549f8c8f
run_build "$OUTDIR/wow_classic_5.5.3.66290_6a20f29e.txt" "$BFT" wow_classic 55f7bad306b57fa7410191d256e6984c 6a20f29e0818bcf7f30f0255549f8c8f "$CDN" "$CDN_PATH" --paths
# wow_classic 5.5.3.66382 bc=12f7cfccb3f417653f63ff712393e15a cc=1be92317d62bc1a2523ebd61e985467f
run_build "$OUTDIR/wow_classic_5.5.3.66382_1be92317.txt" "$BFT" wow_classic 12f7cfccb3f417653f63ff712393e15a 1be92317d62bc1a2523ebd61e985467f "$CDN" "$CDN_PATH" --paths
# wow_classic 5.5.3.66509 bc=99f17ac5d24963f9a6095441e992fe68 cc=393542519f9f624b288bed93837d2d21
run_build "$OUTDIR/wow_classic_5.5.3.66509_39354251.txt" "$BFT" wow_classic 99f17ac5d24963f9a6095441e992fe68 393542519f9f624b288bed93837d2d21 "$CDN" "$CDN_PATH" --paths
# wow_classic 5.5.3.66565 bc=d91993b38b7319d468c9f0679fa78b13 cc=e96ac31b3e09478731646839347892e4
run_build "$OUTDIR/wow_classic_5.5.3.66565_e96ac31b.txt" "$BFT" wow_classic d91993b38b7319d468c9f0679fa78b13 e96ac31b3e09478731646839347892e4 "$CDN" "$CDN_PATH" --paths
# wow_classic 5.5.3.66839 bc=05d5c5a0f3bda2495943ab2864a3e182 cc=b8341765e3e67c5b6a0679cdfbad9ff2
run_build "$OUTDIR/wow_classic_5.5.3.66839_b8341765.txt" "$BFT" wow_classic 05d5c5a0f3bda2495943ab2864a3e182 b8341765e3e67c5b6a0679cdfbad9ff2 "$CDN" "$CDN_PATH" --paths
# wow_classic 5.5.3.67158 bc=70ade877cbbef31b2f400e16b2052e91 cc=a4ae293c478d89f00ef637d12a0c2e04
run_build "$OUTDIR/wow_classic_5.5.3.67158_a4ae293c.txt" "$BFT" wow_classic 70ade877cbbef31b2f400e16b2052e91 a4ae293c478d89f00ef637d12a0c2e04 "$CDN" "$CDN_PATH" --paths
# wow_classic 5.5.3.67509 bc=289239ac8f51a5e66fe04c341043db9d cc=2d8e5f5a523b73403c961ee50e875d70
run_build "$OUTDIR/wow_classic_5.5.3.67509_8610593d.txt" "$BFT" wow_classic 289239ac8f51a5e66fe04c341043db9d 2d8e5f5a523b73403c961ee50e875d70 "$CDN" "$CDN_PATH" --paths
# wow_classic 5.5.4.67849 bc=94f4c222778c98dcdbdfa6c2a6df7d39 cc=938e83ee682e94cc47e7bed94c0a4b55
run_build "$OUTDIR/wow_classic_5.5.4.67849_8610593d.txt" "$BFT" wow_classic 94f4c222778c98dcdbdfa6c2a6df7d39 938e83ee682e94cc47e7bed94c0a4b55 "$CDN" "$CDN_PATH" --paths
# wow_classic 5.5.4.67969 bc=74d42378456e4335f6074fca5e4a9949 cc=8faa0d2387cd2f9b90d2292a127d4dbd
run_build "$OUTDIR/wow_classic_5.5.4.67969_8faa0d23.txt" "$BFT" wow_classic 74d42378456e4335f6074fca5e4a9949 8faa0d2387cd2f9b90d2292a127d4dbd "$CDN" "$CDN_PATH" --paths
# wow_classic 5.5.4.68016 bc=4ee3cad3cb29cd695acf2148dc988efb cc=3f8912b8acc34e1cf78d902ebd03ba18
run_build "$OUTDIR/wow_classic_5.5.4.68016_3f8912b8.txt" "$BFT" wow_classic 4ee3cad3cb29cd695acf2148dc988efb 3f8912b8acc34e1cf78d902ebd03ba18 "$CDN" "$CDN_PATH" --paths
# wow_classic 5.5.4.68042 bc=9f3c86b65454bae85b192692062bcb1d cc=72bd1b18e3ed5bccdc5ee378b54e4644
run_build "$OUTDIR/wow_classic_5.5.4.68042_72bd1b18.txt" "$BFT" wow_classic 9f3c86b65454bae85b192692062bcb1d 72bd1b18e3ed5bccdc5ee378b54e4644 "$CDN" "$CDN_PATH" --paths
# wow_classic 5.5.4.68077 bc=7a50513882e105feb6241874f520c881 cc=b3579e79322821c6be935a05c7a6ee2f
run_build "$OUTDIR/wow_classic_5.5.4.68077_d3d3c884.txt" "$BFT" wow_classic 7a50513882e105feb6241874f520c881 b3579e79322821c6be935a05c7a6ee2f "$CDN" "$CDN_PATH" --paths
# wow_classic 5.5.4.68159 bc=385e98d716641a8e5011281f9c6f5bbc cc=fb665ecbea0fb5665e1ca58b30227472
run_build "$OUTDIR/wow_classic_5.5.4.68159_a84be43d.txt" "$BFT" wow_classic 385e98d716641a8e5011281f9c6f5bbc fb665ecbea0fb5665e1ca58b30227472 "$CDN" "$CDN_PATH" --paths
# wow_classic 5.5.4.68317 bc=284135abf50ae8095e8db7b5f5afaaa5 cc=e0fb5fa94ab1850fef6f8820fea3062d
run_build "$OUTDIR/wow_classic_5.5.4.68317_42eace50.txt" "$BFT" wow_classic 284135abf50ae8095e8db7b5f5afaaa5 e0fb5fa94ab1850fef6f8820fea3062d "$CDN" "$CDN_PATH" --paths
# wow_classic 5.5.4.68571 bc=b141f2b426b86df1e70d72a7bd360a9f cc=f69dda4c5555d0032b60fde41e840463
run_build "$OUTDIR/wow_classic_5.5.4.68571_b141f2b4.txt" "$BFT" wow_classic b141f2b426b86df1e70d72a7bd360a9f f69dda4c5555d0032b60fde41e840463 "$CDN" "$CDN_PATH" --paths
# wow_classic 5.5.4.68716 bc=3e2eb14e3216f836d801fc592505aedb cc=534ded0400b3e158ae129155caec86ca
run_build "$OUTDIR/wow_classic_5.5.4.68716_3e2eb14e.txt" "$BFT" wow_classic 3e2eb14e3216f836d801fc592505aedb 534ded0400b3e158ae129155caec86ca "$CDN" "$CDN_PATH" --paths
# wow_classic 5.5.4.68806 bc=4e3e166daecb389c0831fa94cf669307 cc=567b2b5d89b03ec5b51f4e875d5ed0de
run_build "$OUTDIR/wow_classic_5.5.4.68806_4e3e166d.txt" "$BFT" wow_classic 4e3e166daecb389c0831fa94cf669307 567b2b5d89b03ec5b51f4e875d5ed0de "$CDN" "$CDN_PATH" --paths
# wow_classic 5.5.4.69032 bc=4ddc88e6c1d5fada3bcbae26541bc481 cc=1354dece3c5f48240c3fe3dccb32a361
run_build "$OUTDIR/wow_classic_5.5.4.69032_4ddc88e6.txt" "$BFT" wow_classic 4ddc88e6c1d5fada3bcbae26541bc481 1354dece3c5f48240c3fe3dccb32a361 "$CDN" "$CDN_PATH" --paths
# wow_classic 5.5.4.69078 bc=5bfab3bdf11f4cd87e0aa0b2b1992bb5 cc=13ce4301f8cb9fda1cd03d8b32bcf916
run_build "$OUTDIR/wow_classic_5.5.4.69078_5bfab3bd.txt" "$BFT" wow_classic 5bfab3bdf11f4cd87e0aa0b2b1992bb5 13ce4301f8cb9fda1cd03d8b32bcf916 "$CDN" "$CDN_PATH" --paths
# wow_classic 5.5.4.69155 bc=8ca6e8ce7d7793c68237e40242347f47 cc=9a824cce21b48ebf0b11367ae32d1597
run_build "$OUTDIR/wow_classic_5.5.4.69155_8ca6e8ce.txt" "$BFT" wow_classic 8ca6e8ce7d7793c68237e40242347f47 9a824cce21b48ebf0b11367ae32d1597 "$CDN" "$CDN_PATH" --paths

# wow_classic_era 1.13.2.30786 bc=c8470ae1807bb4f59c1667a6054e6535 cc=3d014cd9e5940b029109685aee932149
run_build "$OUTDIR/wow_classic_era_1.13.2.30786_3d014cd9.txt" "$BFT" wow_classic_era c8470ae1807bb4f59c1667a6054e6535 3d014cd9e5940b029109685aee932149 "$CDN" "$CDN_PATH" --paths
# wow_classic_era 1.13.2.30862 bc=846836d2a8acae42c4fdc7e4382aec67 cc=ae58dd263f346579705f212feefb82c8
run_build "$OUTDIR/wow_classic_era_1.13.2.30862_ae58dd26.txt" "$BFT" wow_classic_era 846836d2a8acae42c4fdc7e4382aec67 ae58dd263f346579705f212feefb82c8 "$CDN" "$CDN_PATH" --paths
# wow_classic_era 1.13.2.31407 bc=a373db049bb20c9b3e1869cd75e35cca cc=0217092a53536772a44d673ecfeb9c22
run_build "$OUTDIR/wow_classic_era_1.13.2.31407_0217092a.txt" "$BFT" wow_classic_era a373db049bb20c9b3e1869cd75e35cca 0217092a53536772a44d673ecfeb9c22 "$CDN" "$CDN_PATH" --paths
# wow_classic_era 1.13.2.31446 bc=a115a2f86cd841f1468468904d8327b3 cc=3043ebe0073cbd28294935b67e4a5896
run_build "$OUTDIR/wow_classic_era_1.13.2.31446_3043ebe0.txt" "$BFT" wow_classic_era a115a2f86cd841f1468468904d8327b3 3043ebe0073cbd28294935b67e4a5896 "$CDN" "$CDN_PATH" --paths
# wow_classic_era 1.13.2.31650 bc=2c915a9a226a3f35af6c65fcc7b6ca4a cc=c54b41b3195b9482ce0d3c6bf0b86cdb
run_build "$OUTDIR/wow_classic_era_1.13.2.31650_c54b41b3.txt" "$BFT" wow_classic_era 2c915a9a226a3f35af6c65fcc7b6ca4a c54b41b3195b9482ce0d3c6bf0b86cdb "$CDN" "$CDN_PATH" --paths
# wow_classic_era 1.13.2.31687 bc=6839bd436f8ec2e2429ff0725e09b63c cc=ba42282b283c46f43b96dc4c3465f321
run_build "$OUTDIR/wow_classic_era_1.13.2.31687_ba42282b.txt" "$BFT" wow_classic_era 6839bd436f8ec2e2429ff0725e09b63c ba42282b283c46f43b96dc4c3465f321 "$CDN" "$CDN_PATH" --paths
# wow_classic_era 1.13.2.31727 bc=0f9b4efb7e262dac08d8e330ffa32126 cc=a75010533695b4ee8d1fa5c13e948cd1
run_build "$OUTDIR/wow_classic_era_1.13.2.31727_a7501053.txt" "$BFT" wow_classic_era 0f9b4efb7e262dac08d8e330ffa32126 a75010533695b4ee8d1fa5c13e948cd1 "$CDN" "$CDN_PATH" --paths
# wow_classic_era 1.13.2.31830 bc=a46865b6b382b963ffe78b3d77f7cfcb cc=3a209c6bbda59a3ae784db6b79b9adfa
run_build "$OUTDIR/wow_classic_era_1.13.2.31830_3a209c6b.txt" "$BFT" wow_classic_era a46865b6b382b963ffe78b3d77f7cfcb 3a209c6bbda59a3ae784db6b79b9adfa "$CDN" "$CDN_PATH" --paths
# wow_classic_era 1.13.2.31882 bc=8a01b815ea1356e7e031aee9e762f99c cc=70dccbfe816129ebd271f34c6d2c65e3
run_build "$OUTDIR/wow_classic_era_1.13.2.31882_70dccbfe.txt" "$BFT" wow_classic_era 8a01b815ea1356e7e031aee9e762f99c 70dccbfe816129ebd271f34c6d2c65e3 "$CDN" "$CDN_PATH" --paths
# wow_classic_era 1.13.2.32089 bc=3635c5789e7688ccfb1742eb626d07b0 cc=e8a2018ced1a890947262afcb256b658
run_build "$OUTDIR/wow_classic_era_1.13.2.32089_e8a2018c.txt" "$BFT" wow_classic_era 3635c5789e7688ccfb1742eb626d07b0 e8a2018ced1a890947262afcb256b658 "$CDN" "$CDN_PATH" --paths
# wow_classic_era 1.13.2.32421 bc=b5a105b40ba80c786884a80058969d33 cc=e9705588a51b6042e81c1dc5919bf037
run_build "$OUTDIR/wow_classic_era_1.13.2.32421_e9705588.txt" "$BFT" wow_classic_era b5a105b40ba80c786884a80058969d33 e9705588a51b6042e81c1dc5919bf037 "$CDN" "$CDN_PATH" --paths
# wow_classic_era 1.13.2.32600 bc=596c212114208f0f849c6b6e596e6680 cc=bf4672a701f0795b21ad63bf6b98ae0a
run_build "$OUTDIR/wow_classic_era_1.13.2.32600_bf4672a7.txt" "$BFT" wow_classic_era 596c212114208f0f849c6b6e596e6680 bf4672a701f0795b21ad63bf6b98ae0a "$CDN" "$CDN_PATH" --paths
# wow_classic_era 1.13.3.32790 bc=eabc7dd92330e4907bc234899dd0cd4b cc=efc95c64488ab6dda10a7f57eca91f19
run_build "$OUTDIR/wow_classic_era_1.13.3.32790_efc95c64.txt" "$BFT" wow_classic_era eabc7dd92330e4907bc234899dd0cd4b efc95c64488ab6dda10a7f57eca91f19 "$CDN" "$CDN_PATH" --paths
# wow_classic_era 1.13.3.32836 bc=d987e36d3d50c61a55ef4a84bae915ec cc=6dd16e012450b105c22d5270bb2bf3ea
run_build "$OUTDIR/wow_classic_era_1.13.3.32836_6dd16e01.txt" "$BFT" wow_classic_era d987e36d3d50c61a55ef4a84bae915ec 6dd16e012450b105c22d5270bb2bf3ea "$CDN" "$CDN_PATH" --paths
# wow_classic_era 1.13.3.32887 bc=289e100e9c14605242193aa351ef16f1 cc=a1737895dbca2a38c6c2d8cfa1253766
run_build "$OUTDIR/wow_classic_era_1.13.3.32887_a1737895.txt" "$BFT" wow_classic_era 289e100e9c14605242193aa351ef16f1 a1737895dbca2a38c6c2d8cfa1253766 "$CDN" "$CDN_PATH" --paths
# wow_classic_era 1.13.3.33155 bc=48dc748014ed1ab293fffe8c624f1924 cc=45902a4a96c071bbb6f0d117faf112e8
run_build "$OUTDIR/wow_classic_era_1.13.3.33155_45902a4a.txt" "$BFT" wow_classic_era 48dc748014ed1ab293fffe8c624f1924 45902a4a96c071bbb6f0d117faf112e8 "$CDN" "$CDN_PATH" --paths
# wow_classic_era 1.13.3.33302 bc=3bc04fd7c5309c47b9f19fb7caec0efc cc=d80a505ffed27dc34e80f06f3b6163e5
run_build "$OUTDIR/wow_classic_era_1.13.3.33302_d80a505f.txt" "$BFT" wow_classic_era 3bc04fd7c5309c47b9f19fb7caec0efc d80a505ffed27dc34e80f06f3b6163e5 "$CDN" "$CDN_PATH" --paths
# wow_classic_era 1.13.3.33526 bc=5bd9e122809e9566dbd353a781cf7324 cc=bd7286c25b285500610be670014c3e48
run_build "$OUTDIR/wow_classic_era_1.13.3.33526_bd7286c2.txt" "$BFT" wow_classic_era 5bd9e122809e9566dbd353a781cf7324 bd7286c25b285500610be670014c3e48 "$CDN" "$CDN_PATH" --paths
# wow_classic_era 1.13.4.33598 bc=2d5c34af4961eeb72f643a5a0c4d4204 cc=4914d14eba83165faf065dc1d46e684e
run_build "$OUTDIR/wow_classic_era_1.13.4.33598_4914d14e.txt" "$BFT" wow_classic_era 2d5c34af4961eeb72f643a5a0c4d4204 4914d14eba83165faf065dc1d46e684e "$CDN" "$CDN_PATH" --paths
# wow_classic_era 1.13.4.33645 bc=eddb723042b8ee7e866af382ecf8ed5f cc=96385859575c86bc9a90da66300ce691
run_build "$OUTDIR/wow_classic_era_1.13.4.33645_96385859.txt" "$BFT" wow_classic_era eddb723042b8ee7e866af382ecf8ed5f 96385859575c86bc9a90da66300ce691 "$CDN" "$CDN_PATH" --paths
# wow_classic_era 1.13.4.33728 bc=4756dfc0367df50312b250070498e024 cc=b35fb2521d53b547e450c9635b98f60d
run_build "$OUTDIR/wow_classic_era_1.13.4.33728_b35fb252.txt" "$BFT" wow_classic_era 4756dfc0367df50312b250070498e024 b35fb2521d53b547e450c9635b98f60d "$CDN" "$CDN_PATH" --paths
# wow_classic_era 1.13.4.33920 bc=2851a35aa1afa1de57044b34b6000116 cc=e3ee09e8f58f57584b124a3bea61374f
run_build "$OUTDIR/wow_classic_era_1.13.4.33920_e3ee09e8.txt" "$BFT" wow_classic_era 2851a35aa1afa1de57044b34b6000116 e3ee09e8f58f57584b124a3bea61374f "$CDN" "$CDN_PATH" --paths
# wow_classic_era 1.13.4.34219 bc=38cfb8f68cca92d6127f4e27a2324006 cc=5187cdfd6fee12b4a0d53003e8249635
run_build "$OUTDIR/wow_classic_era_1.13.4.34219_5187cdfd.txt" "$BFT" wow_classic_era 38cfb8f68cca92d6127f4e27a2324006 5187cdfd6fee12b4a0d53003e8249635 "$CDN" "$CDN_PATH" --paths
# wow_classic_era 1.13.4.34266 bc=f059cc7b8ef018d03343db634734536d cc=4080392e2318cba8480d50fd2b3545ce
run_build "$OUTDIR/wow_classic_era_1.13.4.34266_4080392e.txt" "$BFT" wow_classic_era f059cc7b8ef018d03343db634734536d 4080392e2318cba8480d50fd2b3545ce "$CDN" "$CDN_PATH" --paths
# wow_classic_era 1.13.4.34600 bc=016b016529281241bf712e3c24bf093a cc=a9cff2e39633dbfe1885d3aa6c2805c5
run_build "$OUTDIR/wow_classic_era_1.13.4.34600_a9cff2e3.txt" "$BFT" wow_classic_era 016b016529281241bf712e3c24bf093a a9cff2e39633dbfe1885d3aa6c2805c5 "$CDN" "$CDN_PATH" --paths
# wow_classic_era 1.13.4.34835 bc=77a2fb029d38b05f6325d47e18183429 cc=9905ae5744462f7d237cc15a1c68d6ef
run_build "$OUTDIR/wow_classic_era_1.13.4.34835_9905ae57.txt" "$BFT" wow_classic_era 77a2fb029d38b05f6325d47e18183429 9905ae5744462f7d237cc15a1c68d6ef "$CDN" "$CDN_PATH" --paths
# wow_classic_era 1.13.5.35000 bc=f8a2a2cdb41bdac6c8c145452ded4615 cc=563871c878ef97fe38f9ae4b882c2416
run_build "$OUTDIR/wow_classic_era_1.13.5.35000_563871c8.txt" "$BFT" wow_classic_era f8a2a2cdb41bdac6c8c145452ded4615 563871c878ef97fe38f9ae4b882c2416 "$CDN" "$CDN_PATH" --paths
# wow_classic_era 1.13.5.35186 bc=1747ffa887e10e50b864776a788b367b cc=b9cfc3fe1f5fd897fe490d9e109adeef
run_build "$OUTDIR/wow_classic_era_1.13.5.35186_b9cfc3fe.txt" "$BFT" wow_classic_era 1747ffa887e10e50b864776a788b367b b9cfc3fe1f5fd897fe490d9e109adeef "$CDN" "$CDN_PATH" --paths
# wow_classic_era 1.13.5.35395 bc=fd5ca49bf27105a6c2d4da5a2315c436 cc=4afb7c5ea93d6783825b4ae606524181
run_build "$OUTDIR/wow_classic_era_1.13.5.35395_4afb7c5e.txt" "$BFT" wow_classic_era fd5ca49bf27105a6c2d4da5a2315c436 4afb7c5ea93d6783825b4ae606524181 "$CDN" "$CDN_PATH" --paths
# wow_classic_era 1.13.5.35663 bc=c34ddcfbe5266498198080d5f6d8aca2 cc=675b08548f5f33339ea13d9aa5e0c84d
run_build "$OUTDIR/wow_classic_era_1.13.5.35663_675b0854.txt" "$BFT" wow_classic_era c34ddcfbe5266498198080d5f6d8aca2 675b08548f5f33339ea13d9aa5e0c84d "$CDN" "$CDN_PATH" --paths
# wow_classic_era 1.13.5.35705 bc=4401cdcd14380e0de4c40f0cc2e371ce cc=2fb6dfeb299c59f211f5b177f22e51a3
run_build "$OUTDIR/wow_classic_era_1.13.5.35705_2fb6dfeb.txt" "$BFT" wow_classic_era 4401cdcd14380e0de4c40f0cc2e371ce 2fb6dfeb299c59f211f5b177f22e51a3 "$CDN" "$CDN_PATH" --paths
# wow_classic_era 1.13.5.35753 bc=c51ade9bbee175fa364400b800cf0471 cc=68827db24917a18eff6fce153848f22f
run_build "$OUTDIR/wow_classic_era_1.13.5.35753_68827db2.txt" "$BFT" wow_classic_era c51ade9bbee175fa364400b800cf0471 68827db24917a18eff6fce153848f22f "$CDN" "$CDN_PATH" --paths
# wow_classic_era 1.13.5.36035 bc=93715636d14629a1d35c84d3975c2452 cc=a109313184f9ad3d36e8e6f9f33f7463
run_build "$OUTDIR/wow_classic_era_1.13.5.36035_a1093131.txt" "$BFT" wow_classic_era 93715636d14629a1d35c84d3975c2452 a109313184f9ad3d36e8e6f9f33f7463 "$CDN" "$CDN_PATH" --paths
# wow_classic_era 1.13.5.36307 bc=8a43830b0e3606268651e61b9a1cf9e4 cc=7c7549619617220b8bd452b90c19e85f
run_build "$OUTDIR/wow_classic_era_1.13.5.36307_7c754961.txt" "$BFT" wow_classic_era 8a43830b0e3606268651e61b9a1cf9e4 7c7549619617220b8bd452b90c19e85f "$CDN" "$CDN_PATH" --paths
# wow_classic_era 1.13.5.36325 bc=705f74266bd6cdd073bcd6999db5a36c cc=a887a76c0ec1db77a64b34ccb75a98f6
run_build "$OUTDIR/wow_classic_era_1.13.5.36325_a887a76c.txt" "$BFT" wow_classic_era 705f74266bd6cdd073bcd6999db5a36c a887a76c0ec1db77a64b34ccb75a98f6 "$CDN" "$CDN_PATH" --paths
# wow_classic_era 1.13.6.36714 bc=6925a54e477975408ae5a154388bc895 cc=ba51606bc8fc3067856b01f27121835d
run_build "$OUTDIR/wow_classic_era_1.13.6.36714_ba51606b.txt" "$BFT" wow_classic_era 6925a54e477975408ae5a154388bc895 ba51606bc8fc3067856b01f27121835d "$CDN" "$CDN_PATH" --paths
# wow_classic_era 1.13.6.36935 bc=9ad6ad5306deb8eed364b64cc628ac98 cc=7786e3be6cfec81537c55d3fc669d371
run_build "$OUTDIR/wow_classic_era_1.13.6.36935_7786e3be.txt" "$BFT" wow_classic_era 9ad6ad5306deb8eed364b64cc628ac98 7786e3be6cfec81537c55d3fc669d371 "$CDN" "$CDN_PATH" --paths
# wow_classic_era 1.13.6.37497 bc=3f54383a77a7c4774335d74b7e8e8f56 cc=af2693f26cd0ead8fc82687a4186b62e
run_build "$OUTDIR/wow_classic_era_1.13.6.37497_af2693f2.txt" "$BFT" wow_classic_era 3f54383a77a7c4774335d74b7e8e8f56 af2693f26cd0ead8fc82687a4186b62e "$CDN" "$CDN_PATH" --paths
# wow_classic_era 1.13.7.38363 bc=e17045a2b2fa5288054e87a4868e58a4 cc=0263db91f386bf94ebdcf955d23b397c
run_build "$OUTDIR/wow_classic_era_1.13.7.38363_0263db91.txt" "$BFT" wow_classic_era e17045a2b2fa5288054e87a4868e58a4 0263db91f386bf94ebdcf955d23b397c "$CDN" "$CDN_PATH" --paths
# wow_classic_era 1.13.7.38386 bc=fea9e027366e9e85a4c954bf3f310899 cc=f0bebd4529d6e42ef2f0add948b9de8d
run_build "$OUTDIR/wow_classic_era_1.13.7.38386_f0bebd45.txt" "$BFT" wow_classic_era fea9e027366e9e85a4c954bf3f310899 f0bebd4529d6e42ef2f0add948b9de8d "$CDN" "$CDN_PATH" --paths
# wow_classic_era 1.13.7.38475 bc=dbd4a8b2733f619e64ec356d461198a5 cc=9c652476248f2eb83cf2be7d79aefacb
run_build "$OUTDIR/wow_classic_era_1.13.7.38475_9c652476.txt" "$BFT" wow_classic_era dbd4a8b2733f619e64ec356d461198a5 9c652476248f2eb83cf2be7d79aefacb "$CDN" "$CDN_PATH" --paths
# wow_classic_era 1.13.7.38631 bc=4ffc9fd8dd2bf6a604313908898aa78c cc=9cf97a7504d69ef3e25a843244d9efcd
run_build "$OUTDIR/wow_classic_era_1.13.7.38631_9cf97a75.txt" "$BFT" wow_classic_era 4ffc9fd8dd2bf6a604313908898aa78c 9cf97a7504d69ef3e25a843244d9efcd "$CDN" "$CDN_PATH" --paths
# wow_classic_era 1.13.7.38704 bc=30daec22777cbe6ab7a0aa31ce621f1b cc=572649a9eda7c06a42b37858d27fbc0f
run_build "$OUTDIR/wow_classic_era_1.13.7.38704_572649a9.txt" "$BFT" wow_classic_era 30daec22777cbe6ab7a0aa31ce621f1b 572649a9eda7c06a42b37858d27fbc0f "$CDN" "$CDN_PATH" --paths
# wow_classic_era 1.13.7.39605 bc=f7a064c574781e96a483a6d8da2bdefc cc=aea89f4edbafc0736170edb663580849
run_build "$OUTDIR/wow_classic_era_1.13.7.39605_aea89f4e.txt" "$BFT" wow_classic_era f7a064c574781e96a483a6d8da2bdefc aea89f4edbafc0736170edb663580849 "$CDN" "$CDN_PATH" --paths
# wow_classic_era 1.13.7.39692 bc=cba185c92b9466eb21f831dc1ec72443 cc=87d47afa5812795f8c7b442cafb25679
run_build "$OUTDIR/wow_classic_era_1.13.7.39692_87d47afa.txt" "$BFT" wow_classic_era cba185c92b9466eb21f831dc1ec72443 87d47afa5812795f8c7b442cafb25679 "$CDN" "$CDN_PATH" --paths

# wow_classic_era 1.14.0.40347 bc=a7bc6352470c3f66de17765aa9686c85 cc=6c271e1d29aa90aa0ab5e1b590d5c0f8
run_build "$OUTDIR/wow_classic_era_1.14.0.40347_6c271e1d.txt" "$BFT" wow_classic_era a7bc6352470c3f66de17765aa9686c85 6c271e1d29aa90aa0ab5e1b590d5c0f8 "$CDN" "$CDN_PATH" --paths
# wow_classic_era 1.14.0.40441 bc=959343af8ecc666fe0ed07aa858b0e7d cc=2c9c73434ef416dcec320ff6eb3d1ae3
run_build "$OUTDIR/wow_classic_era_1.14.0.40441_2c9c7343.txt" "$BFT" wow_classic_era 959343af8ecc666fe0ed07aa858b0e7d 2c9c73434ef416dcec320ff6eb3d1ae3 "$CDN" "$CDN_PATH" --paths
# wow_classic_era 1.14.0.40618 bc=64a44ad0ceeb7d537bc3c164b739e05c cc=5857563ff3292e357698b8b3c6e8b7aa
run_build "$OUTDIR/wow_classic_era_1.14.0.40618_5857563f.txt" "$BFT" wow_classic_era 64a44ad0ceeb7d537bc3c164b739e05c 5857563ff3292e357698b8b3c6e8b7aa "$CDN" "$CDN_PATH" --paths
# wow_classic_era 1.14.1.40962 bc=40a166fbc0847e0731170bc95940c885 cc=049810b7e0a2038104628a274a92ba11
run_build "$OUTDIR/wow_classic_era_1.14.1.40962_049810b7.txt" "$BFT" wow_classic_era 40a166fbc0847e0731170bc95940c885 049810b7e0a2038104628a274a92ba11 "$CDN" "$CDN_PATH" --paths
# wow_classic_era 1.14.1.41030 bc=e88b0f14cb88e36bfd32e7c898e7372e cc=ee5e88e58ed89c91a6aebf17c6427a1a
run_build "$OUTDIR/wow_classic_era_1.14.1.41030_ee5e88e5.txt" "$BFT" wow_classic_era e88b0f14cb88e36bfd32e7c898e7372e ee5e88e58ed89c91a6aebf17c6427a1a "$CDN" "$CDN_PATH" --paths
# wow_classic_era 1.14.1.41077 bc=83f65b1905280553308ce9f66c892b06 cc=92f8a283aaa73faa6463a3fdaa007d58
run_build "$OUTDIR/wow_classic_era_1.14.1.41077_92f8a283.txt" "$BFT" wow_classic_era 83f65b1905280553308ce9f66c892b06 92f8a283aaa73faa6463a3fdaa007d58 "$CDN" "$CDN_PATH" --paths
# wow_classic_era 1.14.1.41137 bc=2dd2d6422785ca7e76f2292c0d8e09a3 cc=e6deaa3e120688d716f40405128d9c2a
run_build "$OUTDIR/wow_classic_era_1.14.1.41137_e6deaa3e.txt" "$BFT" wow_classic_era 2dd2d6422785ca7e76f2292c0d8e09a3 e6deaa3e120688d716f40405128d9c2a "$CDN" "$CDN_PATH" --paths
# wow_classic_era 1.14.1.41243 bc=d98bb45a8132b143d28f341b2d96e092 cc=eea9ce61507c25a87cf08b27216f4b31
run_build "$OUTDIR/wow_classic_era_1.14.1.41243_eea9ce61.txt" "$BFT" wow_classic_era d98bb45a8132b143d28f341b2d96e092 eea9ce61507c25a87cf08b27216f4b31 "$CDN" "$CDN_PATH" --paths
# wow_classic_era 1.14.1.41511 bc=fdaf1dd551d10d985433e13fb285a4db cc=45f721587ad00985a3ecc8e917d8392f
run_build "$OUTDIR/wow_classic_era_1.14.1.41511_45f72158.txt" "$BFT" wow_classic_era fdaf1dd551d10d985433e13fb285a4db 45f721587ad00985a3ecc8e917d8392f "$CDN" "$CDN_PATH" --paths
# wow_classic_era 1.14.1.41794 bc=8e005b0eb004ba9a15e2d5f189ab0a67 cc=db2639b812039a0332239a18b39df1e8
run_build "$OUTDIR/wow_classic_era_1.14.1.41794_db2639b8.txt" "$BFT" wow_classic_era 8e005b0eb004ba9a15e2d5f189ab0a67 db2639b812039a0332239a18b39df1e8 "$CDN" "$CDN_PATH" --paths
# wow_classic_era 1.14.1.42032 bc=7fdded1c02170276751f13b2069e5aab cc=4cd85e438605cf7eb204463215f1ab0e
run_build "$OUTDIR/wow_classic_era_1.14.1.42032_4cd85e43.txt" "$BFT" wow_classic_era 7fdded1c02170276751f13b2069e5aab 4cd85e438605cf7eb204463215f1ab0e "$CDN" "$CDN_PATH" --paths
# wow_classic_era 1.14.2.42214 bc=26f1c63a8e010ae237c34fc0d9faeece cc=383b05e1442fc89f9aff9ee44887fdea
run_build "$OUTDIR/wow_classic_era_1.14.2.42214_383b05e1.txt" "$BFT" wow_classic_era 26f1c63a8e010ae237c34fc0d9faeece 383b05e1442fc89f9aff9ee44887fdea "$CDN" "$CDN_PATH" --paths
# wow_classic_era 1.14.2.42597 bc=a69c90c78c9f0a6ca7c527d6ba9984b0 cc=c8859e7baf58af3bf9f7abb539df4c1e
run_build "$OUTDIR/wow_classic_era_1.14.2.42597_c8859e7b.txt" "$BFT" wow_classic_era a69c90c78c9f0a6ca7c527d6ba9984b0 c8859e7baf58af3bf9f7abb539df4c1e "$CDN" "$CDN_PATH" --paths
# wow_classic_era 1.14.3.43154 bc=d949aa7c359acc7b24c3b5fc36c8c701 cc=45e74443d0b7c8a25c834f8fe7ce9335
run_build "$OUTDIR/wow_classic_era_1.14.3.43154_45e74443.txt" "$BFT" wow_classic_era d949aa7c359acc7b24c3b5fc36c8c701 45e74443d0b7c8a25c834f8fe7ce9335 "$CDN" "$CDN_PATH" --paths
# wow_classic_era 1.14.3.43401 bc=f1078794393c06b6bb194ed11b4215f1 cc=3f3d80a93eb44a51cf976b0362080fbc
run_build "$OUTDIR/wow_classic_era_1.14.3.43401_3f3d80a9.txt" "$BFT" wow_classic_era f1078794393c06b6bb194ed11b4215f1 3f3d80a93eb44a51cf976b0362080fbc "$CDN" "$CDN_PATH" --paths
# wow_classic_era 1.14.3.43639 bc=19c6e8661d4bd74e9015eb11a6046191 cc=9fc02a191e0b6c098ae51bb87ecae4de
run_build "$OUTDIR/wow_classic_era_1.14.3.43639_9fc02a19.txt" "$BFT" wow_classic_era 19c6e8661d4bd74e9015eb11a6046191 9fc02a191e0b6c098ae51bb87ecae4de "$CDN" "$CDN_PATH" --paths
# wow_classic_era 1.14.3.44016 bc=6b05936a87201877d1665fd826178cdd cc=c1dca7299f4e0989b261da64f0835c60
run_build "$OUTDIR/wow_classic_era_1.14.3.44016_c1dca729.txt" "$BFT" wow_classic_era 6b05936a87201877d1665fd826178cdd c1dca7299f4e0989b261da64f0835c60 "$CDN" "$CDN_PATH" --paths
# wow_classic_era 1.14.3.44170 bc=99e65a3261f5014f3b42f99d8bc5f71d cc=b81194b462542e0df9958ae951e05e7e
run_build "$OUTDIR/wow_classic_era_1.14.3.44170_b81194b4.txt" "$BFT" wow_classic_era 99e65a3261f5014f3b42f99d8bc5f71d b81194b462542e0df9958ae951e05e7e "$CDN" "$CDN_PATH" --paths
# wow_classic_era 1.14.3.44403 bc=e5ae8cf5c152802ddbadd73146d8847d cc=697364f7d3f80f13d382e998169372a4
run_build "$OUTDIR/wow_classic_era_1.14.3.44403_697364f7.txt" "$BFT" wow_classic_era e5ae8cf5c152802ddbadd73146d8847d 697364f7d3f80f13d382e998169372a4 "$CDN" "$CDN_PATH" --paths
# wow_classic_era 1.14.3.44834 bc=465d5610d5f451acb5c5986ecdbf1f23 cc=5325d98983238fc35126a296fa2550d2
run_build "$OUTDIR/wow_classic_era_1.14.3.44834_5325d989.txt" "$BFT" wow_classic_era 465d5610d5f451acb5c5986ecdbf1f23 5325d98983238fc35126a296fa2550d2 "$CDN" "$CDN_PATH" --paths
# wow_classic_era 1.14.3.45437 bc=50d09637ec38b3dba47e2a592c631a2a cc=c2b81c97acbc270db264346d3e6e7958
run_build "$OUTDIR/wow_classic_era_1.14.3.45437_c2b81c97.txt" "$BFT" wow_classic_era 50d09637ec38b3dba47e2a592c631a2a c2b81c97acbc270db264346d3e6e7958 "$CDN" "$CDN_PATH" --paths
# wow_classic_era 1.14.3.46575 bc=e60d271c46224f9116bd6b97c0cacdf8 cc=c37c3412555982fa17491c5840cbce25
run_build "$OUTDIR/wow_classic_era_1.14.3.46575_c37c3412.txt" "$BFT" wow_classic_era e60d271c46224f9116bd6b97c0cacdf8 c37c3412555982fa17491c5840cbce25 "$CDN" "$CDN_PATH" --paths
# wow_classic_era 1.14.3.47658 bc=cffabcecc76535e239123b672e89e5a5 cc=fa134856ec2038df9f79efd25b1b2cde
run_build "$OUTDIR/wow_classic_era_1.14.3.47658_fa134856.txt" "$BFT" wow_classic_era cffabcecc76535e239123b672e89e5a5 fa134856ec2038df9f79efd25b1b2cde "$CDN" "$CDN_PATH" --paths
# wow_classic_era 1.14.3.48611 bc=c8eabe282a438e2718b8799d3c533e87 cc=7e97bf4b06bacf7102aaff0e514a068a
run_build "$OUTDIR/wow_classic_era_1.14.3.48611_7e97bf4b.txt" "$BFT" wow_classic_era c8eabe282a438e2718b8799d3c533e87 7e97bf4b06bacf7102aaff0e514a068a "$CDN" "$CDN_PATH" --paths
# wow_classic_era 1.14.3.49229 bc=350bfb27a691f8c5479170218ddc9266 cc=e095db97f0c55ca2d102fd21a1ce5401
run_build "$OUTDIR/wow_classic_era_1.14.3.49229_e095db97.txt" "$BFT" wow_classic_era 350bfb27a691f8c5479170218ddc9266 e095db97f0c55ca2d102fd21a1ce5401 "$CDN" "$CDN_PATH" --paths
# wow_classic_era 1.14.3.49821 bc=97092d5305121c53afd8fbfcf7180bb5 cc=9560f29f281b083566073dfb192e98a2
run_build "$OUTDIR/wow_classic_era_1.14.3.49821_9560f29f.txt" "$BFT" wow_classic_era 97092d5305121c53afd8fbfcf7180bb5 9560f29f281b083566073dfb192e98a2 "$CDN" "$CDN_PATH" --paths
# wow_classic_era 1.14.4.51001 bc=04ed808235c71cd7ed6d6f758f243516 cc=5b5478be10e126a96c0e7e630ad18f1c
run_build "$OUTDIR/wow_classic_era_1.14.4.51001_5b5478be.txt" "$BFT" wow_classic_era 04ed808235c71cd7ed6d6f758f243516 5b5478be10e126a96c0e7e630ad18f1c "$CDN" "$CDN_PATH" --paths
# wow_classic_era 1.14.4.51056 bc=5c56dbffe8267e2bd41444be5d241f4b cc=9cb9c5a5f0938d980d2846fae72eaba6
run_build "$OUTDIR/wow_classic_era_1.14.4.51056_9cb9c5a5.txt" "$BFT" wow_classic_era 5c56dbffe8267e2bd41444be5d241f4b 9cb9c5a5f0938d980d2846fae72eaba6 "$CDN" "$CDN_PATH" --paths
# wow_classic_era 1.14.4.51146 bc=9849a68e0cfe157aa3b584d40bb2197d cc=d60a9a0ef80f48f92093ff10cf9514ea
run_build "$OUTDIR/wow_classic_era_1.14.4.51146_d60a9a0e.txt" "$BFT" wow_classic_era 9849a68e0cfe157aa3b584d40bb2197d d60a9a0ef80f48f92093ff10cf9514ea "$CDN" "$CDN_PATH" --paths
# wow_classic_era 1.14.4.51311 bc=35b2e9c9f750bbf8954f459e3593140b cc=7ab61ab934e953ad8580ee25dc6a5d1b
run_build "$OUTDIR/wow_classic_era_1.14.4.51311_7ab61ab9.txt" "$BFT" wow_classic_era 35b2e9c9f750bbf8954f459e3593140b 7ab61ab934e953ad8580ee25dc6a5d1b "$CDN" "$CDN_PATH" --paths
# wow_classic_era 1.14.4.51395 bc=275603dbe48deb7c42ee0056b0fece06 cc=2a0b158a900b860cad51a67338cf1848
run_build "$OUTDIR/wow_classic_era_1.14.4.51395_2a0b158a.txt" "$BFT" wow_classic_era 275603dbe48deb7c42ee0056b0fece06 2a0b158a900b860cad51a67338cf1848 "$CDN" "$CDN_PATH" --paths
# wow_classic_era 1.14.4.51535 bc=5a2a414138aee30e11ffe124075f9420 cc=c2be59fa579b0fc3f0f979dc6a6b9af9
run_build "$OUTDIR/wow_classic_era_1.14.4.51535_c2be59fa.txt" "$BFT" wow_classic_era 5a2a414138aee30e11ffe124075f9420 c2be59fa579b0fc3f0f979dc6a6b9af9 "$CDN" "$CDN_PATH" --paths
# wow_classic_era 1.14.4.51829 bc=6554ef4e3835fd30453cfda4ac014ff0 cc=1247c5959ff369305cdfd2a53144093d
run_build "$OUTDIR/wow_classic_era_1.14.4.51829_1247c595.txt" "$BFT" wow_classic_era 6554ef4e3835fd30453cfda4ac014ff0 1247c5959ff369305cdfd2a53144093d "$CDN" "$CDN_PATH" --paths

# wow_classic_era 1.15.0.52146 bc=a9ea4525e0d2ad76cb724014255b5337 cc=f207d3e64b722505373231e3fbf7bf4d
run_build "$OUTDIR/wow_classic_era_1.15.0.52146_f207d3e6.txt" "$BFT" wow_classic_era a9ea4525e0d2ad76cb724014255b5337 f207d3e64b722505373231e3fbf7bf4d "$CDN" "$CDN_PATH" --paths
# wow_classic_era 1.15.0.52186 bc=3d8174d90071419e6962dace46f10172 cc=13342ef057d7fb3bd5ac673bfb555548
run_build "$OUTDIR/wow_classic_era_1.15.0.52186_13342ef0.txt" "$BFT" wow_classic_era 3d8174d90071419e6962dace46f10172 13342ef057d7fb3bd5ac673bfb555548 "$CDN" "$CDN_PATH" --paths
# wow_classic_era 1.15.0.52212 bc=c257105768391c12ace3354a57a73196 cc=c125d9411b0382302475cad1d4200f1d
run_build "$OUTDIR/wow_classic_era_1.15.0.52212_c125d941.txt" "$BFT" wow_classic_era c257105768391c12ace3354a57a73196 c125d9411b0382302475cad1d4200f1d "$CDN" "$CDN_PATH" --paths
# wow_classic_era 1.15.0.52302 bc=39454c8c9421b009de809e7cf6b946e2 cc=ed3cd867dc5f08939dafa969873c1f2a
run_build "$OUTDIR/wow_classic_era_1.15.0.52302_ed3cd867.txt" "$BFT" wow_classic_era 39454c8c9421b009de809e7cf6b946e2 ed3cd867dc5f08939dafa969873c1f2a "$CDN" "$CDN_PATH" --paths
# wow_classic_era 1.15.0.52409 bc=20a02dcddc3b835481cbef7d47be0ca2 cc=7e9468c985d1dbe9e3150f495a2ebf4d
run_build "$OUTDIR/wow_classic_era_1.15.0.52409_7e9468c9.txt" "$BFT" wow_classic_era 20a02dcddc3b835481cbef7d47be0ca2 7e9468c985d1dbe9e3150f495a2ebf4d "$CDN" "$CDN_PATH" --paths
# wow_classic_era 1.15.0.52610 bc=3b40893c622218bf6610b3c36c796906 cc=c762472f80fcfdcb3608a0c0ee375cc7
run_build "$OUTDIR/wow_classic_era_1.15.0.52610_c762472f.txt" "$BFT" wow_classic_era 3b40893c622218bf6610b3c36c796906 c762472f80fcfdcb3608a0c0ee375cc7 "$CDN" "$CDN_PATH" --paths
# wow_classic_era 1.15.1.53181 bc=1bb80969cc0f17e9d2b1cb5424dadf7a cc=7a81eb7ce13ee55f179c259e734357ba
run_build "$OUTDIR/wow_classic_era_1.15.1.53181_7a81eb7c.txt" "$BFT" wow_classic_era 1bb80969cc0f17e9d2b1cb5424dadf7a 7a81eb7ce13ee55f179c259e734357ba "$CDN" "$CDN_PATH" --paths
# wow_classic_era 1.15.1.53247 bc=618b8accda5cf16c1601bf8887f1f659 cc=f0b9d3bc5cf70f71bb9f2fffb65e2bc1
run_build "$OUTDIR/wow_classic_era_1.15.1.53247_f0b9d3bc.txt" "$BFT" wow_classic_era 618b8accda5cf16c1601bf8887f1f659 f0b9d3bc5cf70f71bb9f2fffb65e2bc1 "$CDN" "$CDN_PATH" --paths
# wow_classic_era 1.15.1.53495 bc=3cc6193adaee6b15386e037838f6c9ae cc=6ecda87ed71dae867b9541679828a016
run_build "$OUTDIR/wow_classic_era_1.15.1.53495_6ecda87e.txt" "$BFT" wow_classic_era 3cc6193adaee6b15386e037838f6c9ae 6ecda87ed71dae867b9541679828a016 "$CDN" "$CDN_PATH" --paths
# wow_classic_era 1.15.1.53623 bc=f81395997d878b4d532d5893d659a11a cc=62e1f33234a93167aeb6e9d052fdde86
run_build "$OUTDIR/wow_classic_era_1.15.1.53623_62e1f332.txt" "$BFT" wow_classic_era f81395997d878b4d532d5893d659a11a 62e1f33234a93167aeb6e9d052fdde86 "$CDN" "$CDN_PATH" --paths
# wow_classic_era 1.15.2.54029 bc=668b3aef0423df061ad46ca7aec690f0 cc=9daa882d8e5e116e95fc16fe9569896b
run_build "$OUTDIR/wow_classic_era_1.15.2.54029_9daa882d.txt" "$BFT" wow_classic_era 668b3aef0423df061ad46ca7aec690f0 9daa882d8e5e116e95fc16fe9569896b "$CDN" "$CDN_PATH" --paths
# wow_classic_era 1.15.2.54067 bc=5a468991f9812a6d3b2e08575c1211c2 cc=f0a424af6afeab880d02cc4c9cc36fca
run_build "$OUTDIR/wow_classic_era_1.15.2.54067_f0a424af.txt" "$BFT" wow_classic_era 5a468991f9812a6d3b2e08575c1211c2 f0a424af6afeab880d02cc4c9cc36fca "$CDN" "$CDN_PATH" --paths
# wow_classic_era 1.15.2.54092 bc=c3d0cb83d013aa4e62de4df87021d77a cc=cce2d13c2533262be62f016e8775841a
run_build "$OUTDIR/wow_classic_era_1.15.2.54092_cce2d13c.txt" "$BFT" wow_classic_era c3d0cb83d013aa4e62de4df87021d77a cce2d13c2533262be62f016e8775841a "$CDN" "$CDN_PATH" --paths
# wow_classic_era 1.15.2.54262 bc=1df68567462b3c0071c53abaea4e7d8c cc=1cff9f9c9690683bc0a75d7965aca302
run_build "$OUTDIR/wow_classic_era_1.15.2.54262_1cff9f9c.txt" "$BFT" wow_classic_era 1df68567462b3c0071c53abaea4e7d8c 1cff9f9c9690683bc0a75d7965aca302 "$CDN" "$CDN_PATH" --paths
# wow_classic_era 1.15.2.54332 bc=f1d29e07d5a51596e8462c5a4e0c3adb cc=92ba8234893de2d9cac4c11a64cffc12
run_build "$OUTDIR/wow_classic_era_1.15.2.54332_92ba8234.txt" "$BFT" wow_classic_era f1d29e07d5a51596e8462c5a4e0c3adb 92ba8234893de2d9cac4c11a64cffc12 "$CDN" "$CDN_PATH" --paths
# wow_classic_era 1.15.2.54649 bc=ac936ee6ccbd5e1d14444fffd940f173 cc=596d58a2469c82c1f5b6f3696cf27c0b
run_build "$OUTDIR/wow_classic_era_1.15.2.54649_596d58a2.txt" "$BFT" wow_classic_era ac936ee6ccbd5e1d14444fffd940f173 596d58a2469c82c1f5b6f3696cf27c0b "$CDN" "$CDN_PATH" --paths
# wow_classic_era 1.15.2.54902 bc=5a1af265df80b3d97cc43c265bbb29ff cc=269510908cceac8a21d7bda0bf674b65
run_build "$OUTDIR/wow_classic_era_1.15.2.54902_26951090.txt" "$BFT" wow_classic_era 5a1af265df80b3d97cc43c265bbb29ff 269510908cceac8a21d7bda0bf674b65 "$CDN" "$CDN_PATH" --paths
# wow_classic_era 1.15.2.55002 bc=7c9f2adf2d6c557943d6170913619eaf cc=4a951ad16c2445dccb8e0ee23594deff
run_build "$OUTDIR/wow_classic_era_1.15.2.55002_4a951ad1.txt" "$BFT" wow_classic_era 7c9f2adf2d6c557943d6170913619eaf 4a951ad16c2445dccb8e0ee23594deff "$CDN" "$CDN_PATH" --paths
# wow_classic_era 1.15.2.55140 bc=d49d4ea6641ecbc6ed675b40146f4f97 cc=a4397d3512438f2db3b8bd3d271b2a41
run_build "$OUTDIR/wow_classic_era_1.15.2.55140_a4397d35.txt" "$BFT" wow_classic_era d49d4ea6641ecbc6ed675b40146f4f97 a4397d3512438f2db3b8bd3d271b2a41 "$CDN" "$CDN_PATH" --paths
# wow_classic_era 1.15.3.55515 bc=dc428925e77227248c0fd021c5b573da cc=a427fc06ac4969ae454854f67537635f
run_build "$OUTDIR/wow_classic_era_1.15.3.55515_a427fc06.txt" "$BFT" wow_classic_era dc428925e77227248c0fd021c5b573da a427fc06ac4969ae454854f67537635f "$CDN" "$CDN_PATH" --paths
# wow_classic_era 1.15.3.55563 bc=19dc68a09a1cbaa038d54e0721c78bad cc=71b651b8df1babeeb9e9b849b499bc60
run_build "$OUTDIR/wow_classic_era_1.15.3.55563_71b651b8.txt" "$BFT" wow_classic_era 19dc68a09a1cbaa038d54e0721c78bad 71b651b8df1babeeb9e9b849b499bc60 "$CDN" "$CDN_PATH" --paths
# wow_classic_era 1.15.3.55646 bc=2d87804732008a1e961759839ccc1d8f cc=43566b4b16dfeb64c2116d3ebb5438d5
run_build "$OUTDIR/wow_classic_era_1.15.3.55646_43566b4b.txt" "$BFT" wow_classic_era 2d87804732008a1e961759839ccc1d8f 43566b4b16dfeb64c2116d3ebb5438d5 "$CDN" "$CDN_PATH" --paths
# wow_classic_era 1.15.3.55917 bc=7ab118839fc8371e5829599c374fe715 cc=5530374a58924c0cb0b0770dbe1cb5e4
run_build "$OUTDIR/wow_classic_era_1.15.3.55917_5530374a.txt" "$BFT" wow_classic_era 7ab118839fc8371e5829599c374fe715 5530374a58924c0cb0b0770dbe1cb5e4 "$CDN" "$CDN_PATH" --paths
# wow_classic_era 1.15.3.56488 bc=d8360ce66729320d1a70d821a7dc0cb4 cc=f0111a4a72f285397a8d1504697bcce1
run_build "$OUTDIR/wow_classic_era_1.15.3.56488_f0111a4a.txt" "$BFT" wow_classic_era d8360ce66729320d1a70d821a7dc0cb4 f0111a4a72f285397a8d1504697bcce1 "$CDN" "$CDN_PATH" --paths
# wow_classic_era 1.15.3.56626 bc=9b47c882f5834aef0b831786b488d33b cc=e251e476dc59b9200103d22ff82ae907
run_build "$OUTDIR/wow_classic_era_1.15.3.56626_e251e476.txt" "$BFT" wow_classic_era 9b47c882f5834aef0b831786b488d33b e251e476dc59b9200103d22ff82ae907 "$CDN" "$CDN_PATH" --paths
# wow_classic_era 1.15.4.56738 bc=9b2ac37bf13ab6c5e29dc82588f1dbac cc=7220832da2dbc62648f0287c1758a2c9
run_build "$OUTDIR/wow_classic_era_1.15.4.56738_7220832d.txt" "$BFT" wow_classic_era 9b2ac37bf13ab6c5e29dc82588f1dbac 7220832da2dbc62648f0287c1758a2c9 "$CDN" "$CDN_PATH" --paths
# wow_classic_era 1.15.4.56760 bc=749adf879ea90a62feab05bd361a2071 cc=dca1a128cc256784166b743d30a66b45
run_build "$OUTDIR/wow_classic_era_1.15.4.56760_dca1a128.txt" "$BFT" wow_classic_era 749adf879ea90a62feab05bd361a2071 dca1a128cc256784166b743d30a66b45 "$CDN" "$CDN_PATH" --paths
# wow_classic_era 1.15.4.56817 bc=65ce5e8b78da2b111bcc43b907b36049 cc=3b79f730ab95c8cc1d0d19acb7713c79
run_build "$OUTDIR/wow_classic_era_1.15.4.56817_3b79f730.txt" "$BFT" wow_classic_era 65ce5e8b78da2b111bcc43b907b36049 3b79f730ab95c8cc1d0d19acb7713c79 "$CDN" "$CDN_PATH" --paths
# wow_classic_era 1.15.4.56857 bc=f1d3bf873ea7785d9c9f77f6d8163c2e cc=8268a5575172f7f85b66ccea5eabda11
run_build "$OUTDIR/wow_classic_era_1.15.4.56857_8268a557.txt" "$BFT" wow_classic_era f1d3bf873ea7785d9c9f77f6d8163c2e 8268a5575172f7f85b66ccea5eabda11 "$CDN" "$CDN_PATH" --paths
# wow_classic_era 1.15.4.57134 bc=0fff60e4dc785f9150a2118099f01efd cc=ddcd715134ff6e8ea6b26cd809f18ff9
run_build "$OUTDIR/wow_classic_era_1.15.4.57134_ddcd7151.txt" "$BFT" wow_classic_era 0fff60e4dc785f9150a2118099f01efd ddcd715134ff6e8ea6b26cd809f18ff9 "$CDN" "$CDN_PATH" --paths
# wow_classic_era 1.15.5.57638 bc=783de7086e072ad781bf8f2906f3f570 cc=38b154749c970aac221ac77c95dc8baa
run_build "$OUTDIR/wow_classic_era_1.15.5.57638_38b15474.txt" "$BFT" wow_classic_era 783de7086e072ad781bf8f2906f3f570 38b154749c970aac221ac77c95dc8baa "$CDN" "$CDN_PATH" --paths
# wow_classic_era 1.15.5.57716 bc=daf598a3b431384e78214958429863a3 cc=cad72c255e398657916f08081abcb090
run_build "$OUTDIR/wow_classic_era_1.15.5.57716_cad72c25.txt" "$BFT" wow_classic_era daf598a3b431384e78214958429863a3 cad72c255e398657916f08081abcb090 "$CDN" "$CDN_PATH" --paths
# wow_classic_era 1.15.5.57807 bc=c58599dd71e023f741d14ea1db063199 cc=c0775a7270e4012419963171abf66824
run_build "$OUTDIR/wow_classic_era_1.15.5.57807_c0775a72.txt" "$BFT" wow_classic_era c58599dd71e023f741d14ea1db063199 c0775a7270e4012419963171abf66824 "$CDN" "$CDN_PATH" --paths
# wow_classic_era 1.15.5.57917 bc=3aef00205e3dcb461c85178f05af5cb9 cc=0dab4fac4525f15bbb837b35a050387a
run_build "$OUTDIR/wow_classic_era_1.15.5.57917_0dab4fac.txt" "$BFT" wow_classic_era 3aef00205e3dcb461c85178f05af5cb9 0dab4fac4525f15bbb837b35a050387a "$CDN" "$CDN_PATH" --paths
# wow_classic_era 1.15.5.57979 bc=46febe610077a30555618a85064e1ede cc=032371e665628dd25fbd2f8aeb67d233
run_build "$OUTDIR/wow_classic_era_1.15.5.57979_032371e6.txt" "$BFT" wow_classic_era 46febe610077a30555618a85064e1ede 032371e665628dd25fbd2f8aeb67d233 "$CDN" "$CDN_PATH" --paths
# wow_classic_era 1.15.5.58534 bc=de26dc1f646a765401ed6e81724f5ae0 cc=028dad9e13c40eb52ac2d58913564a8f
run_build "$OUTDIR/wow_classic_era_1.15.5.58534_028dad9e.txt" "$BFT" wow_classic_era de26dc1f646a765401ed6e81724f5ae0 028dad9e13c40eb52ac2d58913564a8f "$CDN" "$CDN_PATH" --paths
# wow_classic_era 1.15.5.58555 bc=39a36a3977d8ac13d8b0f6b82fe23748 cc=97b55666a464b990b578541869791a5c
run_build "$OUTDIR/wow_classic_era_1.15.5.58555_97b55666.txt" "$BFT" wow_classic_era 39a36a3977d8ac13d8b0f6b82fe23748 97b55666a464b990b578541869791a5c "$CDN" "$CDN_PATH" --paths
# wow_classic_era 1.15.6.58797 bc=9646dddb4d0b6a5ed34df514a8d7a3de cc=a4376fbd61dc7425d8d09a8d29f0d58e
run_build "$OUTDIR/wow_classic_era_1.15.6.58797_a4376fbd.txt" "$BFT" wow_classic_era 9646dddb4d0b6a5ed34df514a8d7a3de a4376fbd61dc7425d8d09a8d29f0d58e "$CDN" "$CDN_PATH" --paths
# wow_classic_era 1.15.6.58844 bc=14102511a6f6a0b52f3c18cdcf7620db cc=3737e3fc0318efbf3f894b8628b27744
run_build "$OUTDIR/wow_classic_era_1.15.6.58844_3737e3fc.txt" "$BFT" wow_classic_era 14102511a6f6a0b52f3c18cdcf7620db 3737e3fc0318efbf3f894b8628b27744 "$CDN" "$CDN_PATH" --paths
# wow_classic_era 1.15.6.58866 bc=0e27dcb1b261c9ab23fb8a2d5d76402f cc=28f786943e45b226f7981c5b7e3681f0
run_build "$OUTDIR/wow_classic_era_1.15.6.58866_28f78694.txt" "$BFT" wow_classic_era 0e27dcb1b261c9ab23fb8a2d5d76402f 28f786943e45b226f7981c5b7e3681f0 "$CDN" "$CDN_PATH" --paths
# wow_classic_era 1.15.6.58912 bc=c607ffe1fdbe9319768bd1f132573df4 cc=3dca78af23e971dc805995fdf5810775
run_build "$OUTDIR/wow_classic_era_1.15.6.58912_3dca78af.txt" "$BFT" wow_classic_era c607ffe1fdbe9319768bd1f132573df4 3dca78af23e971dc805995fdf5810775 "$CDN" "$CDN_PATH" --paths
# wow_classic_era 1.15.6.59415 bc=a5d0bd2075f11c1c174dc119f3532d61 cc=55666044a894473e6729aecde166f315
run_build "$OUTDIR/wow_classic_era_1.15.6.59415_55666044.txt" "$BFT" wow_classic_era a5d0bd2075f11c1c174dc119f3532d61 55666044a894473e6729aecde166f315 "$CDN" "$CDN_PATH" --paths
# wow_classic_era 1.15.7.60141 bc=1dce7d32478f06afbae4c49d08946516 cc=beba657f0babb9ed59177a8cbde81653
run_build "$OUTDIR/wow_classic_era_1.15.7.60141_beba657f.txt" "$BFT" wow_classic_era 1dce7d32478f06afbae4c49d08946516 beba657f0babb9ed59177a8cbde81653 "$CDN" "$CDN_PATH" --paths
# wow_classic_era 1.15.7.60191 bc=19151c6a12044a02606e9c8668fe19dd cc=791ccb6ecae3bd570b4e081ac3f364a6
run_build "$OUTDIR/wow_classic_era_1.15.7.60191_791ccb6e.txt" "$BFT" wow_classic_era 19151c6a12044a02606e9c8668fe19dd 791ccb6ecae3bd570b4e081ac3f364a6 "$CDN" "$CDN_PATH" --paths
# wow_classic_era 1.15.7.60249 bc=1ea86e87d6adf10212cfe6a1cdd51750 cc=61e0631c93474d832b6c8380c8ec0107
run_build "$OUTDIR/wow_classic_era_1.15.7.60249_61e0631c.txt" "$BFT" wow_classic_era 1ea86e87d6adf10212cfe6a1cdd51750 61e0631c93474d832b6c8380c8ec0107 "$CDN" "$CDN_PATH" --paths
# wow_classic_era 1.15.7.60277 bc=5c7a305f6e08d77895a97a234a553873 cc=8d8b7000af26689a5ab6e89c14be99df
run_build "$OUTDIR/wow_classic_era_1.15.7.60277_8d8b7000.txt" "$BFT" wow_classic_era 5c7a305f6e08d77895a97a234a553873 8d8b7000af26689a5ab6e89c14be99df "$CDN" "$CDN_PATH" --paths
# wow_classic_era 1.15.7.60663 bc=2f851261b60ea001e190a71c5943fdcb cc=5709e81ff5c5d02ffe7a02453ed5f406
run_build "$OUTDIR/wow_classic_era_1.15.7.60663_5709e81f.txt" "$BFT" wow_classic_era 2f851261b60ea001e190a71c5943fdcb 5709e81ff5c5d02ffe7a02453ed5f406 "$CDN" "$CDN_PATH" --paths
# wow_classic_era 1.15.7.60932 bc=45f5b67f71aa62175f504878c948eb3f cc=4c72c7ae2d90993785e47da4f0e9c7a4
run_build "$OUTDIR/wow_classic_era_1.15.7.60932_4c72c7ae.txt" "$BFT" wow_classic_era 45f5b67f71aa62175f504878c948eb3f 4c72c7ae2d90993785e47da4f0e9c7a4 "$CDN" "$CDN_PATH" --paths
# wow_classic_era 1.15.7.61124 bc=006d04562be7163941efb34f9cd62446 cc=435d7276b42f7ae41a8e4a00463ccf35
run_build "$OUTDIR/wow_classic_era_1.15.7.61124_435d7276.txt" "$BFT" wow_classic_era 006d04562be7163941efb34f9cd62446 435d7276b42f7ae41a8e4a00463ccf35 "$CDN" "$CDN_PATH" --paths
# wow_classic_era 1.15.7.61186 bc=9408e3fa546cae1e9ed21addbc0ab00d cc=09031e74823f27011150c4b0aca7087c
run_build "$OUTDIR/wow_classic_era_1.15.7.61186_09031e74.txt" "$BFT" wow_classic_era 9408e3fa546cae1e9ed21addbc0ab00d 09031e74823f27011150c4b0aca7087c "$CDN" "$CDN_PATH" --paths
# wow_classic_era 1.15.7.61257 bc=d762957a75867e5ebe68b7b9ecbb9ff7 cc=cea70cf029e4ff7e8d4fbf497f87e50e
run_build "$OUTDIR/wow_classic_era_1.15.7.61257_cea70cf0.txt" "$BFT" wow_classic_era d762957a75867e5ebe68b7b9ecbb9ff7 cea70cf029e4ff7e8d4fbf497f87e50e "$CDN" "$CDN_PATH" --paths
# wow_classic_era 1.15.7.61582 bc=ae66faee0ac786fdd7d8b4cf90a8d5b9 cc=63eee50d456a6ddf3b630957c024dda0
run_build "$OUTDIR/wow_classic_era_1.15.7.61582_63eee50d.txt" "$BFT" wow_classic_era ae66faee0ac786fdd7d8b4cf90a8d5b9 63eee50d456a6ddf3b630957c024dda0 "$CDN" "$CDN_PATH" --paths
# wow_classic_era 1.15.7.62797 bc=0e734f235fa7c890445b9251fcce0b8f cc=e3555045ae267d5b2ac4e8c7c4871dbb
run_build "$OUTDIR/wow_classic_era_1.15.7.62797_e3555045.txt" "$BFT" wow_classic_era 0e734f235fa7c890445b9251fcce0b8f e3555045ae267d5b2ac4e8c7c4871dbb "$CDN" "$CDN_PATH" --paths
# wow_classic_era 1.15.7.62915 bc=9afe996c33f2bea3630ca57838099f3b cc=8bea1bcdd3984f541ac42638b1522349
run_build "$OUTDIR/wow_classic_era_1.15.7.62915_8bea1bcd.txt" "$BFT" wow_classic_era 9afe996c33f2bea3630ca57838099f3b 8bea1bcdd3984f541ac42638b1522349 "$CDN" "$CDN_PATH" --paths
# wow_classic_era 1.15.7.63306 bc=0a2afc4702b09c65435399210c224aee cc=ed368ee4aff2538cf29bf37104c2bb4a
run_build "$OUTDIR/wow_classic_era_1.15.7.63306_ed368ee4.txt" "$BFT" wow_classic_era 0a2afc4702b09c65435399210c224aee ed368ee4aff2538cf29bf37104c2bb4a "$CDN" "$CDN_PATH" --paths
# wow_classic_era 1.15.7.63696 bc=d91c49f16fb2405954e593d21edf0349 cc=16219b0cf6797cf1690c8e73d314d5b9
run_build "$OUTDIR/wow_classic_era_1.15.7.63696_16219b0c.txt" "$BFT" wow_classic_era d91c49f16fb2405954e593d21edf0349 16219b0cf6797cf1690c8e73d314d5b9 "$CDN" "$CDN_PATH" --paths
# wow_classic_era 1.15.8.63829 bc=6465fa9053e684b72b41c8591860401f cc=3e3b594599aa6ebba08445631868c2aa
run_build "$OUTDIR/wow_classic_era_1.15.8.63829_3e3b5945.txt" "$BFT" wow_classic_era 6465fa9053e684b72b41c8591860401f 3e3b594599aa6ebba08445631868c2aa "$CDN" "$CDN_PATH" --paths
# wow_classic_era 1.15.8.64057 bc=03b8f17f7270da519272a147b99824b7 cc=11b0ce15656cc74a70013186b1ffb2eb
run_build "$OUTDIR/wow_classic_era_1.15.8.64057_11b0ce15.txt" "$BFT" wow_classic_era 03b8f17f7270da519272a147b99824b7 11b0ce15656cc74a70013186b1ffb2eb "$CDN" "$CDN_PATH" --paths
# wow_classic_era 1.15.8.64130 bc=d4e2d1cb74f0aaa5fe127f78e8ce322e cc=69c097ac500d735376d1a964149b1487
run_build "$OUTDIR/wow_classic_era_1.15.8.64130_69c097ac.txt" "$BFT" wow_classic_era d4e2d1cb74f0aaa5fe127f78e8ce322e 69c097ac500d735376d1a964149b1487 "$CDN" "$CDN_PATH" --paths
# wow_classic_era 1.15.8.64272 bc=c11ce61c5c40f3cbde67fcff1a6930f2 cc=7800a9dd03609bbe28e5b5ec31850b47
run_build "$OUTDIR/wow_classic_era_1.15.8.64272_7800a9dd.txt" "$BFT" wow_classic_era c11ce61c5c40f3cbde67fcff1a6930f2 7800a9dd03609bbe28e5b5ec31850b47 "$CDN" "$CDN_PATH" --paths
# wow_classic_era 1.15.8.64344 bc=739833a7e35af276695486a0a024e316 cc=c3ecc1e87a864955320670a245b3236f
run_build "$OUTDIR/wow_classic_era_1.15.8.64344_c3ecc1e8.txt" "$BFT" wow_classic_era 739833a7e35af276695486a0a024e316 c3ecc1e87a864955320670a245b3236f "$CDN" "$CDN_PATH" --paths
# wow_classic_era 1.15.8.64858 bc=2d295897f5551a89eb8ca5daec841516 cc=9bc1c708f84fbe2da881586ce0b21d1e
run_build "$OUTDIR/wow_classic_era_1.15.8.64858_9bc1c708.txt" "$BFT" wow_classic_era 2d295897f5551a89eb8ca5daec841516 9bc1c708f84fbe2da881586ce0b21d1e "$CDN" "$CDN_PATH" --paths
# wow_classic_era 1.15.8.64907 bc=315b6a49a35e26c90bece38e6b60f401 cc=5c6ef48d2fe7d28504313ef0936188f3
run_build "$OUTDIR/wow_classic_era_1.15.8.64907_5c6ef48d.txt" "$BFT" wow_classic_era 315b6a49a35e26c90bece38e6b60f401 5c6ef48d2fe7d28504313ef0936188f3 "$CDN" "$CDN_PATH" --paths
# wow_classic_era 1.15.8.65300 bc=e2dc540a98cccb45d764025ab28b703a cc=b3ea000d5d26fa6615af24a01e181e5f
run_build "$OUTDIR/wow_classic_era_1.15.8.65300_b3ea000d.txt" "$BFT" wow_classic_era e2dc540a98cccb45d764025ab28b703a b3ea000d5d26fa6615af24a01e181e5f "$CDN" "$CDN_PATH" --paths
# wow_classic_era 1.15.8.65888 bc=c3ee0e3dcdd0817395a42757f2321768 cc=a83dcb55a7c076f863610bde509282c8
run_build "$OUTDIR/wow_classic_era_1.15.8.65888_a83dcb55.txt" "$BFT" wow_classic_era c3ee0e3dcdd0817395a42757f2321768 a83dcb55a7c076f863610bde509282c8 "$CDN" "$CDN_PATH" --paths
# wow_classic_era 1.15.8.65989 bc=903cc3552ca1075d5bdc264eab8e2480 cc=ba594faca2339e0b5fcd41254b45cdfa
run_build "$OUTDIR/wow_classic_era_1.15.8.65989_ba594fac.txt" "$BFT" wow_classic_era 903cc3552ca1075d5bdc264eab8e2480 ba594faca2339e0b5fcd41254b45cdfa "$CDN" "$CDN_PATH" --paths
# wow_classic_era 1.15.8.66129 bc=261359eb73293d65a055361231495beb cc=9ed325ff36f6b9abf8c8dc83055b2a38
run_build "$OUTDIR/wow_classic_era_1.15.8.66129_9ed325ff.txt" "$BFT" wow_classic_era 261359eb73293d65a055361231495beb 9ed325ff36f6b9abf8c8dc83055b2a38 "$CDN" "$CDN_PATH" --paths
# wow_classic_era 1.15.8.66564 bc=19ee40225eb91ac0c5808f497ff9457b cc=e96ac31b3e09478731646839347892e4
run_build "$OUTDIR/wow_classic_era_1.15.8.66564_e96ac31b.txt" "$BFT" wow_classic_era 19ee40225eb91ac0c5808f497ff9457b e96ac31b3e09478731646839347892e4 "$CDN" "$CDN_PATH" --paths
# wow_classic_era 1.15.8.67156 bc=20e35f5a4823e522736fcb683d0f0d3d cc=a4ae293c478d89f00ef637d12a0c2e04
run_build "$OUTDIR/wow_classic_era_1.15.8.67156_8610593d.txt" "$BFT" wow_classic_era 20e35f5a4823e522736fcb683d0f0d3d a4ae293c478d89f00ef637d12a0c2e04 "$CDN" "$CDN_PATH" --paths
# wow_classic_era 1.15.9.68808 bc=7f281ee1deb86ea8f6e582795c2a1cfd cc=1113c05066d6b6d4ca706b9bebad3f1f
run_build "$OUTDIR/wow_classic_era_1.15.9.68808_7f281ee1.txt" "$BFT" wow_classic_era 7f281ee1deb86ea8f6e582795c2a1cfd 1113c05066d6b6d4ca706b9bebad3f1f "$CDN" "$CDN_PATH" --paths
# wow_classic_era 1.15.9.68940 bc=f622d60e2229df3f290a83452599308e cc=d7bdbe77b648d19d4a4449c3d2fe7df8
run_build "$OUTDIR/wow_classic_era_1.15.9.68940_f622d60e.txt" "$BFT" wow_classic_era f622d60e2229df3f290a83452599308e d7bdbe77b648d19d4a4449c3d2fe7df8 "$CDN" "$CDN_PATH" --paths
# wow_classic_era 1.15.9.69109 bc=9f9686341092239cfa4812a0ba153dc6 cc=9a824cce21b48ebf0b11367ae32d1597
run_build "$OUTDIR/wow_classic_era_1.15.9.69109_9f968634.txt" "$BFT" wow_classic_era 9f9686341092239cfa4812a0ba153dc6 9a824cce21b48ebf0b11367ae32d1597 "$CDN" "$CDN_PATH" --paths

# wow_classic_titan 3.80.0.64393 bc=d8ac1242c2655856ef94a6410a48b70b cc=f00286961a0ac7fcb3832996306b6759
run_build "$OUTDIR/wow_classic_titan_3.80.0.64393_f0028696.txt" "$BFT" wow_classic_titan d8ac1242c2655856ef94a6410a48b70b f00286961a0ac7fcb3832996306b6759 "$CDN" "$CDN_PATH" --paths
# wow_classic_titan 3.80.0.64859 bc=d65f0d61a1f0c96904e0bd2717cba4a0 cc=9bc1c708f84fbe2da881586ce0b21d1e
run_build "$OUTDIR/wow_classic_titan_3.80.0.64859_9bc1c708.txt" "$BFT" wow_classic_titan d65f0d61a1f0c96904e0bd2717cba4a0 9bc1c708f84fbe2da881586ce0b21d1e "$CDN" "$CDN_PATH" --paths
# wow_classic_titan 3.80.0.65301 bc=5f6e1fa1d7391c9e7419307ce428c016 cc=b3ea000d5d26fa6615af24a01e181e5f
run_build "$OUTDIR/wow_classic_titan_3.80.0.65301_b3ea000d.txt" "$BFT" wow_classic_titan 5f6e1fa1d7391c9e7419307ce428c016 b3ea000d5d26fa6615af24a01e181e5f "$CDN" "$CDN_PATH" --paths
# wow_classic_titan 3.80.0.65586 bc=42a70653767ced54e90168244f668e74 cc=7d932d208d85448a5343f7a15c13aa72
run_build "$OUTDIR/wow_classic_titan_3.80.0.65586_7d932d20.txt" "$BFT" wow_classic_titan 42a70653767ced54e90168244f668e74 7d932d208d85448a5343f7a15c13aa72 "$CDN" "$CDN_PATH" --paths
# wow_classic_titan 3.80.0.65749 bc=2b8f84b7cd756665b45b867d26fafa05 cc=aaf6c86aebc4b04bd0335555d4494a31
run_build "$OUTDIR/wow_classic_titan_3.80.0.65749_aaf6c86a.txt" "$BFT" wow_classic_titan 2b8f84b7cd756665b45b867d26fafa05 aaf6c86aebc4b04bd0335555d4494a31 "$CDN" "$CDN_PATH" --paths
# wow_classic_titan 3.80.0.65797 bc=8f49567e22a85dc35f5f4fa2d5fc5f40 cc=7abd58c9e192f9b3a72387cde526f142
run_build "$OUTDIR/wow_classic_titan_3.80.0.65797_7abd58c9.txt" "$BFT" wow_classic_titan 8f49567e22a85dc35f5f4fa2d5fc5f40 7abd58c9e192f9b3a72387cde526f142 "$CDN" "$CDN_PATH" --paths
# wow_classic_titan 3.80.0.65889 bc=ed0da9dc1e08c69e64329d0ac487fa7a cc=a83dcb55a7c076f863610bde509282c8
run_build "$OUTDIR/wow_classic_titan_3.80.0.65889_a83dcb55.txt" "$BFT" wow_classic_titan ed0da9dc1e08c69e64329d0ac487fa7a a83dcb55a7c076f863610bde509282c8 "$CDN" "$CDN_PATH" --paths
# wow_classic_titan 3.80.0.65990 bc=a24ff13af04976c0ca74e474125ddaa7 cc=ba594faca2339e0b5fcd41254b45cdfa
run_build "$OUTDIR/wow_classic_titan_3.80.0.65990_ba594fac.txt" "$BFT" wow_classic_titan a24ff13af04976c0ca74e474125ddaa7 ba594faca2339e0b5fcd41254b45cdfa "$CDN" "$CDN_PATH" --paths
# wow_classic_titan 3.80.0.66130 bc=c4533dc67a90aad811a3add48ac0c2d7 cc=9ed325ff36f6b9abf8c8dc83055b2a38
run_build "$OUTDIR/wow_classic_titan_3.80.0.66130_9ed325ff.txt" "$BFT" wow_classic_titan c4533dc67a90aad811a3add48ac0c2d7 9ed325ff36f6b9abf8c8dc83055b2a38 "$CDN" "$CDN_PATH" --paths
# wow_classic_titan 3.80.0.66528 bc=e26f318a51d0170861f66607b2b21150 cc=393542519f9f624b288bed93837d2d21
run_build "$OUTDIR/wow_classic_titan_3.80.0.66528_39354251.txt" "$BFT" wow_classic_titan e26f318a51d0170861f66607b2b21150 393542519f9f624b288bed93837d2d21 "$CDN" "$CDN_PATH" --paths
# wow_classic_titan 3.80.0.66566 bc=148fc90d385d81e557d997c7b8a1241e cc=e96ac31b3e09478731646839347892e4
run_build "$OUTDIR/wow_classic_titan_3.80.0.66566_e96ac31b.txt" "$BFT" wow_classic_titan 148fc90d385d81e557d997c7b8a1241e e96ac31b3e09478731646839347892e4 "$CDN" "$CDN_PATH" --paths
# wow_classic_titan 3.80.1.66860 bc=b08c74abb3792cbe4b53ab12d74acb41 cc=72549984137fa9eeb0040a09a0bfa247
run_build "$OUTDIR/wow_classic_titan_3.80.1.66860_72549984.txt" "$BFT" wow_classic_titan b08c74abb3792cbe4b53ab12d74acb41 72549984137fa9eeb0040a09a0bfa247 "$CDN" "$CDN_PATH" --paths
# wow_classic_titan 3.80.1.66932 bc=94d9e16721c34f6ff2b722b64ad8562a cc=46629e30e9d9944df67fa72929e7a2ae
run_build "$OUTDIR/wow_classic_titan_3.80.1.66932_46629e30.txt" "$BFT" wow_classic_titan 94d9e16721c34f6ff2b722b64ad8562a 46629e30e9d9944df67fa72929e7a2ae "$CDN" "$CDN_PATH" --paths
# wow_classic_titan 3.80.1.66991 bc=7d126be4b65527a278343646cefe3d22 cc=1f2a48ea2ac2c9d1d177a51cd1dfe5ad
run_build "$OUTDIR/wow_classic_titan_3.80.1.66991_1f2a48ea.txt" "$BFT" wow_classic_titan 7d126be4b65527a278343646cefe3d22 1f2a48ea2ac2c9d1d177a51cd1dfe5ad "$CDN" "$CDN_PATH" --paths
# wow_classic_titan 3.80.1.67155 bc=b1904f7eda3f0bec22fafd2226a25586 cc=a4ae293c478d89f00ef637d12a0c2e04
run_build "$OUTDIR/wow_classic_titan_3.80.1.67155_a4ae293c.txt" "$BFT" wow_classic_titan b1904f7eda3f0bec22fafd2226a25586 a4ae293c478d89f00ef637d12a0c2e04 "$CDN" "$CDN_PATH" --paths
# wow_classic_titan 3.80.1.67342 bc=a263fa2eb4b77e159b963766d6cc80d3 cc=197773ddf73eb63cd9892a92f1b26301
run_build "$OUTDIR/wow_classic_titan_3.80.1.67342_197773dd.txt" "$BFT" wow_classic_titan a263fa2eb4b77e159b963766d6cc80d3 197773ddf73eb63cd9892a92f1b26301 "$CDN" "$CDN_PATH" --paths
# wow_classic_titan 3.80.1.67400 bc=036ee0b54994d9871d50a312d740458a cc=904eed9d15d661b6600529b41f1550d1
run_build "$OUTDIR/wow_classic_titan_3.80.1.67400_904eed9d.txt" "$BFT" wow_classic_titan 036ee0b54994d9871d50a312d740458a 904eed9d15d661b6600529b41f1550d1 "$CDN" "$CDN_PATH" --paths
# wow_classic_titan 3.80.1.67621 bc=b669838c928ea2d1c732d3178c7ba76e cc=43a220bee3bf82b2025ee4381215b3d9
run_build "$OUTDIR/wow_classic_titan_3.80.1.67621_8610593d.txt" "$BFT" wow_classic_titan b669838c928ea2d1c732d3178c7ba76e 43a220bee3bf82b2025ee4381215b3d9 "$CDN" "$CDN_PATH" --paths
# wow_classic_titan 3.80.1.68044 bc=938dfd94bfb4c3f1dc224fbfe9b8cec8 cc=013cf1d689abf77218fe3b62470333f5
run_build "$OUTDIR/wow_classic_titan_3.80.1.68044_d3d3c884.txt" "$BFT" wow_classic_titan 938dfd94bfb4c3f1dc224fbfe9b8cec8 013cf1d689abf77218fe3b62470333f5 "$CDN" "$CDN_PATH" --paths
# wow_classic_titan 3.80.1.68181 bc=31c00009559de599ec8ecdb878aade00 cc=9b3d90957c290b0ef95808bec3a6b345
run_build "$OUTDIR/wow_classic_titan_3.80.1.68181_a84be43d.txt" "$BFT" wow_classic_titan 31c00009559de599ec8ecdb878aade00 9b3d90957c290b0ef95808bec3a6b345 "$CDN" "$CDN_PATH" --paths
# wow_classic_titan 3.80.1.68318 bc=e1e1b5aea3a7fb428f1fc7fff1b21bf9 cc=709f9f3e64971b2e8155440cbafd25ca
run_build "$OUTDIR/wow_classic_titan_3.80.1.68318_42eace50.txt" "$BFT" wow_classic_titan e1e1b5aea3a7fb428f1fc7fff1b21bf9 709f9f3e64971b2e8155440cbafd25ca "$CDN" "$CDN_PATH" --paths
# wow_classic_titan 3.80.1.68572 bc=82ce555064a8d245c805d2f03456b549 cc=f69dda4c5555d0032b60fde41e840463
run_build "$OUTDIR/wow_classic_titan_3.80.1.68572_82ce5550.txt" "$BFT" wow_classic_titan 82ce555064a8d245c805d2f03456b549 f69dda4c5555d0032b60fde41e840463 "$CDN" "$CDN_PATH" --paths
# wow_classic_titan 3.80.1.68654 bc=9255b9a6b4ec7c10dd64b59ae6e3c923 cc=2afc17881c95bc4ea963a963706f8e02
run_build "$OUTDIR/wow_classic_titan_3.80.1.68654_9255b9a6.txt" "$BFT" wow_classic_titan 9255b9a6b4ec7c10dd64b59ae6e3c923 2afc17881c95bc4ea963a963706f8e02 "$CDN" "$CDN_PATH" --paths
# wow_classic_titan 3.80.1.68768 bc=1110a75034fcde9f356954a3c1f61b69 cc=dda36dd97a1614b3c24c56f3cfd99184
run_build "$OUTDIR/wow_classic_titan_3.80.1.68768_1110a750.txt" "$BFT" wow_classic_titan 1110a75034fcde9f356954a3c1f61b69 dda36dd97a1614b3c24c56f3cfd99184 "$CDN" "$CDN_PATH" --paths
# wow_classic_titan 3.80.1.68805 bc=8d43cf13c55c95b1d7e060f9ce74547c cc=6781d7fe67ca1bccd3381ccc7cfad9bb
run_build "$OUTDIR/wow_classic_titan_3.80.1.68805_8d43cf13.txt" "$BFT" wow_classic_titan 8d43cf13c55c95b1d7e060f9ce74547c 6781d7fe67ca1bccd3381ccc7cfad9bb "$CDN" "$CDN_PATH" --paths
# wow_classic_titan 3.80.2.68943 bc=9bebac287f82ec81b050a88119f2ab46 cc=72c730bef365effe8a1373203e9c8c56
run_build "$OUTDIR/wow_classic_titan_3.80.2.68943_9bebac28.txt" "$BFT" wow_classic_titan 9bebac287f82ec81b050a88119f2ab46 72c730bef365effe8a1373203e9c8c56 "$CDN" "$CDN_PATH" --paths
# wow_classic_titan 3.80.2.69077 bc=368995a1f9b0709c27dcc80552810ebf cc=13ce4301f8cb9fda1cd03d8b32bcf916
run_build "$OUTDIR/wow_classic_titan_3.80.2.69077_368995a1.txt" "$BFT" wow_classic_titan 368995a1f9b0709c27dcc80552810ebf 13ce4301f8cb9fda1cd03d8b32bcf916 "$CDN" "$CDN_PATH" --paths
# wow_classic_titan 3.80.2.69137 bc=b7ccc184ffe73fafb5e6443778754ffe cc=9a824cce21b48ebf0b11367ae32d1597
run_build "$OUTDIR/wow_classic_titan_3.80.2.69137_b7ccc184.txt" "$BFT" wow_classic_titan b7ccc184ffe73fafb5e6443778754ffe 9a824cce21b48ebf0b11367ae32d1597 "$CDN" "$CDN_PATH" --paths

echo "Done. $TOTAL builds: ok=$OK failed=$FAIL skipped=$SKIPPED ($(ls "$OUTDIR" | wc -l) files in $OUTDIR)" >&2
if ((FAIL > 0)); then
	exit 1
fi
