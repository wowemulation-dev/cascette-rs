//! Port of [Bob Jenkins' `lookup3.c`][0] to Rust.
//!
//! These are not intended for cryptographic purposes.
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
    final_block[..k.len()].copy_from_slice(&k);

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

#[cfg(test)]
mod test {
    use std::collections::HashSet;

    use super::*;

    #[test]
    fn driver4() {
        // Adapted from https://www.burtleburtle.net/bob/c/lookup3.c
        let mut h = 0;
        let mut seen_values = HashSet::new();

        for i in 0..8 {
            h = hashlittle(b"", h);
            assert!(seen_values.insert(h), "duplicate value at {i}: {h}");
        }
    }

    #[test]
    fn driver5() {
        // Adapted from https://www.burtleburtle.net/bob/c/lookup3.c
        // Expected values are expressed as c, b
        let mut b = 0;
        let mut c = 0;
        hashlittle2(b"", &mut c, &mut b);
        assert_eq!(0xdeadbeef, c);
        assert_eq!(0xdeadbeef, b);

        b = 0xdeadbeef;
        c = 0;
        hashlittle2(b"", &mut c, &mut b);
        assert_eq!(0xbd5b7dde, c);
        assert_eq!(0xdeadbeef, b);

        b = 0xdeadbeef;
        c = 0xdeadbeef;
        hashlittle2(b"", &mut c, &mut b);
        assert_eq!(0x9c093ccd, c);
        assert_eq!(0xbd5b7dde, b);

        // Test strings use length of 30, so don't include the null terminator.
        b = 0;
        c = 0;
        hashlittle2(b"Four score and seven years ago", &mut c, &mut b);
        assert_eq!(0x17770551, c);
        assert_eq!(0xce7226e6, b);

        b = 1;
        c = 0;
        hashlittle2(b"Four score and seven years ago", &mut c, &mut b);
        assert_eq!(0xe3607cae, c);
        assert_eq!(0xbd371de4, b);

        b = 0;
        c = 1;
        hashlittle2(b"Four score and seven years ago", &mut c, &mut b);
        assert_eq!(0xcd628161, c);
        assert_eq!(0x6cbea4b3, b);

        assert_eq!(0x17770551, hashlittle(b"Four score and seven years ago", 0));
        assert_eq!(0xcd628161, hashlittle(b"Four score and seven years ago", 1));
    }

