# Encryption and Security - Complete Technical Specification

## Overview

NGDP/TACT implements multiple layers of security including encryption, signature verification, and integrity checking. This document provides complete specifications for all security mechanisms.

## Encryption Systems

### Salsa20 Stream Cipher

#### Algorithm Specification

```c
// Salsa20 Constants
const uint32_t SIGMA[4] = {
    0x61707865,  // "expa"
    0x3320646e,  // "nd 3"
    0x79622d32,  // "2-by"
    0x6b206574   // "te k"
};

const uint32_t TAU[4] = {
    0x61707865,  // "expa"
    0x3120646e,  // "nd 1"
    0x79622d36,  // "6-by"
    0x6b206574   // "te k"
};
```

#### Key Expansion

```python
def salsa20_expand(key, iv, counter=0):
    """
    Expand key and IV into 64-byte block
    
    key: 16 or 32 bytes
    iv: 8 bytes
    counter: 8 bytes (block counter)
    """
    state = [0] * 16
    
    if len(key) == 32:  # 256-bit key
        # Constants "expand 32-byte k"
        state[0] = 0x61707865
        state[5] = 0x3320646e
        state[10] = 0x79622d32
        state[15] = 0x6b206574
        
        # Key material
        state[1:5] = unpack('<4I', key[0:16])
        state[11:15] = unpack('<4I', key[16:32])
    else:  # 128-bit key
        # Constants "expand 16-byte k"
        state[0] = 0x61707865
        state[5] = 0x3120646e
        state[10] = 0x79622d36
        state[15] = 0x6b206574
        
        # Key material (repeated)
        state[1:5] = unpack('<4I', key[0:16])
        state[11:15] = unpack('<4I', key[0:16])
    
    # Counter and IV
    state[6:8] = unpack('<2I', counter)
    state[8:10] = unpack('<2I', iv)
    
    return state
```

#### Core Function

```python
def salsa20_core(state, rounds=20):
    """Apply Salsa20 core function"""
    x = state.copy()
    
    for _ in range(rounds // 2):
        # Column round
        x[4] ^= rotate_left((x[0] + x[12]) & 0xFFFFFFFF, 7)
        x[8] ^= rotate_left((x[4] + x[0]) & 0xFFFFFFFF, 9)
        x[12] ^= rotate_left((x[8] + x[4]) & 0xFFFFFFFF, 13)
        x[0] ^= rotate_left((x[12] + x[8]) & 0xFFFFFFFF, 18)
        
        x[9] ^= rotate_left((x[5] + x[1]) & 0xFFFFFFFF, 7)
        x[13] ^= rotate_left((x[9] + x[5]) & 0xFFFFFFFF, 9)
        x[1] ^= rotate_left((x[13] + x[9]) & 0xFFFFFFFF, 13)
        x[5] ^= rotate_left((x[1] + x[13]) & 0xFFFFFFFF, 18)
        
        x[14] ^= rotate_left((x[10] + x[6]) & 0xFFFFFFFF, 7)
        x[2] ^= rotate_left((x[14] + x[10]) & 0xFFFFFFFF, 9)
        x[6] ^= rotate_left((x[2] + x[14]) & 0xFFFFFFFF, 13)
        x[10] ^= rotate_left((x[6] + x[2]) & 0xFFFFFFFF, 18)
        
        x[3] ^= rotate_left((x[15] + x[11]) & 0xFFFFFFFF, 7)
        x[7] ^= rotate_left((x[3] + x[15]) & 0xFFFFFFFF, 9)
        x[11] ^= rotate_left((x[7] + x[3]) & 0xFFFFFFFF, 13)
        x[15] ^= rotate_left((x[11] + x[7]) & 0xFFFFFFFF, 18)
        
        # Row round
        x[1] ^= rotate_left((x[0] + x[3]) & 0xFFFFFFFF, 7)
        x[2] ^= rotate_left((x[1] + x[0]) & 0xFFFFFFFF, 9)
        x[3] ^= rotate_left((x[2] + x[1]) & 0xFFFFFFFF, 13)
        x[0] ^= rotate_left((x[3] + x[2]) & 0xFFFFFFFF, 18)
        
        x[6] ^= rotate_left((x[5] + x[4]) & 0xFFFFFFFF, 7)
        x[7] ^= rotate_left((x[6] + x[5]) & 0xFFFFFFFF, 9)
        x[4] ^= rotate_left((x[7] + x[6]) & 0xFFFFFFFF, 13)
        x[5] ^= rotate_left((x[4] + x[7]) & 0xFFFFFFFF, 18)
        
        x[11] ^= rotate_left((x[10] + x[9]) & 0xFFFFFFFF, 7)
        x[8] ^= rotate_left((x[11] + x[10]) & 0xFFFFFFFF, 9)
        x[9] ^= rotate_left((x[8] + x[11]) & 0xFFFFFFFF, 13)
        x[10] ^= rotate_left((x[9] + x[8]) & 0xFFFFFFFF, 18)
        
        x[12] ^= rotate_left((x[15] + x[14]) & 0xFFFFFFFF, 7)
        x[13] ^= rotate_left((x[12] + x[15]) & 0xFFFFFFFF, 9)
        x[14] ^= rotate_left((x[13] + x[12]) & 0xFFFFFFFF, 13)
        x[15] ^= rotate_left((x[14] + x[13]) & 0xFFFFFFFF, 18)
    
    # Add original state
    for i in range(16):
        x[i] = (x[i] + state[i]) & 0xFFFFFFFF
    
    return x
```

