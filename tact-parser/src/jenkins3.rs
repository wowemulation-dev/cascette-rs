//! Port of [Bob Jenkins' `lookup3.c`][0] to Rust.
//!
//! These functions are not intended for cryptographic purposes.
//!
//! [0]: https://www.burtleburtle.net/bob/c/lookup3.c

/// Mix 3 `u32` values reversibly.
fn mix(a: &mut u32, b: &mut u32, c: &mut u32) {
    *a = a.wrapping_sub(*c);
    *a ^= c.rotate_left(4);
    *c = c.wrapping_add(*b);

    *b = b.wrapping_sub(*a);
    *b ^= a.rotate_left(6);
    *a = a.wrapping_add(*c);

    *c = c.wrapping_sub(*b);
    *c ^= b.rotate_left(8);
    *b = b.wrapping_add(*a);

    *a = a.wrapping_sub(*c);
    *a ^= c.rotate_left(16);
    *c = c.wrapping_add(*b);

    *b = b.wrapping_sub(*a);
    *b ^= a.rotate_left(19);
    *a = a.wrapping_add(*c);

    *c = c.wrapping_sub(*b);
    *c ^= b.rotate_left(4);
    *b = b.wrapping_add(*a);
}

/// Final mixing of 3 `u32` values.
fn final_(a: &mut u32, b: &mut u32, c: &mut u32) {
    *c ^= *b;
    *c = c.wrapping_sub(b.rotate_left(14));

    *a ^= *c;
    *a = a.wrapping_sub(c.rotate_left(11));

    *b ^= *a;
    *b = b.wrapping_sub(a.rotate_left(25));

    *c ^= *b;
    *c = c.wrapping_sub(b.rotate_left(16));

    *a ^= *c;
    *a = a.wrapping_sub(c.rotate_left(4));

    *b ^= *a;
    *b = b.wrapping_sub(a.rotate_left(14));

    *c ^= *b;
    *c = c.wrapping_sub(b.rotate_left(24));
}

/// Hash a variable-length key into a `u32`.
pub fn hashlittle(key: &[u8], mut initval: u32) -> u32 {
    // hashlittle is hashlittle2, but using pc only
    hashlittle2(key, &mut initval, &mut 0);
    initval
}

/// Returns 2 32-bit hash values, reading `key` in chunks of 3
/// little-endian `u32`s.
pub fn hashlittle2(key: &[u8], pc: &mut u32, pb: &mut u32) {
    let mut a = 0xdeadbeef_u32
        .wrapping_add((key.len() & (u32::MAX as usize)) as u32)
        .wrapping_add(*pc);
    let mut b = a;
    let mut c = a.wrapping_add(*pb);
    let mut k = key;

    if k.is_empty() {
        // Empty strings need no mixing
        *pc = c;
        *pb = b;
        return;
    }

    // The original C version recasted `uint&_t*` as `uint32_t*`, so had to
    // handle alignment issues. Instead, we always copy the data into aligned
    // variables.
    while k.len() > 12 {
        // SAFETY: These unwraps are safe because we check k.len() > 12 above,
        // and the slice ranges [0..4], [4..8], [8..12] are exactly 4 bytes each
        a = a.wrapping_add(u32::from_le_bytes(k[0..4].try_into().unwrap()));
        b = b.wrapping_add(u32::from_le_bytes(k[4..8].try_into().unwrap()));
        c = c.wrapping_add(u32::from_le_bytes(k[8..12].try_into().unwrap()));
        mix(&mut a, &mut b, &mut c);
        k = &k[12..];
    }

    // Handle last, possibly-short block
    //
    // The C implementation does fall-through switch statements with short
    // reads, effectively treating missing high bytes as 0.
    //
    // The simpler implementation is to just make that buffer ourselves.
    let mut final_block = [0; 12];
    final_block[..k.len()].copy_from_slice(k);

    // SAFETY: These unwraps are safe because final_block is exactly 12 bytes,
    // and the slice ranges [0..4], [4..8], [8..12] are exactly 4 bytes each
    a = a.wrapping_add(u32::from_le_bytes(final_block[0..4].try_into().unwrap()));
    if k.len() > 4 {
        b = b.wrapping_add(u32::from_le_bytes(final_block[4..8].try_into().unwrap()));
    }
    if k.len() > 8 {
        c = c.wrapping_add(u32::from_le_bytes(final_block[8..12].try_into().unwrap()));
    }

    final_(&mut a, &mut b, &mut c);

    *pc = c;
    *pb = b;
}
