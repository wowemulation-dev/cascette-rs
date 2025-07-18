use std::{collections::BTreeMap, io::Cursor};
use tact_parser::{
    MaybePair,
    config::{BuildConfig, CdnConfig},
};

/// Parse stripped down version of retail CDN config
/// `c8940696493179b5c4f9d59cf4fc9a9b`.
///
/// Most products look like this.
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
        ..Default::default()
    };
    let i = include_str!("data/cdn_config_retail");
    let actual = CdnConfig::parse_config(Cursor::new(i)).unwrap();

    assert_eq!(expected, actual);
}

/// Parse stripped down version of viper CDN config
/// `a998cf3e0a5c9829dd5c2c194dbadc04`.
///
/// This doesn't have any patch records.
#[test]
fn cdn_config_viper() {
    let _ = tracing_subscriber::fmt::try_init();
    let expected = CdnConfig {
        archives: Some(vec![
            *b"\r\x11\xf2\xd8\r\xf6Y\xb7Uv\xaeer\xfcw|",
            *b"\x1b\xd7f\x11\x93\x81z*\xa2\xa9L\xe0\xb14d+",
        ]),
        archives_index_size: Some(vec![49468, 57708]),
        archive_group: Some(*b"\xab\x13\x1e\xbe\x91\xec\xf08[5\xb9\x06\x8d\x8e\x029"),
        file_index: Some(*b"x\x82\x0fh\xbf\xa4\xaaE\x9e\x98\xdbIr\x8f\x1d "),
        file_index_size: Some(32988),
        ..Default::default()
    };
    let i = include_str!("data/cdn_config_viper");
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
        ..Default::default()
    };

    let i = include_str!("data/build_config_retail");
    let actual = BuildConfig::parse_config(Cursor::new(i)).unwrap();

    assert_eq!(expected, actual);
}

/// Parse Battle.net App build config `1737c735e7ad4025146b93fea0cc251f`.
#[test]
fn build_config_bna() {
    let _ = tracing_subscriber::fmt::try_init();
    let expected = BuildConfig {
        root: Some(*b"\xc6\xba\x14\x1e\xe1\xc3\xces\x91d\xec\x99\xe7_ia"),
        install: Some(MaybePair::Pair(
            *b"\x81.xW\xc3w\x0fD\xfc\t\x96O^\\5\xdc",
            *b"\xacw{\x18\xabXs\xff \xdaG\x8f\xd2\xfc_u",
        )),
        install_size: Some(MaybePair::Pair(112287, 109985)),
        download: Some(MaybePair::Pair(
            *b"\xe9\xc17'\x903\xef8\x9ck\xf3UH\xd7v\xbe",
            *b"A\x02NUC77W\xf198\x836\x85U\xe1",
        )),
        download_size: Some(MaybePair::Pair(23701, 21521)),
        size: Some((
            *b"\xaf\x98y\x93\xf8\xf1\xec@\xf7A\x9cK\x03\tM\x94",
            *b"\x14F#\x96`\xe8\r$sx\xca\x0b\x8e6\x0cP",
        )),
        size_size: Some((15011, 12940)),
        encoding: Some(MaybePair::Pair(
            *b"W6\r(\xe7\xf0_\xb7%r,\nn\xd0\x8a\xc3",
            *b"@D\x94\x1a\x1bNi\xe3\xcd`3\xa7\xce\xe2v\xaf",
        )),
        encoding_size: Some(MaybePair::Pair(66196, 66372)),
        patch_index: Some(MaybePair::Pair(
            *b"\xdd\xe3K\xee\xbe\xc64\x95\xc7*\x99\x18 \x97\xda\x0c",
            *b",\x00B>[\xe7\xc4\xb1\x86u\xf5\xfc\x0bo\xe8\xb3",
        )),
        patch_index_size: Some(MaybePair::Pair(24103, 8935)),
        patch: Some(*b"\xbd\xbd\xebv\x00\xc2\x07\x95wH'\x84\x1c\x9f%C"),
        patch_size: Some(13449),
        patch_config: Some(*b"\xef\xd3V\xdd\x8eRY\xc1%7\xb1\x81e\x16R\x16"),
        build_num: Some(15492),
        build_name: Some("15492_release_2.44.1".to_string()),
        build_branch: Some("release/2.44.1".to_string()),
        build_attributes: Some("public".to_string()),
        build_product: Some("Phoenix".to_string()),
        build_uid: Some("bna".to_string()),
        build_critical_patch_seqn: Some(18),
        ..Default::default()
    };

    let i = include_str!("data/build_config_bna");
    let actual = BuildConfig::parse_config(Cursor::new(i)).unwrap();

    assert_eq!(expected, actual);
}

