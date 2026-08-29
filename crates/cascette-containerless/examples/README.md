# cascette-containerless Examples

## containerless_encrypted

Demonstrates containerless storage with Salsa20 database encryption. Creates an encrypted storage instance, writes files, flushes the encrypted database to disk, reopens it, verifies data survived the round-trip, and exercises direct FileDatabase encrypted export/import.

```bash
cargo run -p cascette-containerless --example containerless_encrypted
```

## containerless_storage

Demonstrates basic containerless file storage operations: opening storage, writing and reading files by encoding key, querying file entries, checking residency, listing files, computing statistics, and running integrity verification.

```bash
cargo run -p cascette-containerless --example containerless_storage
```

## containerless_verification

Verifies containerless storage initialization and round-trip correctness. Creates an in-memory database, confirms the schema is empty, writes a blob, reads it back, and checks storage statistics.

```bash
cargo run -p cascette-containerless --example containerless_verification
```
