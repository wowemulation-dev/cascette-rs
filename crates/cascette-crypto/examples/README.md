# cascette-crypto Examples

## armadillo_keys

Demonstrates Armadillo `.ak` file parsing, hex key parsing, round-trip serialization, and `ChainedKeyProvider` priority-based key lookup.

```sh
cargo run -p cascette-crypto --example armadillo_keys
```

No prerequisites. Uses hardcoded test data.

## crypto_primitives

Demonstrates MD5-based ContentKey and EncodingKey generation, Jenkins96 name hashing for root file lookups, `hashlittle`/`hashlittle2` with reference test vectors, IDX bucket calculation, and FileDataId operations.

```sh
cargo run -p cascette-crypto --example crypto_primitives
```

No prerequisites. Uses hardcoded test data.

## salsa20_encryption

Demonstrates the Salsa20 stream cipher used by CASC for BLTE content encryption, including round-trip encrypt/decrypt, 4-byte vs 8-byte IV handling, per-block nonce derivation, and streaming vs one-shot cipher modes.

```sh
cargo run -p cascette-crypto --example salsa20_encryption
```

No prerequisites. Uses hardcoded test data.

## tact_keyring

Demonstrates the TACT encryption key store: hardcoded key listing, key lookup by ID, manual key addition, loading keys from CSV and TXT formats, and the `UnifiedKeyStore` trait abstraction.

```sh
cargo run -p cascette-crypto --example tact_keyring
```

No prerequisites. Uses hardcoded test data.