/// Parse stripped-down version of Classic Era build config
/// `ae66faee0ac786fdd7d8b4cf90a8d5b9`.
#[test]
fn build_config_classic_era() {
    let _ = tracing_subscriber::fmt::try_init();
    let expected = BuildConfig {
        root: Some(*b"\xea\x8a\xef\xde\xbd\xbdd)\xda\x90\\\x8cj+\x18\x13"),
        install: Some(MaybePair::Pair(
            *b"T\xc1\x89\xd6\x003\xf9?B\xe7\xb9\x11e\xe7\xde\x1c",
            *b"\xa9\xdc\xeeI\xab?\x95-iD\x1e\xb3\xfd\x91\xc1Y",
        )),
        install_size: Some(MaybePair::Pair(23038, 22281)),
        download: Some(MaybePair::Pair(
            *b"B\xa7\xbb3\xcd\x1e\x9a{r\xbe\xf6\xee\x14q\x9bX",
            *b"S\xba\x96\xf0\x96Z\xdc0m-\x0c\xf3\xb4W\x94\x9c",
        )),
        download_size: Some(MaybePair::Pair(5606744, 4818287)),
        size: Some((
            *b"\xd1\xd9\xe6\x12\xa6E\xccz~KBb\x8b\xde!\xce",
            *b"\rW\x04s_I\x85\xe5U\x90z~vG\t\x9a",
        )),
        size_size: Some((3637629, 3076687)),
        encoding: Some(MaybePair::Pair(
            *b"\xb0{\x88\x1fE'\xbd\xa7\xcf\x8a\x1a/\x99\xe8b.",
            *b"\xbb\xf0ntv8,\xfa\xa3\x96\xcf\xf0\x04\x9d5k",
        )),
        encoding_size: Some(MaybePair::Pair(14004322, 14003043)),
        patch_index: Some(MaybePair::Pair(
            *b"Tr\xee$\xb5\xb9\xd1H\xac\xfd*Co\xc5\x14\xbe",
            *b"v\xce\x88\xec\xb7\x04\xdc\x93\x84\x9d\xef\x9f\xb4\x89\xa6\xfb",
        )),
        patch_index_size: Some(MaybePair::Pair(16783, 6591)),
        patch: Some(*b"O\x18[J\x83}J6;$\x90C*\xae\xf0\x92"),
        patch_size: Some(11017),
        patch_config: Some(*b"GK\x960\xdf[F\xdf]\x98\xec'\xc5\xf7\x8d\x07"),
        build_name: Some("WOW-61582patch1.15.7_ClassicRetail".to_string()),
        build_uid: Some("wow_classic_era".to_string()),
        build_product: Some("WoW".to_string()),
        build_playbuild_installer: Some("ngdptool_casc2".to_string()),
        vfs_root: Some((
            *b"\xe6l\x10\x80NQ\x80\xc6\x1f\xb8YUk\x07\x0eu",
            *b"\x9b:\xce\xdbN\xf7cy\xf3oYI\x9b\xd8\x08\x14",
        )),
        vfs_root_size: Some((14703, 10256)),
        vfs: Some(BTreeMap::from([
            (
                1,
                (
                    *b"\xe6l\x10\x80NQ\x80\xc6\x1f\xb8YUk\x07\x0eu",
                    *b"\x9b:\xce\xdbN\xf7cy\xf3oYI\x9b\xd8\x08\x14",
                ),
            ),
            (
                2,
                (
                    *b"u'H\xad\x87\xe6f\xc5r\xc8\xb1\xa6\xd6\xd8\xcfx",
                    *b"\xb1\xf3D\xb5\xb2hFP%\xa5Q\xe9]V\xd7\x0f",
                ),
            ),
        ])),
        vfs_size: Some(BTreeMap::from([(1, (14703, 10256)), (2, (867, 668))])),
        ..Default::default()
    };

    let i = include_str!("data/build_config_classic_era");
    let actual = BuildConfig::parse_config(Cursor::new(i)).unwrap();

    assert_eq!(expected, actual);
}
