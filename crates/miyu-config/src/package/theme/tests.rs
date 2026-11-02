//! `[theme]` 的读法（施工 TH-1）：新写法、老写法、只带网页、两样都带、都没带，和每种写错报哪个代码。

use crate::package::{Code, PackageKind, Theme, read};

/// 一张 `[package]` 加上 `tables`。
fn manifest(tables: &str) -> String {
    format!("[package]\nname = {{ en = \"Night\" }}\n\n{tables}")
}

/// 读成了的 `[theme]`。
fn theme(tables: &str) -> Theme {
    let manifest = read(&manifest(tables)).unwrap_or_else(|problem| panic!("{problem:?}"));
    assert_eq!(manifest.kind, PackageKind::Theme);
    manifest.theme.expect("有 [theme]")
}

/// 读错了的：代码、第几行、说的哪一格。
fn wrong(tables: &str) -> (Code, Option<usize>, String) {
    let problem = read(&manifest(tables)).expect_err("应该读不成");
    (problem.code, problem.line, problem.detail)
}

/// 两样里带了哪几样。
fn carried(tui: Option<&str>, web: Option<&str>) -> Theme {
    Theme {
        tui: tui.map(str::to_string),
        web: web.map(str::to_string),
    }
}

#[test]
fn the_new_spelling_reads_a_terminal_file_and_a_web_dir() {
    assert_eq!(
        theme("[theme.tui]\nfile = \"look/theme.json\"\n\n[theme.web]\ndir = \"web\"\n"),
        carried(Some("look/theme.json"), Some("web"))
    );
    assert_eq!(
        theme("[theme.tui]\nfile = \"theme.json\"\n"),
        carried(Some("theme.json"), None)
    );
    assert_eq!(
        theme("[theme.web]\ndir = \"site/web\"\n"),
        carried(None, Some("site/web"))
    );
    // 写成行内表的同样认。
    assert_eq!(
        theme("[theme]\nweb = { dir = \"web\" }\n"),
        carried(None, Some("web"))
    );
}

#[test]
fn the_old_spelling_is_read_as_the_terminal_file() {
    assert_eq!(
        theme("[theme]\nfile = \"theme.json\"\n"),
        carried(Some("theme.json"), None)
    );
    // 老写法加上网页主题：老的那份照旧是终端主题。
    assert_eq!(
        theme("[theme]\nfile = \"theme.json\"\n\n[theme.web]\ndir = \"web\"\n"),
        carried(Some("theme.json"), Some("web"))
    );
}

#[test]
fn a_theme_with_neither_is_missing_a_key() {
    let (code, line, detail) = wrong("[theme]\n");
    assert_eq!((code, line), (Code::MissingKey, Some(4)));
    assert_eq!(detail, "theme.tui / theme.web");
}

#[test]
fn each_mistake_says_its_code_line_and_key() {
    let cases: [(&str, Code, usize, &str); 9] = [
        (
            "[theme]\nfile = \"a.json\"\n\n[theme.tui]\nfile = \"b.json\"\n",
            Code::UnknownKey,
            5,
            "theme.file",
        ),
        (
            "[theme.tui]\nfile = \"../a.json\"\n",
            Code::BadThemeFile,
            5,
            "theme.tui.file",
        ),
        (
            "[theme.tui]\nmodel = \"a.json\"\n",
            Code::UnknownKey,
            5,
            "theme.tui.model",
        ),
        (
            "[theme.tui]\n\n[theme.web]\ndir = \"web\"\n",
            Code::MissingKey,
            4,
            "theme.tui.file",
        ),
        (
            "[theme]\ntui = \"a.json\"\n",
            Code::BadThemeFile,
            5,
            "theme.tui",
        ),
        (
            "[theme.web]\ndir = \"/web\"\n",
            Code::BadThemeDir,
            5,
            "theme.web.dir",
        ),
        (
            "[theme.web]\ndir = \"a/../web\"\n",
            Code::BadThemeDir,
            5,
            "theme.web.dir",
        ),
        (
            "[theme.web]\nfile = \"index.js\"\n",
            Code::UnknownKey,
            5,
            "theme.web.file",
        ),
        ("[theme]\nweb = 1\n", Code::BadThemeDir, 5, "theme.web"),
    ];
    for (tables, code, line, detail) in cases {
        assert_eq!(
            wrong(tables),
            (code, Some(line), detail.to_string()),
            "{tables}"
        );
    }
}

#[test]
fn a_web_theme_still_carries_no_mascot_and_no_program() {
    for extra in ["[mascot]\nmodel = \"m.json\"\n", "[page]\ndir = \"page\"\n"] {
        let (code, ..) = wrong(&format!("[theme.web]\ndir = \"web\"\n\n{extra}"));
        assert_eq!(code, Code::WrongKind, "{extra}");
    }
}
