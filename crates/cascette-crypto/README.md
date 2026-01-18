# cascette-crypto

Cryptographic operations for NGDP/CASC content verification and encryption.

## Status

Working implementation of all required cryptographic operations.

## Features

- MD5 hashing for content keys
- Jenkins96 hash for path lookup
- Salsa20 stream cipher for BLTE encryption
- ARC4 stream cipher for legacy encryption
- TACT key management with 19,000+ known keys

## Components

- `md5` - MD5 hashing for content keys
- `jenkins` - Jenkins96 hash for path lookup
- `salsa20` - Salsa20 stream cipher for BLTE encryption
- `arc4` - ARC4 stream cipher for legacy encryption
- `keys` - TACT key management with 19,000+ known keys

## Dependencies

- `md5` - MD5 hashing
- `thiserror` - Error handling
- `hex` - Hexadecimal encoding

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](../../LICENSE-APACHE) or
  <http://www.apache.org/licenses/LICENSE-2.0>)
- MIT license ([LICENSE-MIT](../../LICENSE-MIT) or
  <http://opensource.org/licenses/MIT>)

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.

---

**Note**: This project is not affiliated with Blizzard Entertainment. It is
an independent implementation based on reverse engineering by the World of
Warcraft emulation community.
