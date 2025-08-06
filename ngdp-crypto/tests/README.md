# ngdp-crypto Tests

This directory contains unit tests for the `ngdp-crypto` crate.

## Test Coverage

The tests are located in the source files under `#[cfg(test)]` modules and cover:

### Key Service (`src/key_service.rs`)
- Loading keys from CSV files
- Loading keys from TXT files with descriptions
- Loading keys from TSV files
- Adding keys programmatically
- Key lookup operations
- Hardcoded key verification

### Salsa20 Cipher (`src/salsa20.rs`)
- Key extension (16 bytes → 32 bytes)
- IV handling and block index operations
- Round-trip encryption/decryption
- Invalid input handling

### ARC4 Cipher (`src/arc4.rs`)
- Key construction with IV and block index
- Round-trip encryption/decryption
- Different keys produce different outputs
- Block index affects output
- Empty data handling
- Invalid IV size handling

## Key File Formats Tested

### CSV Format
```
FA505078126ACB3E,BDC51862ABED79B2DE48C8E7E66C6200
```

### TXT Format
```
FA505078126ACB3E BDC51862ABED79B2DE48C8E7E66C6200 Description
```

### TSV Format
```
FA505078126ACB3E	BDC51862ABED79B2DE48C8E7E66C6200
```

## Running Tests

```bash
# Run all ngdp-crypto tests
cargo test -p ngdp-crypto

# Run with output
cargo test -p ngdp-crypto -- --nocapture

# Run specific cipher tests
cargo test -p ngdp-crypto salsa20
cargo test -p ngdp-crypto arc4
cargo test -p ngdp-crypto key_service
```

## Test Data

Tests use known test vectors and synthetic key data to ensure cryptographic operations work correctly. Real encryption key testing is performed using the hardcoded keys included in the library.