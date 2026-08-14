// Throwaway cross-verification: decode a BLTE produced by the C++ LZ4HC encoder
// using cascette-rs (lz4_flex), proving bidirectional interop.
use cascette_formats::CascFormat;
use cascette_formats::blte::BlteFile;
use std::env;

fn main() {
    let path = env::args()
        .nth(1)
        .expect("usage: decode_lz4_cross <file.blte>");
    let data = std::fs::read(&path).expect("read");
    let blte = BlteFile::parse(&data).expect("parse");
    let out = blte.decompress().expect("decompress");
    assert_eq!(out.len(), 2000, "decompressed size mismatch");
    for (i, &b) in out.iter().enumerate() {
        assert_eq!(b, (i & 0xFF) as u8, "byte {} mismatch", i);
    }
    println!("OK: decoded {} bytes, all match", out.len());
}