    #[test]
    fn ruby() {
        // https://github.com/zed-0xff/hashlittle/blob/master/spec/hashlittle2_spec.rb
        // Expected values are expressed as u64, with b in the high bytes.
        const EXPECTED_VALUES: [u64; 0x100] = [
            0xdeadbeefdeadbeef,
            0x5559b6c48ba9414b,
            0xb29a7b0cdf0d39c9,
            0x95346f906b12f277,
            0x13b4f97be4cf1d42,
            0xea5bae4f1d4833cc,
            0x7d4008d05229ee9e,
            0x57b5bcaf589511d8,
            0x71683417d4ccb93c,
            0x94d25656eca51474,
            0x54b5ea10610fc1e3,
            0x032e5ed42ca6817a,
            0x0cb6e7e95e4aa593,
            0x6bff0f35bc9d6816,
            0x749a42bf9609e9d3,
            0x4391a77d63a6e503,
            0xc62842075e84eafc,
            0x6bb43f658c3532cb,
            0x59cdae44d732003d,
            0x2c0e537474abad21,
            0xb7a4dbc994b659b5,
            0xb5c589d28709c37b,
            0x0aaeb4763190cf5d,
            0x311a6808a5ff3c58,
            0x9f4110029c0add53,
            0x68519ca63a882244,
            0x753ec2a2a91dd5db,
            0x360473c3761df2cd,
            0xd746a3b09255ed6e,
            0xdc09846c9c75ff61,
            0x9d942a7a308ed059,
            0xeecffdb2dfa1dc11,
            0x1759d92121362cfe,
            0x3907f0250e62ec96,
            0x2796f2c6d265c4e3,
            0xe22c5bf7e78281da,
            0x2c078b54653b9ba4,
            0xb28b69dc12926a31,
            0x893f213031c1d4b1,
            0xc06b25ecc81ceeae,
            0x592f87c11c9fa888,
            0xdacb4349d70589e9,
            0x8923764a9e14842a,
            0xe986c32e5db343d9,
            0x147d54673a104df7,
            0x2410e99c0a40ca13,
            0x774a91e70b260a2c,
            0xb7e39ce12088dbc3,
            0xa3beb4f32f4775ab,
            0x63734e2982920552,
            0xd888967016464e80,
            0x331164b39ba06438,
            0x674b580384f3ce0c,
            0x1b8ef32d4725655a,
            0xdf5fc0a7d1ebb17f,
            0xe714da7a8aeb6d5b,
            0x2d470fd79285001f,
            0x19678d4b86c6b345,
            0x58832f43fa7238f1,
            0x3ff24b2c278852ed,
            0x5fdf152bf9bec400,
            0xe0c95f93beab009a,
            0x3bd939ffcb1bf3aa,
            0x46c0482624fcde8a,
            0xb56973a79ff036cc,
            0xafe7259509e22d6f,
            0xe4a649dbdb884e9c,
            0x29b48f490caed6ba,
            0x6ca7578c8da7d795,
            0x47cb255a6e549a11,
            0x3b5c76317269d11b,
            0xb854b56520223a57,
            0xd5f7146184dd8038,
            0xc1e725154e46aa4c,
            0xc56aeecd1bc5fa39,
            0x57eaf76bac4a1927,
            0x9abfc9f40610425c,
            0xb92f541c92d9a532,
            0xf72a9e6ccfb611e1,
            0xc8bd73c35b0553c5,
            0x225cd4ea1055189c,
            0x12847bd8167da6c0,
            0x02d2c5f539860cb1,
            0x6b67d4294af17916,
            0xaae1b3720b3d7409,
            0x3d08036d20990d24,
            0x33cba4d99787aafc,
            0xc24308eca60ad88d,
            0x07e66b8264f05288,
            0x2bf30075d2ba316e,
            0x8b8bc0f3d980a8c4,
            0x3bf716bc4b1c9f85,
            0x6d38b681883a3708,
            0x6cef0bf66ae85365,
            0x3ba915b420e680b9,
            0x8e99d2389bd79302,
            0x6b35c13e593edd31,
            0xfcc9a9152d03691f,
            0x11f8fbc7d77a3dc1,
            0x68bbf75f0ff089f9,
            0xb62a98e9682b4c0b,
            0x6e8207040a689d55,
            0xc2c873b8631d3f70,
            0x5868ceff270bef8b,
            0x09e801326d97f10a,
            0x5384cc34799d3b35,
            0x98e1e4f15813fefc,
            0xfd85dbc30a1864b5,
            0x5dff3844cf12111b,
            0x1d7130b198bbc502,
            0xe598505603bd3c7a,
            0x8a4e816d9467e2bc,
            0x525f4b8404ffccb6,
            0xb767d7e41bb0ae56,
            0x20a032b65e842210,
            0xb28cac5c6fc870c3,
            0x2771902d25caa6ea,
            0xa0303123458eeb83,
            0xe8f20167af14cfa5,
            0xcc435ab2c2e4665f,
            0x841f296066e7d2a3,
            0xbd08544bb709053d,
            0xac28658855c24261,
            0xa24f87a7a032a479,
            0x350a6bed35501134,
            0xbe07d2c4b3b088db,
            0x8ad6c46ea8ed4fd4,
            0x97cf4704fc761aa0,
            0x848cffb24a200281,
            0xe62ccef6da8061b9,
            0xfdfb67ee9e9ce4b1,
            0x055820e5c02f13d4,
            0x79f3d2db50d48f27,
            0x89c1995c11421187,
            0x67902cd4a79a9608,
            0x0f747585ed3e613c,
            0xd64c5b56073b5a3b,
            0x20f754c3c25e859f,
            0x349aba69e6736c9f,
            0x15f70f9164bc43c2,
            0x62af988ba13f664f,
            0x8f9f91279c7d0c40,
            0x8aa0b51085f95456,
            0x0f7f5b0cd64cf605,
            0x2a65b4885155dc33,
            0x44b3c0d50959844f,
            0x1bdd09a227356a1e,
            0x43c73246bebeb84b,
            0x31580a32bb87f13a,
            0x85be2d5db0a6f9cf,
            0xb65eac79db6b37d3,
            0x50857cf322b88eba,
            0x7d0ecdd7d1f2053f,
            0xee3692b2196660b9,
            0x3d393556eaa6c874,
            0x6978110dada985ce,
            0x08803a6989b07339,
            0x868a8379b616ff1f,
            0xb81e106ca6325b04,
            0xbcb698726954113c,
            0x88618ea5aab3aab5,
            0x4e34eb7aa79b1197,
            0x01e89c398972f6a9,
            0x85c78e9ae95433c2,
            0x190c9b47100d09d7,
            0xc0bf0bae1d35877a,
            0x06e60db9468ba086,
            0x5b6e14c540ef34c4,
            0x2034489073557459,
            0x7b429b1fcaeb1e65,
            0xe4282561ee95207c,
            0x1b659a23ae63513e,
            0x55947817af869b46,
            0xd497b20d8c67df87,
            0xfd044f6fb7b6262f,
            0xe9e2512dd592c285,
            0xde98a3aa67e79113,
            0xe9f5652573b72b71,
            0x83281362aa1578e5,
            0xd8abdfaa028568d4,
            0x0aa395ef8e457ef5,
            0x068161a7f22dd334,
            0xf120ce6462aeacb8,
            0x7e2641cb283c60eb,
            0xf98c95cbd2333d25,
            0xa6502b97eda39cc1,
            0xef0351a06449ab55,
            0xf30abb1479b711fc,
            0x3b47c0786bef7247,
            0xbc3be9171e217934,
            0xc045872193fcc6ef,
            0x7cedd3b80dfc94f1,
            0xb74b7a9020336cb3,
            0x77a26a6d7c9aa391,
            0x9dffe57c3dba9c59,
            0x2f563a20f37995c8,
            0x44f0a5063a48798f,
            0xaab08fe9f4f93cce,
            0x6fa174ca1bdae23d,
            0x587d2e5f52a47007,
            0xbf74b2270a07b16b,
            0xe5a45f5ce85cf07b,
            0x76c81d6669d450c5,
            0x5fc21c63ab6c74fa,
            0xdbcd15223eda7834,
            0x9585b4f2e440fa4a,
            0x2673119cf71c0acd,
            0xb25d6792e564f374,
            0x3c43a3898570e765,
            0x2a64a5ba7b653530,
            0x04b2cd71b37434c2,
            0x805e98cc3b7989e5,
            0x8c45bf78609d9c33,
            0xb9c2d826399c76b7,
            0x3f7a7db367997c66,
            0xb2321d695730d33a,
            0x62663fe653255f36,
            0x067c83e963daca6a,
            0xeafad56659a089ef,
            0x2cb693c8f3bb77da,
            0x16788ac3dd68870e,
            0xe04021e18c4c1e92,
            0xc9b3fe4f7823c7a8,
            0x1fcd390249c48792,
            0x5f2d3aa7ad680b0c,
            0xdcb2a35eba7d9fe9,
            0x1c5d0b6970de140f,
            0x30c2c52d5fe6a733,
            0x27caf1eca2b911c7,
            0xb7fbe41a2a886f66,
            0x5c31674d24740ca7,
            0xa3ede800a256e64e,
            0xe94ea18f4cb60cc2,
            0xd7de2fd4da955a60,
            0xde060c9837db05cc,
            0x191272ed13f407fc,
            0xc6b1d38563291acd,
            0xa0a006bf220cb914,
            0x561d7110e64a7535,
            0xcb7994d8104b49a1,
            0x7fcb5058e89e9f6f,
            0x190e460f739c3e07,
            0x50b189c26f4a06c7,
            0xe0dd2b4d58d22f34,
            0x72ce9a57c9907254,
            0x63baedd9b88a3b6c,
            0x68057435ee854134,
            0x90d827d2164b10d2,
            0xca1588535195b673,
            0x9ff868a8e1de05cb,
            0x6cefb654441354f2,
            0xae824a723c88839c,
            0x176af1737f0a6453,
            0x5f7e5aa9f200d16c,
            0x000df2377b285f15,
            0x5899469c7aec5e78,
        ];

        for (i, &expected) in EXPECTED_VALUES.iter().enumerate() {
            let i = i as u8;
            let expected_b = (expected >> 32) as u32;
            let expected_c = (expected & 0xffffffff) as u32;
            let mut b = 0;
            let mut c = 0;

            let s: Vec<u8> = (0..i).collect();

            hashlittle2(s.as_slice(), &mut c, &mut b);
            assert_eq!(
                expected_b, b,
                "at index {i}, {expected_b:#010x} != {b:#010x}"
            );
            assert_eq!(
                expected_c, c,
                "at index {i}, {expected_c:#010x} != {c:#010x}"
            );
        }
    }
}
