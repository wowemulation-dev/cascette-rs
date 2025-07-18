use tact_parser::wow_root::*;

#[test]
fn locales() {
    let en_us_only = LocaleFlags::new().with_en_us(true);
    assert_eq!(en_us_only, LocaleFlags::from(0x2));

    assert!(LocaleFlags::from(0xffffffff).en_us());
}

#[test]
fn content_flags() {
    let win64 = ContentFlags::new()
        .with_windows(true)
        .with_x86_64(true)
        .with_install(true);
    assert_eq!(win64, ContentFlags::from(0x4c));
}
