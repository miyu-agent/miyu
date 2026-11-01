//! `miyu pkg info <路径>`（施工 IN-4，照 pacman 的 `-Qip`）：普通终端里写包文件夹的，照现在的目录换成绝对的、发
//! `package.info {"path"}`，印成「键 : 值」，什么都不装；装不上的照真装那样报；`-Qi`、`-Si` 带路径的是用法错，退出码 2。

use serde_json::Value;

use crate::pkg_source::pkg_speaking;
use crate::support::Home;
use miyu_cli::language::Language;
use miyu_session::testkit::Script;

/// 清单：中英文的名字、说明，作者。
const MANIFEST: &str = "[package]\nversion = \"1.2\"\nname = { en = \"My Skill\", zh = \"我的技能\" }\nsummary = { en = \"Does things\", zh = \"做点事\" }\nauthor = \"Ann\"\n";

/// 一个技能、一个脚本、一个读不成的技能。
fn carried() -> [(&'static str, String); 3] {
    [
        (
            "skills/notes/SKILL.md",
            "---\nname: notes\ndescription: Take notes.\n---\nbody\n".to_string(),
        ),
        (
            "scripts/hello.sh",
            "#!/bin/sh\n# Description: Echo hello.\n# Parameters: {\"type\": \"object\", \"properties\": {}}\necho hello\n".to_string(),
        ),
        ("skills/bad/SKILL.md", "no front matter\n".to_string()),
    ]
}

/// 现在的目录（`home.outside()`）下放一个包文件夹 `id`：清单 `manifest`，外加 `files`。交回一共多少字节。
fn folder(home: &Home, id: &str, manifest: &str, files: &[(&str, String)]) -> usize {
    let dir = home.outside().join(id);
    std::fs::create_dir_all(&dir).expect("建得了");
    std::fs::write(dir.join("package.toml"), manifest).expect("写得进");
    for (path, text) in files {
        let file = dir.join(path);
        std::fs::create_dir_all(file.parent().expect("有上一层")).expect("建得了");
        std::fs::write(file, text).expect("写得进");
    }
    manifest.len() + files.iter().map(|(_, text)| text.len()).sum::<usize>()
}

fn home() -> Home {
    Home::new(std::sync::Arc::new(Script::new([])))
}

fn placed(home: &Home, id: &str) -> bool {
    home.root.path().join("packages").join(id).exists()
}

/// 印出来的几行里，「无法读取」那一行只认开头：为什么是技能那边的原话。
fn split_skipped(out: &str, head: &str) -> (String, Option<String>) {
    let mut kept = String::new();
    let mut skipped = None;
    for line in out.lines() {
        match line.starts_with(head) {
            true => skipped = Some(line.to_string()),
            false => {
                kept.push_str(line);
                kept.push('\n');
            }
        }
    }
    (kept, skipped)
}

#[tokio::test]
async fn a_folder_prints_like_an_installed_package_and_installs_nothing() {
    let home = home();
    let bytes = folder(&home, "my-skill", MANIFEST, &carried());
    let (code, tape) = pkg_speaking(&home, &["info", "./my-skill"], b"", Language::Chinese).await;
    let (out, err) = (tape.text(|err| !err), tape.text(|err| err));
    assert_eq!((code, err.as_str()), (0, ""), "{out}");
    let (kept, skipped) = split_skipped(&out, "无法读取    : ");
    assert_eq!(
        kept,
        format!(
            "编号        : my-skill\n名称        : 我的技能\n版本        : 1.2\n描述        : 做点事\n\
             依赖        : 无\n推荐        : 无\n提供        : 技能 notes、脚本 hello\n作者        : Ann\n\
             安装后大小  : {bytes} B\n"
        )
    );
    let skipped = skipped.unwrap_or_default();
    assert!(
        skipped.starts_with("无法读取    : skills/bad/SKILL.md（") && skipped.ends_with('）'),
        "{out}"
    );
    assert!(
        out.find("提供") < out.find("无法读取") && out.find("无法读取") < out.find("作者"),
        "读不成的接在「提供」后面：{out}"
    );
    assert!(!placed(&home, "my-skill"), "什么都不装");
}

#[tokio::test]
async fn english_and_json_say_the_same() {
    let home = home();
    let bytes = folder(&home, "my-skill", MANIFEST, &carried());
    let (code, tape) = pkg_speaking(&home, &["info", "my-skill/"], b"", Language::English).await;
    let out = tape.text(|err| !err);
    assert_eq!(code, 0, "{out}");
    let (kept, skipped) = split_skipped(&out, "Unreadable      : ");
    assert_eq!(
        kept,
        format!(
            "Name            : my-skill\nTitle           : My Skill\nVersion         : 1.2\n\
             Description     : Does things\nDepends On      : None\nRecommends      : None\n\
             Provides        : skill notes, script hello\nAuthor          : Ann\n\
             Installed Size  : {bytes} B\n"
        )
    );
    assert!(
        skipped.is_some_and(|line| line.contains("skills/bad/SKILL.md (")),
        "{out}"
    );

    let (code, tape) = pkg_speaking(
        &home,
        &["info", "./my-skill", "--format", "json"],
        b"",
        Language::English,
    )
    .await;
    let out = tape.text(|err| !err);
    assert_eq!(code, 0, "{out}");
    let said: Value = serde_json::from_str(out.trim()).expect("印的是一份 JSON");
    assert_eq!(said["from"], "file", "{out}");
    assert_eq!(said["package"], "my-skill", "{out}");
    let path = home.outside().join("my-skill");
    assert_eq!(
        said["path"],
        path.to_string_lossy().as_ref(),
        "绝对路径：{out}"
    );
}

#[tokio::test]
async fn what_would_not_install_is_said_the_same_way() {
    let home = home();
    folder(&home, "broken", "[package]\n\n[ui]\n\n[process]\n", &[]);
    let (code, tape) = pkg_speaking(&home, &["info", "./broken"], b"", Language::Chinese).await;
    let (out, err) = (tape.text(|err| !err), tape.text(|err| err));
    assert_eq!((code, out.as_str()), (1, ""), "{err}");
    assert!(
        err.starts_with("miyu: 无法安装 ./broken：package.toml:5: "),
        "照真装那样报：{err}"
    );
}

#[tokio::test]
async fn pacman_forms_take_an_id_not_a_path() {
    let home = home();
    folder(&home, "my-skill", MANIFEST, &[]);
    for (words, flag) in [
        (["-Qi", "./my-skill"], "-Qi"),
        (["-Si", "./my-skill"], "-Si"),
    ] {
        let (code, tape) = pkg_speaking(&home, &words, b"", Language::Chinese).await;
        let (out, err) = (tape.text(|err| !err), tape.text(|err| err));
        assert_eq!((code, out.as_str()), (2, ""), "{words:?}：{err}");
        assert!(
            err.starts_with(&format!("miyu: {flag} 只接软件包编号")),
            "{words:?}：{err}"
        );
    }
}
