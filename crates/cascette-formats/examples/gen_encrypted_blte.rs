// Throwaway: generate an encrypted BLTE via cascette-rs's public BlteBuilder for
// C++ cross-verification. Uses a fixed key/key_name so the C++ test can register
// it in an InMemoryKeyProvider. Inner mode is 'N' (default), matching the
// Agent's 'E'-decode expectation.
use cascette_formats::blte::{BlteBuilder, EncryptionSpec};
use cascette_formats::CascFormat;

const KEY: [u8; 16] = [
    0xDE, 0xAD, 0xBE, 0xEF, 0x00, 0x11, 0x22, 0x33,
    0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xAA, 0xBB,
];
const KEY_NAME: u64 = 0x1234_5678_9ABC_DEF0;
const IV: [u8; 4] = [0xCA, 0xFE, 0xBA, 0xBE];

fn main() {
    // Payload: byte i = (i & 0xFF), 2000 bytes -- multi-chunk at 256B unchecked.
    let payload: Vec<u8> = (0..2000u32).map(|i| (i & 0xFF) as u8).collect();

    let blte = BlteBuilder::new()
        .with_encryption(EncryptionSpec::salsa20(KEY_NAME, IV), KEY)
        .with_chunk_size_unchecked(256)
        .add_data(&payload)
        .expect("add")
        .build()
        .expect("build");
    let bytes = blte.build().expect("serialize");

    std::fs::write("/tmp/cross_encrypted.blte", &bytes).expect("write");
    println!("wrote {} bytes, {} chunks", bytes.len(), payload.len());
}