#### TACT-Specific Implementation

```python
class TACTSalsa20:
    def __init__(self, key_name):
        self.key = self.lookup_key(key_name)
        self.rounds = 20  # Always 20 rounds for TACT
    
    def decrypt_block(self, data, chunk_index=0):
        """
        Decrypt BLTE encrypted block
        
        data format:
        - key_name_size (1 byte)
        - key_name (key_name_size bytes)
        - iv_size (1 byte)
        - iv (iv_size bytes)
        - encryption_type (1 byte, must be 'S')
        - encrypted_data
        """
        offset = 0
        
        # Parse key name
        key_name_size = data[offset]
        offset += 1
        key_name = data[offset:offset+key_name_size]
        offset += key_name_size
        
        # Parse IV
        iv_size = data[offset]
        offset += 1
        iv = data[offset:offset+iv_size]
        offset += iv_size
        
        # Check encryption type
        enc_type = data[offset]
        offset += 1
        if enc_type != ord('S'):
            raise ValueError(f"Expected Salsa20, got {chr(enc_type)}")
        
        # Expand IV for chunk
        expanded_iv = bytearray(8)
        expanded_iv[:len(iv)] = iv
        
        # XOR with chunk index for multi-chunk files
        for i in range(8):
            expanded_iv[i] ^= (chunk_index >> (i * 8)) & 0xFF
        
        # Decrypt
        cipher = Salsa20(self.key, expanded_iv)
        decrypted = cipher.decrypt(data[offset:])
        
        # Decrypted data is another BLTE structure
        return decrypted
```

### ARC4/RC4 Stream Cipher

#### Algorithm Implementation

```python
class ARC4:
    def __init__(self, key):
        """Initialize ARC4 with key"""
        self.state = list(range(256))
        self.x = 0
        self.y = 0
        
        # Key scheduling algorithm (KSA)
        j = 0
        for i in range(256):
            j = (j + self.state[i] + key[i % len(key)]) % 256
            self.state[i], self.state[j] = self.state[j], self.state[i]
    
    def encrypt(self, data):
        """Encrypt/decrypt data (same operation)"""
        output = bytearray()
        
        for byte in data:
            self.x = (self.x + 1) % 256
            self.y = (self.y + self.state[self.x]) % 256
            self.state[self.x], self.state[self.y] = self.state[self.y], self.state[self.x]
            
            k = self.state[(self.state[self.x] + self.state[self.y]) % 256]
            output.append(byte ^ k)
        
        return bytes(output)
    
    decrypt = encrypt  # Symmetric operation
```

#### TACT-Specific Usage

```python
def decrypt_arc4_block(data):
    """Decrypt ARC4 encrypted BLTE block"""
    offset = 0
    
    # Parse header (same as Salsa20)
    key_name_size = data[offset]
    key_name = data[offset+1:offset+1+key_name_size]
    offset += 1 + key_name_size
    
    iv_size = data[offset]
    iv = data[offset+1:offset+1+iv_size]
    offset += 1 + iv_size
    
    enc_type = data[offset]
    offset += 1
    
    if enc_type != ord('A'):  # 'A' for ARC4
        raise ValueError(f"Expected ARC4, got {chr(enc_type)}")
    
    # Get key from key service
    key = lookup_encryption_key(key_name)
    
    # Initialize cipher
    cipher = ARC4(key)
    
    # Discard IV bytes from keystream
    cipher.encrypt(iv)
    
    # Decrypt data
    return cipher.decrypt(data[offset:])
```

