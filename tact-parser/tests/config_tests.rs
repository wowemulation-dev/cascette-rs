use std::{collections::BTreeMap, io::Cursor};
use tact_parser::{
    MaybePair,
    config::{BuildConfig, CdnConfig},
};

/// Parse stripped down version of retail CDN config
/// `c8940696493179b5c4f9d59cf4fc9a9b`.
#[test]
fn cdn_config_retail() {
    let _ = tracing_subscriber::fmt::try_init();
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

/// Parse stripped-down version of retail build config
/// `bf2689888ce3ac287273acd93158e46b`.
#[test]
fn build_config_retail() {
    let _ = tracing_subscriber::fmt::try_init();
    let expected = BuildConfig {
        root: Some(*b"\xb8\xc4\xc2\xf4\xba\xce(\xf4\xf9\x97\xdd\xbeb\xb6\xc84"),
        install: Some(MaybePair::Pair(
            *b"\xa5}\xbd\x98\x7f\x02\x9ef\x9c\xb7\xc8!\x81\x94s`",
            *b"k\x0e\x16\xe1\xf4_\x83b\xc1\xceH\xfb6f \xe9",
        )),
        install_size: Some(MaybePair::Pair(22214, 21423)),
        download: Some(MaybePair::Pair(
            *b"&\x15\xf4\x0f\xcd\xd3\xb0L\xef\xfe\xaa\x84\xa5X\xd0\xbe",
            *b"7)\xb7\xdf\xba\x01ol\xfb\x0f\x82\xeb\xbe\xd5\xa1\xda",
        )),
        download_size: Some(MaybePair::Pair(65024386, 55581360)),
        size: Some((
            *b"w\xc6]\xf4L\xf7A4\xf7\x8a\xdfk\xfaGg*",
            *b"g\xa0a\x92C\xab+i\x88\xa0\xc3?\xa1\x87\xbf\xbd",
        )),
        size_size: Some((42297509, 35940101)),
        encoding: Some(MaybePair::Pair(
            *b"e\x92xG~\xa9\x9f/7\xd4\xda%\x98n\xfas",
            *b"\x89C\xc5\xeby^\x90\xce\xefk\x95\xf2H\xf4tL",
        )),
        encoding_size: Some(MaybePair::Pair(172053000, 163371663)),
        patch_index: Some(MaybePair::Pair(
            *b"\xe4$\xf6(\xec\xd0\xa0K\xce#\xa3\x9f\xdbF\xac\xdd",
            *b"\xe9%\xd7\xd8\xc3YU4\x02\xc4\x1d\x0eDf\xeb\xde",
        )),
        patch_index_size: Some(MaybePair::Pair(533697, 481110)),
        patch: Some(*b"l\xdep\x9e$\xd8l\xa0\xecw\x05-\xe09p\xb3"),
        patch_size: Some(280909),
        patch_config: Some(*b"\xe1\x00&\xed\x8b\xbe\x05\x9c\xbeH\xd2-\xddO:\xc4"),
        build_name: Some("WOW-61967patch11.1.7_Retail".to_string()),
        build_uid: Some("wow".to_string()),
        build_product: Some("WoW".to_string()),
        build_playbuild_installer: Some("ngdptool_casc2".to_string()),
        build_partial_priority: Some(vec![
            (*b"\"7\"O\xc7 \x8d\x83d\xfe\xd9\xee\xd70f\xac", 262144),
            (*b"/\x86\xa9\xa59\\\xcb\xdd~\xfa .\xe5\x85\xbd\xe1", 262144),
            (*b"\x959B\xf1\x0c1\xd9BiS3D1\xa6I\xe4", 262144),
        ]),
        vfs_root: Some((
            *b"\xd8\x15R\xc9\xc2\x9a57\xa0g\x07\x81rS#\xa4",
            *b"&\x1e\xd7Rn\xa1\x1d\\*Uu\xdf\x12\xb7\x80\xb7",
        )),
        vfs_root_size: Some((50666, 33890)),
        vfs: Some(BTreeMap::from([
            (
                1,
                (
                    *b"\xd8\x15R\xc9\xc2\x9a57\xa0g\x07\x81rS#\xa4",
                    *b"&\x1e\xd7Rn\xa1\x1d\\*Uu\xdf\x12\xb7\x80\xb7",
                ),
            ),
            (
                2,
                (
                    *b"\x08)\xddV\x85b\xd94\x94\x8dv\x9d\xacQ~\x10",
                    *b"\x16\x98\xbe\xe1\x05\x10\xa6\x19\x82\xee\xbb\x06p\x06\x06\x16",
                ),
            ),
        ])),
        vfs_size: Some(BTreeMap::from([(1, (50666, 33890)), (2, (609, 568))])),
    };

    let i = include_str!("data/build_config_retail");
    let actual = BuildConfig::parse_config(Cursor::new(i)).unwrap();

    assert_eq!(expected, actual);
}
