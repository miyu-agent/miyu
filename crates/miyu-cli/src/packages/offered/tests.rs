use serde_json::json;

use super::*;

/// 官方的先找，再照名字找另加的源；索引坏了的、没有 `provides` 的跳过；哪个都不带的没有。
#[test]
fn the_first_index_offering_the_command_names_the_package() {
    let dir = std::env::temp_dir().join(format!("miyu-cli-offered-{}", std::process::id()));
    let write = |path: &str, value: serde_json::Value| {
        let file = dir.join(path);
        std::fs::create_dir_all(file.parent().expect("有上一级")).expect("建得了");
        std::fs::write(file, value.to_string()).expect("写得进");
    };
    assert_eq!(offering_under(&dir, "web"), None, "什么都没存过");
    write(
        ".sync/index.json",
        json!({"packages": [{"id": "miyu-tui", "provides": {"command": ["tui"]}}, {"id": "theme"}]}),
    );
    write(
        ".sources/a-local/index.json",
        json!({"packages": [{"id": "miyu-webui", "provides": {"command": ["web"]}}]}),
    );
    write(
        ".sources/b-other/index.json",
        json!({"packages": [{"id": "someone-tui", "provides": {"command": ["tui", "web"]}}]}),
    );
    std::fs::create_dir_all(dir.join(".sources/0-broken")).expect("建得了");
    std::fs::write(dir.join(".sources/0-broken/index.json"), "{").expect("写得进");
    assert_eq!(
        offering_under(&dir, "tui").as_deref(),
        Some("miyu-tui"),
        "官方的先找"
    );
    assert_eq!(
        offering_under(&dir, "web").as_deref(),
        Some("miyu-webui"),
        "另加的照名字"
    );
    assert_eq!(offering_under(&dir, "nope"), None);
    std::fs::remove_dir_all(&dir).expect("删得掉临时目录");
}