## Key Management

### Key Service Architecture

```python
class KeyService:
    def __init__(self):
        self.keys = {}
        self.key_sources = []
    
    def add_key(self, key_name, key_data):
        """Add encryption key"""
        # Key names are 64-bit integers
        if isinstance(key_name, str):
            key_name = int(key_name, 16)
        
        self.keys[key_name] = key_data
    
    def lookup_key(self, key_name):
        """Find encryption key"""
        if isinstance(key_name, bytes):
            # Convert 8-byte key name to integer
            key_name = int.from_bytes(key_name, 'little')
        
        if key_name in self.keys:
            return self.keys[key_name]
        
        # Try external sources
        for source in self.key_sources:
            key = source.get_key(key_name)
            if key:
                self.keys[key_name] = key
                return key
        
        raise KeyError(f"Encryption key {key_name:016x} not found")
```

### Key Sources

```python
class KeyRingSource:
    """Load keys from KeyRing file"""
    
    def __init__(self, keyring_data):
        self.keys = self.parse_keyring(keyring_data)
    
    def parse_keyring(self, data):
        """Parse KeyRing format"""
        keys = {}
        
        # Format: key_name:key_data pairs
        for line in data.decode('utf-8').split('\n'):
            if ':' in line:
                name, key = line.split(':', 1)
                key_name = int(name, 16)
                key_data = bytes.fromhex(key)
                keys[key_name] = key_data
        
        return keys

class TACTKeySource:
    """Load keys from TACT key database"""
    
    KNOWN_KEYS = {
        # World of Warcraft keys
        0xFA505078126ACB3E: bytes.fromhex('BDC51862ABED79B2DE48C8E7E66C6200'),
        0xFF813F7D062AC0BC: bytes.fromhex('AA0B5C77F088CCC2D39049BD267F066D'),
        0xD1E9B5EDF9283668: bytes.fromhex('8E4A2579894E38B4AB9058BA5C7328EE'),
        # ... more keys ...
    }
    
    def get_key(self, key_name):
        return self.KNOWN_KEYS.get(key_name)
```

## Signature Verification

### Ribbit Response Signatures

#### MIME Structure

```
--boundary
Content-Type: application/octet-stream
Content-Encoding: raw

[Response data]

--boundary
Content-Type: application/octet-stream
Content-Encoding: raw

[ASN.1 signature blob]

--boundary--
```

#### ASN.1 Signature Structure

```python
from pyasn1.codec.der import decoder
from cryptography import x509
from cryptography.hazmat.primitives import hashes
from cryptography.hazmat.primitives.asymmetric import padding

def verify_ribbit_signature(data, signature_blob):
    """Verify Ribbit response signature"""
    
    # Parse ASN.1 structure
    content_info, _ = decoder.decode(signature_blob)
    
    # Extract signed data
    signed_data = content_info['content']
    
    # Extract certificates
    certificates = []
    for cert_data in signed_data['certificates']:
        cert = x509.load_der_x509_certificate(cert_data)
        certificates.append(cert)
    
    # Extract signature
    signer_info = signed_data['signerInfos'][0]
    signature = signer_info['signature']
    
    # Verify certificate chain
    verify_certificate_chain(certificates)
    
    # Compute message digest
    message_digest = hashlib.sha256(data).digest()
    
    # Verify signature
    signer_cert = certificates[0]
    public_key = signer_cert.public_key()
    
    public_key.verify(
        signature,
        message_digest,
        padding.PKCS1v15(),
        hashes.SHA256()
    )
```

### Certificate Chain Validation

```python
def verify_certificate_chain(certificates):
    """Validate X.509 certificate chain"""
    
    # Build chain
    store = x509.Store()
    
    # Add trusted root CAs
    for root_ca in TRUSTED_ROOT_CAS:
        store.add_cert(root_ca)
    
    # Verify each certificate
    for i, cert in enumerate(certificates):
        # Check validity period
        now = datetime.utcnow()
        if now < cert.not_valid_before or now > cert.not_valid_after:
            raise ValueError(f"Certificate {i} expired")
        
        # Check signature (except root)
        if i < len(certificates) - 1:
            issuer_cert = certificates[i + 1]
            verify_certificate_signature(cert, issuer_cert)
        
        # Check revocation (OCSP)
        if not check_ocsp_status(cert):
            raise ValueError(f"Certificate {i} revoked")
```

