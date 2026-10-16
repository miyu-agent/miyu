//! 包文件夹在哪（施工 S-6 上）：设了 `MIYU_PACKAGE_DIR` 的照它，没设、空的退回资源目录里出厂的那一份。

use std::ffi::OsStr;
use std::path::Path;

use super::*;

#[test]
fn the_given_folder_wins_and_an_empty_or_missing_one_falls_back() {
    let resources = ResourceRoot::at("/opt/miyu/resources");
    let given = Package::locate(
        Some(OsStr::new("/data/home/admin/packages/onebot")),
        &resources,
    );
    assert_eq!(given.dir(), Path::new("/data/home/admin/packages/onebot"));
    assert_eq!(
        given.data(),
        Path::new("/data/home/admin/packages/onebot").join("data")
    );
    let shipped = Path::new("/opt/miyu/resources")
        .join("packages")
        .join("onebot");
    for missing in [None, Some(OsStr::new(""))] {
        let found = Package::locate(missing, &resources);
        assert_eq!(found.dir(), shipped, "{missing:?}");
        assert_eq!(found.data(), shipped.join("data"));
    }
}
