use std::{io::Cursor, path::PathBuf};
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
        Some(PathBuf::from("interface/cinematics/logo_1024.avi").as_path()),
        parser.get_path_for_fid(21),
    );

    assert_eq!(Some(99), parser.get_fid_from_path("fake/path.exe"));

    Ok(())
}

#[test]
fn normalise_listfile() {
    assert_eq!(
        listfile_normalise("interface/cinematics/logo_1024.avi"),
        PathBuf::from("interface/cinematics/logo_1024.avi"),
    );
    assert_eq!(
        listfile_normalise("interface\\cinematics\\logo_1024.avi"),
        PathBuf::from("interface/cinematics/logo_1024.avi"),
    );
    assert_eq!(
        listfile_normalise("InTeRfAcE\\CiNeMaTiCs/LOGo_1024.aVI"),
        PathBuf::from("interface/cinematics/logo_1024.avi"),
    );
}
