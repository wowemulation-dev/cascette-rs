use std::io::Cursor;

use tact_parser::config::CdnConfig;

/// Parse stripped down version of retail CDN config
/// `c8940696493179b5c4f9d59cf4fc9a9b`.
#[test]
fn retail() {
    let expected = CdnConfig {
        archives: Some(vec![
            *b"\x00\x17\xa4\x02\xf5V\xfb\xec\xe4l8\xdcC\x1a,\x9b",
            *b"\x00;\x14w0\xa1\t\xe3\xa4\x80\xd3*T(\tU",
        ]),
        archives_index_size: Some(vec![135988, 173068]),
        archive_group: Some(*b"k:$\xb724m\xe5\xc0\x91\"\xc4R\xfek1"),
        patch_archives: Some(vec![
            *b"\x06\x11\x8e\xd7\xd0\xb9\x97\xd4\x91\n\xa3\xd6\x9c\xfaQ\xe6",
            *b"\x063\xb2dZ\xc4\xf3\xf9\xe05\xe9Jj\x19\xe8\xa9",
        ]),
        patch_archives_index_size: Some(vec![564468, 535628]),
        patch_archive_group: Some(*b"w\x14h\x80\xb8\xf6\x96\x81B\x07\x1d\xb0ls@D"),
        file_index: Some(*b"\x12(#\x9c\x8e5\xfc\xb2\xcf\xcb\x8d\xa9/c\x81\xf6"),
        file_index_size: Some(420268),
        patch_file_index: Some(*b"\xa8h\x13\xf8 \xb1\xed\x9cM\xc2q: \x99\x18|"),
        patch_file_index_size: Some(181308),
        builds: None,
    };
    let i = include_str!("data/cdn_config_retail");
    let actual = CdnConfig::parse_config(Cursor::new(i)).unwrap();

    assert_eq!(expected, actual);
}
