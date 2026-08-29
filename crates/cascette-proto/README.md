# cascette-proto

Generated protobuf types for the Blizzard TACT/CASC protocol.

Types are generated at build time from `.proto` schema files using
[prost](https://crates.io/crates/prost). The primary schema is
`proto_database.proto`, which defines the wire format for `.product.db`
and `product.db` files.

## Build requirements

- `protoc` (Protocol Buffers compiler) must be on `PATH`
- Use `mise install` to set up the toolchain from the workspace root
