// Throwaway: emit a Salsa20 test vector from cascette-rs (lz4_flex-independent,
// pure Salsa20) for cross-verification with the C++ Salsa20Cipher.
use cascette_crypto::salsa20::encrypt_salsa20;

fn hex(bytes: &[u8]) -> String {
    bytes.iter().fold(String::new(), |mut s, b| {
        use std::fmt::Write as _;
        let _ = write!(s, "{b:02x}");
        s
    })
}

fn main() {
    let key = [0x42u8; 16];
    let iv = [0x11u8, 0x22, 0x33, 0x44];
    let plaintext = b"Cross-verify Salsa20 CASC variant round-trip.";
    let block_index = 7usize;

    let ct = encrypt_salsa20(plaintext, &key, &iv, block_index).expect("encrypt");

    println!("key   {}", hex(&key));
    println!("iv    {}", hex(&iv));
    println!("idx   {}", block_index);
    println!("plain {}", hex(plaintext));
    println!("cipher {}", hex(&ct));
}
