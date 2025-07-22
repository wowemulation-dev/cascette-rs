use std::io::Cursor;
use tact_parser::listfile::{ListfileNameResolver, listfile_normalise};

/// Test with a fake version of the community listfile.
#[test]
fn test_listfile() -> Result<(), Box<dyn std::error::Error>> {
    let i = include_str!("data/fake_listfile.txt");
    let mut f = Cursor::new(i);

    let parser = ListfileNameResolver::new(&mut f)?;
    assert_eq!(
        Some(21),
        parser.get_fid_from_path("interface/cinematics/logo_1024.avi"),
    );
    assert_eq!(
        Some(21),
        parser.get_fid_from_path("interface\\cinematics\\logo_1024.avi"),
    );
    assert_eq!(
        Some(21),
        parser.get_fid_from_path("Interface\\Cinematics\\logo_1024.avi"),
    );
    assert_eq!(None, parser.get_fid_from_path("eicar.com"));
    assert_eq!(
        None,
        parser.get_fid_from_path("C:\\windows\\system32\\drivers\\etc\\hosts")
    );

    assert_eq!(
        Some("interface/cinematics/logo_1024.avi"),
        parser.get_path_for_fid(21),
    );

    assert_eq!(Some(99), parser.get_fid_from_path("fake/path.exe"));

    Ok(())
}

#[test]
fn normalise_listfile() {
    let tests = vec![
        // expected, input
        ("a/b/c.avi", "a/b/c.avi"),
        ("a/b/c.avi", "a\\b\\c.avi"),
        ("a/b/c.avi", "a\\..\\..\\b\\c.avi"),
        ("a/foo/b/c.avi", "\\\\a\\foo\\..\\b\\c.avi"),
        ("http/example.com/foo.txt", "http://example.com/foo.txt"),
        ("a/bcde/f/g", "a/BcDe/f/g"),
        ("foo/bar", "foo/bar/"),
    ];

    for (expected, input) in tests.iter() {
        let actual = listfile_normalise(input);
        assert_eq!(expected, &actual, "for {input}");
    }
}
