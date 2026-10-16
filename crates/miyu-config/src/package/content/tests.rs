//! `[content.<包>]`（施工 X-22）：读成什么、写错了报哪个代码、哪种包能写。

use crate::package::{Code, Content, PackageKind, read};

/// 只带内容的包：没有程序表，是 `bundle`。
const PACK: &str = r#"[package]
name = { en = "Stickers", zh = "表情包合集" }

[content.miyu-memes]
dir = "memes"

[content.another]
dir = "cards/v1"
"#;

#[test]
fn a_bundle_carries_content_for_other_packages_sorted_by_package() {
    let manifest = read(PACK).unwrap();
    assert_eq!(manifest.kind, PackageKind::Bundle);
    assert_eq!(
        manifest.content,
        [
            Content {
                target: "another".into(),
                dir: "cards/v1".into()
            },
            Content {
                target: "miyu-memes".into(),
                dir: "memes".into()
            },
        ]
    );
    let bare = read("[package]\nname = { en = \"Empty\" }\n").unwrap();
    assert!(bare.content.is_empty(), "没写的是空的");
}

#[test]
fn a_bad_target_or_dir_is_bad_content_on_its_line() {
    for (bad, line) in [
        ("[content.Memes]\ndir = \"memes\"\n", 3),
        ("[content.memes]\ndir = \"../memes\"\n", 4),
        ("[content.memes]\ndir = \"/memes\"\n", 4),
        ("[content.memes]\ndir = \"a\\\\b\"\n", 4),
        ("[content]\nmemes = \"memes\"\n", 4),
    ] {
        let problem = read(&format!("[package]\nname = {{ en = \"P\" }}\n{bad}")).expect_err(bad);
        assert_eq!(
            (problem.code, problem.line),
            (Code::BadContent, Some(line)),
            "{bad}"
        );
    }
    let missing =
        read("[package]\nname = { en = \"P\" }\n[content.memes]\n").expect_err("dir 必写");
    assert_eq!(missing.code, Code::MissingKey);
    let extra = read("[package]\nname = { en = \"P\" }\n[content.memes]\ndir = \"m\"\nfiles = 1\n")
        .expect_err("只认 dir");
    assert_eq!(extra.code, Code::UnknownKey);
}

#[test]
fn only_bundles_and_extensions_carry_content() {
    let extension = "[package]\nprotocol = [1, 1]\nname = { en = \"E\" }\n[command]\nname = \"e\"\nprogram = \"miyu-e\"\n\
                     about = { en = \"E\" }\n[process]\n[content.memes]\ndir = \"m\"\n";
    assert_eq!(read(extension).unwrap().content.len(), 1);
    let builtin = "[package]\nprotocol = [1, 1]\nname = { en = \"B\" }\n[builtin]\n[content.memes]\ndir = \"m\"\n";
    assert_eq!(
        read(builtin).expect_err("内置包不能写").code,
        Code::WrongKind
    );
}
