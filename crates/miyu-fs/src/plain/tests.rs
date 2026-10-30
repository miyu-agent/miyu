use std::path::Path;

use super::plain;

#[test]
fn the_verbatim_prefix_is_dropped_and_unc_comes_back_as_two_slashes() {
    assert_eq!(
        plain(Path::new(r"\\?\C:\Users\me\.miyu\workspace")),
        r"C:\Users\me\.miyu\workspace"
    );
    assert_eq!(plain(Path::new(r"\\?\UNC\host\share\a")), r"\\host\share\a");
    assert_eq!(plain(Path::new(r"C:\plain")), r"C:\plain");
    assert_eq!(plain(Path::new("/home/u/a.rs")), "/home/u/a.rs");
}
