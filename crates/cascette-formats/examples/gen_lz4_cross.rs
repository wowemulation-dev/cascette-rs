// Throwaway cross-verification generator: builds an LZ4 BLTE with cascette-rs
// (lz4_flex) so the C++ LZ4HCDecoder can decode it, proving interop.
use cascette_formats::blte::{BlteBuilder, CompressionMode};
use cascette_formats::CascFormat;

fn main() {
    let data: Vec<u8> = (0..2000u32).map(|i| (i & 0xFF) as u8).collect();

    let single = BlteBuilder::new()
        .with_compression(CompressionMode::LZ4)
        .add_data(&data)
        .expect("add")
        .build()
        .expect("build");
    let bytes = single.build().expect("serialize");
    std::fs::write("/tmp/cross_single_lz4.blte", &bytes).expect("write single");

    let multi = BlteBuilder::new()
        .with_compression(CompressionMode::LZ4)
        .with_chunk_size_unchecked(256)
        .add_data(&data)
        .expect("add")
        .build()
        .expect("build");
    let bytes = multi.build().expect("serialize");
    std::fs::write("/tmp/cross_multi_lz4.blte", &bytes).expect("write multi");

    println!("single={} multi={}", data.len(), data.len());
}
