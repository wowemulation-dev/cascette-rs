use cascette_formats::blte::BlteFile;
use cascette_formats::CascFormat;
use std::env;
fn main() {
    let data = std::fs::read(env::args().nth(1).expect("file")).expect("read");
    let blte = BlteFile::parse(&data).expect("parse");
    let out = blte.decompress().expect("decompress");
    println!("cascette-rs decompressed size: {}", out.len());
}
