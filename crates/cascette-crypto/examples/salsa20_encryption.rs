#![allow(clippy::expect_used, clippy::panic)]

//! BLTE Salsa20 encryption and decryption
//!
//! Demonstrates the Salsa20 stream cipher variant used by CASC for
//! content encryption, including round-trip verification, IV handling,
//! and multi-block BLTE usage.
//!
//! ```text
//! cargo run -p cascette-crypto --example salsa20_encryption
//! ```

use cascette_crypto::salsa20::{Salsa20Cipher, decrypt_salsa20, encrypt_salsa20};

fn main() {
    basic_round_trip();
    iv_size_handling();
    block_index_usage();
    streaming_cipher();
}

fn basic_round_trip() {
    println!("=== Basic Encrypt/Decrypt Round-Trip ===");

    let key: [u8; 16] = [
        0xBD, 0xC5, 0x18, 0x62, 0xAB, 0xED, 0x79, 0xB2, 0xDE, 0x48, 0xC8, 0xE7, 0xE6, 0x6C, 0x62,
        0x00,
    ];
    let iv: [u8; 4] = [0x11, 0x22, 0x33, 0x44];
    let plaintext = b"This is sample BLTE content that would normally be a game asset.";

    println!("Key:       {}", hex::encode(key));
    println!("IV:        {}", hex::encode(iv));
    println!("Plaintext: {} bytes", plaintext.len());

    // Encrypt
    let ciphertext = encrypt_salsa20(plaintext, &key, &iv, 0).expect("encryption should succeed");
    println!(
        "Ciphertext: {} (first 16 bytes)",
        hex::encode(&ciphertext[..16])
    );
    assert_ne!(&ciphertext[..], &plaintext[..]);

    // Decrypt
    let decrypted = decrypt_salsa20(&ciphertext, &key, &iv, 0).expect("decryption should succeed");
    assert_eq!(&decrypted[..], &plaintext[..]);
    println!("Decrypted matches plaintext: verified");
    println!();
}

fn iv_size_handling() {
    println!("=== IV Size Handling (4-byte vs 8-byte) ===");

    let key: [u8; 16] = [0x42; 16];
    let plaintext = b"Testing IV size behavior in CASC Salsa20";

    // 4-byte IV is zero-padded to 8 bytes internally
    let iv4: [u8; 4] = [0x11, 0x22, 0x33, 0x44];
    let iv8_padded: [u8; 8] = [0x11, 0x22, 0x33, 0x44, 0x00, 0x00, 0x00, 0x00];
    let iv8_different: [u8; 8] = [0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88];

    let ct_4byte =
        encrypt_salsa20(plaintext, &key, &iv4, 0).expect("4-byte IV encryption should succeed");
    let ct_8byte_padded = encrypt_salsa20(plaintext, &key, &iv8_padded, 0)
        .expect("8-byte zero-padded IV encryption should succeed");
    let ct_8byte_different = encrypt_salsa20(plaintext, &key, &iv8_different, 0)
        .expect("8-byte IV encryption should succeed");

    println!(
        "4-byte IV {}: {}",
        hex::encode(iv4),
        hex::encode(&ct_4byte[..8])
    );
    println!(
        "8-byte IV {} (zero-padded): {}",
        hex::encode(iv8_padded),
        hex::encode(&ct_8byte_padded[..8])
    );
    println!(
        "8-byte IV {} (different):   {}",
        hex::encode(iv8_different),
        hex::encode(&ct_8byte_different[..8])
    );

    // 4-byte IV zero-padded should produce identical output to explicit 8-byte with trailing zeros
    assert_eq!(ct_4byte, ct_8byte_padded);
    println!("4-byte IV == 8-byte zero-padded: verified");

    // Non-zero upper bytes produce different ciphertext
    assert_ne!(ct_4byte, ct_8byte_different);
    println!("4-byte IV != 8-byte different:   verified");

    // Invalid IV sizes are rejected
    let result = encrypt_salsa20(plaintext, &key, &[0x01, 0x02], 0);
    assert!(result.is_err());
    println!("2-byte IV rejected: verified");

    let result = encrypt_salsa20(plaintext, &key, &[0x01; 6], 0);
    assert!(result.is_err());
    println!("6-byte IV rejected: verified");
    println!();
}

fn block_index_usage() {
    println!("=== Block Index for Multi-Block BLTE ===");

    // BLTE files can contain multiple encoded blocks. Each block is encrypted
    // with the same key and IV, but a different block_index. The block_index
    // is XORed with the first 4 bytes of the IV to derive a per-block nonce.
    let key: [u8; 16] = [0xAA; 16];
    let iv: [u8; 4] = [0x10, 0x20, 0x30, 0x40];
    let block_data = b"Block content that repeats across BLTE blocks";

    println!("Key: {}", hex::encode(key));
    println!("IV:  {}", hex::encode(iv));
    println!();

    let mut ciphertexts = Vec::new();
    for block_index in 0..4 {
        let ct = encrypt_salsa20(block_data, &key, &iv, block_index)
            .expect("block encryption should succeed");
        println!(
            "  Block {block_index}: first 8 bytes = {}",
            hex::encode(&ct[..8])
        );
        ciphertexts.push(ct);
    }

    // Each block index produces different ciphertext even for identical input
    for i in 0..ciphertexts.len() {
        for j in (i + 1)..ciphertexts.len() {
            assert_ne!(ciphertexts[i], ciphertexts[j]);
        }
    }
    println!("All block indices produce distinct ciphertext: verified");

    // Each block decrypts correctly with its own index
    for (block_index, ct) in ciphertexts.iter().enumerate() {
        let decrypted =
            decrypt_salsa20(ct, &key, &iv, block_index).expect("block decryption should succeed");
        assert_eq!(&decrypted[..], &block_data[..]);
    }
    println!("All blocks decrypt correctly with matching index: verified");
    println!();
}

fn streaming_cipher() {
    println!("=== Streaming Cipher with apply_keystream ===");

    let key: [u8; 16] = [0x55; 16];
    let iv: [u8; 4] = [0xAA, 0xBB, 0xCC, 0xDD];
    let plaintext = b"Streaming encryption processes data in-place without allocating.";

    // Salsa20Cipher::new() + apply_keystream() for in-place operation
    let mut cipher = Salsa20Cipher::new(&key, &iv, 0).expect("cipher creation should succeed");

    let mut buffer = plaintext.to_vec();
    println!("Before: {} (first 16 bytes)", hex::encode(&buffer[..16]));

    cipher.apply_keystream(&mut buffer);
    println!("After:  {} (first 16 bytes)", hex::encode(&buffer[..16]));
    assert_ne!(&buffer[..], &plaintext[..]);

    // Decrypt by creating a fresh cipher with the same parameters
    let mut cipher2 = Salsa20Cipher::new(&key, &iv, 0).expect("cipher creation should succeed");
    cipher2.apply_keystream(&mut buffer);
    assert_eq!(&buffer[..], &plaintext[..]);
    println!("Round-trip with streaming cipher: verified");

    // Streaming and one-shot produce identical results
    let one_shot =
        encrypt_salsa20(plaintext, &key, &iv, 0).expect("one-shot encryption should succeed");
    let mut streaming_buf = plaintext.to_vec();
    let mut cipher3 = Salsa20Cipher::new(&key, &iv, 0).expect("cipher creation should succeed");
    cipher3.apply_keystream(&mut streaming_buf);
    assert_eq!(one_shot, streaming_buf);
    println!("Streaming matches one-shot: verified");
}
