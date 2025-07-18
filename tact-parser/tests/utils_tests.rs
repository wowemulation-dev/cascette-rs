use tact_parser::utils::jenkins3_hashpath;

#[test]
fn hashpath() {
    // Verified using WoW TACT root
    let r = jenkins3_hashpath("interface/cinematics/logo_1024.avi");
    assert_eq!(9993239704054654754, r);
}
