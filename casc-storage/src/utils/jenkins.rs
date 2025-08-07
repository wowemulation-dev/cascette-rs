//! Jenkins lookup3 hash implementation for CASC

/// Jenkins lookup3 hash function
pub fn jenkins_lookup3(data: &[u8], init_val: u32) -> u32 {
    let mut a = 0xdeadbeef_u32.wrapping_add(data.len() as u32).wrapping_add(init_val);
    let mut b = a;
    let mut c = a;

    let mut i = 0;
    while i + 12 <= data.len() {
        a = a.wrapping_add(u32::from_le_bytes([data[i], data[i+1], data[i+2], data[i+3]]));
        b = b.wrapping_add(u32::from_le_bytes([data[i+4], data[i+5], data[i+6], data[i+7]]));
        c = c.wrapping_add(u32::from_le_bytes([data[i+8], data[i+9], data[i+10], data[i+11]]));

        // Mix
        a = a.wrapping_sub(c); a ^= c.rotate_left(4);  c = c.wrapping_add(b);
        b = b.wrapping_sub(a); b ^= a.rotate_left(6);  a = a.wrapping_add(c);
        c = c.wrapping_sub(b); c ^= b.rotate_left(8);  b = b.wrapping_add(a);
        a = a.wrapping_sub(c); a ^= c.rotate_left(16); c = c.wrapping_add(b);
        b = b.wrapping_sub(a); b ^= a.rotate_left(19); a = a.wrapping_add(c);
        c = c.wrapping_sub(b); c ^= b.rotate_left(4);  b = b.wrapping_add(a);

        i += 12;
    }

    // Handle remaining bytes
    let remaining = &data[i..];
    match remaining.len() {
        11 => {
            c = c.wrapping_add((remaining[10] as u32) << 24);
            c = c.wrapping_add((remaining[9] as u32) << 16);
            c = c.wrapping_add((remaining[8] as u32) << 8);
            b = b.wrapping_add(u32::from_le_bytes([remaining[4], remaining[5], remaining[6], remaining[7]]));
            a = a.wrapping_add(u32::from_le_bytes([remaining[0], remaining[1], remaining[2], remaining[3]]));
        }
        10 => {
            c = c.wrapping_add((remaining[9] as u32) << 16);
            c = c.wrapping_add((remaining[8] as u32) << 8);
            b = b.wrapping_add(u32::from_le_bytes([remaining[4], remaining[5], remaining[6], remaining[7]]));
            a = a.wrapping_add(u32::from_le_bytes([remaining[0], remaining[1], remaining[2], remaining[3]]));
        }
        9 => {
            c = c.wrapping_add((remaining[8] as u32) << 8);
            b = b.wrapping_add(u32::from_le_bytes([remaining[4], remaining[5], remaining[6], remaining[7]]));
            a = a.wrapping_add(u32::from_le_bytes([remaining[0], remaining[1], remaining[2], remaining[3]]));
        }
        8 => {
            b = b.wrapping_add(u32::from_le_bytes([remaining[4], remaining[5], remaining[6], remaining[7]]));
            a = a.wrapping_add(u32::from_le_bytes([remaining[0], remaining[1], remaining[2], remaining[3]]));
        }
        7 => {
            b = b.wrapping_add((remaining[6] as u32) << 24);
            b = b.wrapping_add((remaining[5] as u32) << 16);
            b = b.wrapping_add((remaining[4] as u32) << 8);
            a = a.wrapping_add(u32::from_le_bytes([remaining[0], remaining[1], remaining[2], remaining[3]]));
        }
        6 => {
            b = b.wrapping_add((remaining[5] as u32) << 16);
            b = b.wrapping_add((remaining[4] as u32) << 8);
            a = a.wrapping_add(u32::from_le_bytes([remaining[0], remaining[1], remaining[2], remaining[3]]));
        }
        5 => {
            b = b.wrapping_add((remaining[4] as u32) << 8);
            a = a.wrapping_add(u32::from_le_bytes([remaining[0], remaining[1], remaining[2], remaining[3]]));
        }
        4 => {
            a = a.wrapping_add(u32::from_le_bytes([remaining[0], remaining[1], remaining[2], remaining[3]]));
        }
        3 => {
            a = a.wrapping_add((remaining[2] as u32) << 24);
            a = a.wrapping_add((remaining[1] as u32) << 16);
            a = a.wrapping_add((remaining[0] as u32) << 8);
        }
        2 => {
            a = a.wrapping_add((remaining[1] as u32) << 16);
            a = a.wrapping_add((remaining[0] as u32) << 8);
        }
        1 => {
            a = a.wrapping_add((remaining[0] as u32) << 8);
        }
        _ => {}
    }

    // Final mix
    c ^= b; c = c.wrapping_sub(b.rotate_left(14));
    a ^= c; a = a.wrapping_sub(c.rotate_left(11));
    b ^= a; b = b.wrapping_sub(a.rotate_left(25));
    c ^= b; c = c.wrapping_sub(b.rotate_left(16));
    a ^= c; a = a.wrapping_sub(c.rotate_left(4));
    b ^= a; b = b.wrapping_sub(a.rotate_left(14));
    c ^= b; c = c.wrapping_sub(b.rotate_left(24));

    c
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_jenkins_lookup3() {
        // Basic functionality test - ensure no panics
        jenkins_lookup3(b"", 0);
        jenkins_lookup3(b"test", 0);
        jenkins_lookup3(b"The quick brown fox jumps over the lazy dog", 0);
        
        // Ensure different inputs produce different outputs
        let hash1 = jenkins_lookup3(b"test1", 0);
        let hash2 = jenkins_lookup3(b"test2", 0);
        assert_ne!(hash1, hash2);
    }
}