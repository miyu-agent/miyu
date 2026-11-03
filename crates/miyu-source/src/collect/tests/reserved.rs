//! 编号是数据根里保留的名字的包（施工 FH-5下，设计 55 第三节第 3 条）：收录时跳过、记一笔、不占编号；官方来源、自建的文件夹
//! 都一样。

use super::*;
use crate::collect::folder::from_folder;

#[test]
fn an_official_release_with_a_reserved_id_is_skipped_and_claims_nothing() {
    let scratch = Scratch::new();
    let mut net = Net::default();
    net.release(
        "miyu-agent/token",
        &[package(scratch.path(), "token", "1.0", "", None)],
    );
    net.release(
        "miyu-agent/fine",
        &[package(scratch.path(), "fine", "1.0", "", None)],
    );
    let rules = rules(&["miyu-agent/token", "miyu-agent/fine"]);
    let got = collect(
        &rules,
        &net,
        API,
        &scratch_dir(&scratch),
        "2026-11-03T03:00:00Z",
    );
    let ids: Vec<&str> = got
        .index
        .packages
        .iter()
        .map(|one| one.id.as_str())
        .collect();
    assert_eq!(ids, ["fine"], "{:?}", got.notes);
    let said = got.notes.join("\n");
    assert!(
        said.contains("miyu-agent/token: skipped, ")
            && said.contains("token is reserved in the data root"),
        "{said}"
    );
    assert!(!got.names.contains_key("token"), "保留的名字不占");
}

#[test]
fn a_folder_package_with_a_reserved_id_is_skipped() {
    let scratch = Scratch::new();
    let folder = scratch.path().join("incoming");
    std::fs::create_dir_all(&folder).expect("建得了");
    for (name, bytes) in [
        package(scratch.path(), "mcp", "0.1.0", "", None),
        package(scratch.path(), "web", "0.1.0", "", None),
    ] {
        std::fs::write(folder.join(name), bytes).expect("写得进");
    }
    let got = from_folder(
        &folder,
        "http://192.0.2.1/miyu/",
        &scratch_dir(&scratch),
        "2026-11-03T03:00:00Z",
    )
    .expect("收得成");
    let ids: Vec<&str> = got
        .index
        .packages
        .iter()
        .map(|one| one.id.as_str())
        .collect();
    assert_eq!(ids, ["web"], "{:?}", got.notes);
    assert!(
        got.notes
            .join("\n")
            .contains("mcp is reserved in the data root"),
        "{:?}",
        got.notes
    );
}
