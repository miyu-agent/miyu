//! 交给程序的环境变量（施工 FH-2下）：包拿到包的文件夹和状态目录，名字照名字表；状态目录照数据根的方法算。

use std::path::{Path, PathBuf};

use super::*;

#[test]
fn a_package_gets_its_folder_and_its_state() {
    let root = DataRoot::at(PathBuf::from("/data"));
    let dirs = PackageDirs::of(&root, "miyu-im", PathBuf::from("/data/packages/miyu-im"));
    assert_eq!(dirs.state, root.package_state("miyu-im"));
    let env = dirs.env(Path::new("/real/state"));
    assert_eq!(
        env,
        [
            ("MIYU_PACKAGE_DIR", PathBuf::from("/data/packages/miyu-im")),
            ("MIYU_PACKAGE_STATE", PathBuf::from("/real/state")),
        ],
        "状态目录交的是建好、换成真实位置的那一个"
    );
}

#[test]
fn every_name_in_a_table_points_at_the_same_place() {
    const TWO: Names = &["A", "B"];
    let place = Path::new("/x");
    let pairs: Vec<_> = named(TWO, place).collect();
    assert_eq!(
        pairs,
        [("A", PathBuf::from("/x")), ("B", PathBuf::from("/x"))]
    );
}
