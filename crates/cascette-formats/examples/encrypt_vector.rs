// Throwaway: emit an encrypted BLTE chunk (cascette-rs) for C++ cross-verification.
// The encrypted content is ['N'] + payload (inner uncompressed mode), matching
// what the Agent's 'E' decode expects to recurse on.
use cascette_formats::blte::EncryptionSpec;
use cascette_formats::blte::compression::encrypt_chunk_with_key;

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{:02x}", b)).collect()
}

fn main() {
    let key: [u8; 16] = [
        0xDE, 0xAD, 0xBE, 0xEF, 0x00, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xAA,
        0xBB,
    ];
    let key_name: u64 = 0x1234_5678_9ABC_DEF0;
    let iv: [u8; 4] = [0xCA, 0xFE, 0xBA, 0xBE];

    // Inner content = mode 'N' + plaintext payload.
    let payload = b"Encrypted BLTE cross-verify payload.";
    let mut inner = vec![b'N'];
    inner.extend_from_slice(payload);

    let body = encrypt_chunk_with_key(&inner, EncryptionSpec::salsa20(key_name, iv), &key, 0)
        .expect("encrypt");

    // Full 'E' block = 'E' + body (header + encrypted payload).
    let mut block = vec![b'E'];
    block.extend_from_slice(&body);

    println!("key_name {:016x}", key_name);
    println!("key   {}", hex(&key));
    println!("iv    {}", hex(&iv));
    println!("payload_len {}", payload.len());
    println!("payload {}", hex(payload));
    println!("e_block {}", hex(&block));
}
