//! 往下走目录的 `glob`、`grep` 从家目录搜下来，别的程序的钥匙、登录照搜（施工 PV-RM，2026-11-16 项目主人定去掉私密目录：只读
//! 就是不让写，不替人藏东西）；挡住的只剩数据根（`glob.rs`、`grep.rs` 守着）。场地的假家目录是 `home/`。

use crate::support::Site;

#[tokio::test]
async fn searching_from_the_home_finds_key_folders_too() {
    let site = Site::new();
    site.file("home/.ssh/id_ed25519", b"needle\n");
    site.file("home/.config/gh/hosts.yml", b"needle\n");
    site.file("home/notes.txt", b"needle\n");
    let home = site.0.join("home").to_string_lossy().into_owned();
    for (tool, args) in [
        ("glob", serde_json::json!({"pattern": "*", "path": home})),
        (
            "grep",
            serde_json::json!({"pattern": "needle", "path": home}),
        ),
    ] {
        let (error, text) = site.call(tool, args).await;
        assert!(!error, "{tool}：{text}");
        for name in ["notes.txt", "id_ed25519", "hosts.yml"] {
            assert!(text.contains(name), "{tool}：{name}：{text}");
        }
    }
}
