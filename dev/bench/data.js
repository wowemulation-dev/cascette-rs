window.BENCHMARK_DATA = {
  "lastUpdate": 1771652536680,
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
          "id": "4046e4170f32f6f17831a6ccae62684d53d454f2",
          "message": "docs: update changelog with format validation fixes",
          "timestamp": "2026-02-16T23:58:04+07:00",
          "tree_id": "4d817238c7da3161ca532e11464b116fad9bfdc2",
          "url": "https://github.com/wowemulation-dev/cascette-rs/commit/4046e4170f32f6f17831a6ccae62684d53d454f2"
        },
        "date": 1771261199281,
        "tool": "cargo",
        "benches": [
          {
            "name": "bpsv_generation/versions/wow",
            "value": 1864,
            "range": "± 23",
            "unit": "ns/iter"
          },
          {
            "name": "bpsv_generation/cdns/wow",
            "value": 1235,
            "range": "± 4",
            "unit": "ns/iter"
          },
          {
            "name": "bpsv_generation/summary/all",
            "value": 203,
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
          "id": "4fb9d7e4bf6359b4bd2fdb5b7d19aeb666a32cf5",
          "message": "Merge pull request #33 from wowemulation-dev/fix/config-accessors\n\nfix: add missing typed config accessors",
          "timestamp": "2026-02-17T00:16:13+07:00",
          "tree_id": "4c6c2d3469f63bb7e350ed96df6115e151fc7d2f",
          "url": "https://github.com/wowemulation-dev/cascette-rs/commit/4fb9d7e4bf6359b4bd2fdb5b7d19aeb666a32cf5"
        },
        "date": 1771262284442,
        "tool": "cargo",
        "benches": [
          {
            "name": "bpsv_generation/versions/wow",
            "value": 1869,
            "range": "± 60",
            "unit": "ns/iter"
          },
          {
            "name": "bpsv_generation/cdns/wow",
            "value": 1169,
            "range": "± 4",
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
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "21261da64c1f4ce6d066a5a0253e9012d02487f8",
          "message": "Merge pull request #34 from wowemulation-dev/fix/tvfs-and-root-issues\n\nfix: correct TVFS and root file format issues",
          "timestamp": "2026-02-17T00:58:18+07:00",
          "tree_id": "589fd7a3f355e115324e6d92e4b7a05282edf206",
          "url": "https://github.com/wowemulation-dev/cascette-rs/commit/21261da64c1f4ce6d066a5a0253e9012d02487f8"
        },
        "date": 1771264813557,
        "tool": "cargo",
        "benches": [
          {
            "name": "bpsv_generation/versions/wow",
            "value": 1628,
            "range": "± 5",
            "unit": "ns/iter"
          },
          {
            "name": "bpsv_generation/cdns/wow",
            "value": 1206,
            "range": "± 31",
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
          "id": "a626096debf4e45d680852559e4f77224f21b8e6",
          "message": "Merge pull request #35 from wowemulation-dev/fix/encoding-lookup-and-toc-hash\n\nfix: encoding page lookup and archive group TOC hash",
          "timestamp": "2026-02-17T06:57:21+07:00",
          "tree_id": "4aaeb16dcec07687c495bab21f539e4c0492c4ea",
          "url": "https://github.com/wowemulation-dev/cascette-rs/commit/a626096debf4e45d680852559e4f77224f21b8e6"
        },
        "date": 1771286355460,
        "tool": "cargo",
        "benches": [
          {
            "name": "bpsv_generation/versions/wow",
            "value": 1843,
            "range": "± 17",
            "unit": "ns/iter"
          },
          {
            "name": "bpsv_generation/cdns/wow",
            "value": 1188,
            "range": "± 10",
            "unit": "ns/iter"
          },
          {
            "name": "bpsv_generation/summary/all",
            "value": 204,
            "range": "± 5",
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
          "id": "0a42c6729bd30d4234e91792a8aba3c6f368a007",
          "message": "Merge pull request #36 from wowemulation-dev/feat/port-client-storage\n\nfeat: add cascette-client-storage crate",
          "timestamp": "2026-02-17T15:02:05+07:00",
          "tree_id": "80198bc207c392bcee8386a10a097d7568f28d66",
          "url": "https://github.com/wowemulation-dev/cascette-rs/commit/0a42c6729bd30d4234e91792a8aba3c6f368a007"
        },
        "date": 1771315522828,
        "tool": "cargo",
        "benches": [
          {
            "name": "bpsv_generation/versions/wow",
            "value": 1842,
            "range": "± 39",
            "unit": "ns/iter"
          },
          {
            "name": "bpsv_generation/cdns/wow",
            "value": 1210,
            "range": "± 7",
            "unit": "ns/iter"
          },
          {
            "name": "bpsv_generation/summary/all",
            "value": 209,
            "range": "± 4",
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
          "id": "d2ec0a9f2b56571b9e43b55ffdf8000176387bdd",
          "message": "Merge pull request #37 from wowemulation-dev/fix/formats-agent-comparison\n\nfix: format and protocol alignment issues",
          "timestamp": "2026-02-17T21:19:23+07:00",
          "tree_id": "b81273c1ef8e7c909dec970a0d58e75041bbea35",
          "url": "https://github.com/wowemulation-dev/cascette-rs/commit/d2ec0a9f2b56571b9e43b55ffdf8000176387bdd"
        },
        "date": 1771338161694,
        "tool": "cargo",
        "benches": [
          {
            "name": "bpsv_generation/versions/wow",
            "value": 1531,
            "range": "± 5",
            "unit": "ns/iter"
          },
          {
            "name": "bpsv_generation/cdns/wow",
            "value": 1161,
            "range": "± 8",
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
          "id": "8ea72ca55d94b06229fd03901dfef1971b305482",
          "message": "Merge pull request #38 from wowemulation-dev/refactor/client-storage-agent-compat\n\nrefactor: implement Agent.exe-compatible client-storage architecture",
          "timestamp": "2026-02-19T18:46:34+07:00",
          "tree_id": "f1909697127c47793e168e76f4cc6db35b0f5f18",
          "url": "https://github.com/wowemulation-dev/cascette-rs/commit/8ea72ca55d94b06229fd03901dfef1971b305482"
        },
        "date": 1771501786634,
        "tool": "cargo",
        "benches": [
          {
            "name": "bpsv_generation/versions/wow",
            "value": 1824,
            "range": "± 17",
            "unit": "ns/iter"
          },
          {
            "name": "bpsv_generation/cdns/wow",
            "value": 1238,
            "range": "± 9",
            "unit": "ns/iter"
          },
          {
            "name": "bpsv_generation/summary/all",
            "value": 212,
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
          "id": "75e49f7e9e1f5ff3f87bfd8bd6c41dc6479e4a4d",
          "message": "Merge pull request #39 from wowemulation-dev/fix/install-script-double-v\n\nfix: replace hardcoded install scripts with unified generic versions",
          "timestamp": "2026-02-19T19:02:16+07:00",
          "tree_id": "baf2f8d506748b3ba5acfc689cc9fef2368980d8",
          "url": "https://github.com/wowemulation-dev/cascette-rs/commit/75e49f7e9e1f5ff3f87bfd8bd6c41dc6479e4a4d"
        },
        "date": 1771502652372,
        "tool": "cargo",
        "benches": [
          {
            "name": "bpsv_generation/versions/wow",
            "value": 1871,
            "range": "± 34",
            "unit": "ns/iter"
          },
          {
            "name": "bpsv_generation/cdns/wow",
            "value": 1106,
            "range": "± 17",
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
            "email": "noreply@github.com",
            "name": "GitHub",
            "username": "web-flow"
          },
          "distinct": true,
          "id": "4921cb04058b67775904d08f5dd2498b2fcbcba5",
          "message": "Merge pull request #41 from wowemulation-dev/fix/install-tag-bit-ordering\n\nfix: correct install tag bit mask to MSB-first ordering",
          "timestamp": "2026-02-20T14:14:43+07:00",
          "tree_id": "ec02acd315aaf514be3d47a586bd27275feb14b6",
          "url": "https://github.com/wowemulation-dev/cascette-rs/commit/4921cb04058b67775904d08f5dd2498b2fcbcba5"
        },
        "date": 1771571800781,
        "tool": "cargo",
        "benches": [
          {
            "name": "bpsv_generation/versions/wow",
            "value": 1834,
            "range": "± 36",
            "unit": "ns/iter"
          },
          {
            "name": "bpsv_generation/cdns/wow",
            "value": 1198,
            "range": "± 25",
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
          "id": "727f21df2f17f633c322246d0e16721e7c22acf3",
          "message": "docs: standardize README badges\n\nReorder badges: chat, sponsor, CI, WASM, MSRV, licenses.\nAdd CI status and MSRV badges.",
          "timestamp": "2026-02-20T15:11:14+07:00",
          "tree_id": "027bd3f243cb1c61cab3a4b939309dd06123aaf0",
          "url": "https://github.com/wowemulation-dev/cascette-rs/commit/727f21df2f17f633c322246d0e16721e7c22acf3"
        },
        "date": 1771575210156,
        "tool": "cargo",
        "benches": [
          {
            "name": "bpsv_generation/versions/wow",
            "value": 1817,
            "range": "± 44",
            "unit": "ns/iter"
          },
          {
            "name": "bpsv_generation/cdns/wow",
            "value": 1193,
            "range": "± 9",
            "unit": "ns/iter"
          },
          {
            "name": "bpsv_generation/summary/all",
            "value": 209,
            "range": "± 3",
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
          "id": "c34b418f73d37d14e1553b8277b4236173db9740",
          "message": "chore: sync license files from org and remove outdated CoC\n\nCopy LICENSE-APACHE and LICENSE-MIT from org repo with correct copyright.\nRemove CODE_OF_CONDUCT.md (v1.3.0); org-level v2.1 applies via fallthrough.",
          "timestamp": "2026-02-20T15:20:51+07:00",
          "tree_id": "e8b93c9da77acbba8ae54a6345a8fb774075fadd",
          "url": "https://github.com/wowemulation-dev/cascette-rs/commit/c34b418f73d37d14e1553b8277b4236173db9740"
        },
        "date": 1771575810645,
        "tool": "cargo",
        "benches": [
          {
            "name": "bpsv_generation/versions/wow",
            "value": 1807,
            "range": "± 11",
            "unit": "ns/iter"
          },
          {
            "name": "bpsv_generation/cdns/wow",
            "value": 1179,
            "range": "± 17",
            "unit": "ns/iter"
          },
          {
            "name": "bpsv_generation/summary/all",
            "value": 206,
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
            "email": "daniel@kogito.network",
            "name": "Daniel S. Reichenbach",
            "username": "danielsreichenbach"
          },
          "distinct": true,
          "id": "3cc9139b0abf94212cc8a2d464c8f0899baef81b",
          "message": "docs: update sponsor link and tailor .editorconfig\n\n- Update sponsor link to danielsreichenbach\n- Tailor .editorconfig to repo file types",
          "timestamp": "2026-02-20T15:34:53+07:00",
          "tree_id": "44d21dfe094480e51b944890027f70f6e7f102ab",
          "url": "https://github.com/wowemulation-dev/cascette-rs/commit/3cc9139b0abf94212cc8a2d464c8f0899baef81b"
        },
        "date": 1771576625192,
        "tool": "cargo",
        "benches": [
          {
            "name": "bpsv_generation/versions/wow",
            "value": 1821,
            "range": "± 14",
            "unit": "ns/iter"
          },
          {
            "name": "bpsv_generation/cdns/wow",
            "value": 1179,
            "range": "± 4",
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
          "id": "a7af0dda53ceded2572f0e43547e1aa38c6fabce",
          "message": "Merge pull request #43 from wowemulation-dev/fix/macos-variadic-mode-t-cast\n\nfix: cast mode_t to c_uint for variadic shm_open on macOS",
          "timestamp": "2026-02-21T08:07:01+07:00",
          "tree_id": "45d1fbf5aac548006591f7ded4437734791017f3",
          "url": "https://github.com/wowemulation-dev/cascette-rs/commit/a7af0dda53ceded2572f0e43547e1aa38c6fabce"
        },
        "date": 1771636148235,
        "tool": "cargo",
        "benches": [
          {
            "name": "bpsv_generation/versions/wow",
            "value": 1514,
            "range": "± 17",
            "unit": "ns/iter"
          },
          {
            "name": "bpsv_generation/cdns/wow",
            "value": 1140,
            "range": "± 2",
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
          "id": "ab4f4e1e7a294947eda5fcbeaf150049ea3e3783",
          "message": "chore: add trailing newline to .gitignore",
          "timestamp": "2026-02-21T12:28:28+07:00",
          "tree_id": "32260937164b1bd7a418d8f9102156746cfe0eb7",
          "url": "https://github.com/wowemulation-dev/cascette-rs/commit/ab4f4e1e7a294947eda5fcbeaf150049ea3e3783"
        },
        "date": 1771651859663,
        "tool": "cargo",
        "benches": [
          {
            "name": "bpsv_generation/versions/wow",
            "value": 1849,
            "range": "± 38",
            "unit": "ns/iter"
          },
          {
            "name": "bpsv_generation/cdns/wow",
            "value": 1129,
            "range": "± 11",
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
          "id": "d43f377dc4bde40f556df728e3cf5bf3f29d9e2f",
          "message": "fix: use absolute path for mdbook in readthedocs build\n\nThe cargo install directory may not be in PATH during the build step.",
          "timestamp": "2026-02-21T12:40:22+07:00",
          "tree_id": "bad157161d29125742ce8066f40be1734fece22e",
          "url": "https://github.com/wowemulation-dev/cascette-rs/commit/d43f377dc4bde40f556df728e3cf5bf3f29d9e2f"
        },
        "date": 1771652536351,
        "tool": "cargo",
        "benches": [
          {
            "name": "bpsv_generation/versions/wow",
            "value": 1790,
            "range": "± 49",
            "unit": "ns/iter"
          },
          {
            "name": "bpsv_generation/cdns/wow",
            "value": 1209,
            "range": "± 6",
            "unit": "ns/iter"
          },
          {
            "name": "bpsv_generation/summary/all",
            "value": 207,
            "range": "± 2",
            "unit": "ns/iter"
          }
        ]
      }
    ]
  }
}