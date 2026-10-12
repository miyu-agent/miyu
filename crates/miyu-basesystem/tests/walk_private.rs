//! 往下走目录的 `glob`、`grep` 不进别的程序的私密目录（施工 5-13，`11-权限与沙盒.md` 第四节）：读它们要问人，从上面搜
//! 下来不该绕过去。搜的那个目录就在里面的（问过了才走得到）、工作目录那一片照样进。场地的假家目录是 `home/`。

use crate::support::Site;

#[tokio::test]
async fn searching_from_above_skips_the_private_dirs() {
    let site = Site::new();
    site.file("home/.ssh/id_ed25519", b"needle\n");
    site.file("home/.config/gh/hosts.yml", b"needle\n");
    site.file("home/.config/other/settings.txt", b"needle\n");
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
        assert!(text.contains("notes.txt"), "{tool}：{text}");
        assert!(text.contains("settings.txt"), "{tool}：旁边的照找：{text}");
        assert!(!text.contains("id_ed25519"), "{tool}：{text}");
        assert!(!text.contains("hosts.yml"), "{tool}：{text}");
    }
}

#[tokio::test]
async fn searching_inside_a_private_dir_goes_all_the_way_down() {
    let site = Site::new();
    site.file("home/.ssh/keys/id_ed25519", b"needle\n");
    let ssh = site.0.join("home/.ssh").to_string_lossy().into_owned();
    let (error, text) = site
        .call(
            "grep",
            serde_json::json!({"pattern": "needle", "path": ssh}),
        )
        .await;
    assert!(!error, "{text}");
    assert!(text.contains("id_ed25519"), "搜的就是它：问过了：{text}");
}

#[tokio::test]
async fn a_workspace_inside_a_private_dir_is_searched() {
    let site = Site::new();
    site.file("home/.claude/proj/a.rs", b"");
    site.file("home/.claude/secret.json", b"");
    let (error, text) = site
        .call_in(
            "home/.claude/proj",
            "glob",
            serde_json::json!({"pattern": "*"}),
        )
        .await;
    assert!(!error, "{text}");
    assert!(text.contains("a.rs"), "{text}");
    let claude = site.0.join("home/.claude").to_string_lossy().into_owned();
    let home = site.0.join("home").to_string_lossy().into_owned();
    let (_, text) = site
        .call_in(
            "home/.claude/proj",
            "glob",
            serde_json::json!({"pattern": "*.json", "path": home}),
        )
        .await;
    assert!(
        !text.contains("secret.json"),
        "从家目录搜下来照样不进：{text}"
    );
    let (_, text) = site
        .call_in(
            "home/.claude/proj",
            "glob",
            serde_json::json!({"pattern": "*.json", "path": claude}),
        )
        .await;
    assert!(text.contains("secret.json"), "搜的就是它：{text}");
}
