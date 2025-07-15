use std::io::Cursor;

use tact_parser::archive::ArchiveIndexFooter;

#[test]
fn archive_index_test() {
    let _ = tracing_subscriber::fmt::try_init();
    let hash = b"\x00\x17\xa4\x02\xf5V\xfb\xec\xe4l8\xdcC\x1a,\x9b";
    let expected = ArchiveIndexFooter {
        toc_hash: vec![122, 251, 115, 207, 0, 207, 164, 22],
        format_revision: 1,
        flags0: 0,
        flags1: 0,
        block_size_bytes: 4096,
        offset_bytes: 4,
        size_bytes: 4,
        key_bytes: 16,
        hash_bytes: 8,
        num_elements: 7060,
    };

    // Stripped down footer from 0017a402f556fbece46c38dc431a2c9b.index.
    //
    // This puts some dummy data at the start of the index to simulate other
    // entries.
    let mut b = vec![0; 4096 * 3];
    b.append(&mut hex::decode("7afb73cf00cfa4160100000404041008941b0000c2e814eb60ab8cf8").unwrap());

    let actual = ArchiveIndexFooter::parse(&mut Cursor::new(b), &hash).unwrap();
    assert_eq!(expected, actual);
}