## Integrity Verification

### MD5 Checksums

```python
def verify_content_integrity(data, expected_ckey):
    """Verify file content against CKey"""
    actual_hash = hashlib.md5(data).digest()
    
    if actual_hash != expected_ckey:
        raise IntegrityError(
            f"Content hash mismatch: "
            f"expected {expected_ckey.hex()}, "
            f"got {actual_hash.hex()}"
        )
```

### Multi-Level Verification

```python
class IntegrityVerifier:
    def verify_blte(self, data, expected_ekey):
        """Verify BLTE file integrity"""
        # Level 1: EKey verification
        actual_ekey = hashlib.md5(data).digest()
        assert actual_ekey == expected_ekey
        
        # Level 2: Chunk checksums
        header = parse_blte_header(data)
        for chunk in header.chunks:
            chunk_data = extract_chunk(data, chunk)
            actual_hash = hashlib.md5(chunk_data).digest()
            assert actual_hash == chunk.checksum
        
        # Level 3: Decompressed content
        decompressed = decompress_blte(data)
        content_hash = hashlib.md5(decompressed).digest()
        
        return content_hash
```

## Security Best Practices

### Key Storage

```python
class SecureKeyStorage:
    def __init__(self):
        self.keys = {}
        self._lock = threading.Lock()
    
    def store_key(self, key_name, key_data):
        """Securely store encryption key"""
        with self._lock:
            # Never log keys
            # Store in memory only
            # Clear on shutdown
            self.keys[key_name] = key_data
    
    def __del__(self):
        """Secure cleanup"""
        # Overwrite keys in memory
        for key_name in self.keys:
            key_data = self.keys[key_name]
            # Overwrite with random data
            os.urandom(len(key_data))
        self.keys.clear()
```

### Error Handling

```python
class SecurityError(Exception):
    """Base security exception"""
    pass

class DecryptionError(SecurityError):
    """Decryption failed"""
    pass

class KeyNotFoundError(SecurityError):
    """Encryption key not available"""
    pass

class SignatureError(SecurityError):
    """Signature verification failed"""
    pass

class IntegrityError(SecurityError):
    """Integrity check failed"""
    pass
```

## Rust Implementation

```rust
use salsa20::Salsa20;
use rc4::{Rc4, KeyInit, StreamCipher};
use md5::{Md5, Digest};
use x509_cert::Certificate;

pub struct TactEncryption {
    key_service: KeyService,
}

impl TactEncryption {
    pub fn decrypt_blte_block(&self, data: &[u8]) -> Result<Vec<u8>, SecurityError> {
        let mut offset = 0;
        
        // Parse encryption header
        let key_name_size = data[offset] as usize;
        offset += 1;
        
        let key_name = &data[offset..offset + key_name_size];
        offset += key_name_size;
        
        let iv_size = data[offset] as usize;
        offset += 1;
        
        let iv = &data[offset..offset + iv_size];
        offset += iv_size;
        
        let enc_type = data[offset];
        offset += 1;
        
        match enc_type {
            b'S' => self.decrypt_salsa20(&data[offset..], key_name, iv),
            b'A' => self.decrypt_arc4(&data[offset..], key_name, iv),
            _ => Err(SecurityError::UnknownEncryption(enc_type)),
        }
    }
    
    fn decrypt_salsa20(&self, data: &[u8], key_name: &[u8], iv: &[u8]) -> Result<Vec<u8>, SecurityError> {
        let key = self.key_service.lookup_key(key_name)?;
        
        let mut cipher = Salsa20::new(&key.into(), &iv.into());
        let mut buffer = data.to_vec();
        cipher.apply_keystream(&mut buffer);
        
        Ok(buffer)
    }
}
```

## Known Vulnerabilities and Mitigations

### RC4 Weaknesses

- **Issue**: RC4 has known statistical biases
- **Mitigation**: Prefer Salsa20 for new content
- **Note**: RC4 still used for legacy compatibility

### MD5 Collisions

- **Issue**: MD5 is cryptographically broken
- **Mitigation**: Used only for content addressing, not security
- **Future**: Transition to SHA-256 planned

### Key Distribution

- **Issue**: Keys must be distributed separately
- **Current**: Manual key updates required
- **Future**: Automated key exchange protocol