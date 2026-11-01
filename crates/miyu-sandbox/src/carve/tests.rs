use std::collections::BTreeMap;

use super::*;

/// 假的目录树：每个目录下面有哪几项。
fn tree(dirs: &[(&str, &[&str])]) -> impl Fn(&Path) -> io::Result<Vec<PathBuf>> {
    let map: BTreeMap<PathBuf, Vec<PathBuf>> = dirs
        .iter()
        .map(|(dir, names)| {
            let dir = PathBuf::from(dir);
            let entries = names.iter().map(|name| dir.join(name)).collect();
            (dir, entries)
        })
        .collect();
    move |dir: &Path| {
        map.get(dir)
            .cloned()
            .ok_or_else(|| io::Error::from(io::ErrorKind::NotFound))
    }
}

/// 没有能写的落在藏起来的外面时：同 SW-6。
fn carve_open(
    hidden: &[PathBuf],
    open: &[PathBuf],
    list: impl Fn(&Path) -> io::Result<Vec<PathBuf>>,
) -> Vec<PathBuf> {
    carve(hidden, (&[], open), list)
}

fn paths(list: &[&str]) -> Vec<PathBuf> {
    list.iter().map(PathBuf::from).collect()
}

#[test]
fn nothing_open_inside_keeps_the_whole_hidden_entry() {
    let list = tree(&[]);
    assert_eq!(
        carve_open(&paths(&["/h/.miyu", "/h/.ssh"]), &paths(&["/work"]), list),
        paths(&["/h/.miyu", "/h/.ssh"])
    );
}

#[test]
fn open_paths_inside_are_carved_out_level_by_level() {
    let list = tree(&[
        (
            "/h/.miyu",
            &["run", "secrets.toml", "workspace", "packages"],
        ),
        ("/h/.miyu/workspace", &["a", "b"]),
        ("/h/.miyu/packages", &["office", "other"]),
    ]);
    assert_eq!(
        carve_open(
            &paths(&["/h/.miyu", "/h/.ssh"]),
            &paths(&["/h/.miyu/workspace/a", "/h/.miyu/packages/office"]),
            list,
        ),
        paths(&[
            "/h/.miyu/run",
            "/h/.miyu/secrets.toml",
            "/h/.miyu/workspace/b",
            "/h/.miyu/packages/other",
            "/h/.ssh",
        ]),
        "路上那几层和透出来的本身都不禁，旁边的照禁"
    );
}

#[test]
fn an_unlistable_level_keeps_the_entry_hidden_whole() {
    let list = tree(&[("/h/.miyu", &["run", "workspace"])]);
    assert_eq!(
        carve_open(
            &paths(&["/h/.miyu"]),
            &paths(&["/h/.miyu/workspace/a"]),
            list,
        ),
        paths(&["/h/.miyu/run", "/h/.miyu/workspace"]),
        "列不出 workspace：它整条禁，宁可透不出来"
    );
    assert_eq!(
        carve_open(&paths(&["/h/.miyu"]), &paths(&["/h/.miyu/x"]), tree(&[])),
        paths(&["/h/.miyu"]),
        "数据根本身列不出：整条禁"
    );
}

#[test]
fn only_an_open_path_equal_to_the_hidden_one_lets_it_through() {
    assert!(
        carve_open(&paths(&["/h/.ssh"]), &paths(&["/h/.ssh"]), tree(&[])).is_empty(),
        "放开的就是藏起来的这一条：照放开的"
    );
    assert_eq!(
        carve_open(&paths(&["/h/.ssh"]), &paths(&["/h"]), tree(&[])),
        paths(&["/h/.ssh"]),
        "放开的包住藏起来的：照旧禁"
    );
}

#[test]
fn entries_lists_a_real_directory() {
    let dir = std::env::temp_dir().join(format!("miyu-carve-{}", std::process::id()));
    std::fs::create_dir_all(dir.join("inner")).expect("建得了目录");
    std::fs::write(dir.join("file"), b"").expect("写得进");
    let mut found = entries(&dir).expect("列得出");
    found.sort();
    assert_eq!(found, vec![dir.join("file"), dir.join("inner")]);
    assert!(entries(&dir.join("missing")).is_err());
    std::fs::remove_dir_all(&dir).expect("删得掉");
}

#[test]
fn a_hidden_entry_inside_a_writable_one_is_not_carved() {
    let list = tree(&[
        ("/h/.miyu", &["run", "workspace"]),
        ("/h/.miyu/workspace", &["a", "b"]),
    ]);
    let write = paths(&["/h", "/h/.miyu/workspace/a"]);
    assert_eq!(
        carve(&paths(&["/h/.miyu", "/h/.ssh"]), (&write, &write), list),
        paths(&["/h/.miyu", "/h/.ssh"]),
        "能写的 `~` 包住数据根：挖开的话数据根、workspace 那两层能新建、能改名，整条禁"
    );
}
