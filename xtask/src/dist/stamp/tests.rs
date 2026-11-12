//! 头的版本（施工 RL-6）：只改那一段的那一行、别的一字不差；数组段、别的段里的 version 不算；读得出；没有的说没有。

use super::*;

const TUI: &str = "[package]\nname = \"miyu-tui\"\nversion = \"0.1.0\"\nedition = \"2024\"\n\n[dependencies]\nfoo = { version = \"1\" }\n\n[[bin]]\nname = \"miyu-tui\"\n";
const WEB: &str = "[workspace]\nmembers = [\"crates/*\"]\n\n[workspace.package]\nlicense = \"AGPL\"\nversion = \"0.1.0\"\r\n\n[workspace.dependencies]\nserde = { version = \"1\" }\n";

#[test]
fn only_the_version_line_of_that_section_changes() {
    let tui = stamped(TUI, "package", "2026.11.0-rc.3").expect("有那一行");
    assert_eq!(
        tui,
        TUI.replace("version = \"0.1.0\"", "version = \"2026.11.0-rc.3\"")
    );
    let web = stamped_web();
    assert_eq!(
        web,
        WEB.replace(
            "version = \"0.1.0\"\r\n",
            "version = \"2026.11.0-rc.3\"\r\n"
        ),
        "换行照原样"
    );
    assert_eq!(stamped(TUI, "workspace.package", "x"), None, "没有这一段");
    assert_eq!(
        stamped("[package]\nname = \"a\"\n", "package", "x"),
        None,
        "这一段没有 version"
    );
    assert_eq!(
        stamped(
            "[package]\nversion = \"1\"\nversion = \"2\"\n",
            "package",
            "x"
        )
        .as_deref(),
        Some("[package]\nversion = \"x\"\nversion = \"2\"\n"),
        "只改头一行"
    );
}

fn stamped_web() -> String {
    stamped(WEB, "workspace.package", "2026.11.0-rc.3").expect("有那一行")
}

#[test]
fn the_version_is_read_from_that_section_only() {
    assert_eq!(version_in(TUI, "package").as_deref(), Some("0.1.0"));
    assert_eq!(
        version_in(WEB, "workspace.package").as_deref(),
        Some("0.1.0")
    );
    assert_eq!(version_in(WEB, "package"), None);
    assert_eq!(
        version_in("[[package]]\nversion = \"9\"\n", "package"),
        None,
        "数组段不算"
    );
}

#[test]
fn only_rc_versions_are_rehearsals() {
    assert!(rehearsal("2026.11.0-rc.1"));
    assert!(!rehearsal("2026.11.0"));
}

#[test]
fn a_formal_version_must_match_every_place() {
    let dir = std::env::temp_dir().join(format!("miyu-stamp-{}", std::process::id()));
    std::fs::create_dir_all(dir.join("tui")).expect("建得了");
    std::fs::write(dir.join("Cargo.toml"), TUI.replace("0.1.0", "2026.11.0")).expect("写得进");
    std::fs::write(
        dir.join("tui/package.toml"),
        "[package]\nid = \"tui\"\nversion = \"0.1.0\"\n",
    )
    .expect("写得进");
    let refused = head("tui", &dir, "2026.11.0").expect_err("包清单还是 0.1.0");
    assert!(
        refused.contains("tui/package.toml") && refused.contains("0.1.0"),
        "{refused}"
    );
    std::fs::write(
        dir.join("tui/package.toml"),
        "[package]\nid = \"tui\"\nversion = \"2026.11.0\"\n",
    )
    .expect("写得进");
    assert_eq!(head("tui", &dir, "2026.11.0"), Ok(()));
    assert!(head("nobody", &dir, "2026.11.0").is_err(), "不认识的头");
    std::fs::remove_dir_all(&dir).expect("删得了");
}
