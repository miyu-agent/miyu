//! 往下走目录的 `glob`、`grep` 从家目录搜下来，别的程序的钥匙、登录照搜（施工 PV-RM，2026-11-16 项目主人定去掉私密目录：只读
//! 就是不让写，不替人藏东西）；挡住的只剩从上面路过的数据根，起点在数据根里的照常搜（施工 RO-2）。场地的假家目录是 `home/`，
//! 数据根是 `data/`。

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

/// 从数据根上面搜下来不进数据根（施工 RO-2，2026-11-16 定：同版本库目录一个道理，不算藏）：场地的根包住 `home/` 和数据根
/// `data/`，`grep`、`glob` 都不出数据根里的。
#[tokio::test]
async fn searching_from_above_passes_the_data_root_by() {
    let site = Site::new();
    site.file("home/notes.txt", b"needle\n");
    site.file("data/etc/private/secrets.toml", b"key = \"needle\"\n");
    let root = site.0.to_string_lossy().into_owned();
    for (tool, args) in [
        ("glob", serde_json::json!({"pattern": "*", "path": root})),
        (
            "grep",
            serde_json::json!({"pattern": "needle", "path": root}),
        ),
    ] {
        let (error, text) = site.call(tool, args).await;
        assert!(!error, "{tool}：{text}");
        assert!(text.contains("notes.txt"), "{tool}：{text}");
        assert!(!text.contains("secrets.toml"), "{tool}：{text}");
    }
}

/// 起点本身在数据根里的照常往下走（施工 RO-2）：从 `data/etc` 搜，`private/` 里的密钥搜得到，那一行照给。
#[tokio::test]
async fn searching_inside_the_data_root_finds_the_secrets() {
    let site = Site::new();
    site.file("data/etc/private/secrets.toml", b"key = \"FAKE-KEY-1\"\n");
    let etc = site.0.join("data/etc").to_string_lossy().into_owned();
    let (error, text) = site
        .call(
            "grep",
            serde_json::json!({"pattern": "FAKE-KEY", "path": etc, "output_mode": "content"}),
        )
        .await;
    assert!(!error, "{text}");
    assert!(text.contains("secrets.toml"), "{text}");
    assert!(text.contains("key = \"FAKE-KEY-1\""), "{text}");
    let (error, text) = site
        .call(
            "glob",
            serde_json::json!({"pattern": "**/*.toml", "path": etc}),
        )
        .await;
    assert!(!error, "{text}");
    assert!(text.contains("secrets.toml"), "{text}");
}
