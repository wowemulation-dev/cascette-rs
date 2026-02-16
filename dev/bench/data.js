window.BENCHMARK_DATA = {
  "lastUpdate": 1771260807483,
  "repoUrl": "https://github.com/wowemulation-dev/cascette-rs",
  "entries": {
    "Benchmark": [
      {
        "commit": {
          "author": {
            "email": "daniel@kogito.network",
            "name": "Daniel S. Reichenbach",
            "username": "danielsreichenbach"
          },
          "committer": {
            "email": "daniel@kogito.network",
            "name": "Daniel S. Reichenbach",
            "username": "danielsreichenbach"
          },
          "distinct": true,
          "id": "b2e05d3db1c6cca99ca209855ba14136bda858d6",
          "message": "fix: add write permissions to profiling workflow for gh-pages branch",
          "timestamp": "2026-02-15T18:21:32+07:00",
          "tree_id": "65f26ead7e32d97ad6d02dc4eb1f214b9c92d24b",
          "url": "https://github.com/wowemulation-dev/cascette-rs/commit/b2e05d3db1c6cca99ca209855ba14136bda858d6"
        },
        "date": 1771154717786,
        "tool": "cargo",
        "benches": [
          {
            "name": "bpsv_generation/versions/wow",
            "value": 1868,
            "range": "± 68",
            "unit": "ns/iter"
          },
          {
            "name": "bpsv_generation/cdns/wow",
            "value": 1275,
            "range": "± 20",
            "unit": "ns/iter"
          },
          {
            "name": "bpsv_generation/summary/all",
            "value": 204,
            "range": "± 2",
            "unit": "ns/iter"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "daniel@kogito.network",
            "name": "Daniel S. Reichenbach",
            "username": "danielsreichenbach"
          },
          "committer": {
            "email": "daniel@kogito.network",
            "name": "Daniel S. Reichenbach",
            "username": "danielsreichenbach"
          },
          "distinct": false,
          "id": "f331118faadcb3c34b736644be3964ae9a37e7fc",
          "message": "fix: replace aws-lc-rs with pure-Rust ring crypto provider\n\n- Change reqwest to use rustls-no-provider feature instead of rustls\n  (which pulls in aws-lc-rs by default)\n- Change axum-server to use tls-rustls-no-provider feature\n  (which avoids aws-lc-rs dependency)\n- Add hyper-rustls workspace dependency with ring feature to override\n  default aws-lc-rs selection\n- Remove aws-lc-rs/aws-lc-sys from deny.toml skip list\n- Remove OpenSSL license from allowed licenses\n- Fix salsa20 IV padding: remove duplicate IV copy, correctly zero-pad\n\nAll crypto operations now use ring (pure Rust) instead of aws-lc-rs\n(requires C compiler and native dependencies).",
          "timestamp": "2026-02-15T18:36:38+07:00",
          "tree_id": "9fc4922df3fa35ca89e7b5223a9de3d7c5dcca58",
          "url": "https://github.com/wowemulation-dev/cascette-rs/commit/f331118faadcb3c34b736644be3964ae9a37e7fc"
        },
        "date": 1771165777518,
        "tool": "cargo",
        "benches": [
          {
            "name": "bpsv_generation/versions/wow",
            "value": 1851,
            "range": "± 5",
            "unit": "ns/iter"
          },
          {
            "name": "bpsv_generation/cdns/wow",
            "value": 1217,
            "range": "± 3",
            "unit": "ns/iter"
          },
          {
            "name": "bpsv_generation/summary/all",
            "value": 203,
            "range": "± 2",
            "unit": "ns/iter"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "daniel@kogito.network",
            "name": "Daniel S. Reichenbach",
            "username": "danielsreichenbach"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "a2d5d0561e79c6ba7c6c0f5e361bb647751a1a55",
          "message": "Merge pull request #29 from wowemulation-dev/fix/agent-verified-parser-bugs\n\nfix: correct format parser bugs verified against Agent.exe",
          "timestamp": "2026-02-15T21:47:23+07:00",
          "tree_id": "9c4f1d148783acce2b5878e7e5584d9db59157ba",
          "url": "https://github.com/wowemulation-dev/cascette-rs/commit/a2d5d0561e79c6ba7c6c0f5e361bb647751a1a55"
        },
        "date": 1771166958248,
        "tool": "cargo",
        "benches": [
          {
            "name": "bpsv_generation/versions/wow",
            "value": 1873,
            "range": "± 7",
            "unit": "ns/iter"
          },
          {
            "name": "bpsv_generation/cdns/wow",
            "value": 1222,
            "range": "± 10",
            "unit": "ns/iter"
          },
          {
            "name": "bpsv_generation/summary/all",
            "value": 205,
            "range": "± 1",
            "unit": "ns/iter"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "daniel@kogito.network",
            "name": "Daniel S. Reichenbach",
            "username": "danielsreichenbach"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "33b2270f8cb583da5f545690353dbf3081ee68a9",
          "message": "Merge pull request #30 from wowemulation-dev/feat/size-manifest\n\nfeat: Size manifest parser and ESpec parser fixes",
          "timestamp": "2026-02-15T23:10:59+07:00",
          "tree_id": "ea7ae21f4a83598f83d4830c663e778b6ecdc2c3",
          "url": "https://github.com/wowemulation-dev/cascette-rs/commit/33b2270f8cb583da5f545690353dbf3081ee68a9"
        },
        "date": 1771171973110,
        "tool": "cargo",
        "benches": [
          {
            "name": "bpsv_generation/versions/wow",
            "value": 1836,
            "range": "± 10",
            "unit": "ns/iter"
          },
          {
            "name": "bpsv_generation/cdns/wow",
            "value": 1239,
            "range": "± 15",
            "unit": "ns/iter"
          },
          {
            "name": "bpsv_generation/summary/all",
            "value": 207,
            "range": "± 1",
            "unit": "ns/iter"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "daniel@kogito.network",
            "name": "Daniel S. Reichenbach",
            "username": "danielsreichenbach"
          },
          "committer": {
            "email": "daniel@kogito.network",
            "name": "Daniel S. Reichenbach",
            "username": "danielsreichenbach"
          },
          "distinct": true,
          "id": "e71057c77ea15e9e2ebbe271ee31b65bc916e1bd",
          "message": "chore: remove FUNDING.yml, use org-level",
          "timestamp": "2026-02-16T09:30:40+07:00",
          "tree_id": "7008179b444da4a5b05d44ec212802c6b0247215",
          "url": "https://github.com/wowemulation-dev/cascette-rs/commit/e71057c77ea15e9e2ebbe271ee31b65bc916e1bd"
        },
        "date": 1771209331063,
        "tool": "cargo",
        "benches": [
          {
            "name": "bpsv_generation/versions/wow",
            "value": 1827,
            "range": "± 112",
            "unit": "ns/iter"
          },
          {
            "name": "bpsv_generation/cdns/wow",
            "value": 1216,
            "range": "± 31",
            "unit": "ns/iter"
          },
          {
            "name": "bpsv_generation/summary/all",
            "value": 206,
            "range": "± 1",
            "unit": "ns/iter"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "daniel@kogito.network",
            "name": "Daniel S. Reichenbach",
            "username": "danielsreichenbach"
          },
          "committer": {
            "email": "daniel@kogito.network",
            "name": "Daniel S. Reichenbach",
            "username": "danielsreichenbach"
          },
          "distinct": true,
          "id": "8472bfde9a3ab580146e1378becb5b6690172487",
          "message": "chore: simplify mise configuration\n\nRemove task definitions and shell aliases. Pin tools to latest where\nversion-specific pinning is not needed. Add cross for cross-compilation.",
          "timestamp": "2026-02-16T23:11:53+07:00",
          "tree_id": "ec038811e835babc350114d44b6f62bbc065bfe2",
          "url": "https://github.com/wowemulation-dev/cascette-rs/commit/8472bfde9a3ab580146e1378becb5b6690172487"
        },
        "date": 1771258433430,
        "tool": "cargo",
        "benches": [
          {
            "name": "bpsv_generation/versions/wow",
            "value": 1856,
            "range": "± 6",
            "unit": "ns/iter"
          },
          {
            "name": "bpsv_generation/cdns/wow",
            "value": 1198,
            "range": "± 12",
            "unit": "ns/iter"
          },
          {
            "name": "bpsv_generation/summary/all",
            "value": 204,
            "range": "± 2",
            "unit": "ns/iter"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "daniel@kogito.network",
            "name": "Daniel S. Reichenbach",
            "username": "danielsreichenbach"
          },
          "committer": {
            "email": "daniel@kogito.network",
            "name": "Daniel S. Reichenbach",
            "username": "danielsreichenbach"
          },
          "distinct": true,
          "id": "b90ec4afee45b9dd181b53608c8ac749e881f57c",
          "message": "docs: update changelog with format parser bug fixes",
          "timestamp": "2026-02-16T23:17:01+07:00",
          "tree_id": "46f1e348930dca188f488eb52208bd66f1edef52",
          "url": "https://github.com/wowemulation-dev/cascette-rs/commit/b90ec4afee45b9dd181b53608c8ac749e881f57c"
        },
        "date": 1771258755890,
        "tool": "cargo",
        "benches": [
          {
            "name": "bpsv_generation/versions/wow",
            "value": 1903,
            "range": "± 23",
            "unit": "ns/iter"
          },
          {
            "name": "bpsv_generation/cdns/wow",
            "value": 1235,
            "range": "± 6",
            "unit": "ns/iter"
          },
          {
            "name": "bpsv_generation/summary/all",
            "value": 207,
            "range": "± 0",
            "unit": "ns/iter"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "daniel@kogito.network",
            "name": "Daniel S. Reichenbach",
            "username": "danielsreichenbach"
          },
          "committer": {
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "98041c15829bf72bf4da9a511d47c249a6f613f4",
          "message": "Merge pull request #32 from wowemulation-dev/fix/format-validation\n\nfix: add format validation matching Agent.exe constraints",
          "timestamp": "2026-02-16T23:51:31+07:00",
          "tree_id": "8f32ca232f2665479dd6779f4314b07cacdd66c3",
          "url": "https://github.com/wowemulation-dev/cascette-rs/commit/98041c15829bf72bf4da9a511d47c249a6f613f4"
        },
        "date": 1771260806624,
        "tool": "cargo",
        "benches": [
          {
            "name": "bpsv_generation/versions/wow",
            "value": 1817,
            "range": "± 5",
            "unit": "ns/iter"
          },
          {
            "name": "bpsv_generation/cdns/wow",
            "value": 1198,
            "range": "± 8",
            "unit": "ns/iter"
          },
          {
            "name": "bpsv_generation/summary/all",
            "value": 209,
            "range": "± 1",
            "unit": "ns/iter"
          }
        ]
      }
    ]
  }
}