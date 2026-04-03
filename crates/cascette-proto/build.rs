#![allow(clippy::expect_used)]

fn main() {
    prost_build::compile_protos(&["proto/proto_database.proto"], &["proto/"])
        .expect("failed to compile proto_database.proto");
}
