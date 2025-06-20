//! Benchmarks for TACT response parsing

use criterion::{Criterion, criterion_group, criterion_main};
use std::hint::black_box;
use tact_client::response_types::{parse_cdns, parse_versions};

fn bench_parse_versions(c: &mut Criterion) {
    let versions_data = r#"Region!STRING:0|BuildConfig!STRING:0|CDNConfig!STRING:0|KeyRing!STRING:0|BuildId!DEC:4|VersionsName!STRING:0|ProductConfig!STRING:0
us|5a3ae26ccff8e4974b1a6e6c9b5c9526|ec1c51de5c6d93cf900bb3ad963ad366|b0bd7d6e83c97808177b2b2e9aa58cf9|56196|11.0.7.56196|9f3d17789e41ba3bb00f604f1bd96d0f
eu|5a3ae26ccff8e4974b1a6e6c9b5c9526|ec1c51de5c6d93cf900bb3ad963ad366|b0bd7d6e83c97808177b2b2e9aa58cf9|56196|11.0.7.56196|9f3d17789e41ba3bb00f604f1bd96d0f
cn|5a3ae26ccff8e4974b1a6e6c9b5c9526|ec1c51de5c6d93cf900bb3ad963ad366|b0bd7d6e83c97808177b2b2e9aa58cf9|56196|11.0.7.56196|9f3d17789e41ba3bb00f604f1bd96d0f
kr|5a3ae26ccff8e4974b1a6e6c9b5c9526|ec1c51de5c6d93cf900bb3ad963ad366|b0bd7d6e83c97808177b2b2e9aa58cf9|56196|11.0.7.56196|9f3d17789e41ba3bb00f604f1bd96d0f
tw|5a3ae26ccff8e4974b1a6e6c9b5c9526|ec1c51de5c6d93cf900bb3ad963ad366|b0bd7d6e83c97808177b2b2e9aa58cf9|56196|11.0.7.56196|9f3d17789e41ba3bb00f604f1bd96d0f"#;

    c.bench_function("parse_versions", |b| {
        b.iter(|| {
            let result = parse_versions(black_box(versions_data));
            assert!(result.is_ok());
            assert_eq!(result.unwrap().len(), 5);
        })
    });
}

fn bench_parse_cdns(c: &mut Criterion) {
    let cdns_data = r#"Name!STRING:0|Path!STRING:0|Hosts!STRING:0|Servers!STRING:0|ConfigPath!STRING:0
us|tpr/wow|blzddist1-a.akamaihd.net level3.blizzard.com us.cdn.blizzard.com|http://blzddist1-a.akamaihd.net/?maxhosts=4&fallback=1 http://level3.blizzard.com/?maxhosts=4 http://us.cdn.blizzard.com/?maxhosts=4 https://blzddist1-a.akamaihd.net/?maxhosts=4&fallback=1 https://level3.ssl.blizzard.com/?maxhosts=4&fallback=1 https://us.cdn.blizzard.com/?maxhosts=4&fallback=1|tpr/configs/data
eu|tpr/wow|blzddist1-a.akamaihd.net level3.blizzard.com eu.cdn.blizzard.com|http://eu.cdn.blizzard.com/?maxhosts=4 http://level3.blizzard.com/?maxhosts=4 https://blzddist1-a.akamaihd.net/?fallback=1&maxhosts=4 https://eu.cdn.blizzard.com/?maxhosts=4&fallback=1|tpr/configs/data
cn|tpr/wow|client02.pdl.wow.battlenet.com.cn client01.pdl.wow.battlenet.com.cn cdn.blizzard.cn|http://client02.pdl.wow.battlenet.com.cn/?maxhosts=4 http://client01.pdl.wow.battlenet.com.cn/?maxhosts=4 https://cdn.blizzard.cn/?maxhosts=4&fallback=1|tpr/configs/data
kr|tpr/wow|blzddist1-a.akamaihd.net level3.blizzard.com kr.cdn.blizzard.com blzddistkr1-a.akamaihd.net|http://blzddist1-a.akamaihd.net/?maxhosts=4 http://level3.blizzard.com/?maxhosts=4 http://kr.cdn.blizzard.com/?maxhosts=4 https://blzddist1-a.akamaihd.net/?fallback=1&maxhosts=4 https://blzddistkr1-a.akamaihd.net/?fallback=1&maxhosts=4|tpr/configs/data
tw|tpr/wow|level3.blizzard.com us.cdn.blizzard.com|http://level3.blizzard.com/?maxhosts=4 http://us.cdn.blizzard.com/?maxhosts=4 https://level3.ssl.blizzard.com/?fallback=1&maxhosts=4 https://us.cdn.blizzard.com/?maxhosts=4&fallback=1|tpr/configs/data"#;

    c.bench_function("parse_cdns", |b| {
        b.iter(|| {
            let result = parse_cdns(black_box(cdns_data));
            assert!(result.is_ok());
            let entries = result.unwrap();
            assert_eq!(entries.len(), 5);
            // Verify servers are parsed correctly
            assert!(!entries[0].servers.is_empty());
        })
    });
}

fn bench_parse_cdns_with_empty_servers(c: &mut Criterion) {
    let cdns_data = r#"Name!STRING:0|Path!STRING:0|Hosts!STRING:0|Servers!STRING:0|ConfigPath!STRING:0
us|tpr/wow|host1.com host2.com host3.com||tpr/configs/data
eu|tpr/wow|host4.com host5.com host6.com||tpr/configs/data
cn|tpr/wow|host7.com host8.com host9.com||tpr/configs/data"#;

    c.bench_function("parse_cdns_empty_servers", |b| {
        b.iter(|| {
            let result = parse_cdns(black_box(cdns_data));
            assert!(result.is_ok());
            let entries = result.unwrap();
            assert_eq!(entries.len(), 3);
            // Verify servers are empty
            assert!(entries[0].servers.is_empty());
        })
    });
}

fn bench_parse_large_cdns(c: &mut Criterion) {
    // Create a larger dataset with many regions
    let mut cdns_lines = vec![
        "Name!STRING:0|Path!STRING:0|Hosts!STRING:0|Servers!STRING:0|ConfigPath!STRING:0"
            .to_string(),
    ];

    for i in 0..20 {
        cdns_lines.push(format!(
            "region{}|tpr/wow|host{}.com host{}.com host{}.com|http://server{}.com/?maxhosts=4 http://server{}.com/?maxhosts=4 https://server{}.com/?fallback=1|tpr/configs/data",
            i, i*3, i*3+1, i*3+2, i*3, i*3+1, i*3+2
        ));
    }

    let cdns_data = cdns_lines.join("\n");

    c.bench_function("parse_large_cdns", |b| {
        b.iter(|| {
            let result = parse_cdns(black_box(&cdns_data));
            assert!(result.is_ok());
            assert_eq!(result.unwrap().len(), 20);
        })
    });
}

criterion_group!(
    benches,
    bench_parse_versions,
    bench_parse_cdns,
    bench_parse_cdns_with_empty_servers,
    bench_parse_large_cdns
);
criterion_main!(benches);
