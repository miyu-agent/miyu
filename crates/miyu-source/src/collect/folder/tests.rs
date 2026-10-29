//! 从一个文件夹收（施工 G-13）：几个包、一个包几种系统、同一个编号两个版本、读不成的、不是包文件的。

use super::*;
use crate::test_support::{Scratch, package};
use miyu_config::package::Target;

const URL: &str = "http://192.0.2.1:8080/miyu/";

/// 把打好的包文件放进 `folder`。
fn drop_in(folder: &Path, (name, bytes): (String, Vec<u8>)) {
    std::fs::create_dir_all(folder).expect("建得了");
    std::fs::write(folder.join(name), bytes).expect("写得进");
}

#[test]
fn every_package_in_the_folder_becomes_an_entry_served_under_files() {
    let scratch = Scratch::new();
    let folder = scratch.path().join("incoming");
    let more = "targets = [\"linux-x86_64\", \"macos-aarch64\"]\n";
    drop_in(&folder, package(scratch.path(), "web", "0.1.0", "", None));
    drop_in(
        &folder,
        package(
            scratch.path(),
            "tui",
            "0.0.1",
            more,
            Some(Target::MacosAarch64),
        ),
    );
    drop_in(
        &folder,
        package(
            scratch.path(),
            "tui",
            "0.0.1",
            more,
            Some(Target::LinuxX86_64),
        ),
    );
    std::fs::write(folder.join("notes.txt"), b"not a package").expect("写得进");
    let work = scratch.path().join("work");
    let got = from_folder(&folder, URL, &work, "2026-10-29T13:00:00Z").expect("收得成");
    let ids: Vec<&str> = got
        .index
        .packages
        .iter()
        .map(|entry| entry.id.as_str())
        .collect();
    assert_eq!(ids, ["tui", "web"], "{:?}", got.notes);
    let tui = &got.index.packages[0];
    assert_eq!(tui.origin, "http://192.0.2.1:8080/miyu", "来源写源的地址");
    assert!(!tui.official, "自建的源不是官方的");
    let urls: Vec<&str> = tui.files.iter().map(|file| file.url.as_str()).collect();
    assert_eq!(
        urls,
        [
            "http://192.0.2.1:8080/miyu/files/tui-0.0.1-linux-x86_64.miyu",
            "http://192.0.2.1:8080/miyu/files/tui-0.0.1-macos-aarch64.miyu",
        ],
        "一个包几种系统并成一条，照系统排"
    );
    let web = std::fs::read(folder.join("web-0.1.0.miyu")).expect("在");
    assert_eq!(got.index.packages[1].files[0].size, web.len() as u64);
    assert_eq!(got.index.generated, "2026-10-29T13:00:00Z");
    assert!(got.names.is_empty() && got.index.blocked.is_empty());
    Index::parse(&serde_json::to_vec(&got.index).expect("写得成")).expect("写出来的读得回去");
}

#[test]
fn a_second_version_and_an_unreadable_file_are_left_out_and_noted() {
    let scratch = Scratch::new();
    let folder = scratch.path().join("incoming");
    drop_in(&folder, package(scratch.path(), "web", "0.1.0", "", None));
    drop_in(&folder, package(scratch.path(), "web", "0.2.0", "", None));
    std::fs::write(folder.join("broken-1.0.0.miyu"), b"not a zip").expect("写得进");
    let work = scratch.path().join("work");
    let got = from_folder(&folder, URL, &work, "2026-10-29T13:00:00Z").expect("收得成");
    assert_eq!(got.index.packages.len(), 1, "{:?}", got.notes);
    assert_eq!(
        got.index.packages[0].version, "0.1.0",
        "照文件名的先后，先到的留下"
    );
    assert!(
        got.notes
            .iter()
            .any(|note| note.starts_with("web-0.2.0.miyu: skipped")),
        "{:?}",
        got.notes
    );
    assert!(
        got.notes
            .iter()
            .any(|note| note.starts_with("broken-1.0.0.miyu: skipped")),
        "{:?}",
        got.notes
    );
}

#[test]
fn a_missing_folder_says_so() {
    let scratch = Scratch::new();
    let missing = scratch.path().join("nope");
    let work = scratch.path().join("work");
    assert!(from_folder(&missing, URL, &work, "2026-10-29T13:00:00Z").is_err());
}
