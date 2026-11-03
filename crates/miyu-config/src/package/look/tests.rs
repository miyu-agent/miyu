//! 清单的图标、后台页（施工 F-6 上，`package-pages.md`「清单多的几格」）。

use crate::package::{Code, PackageKind, read};

/// 一份扩展的清单，`extra` 接在 `[package]` 后面（`[package]` 里的几格）、`tables` 接在最后（别的表）。
fn process(extra: &str, tables: &str) -> String {
    format!(
        "[package]\nprotocol = [1, 1]\nname = {{ en = \"Bridge\" }}\n{extra}\n[command]\nname = \"bridge\"\nprogram = \"miyu-bridge\"\nabout = {{ en = \"Bridge\" }}\n\n[process]\n{tables}"
    )
}

#[test]
fn icon_and_page_are_read() {
    let manifest = read(&process(
        "icon = \"message-circle\"",
        "\n[page]\ndir = \"page\"\n",
    ))
    .unwrap();
    assert_eq!(manifest.icon.as_deref(), Some("message-circle"));
    assert_eq!(manifest.page.as_deref(), Some("page"));
    let manifest = read(&process("", "")).unwrap();
    assert_eq!((manifest.icon, manifest.page), (None, None));
    let builtin = "[package]\nprotocol = [1, 1]\nname = { en = \"Memory\" }\nicon = \"brain\"\n\n[page]\ndir = \"web/page\"\n\n[builtin]\n";
    let manifest = read(builtin).unwrap();
    assert_eq!(manifest.kind, PackageKind::Builtin);
    assert_eq!(manifest.page.as_deref(), Some("web/page"));
}

#[test]
fn a_bad_icon_is_refused_on_its_line() {
    for icon in ["\"\"", "\"Message\"", "\"1a\"", "\"a_b\"", "\"a b\"", "1"] {
        let problem = read(&process(&format!("icon = {icon}"), "")).unwrap_err();
        assert_eq!(
            (problem.code, problem.line),
            (Code::BadIcon, Some(4)),
            "{icon}"
        );
    }
    let long = format!("icon = \"{}\"", "a".repeat(65));
    assert_eq!(read(&process(&long, "")).unwrap_err().code, Code::BadIcon);
    let longest = format!("icon = \"{}\"", "a".repeat(64));
    assert!(read(&process(&longest, "")).is_ok());
}

#[test]
fn a_bad_page_dir_is_refused() {
    for dir in [
        "\"\"",
        "\"/page\"",
        "\"../page\"",
        "\"a/../b\"",
        "\"a\\\\b\"",
        "\"c:page\"",
        "1",
    ] {
        let problem = read(&process("", &format!("\n[page]\ndir = {dir}\n"))).unwrap_err();
        assert_eq!(problem.code, Code::BadPageDir, "{dir}");
    }
    let missing = read(&process("", "\n[page]\n")).unwrap_err();
    assert_eq!(missing.code, Code::MissingKey);
    let unknown = read(&process(
        "",
        "\n[page]\ndir = \"page\"\nentry = \"x.html\"\n",
    ))
    .unwrap_err();
    assert_eq!(unknown.code, Code::UnknownKey);
}

#[test]
fn only_extensions_and_built_in_packages_have_a_page() {
    let ui =
        "[package]\nprotocol = [1, 1]\nname = { en = \"UI\" }\n\n[page]\ndir = \"page\"\n\n[ui]\n";
    assert_eq!(read(ui).unwrap_err().code, Code::WrongKind);
    let worker = "[package]\nprotocol = [1, 1]\nname = { en = \"W\" }\n\n[worker]\nprogram = \"w\"\n\n[page]\ndir = \"page\"\n";
    assert_eq!(read(worker).unwrap_err().code, Code::WrongKind);
}

/// 包自带的图标（施工 F-18 上）：包目录里一张 `.png`、`.webp` 的相对路径；不许 `..`、开头的 `/`、别的扩展名。
#[test]
fn a_logo_is_a_relative_png_or_webp_path() {
    for logo in ["logo.png", "art/Logo.WEBP"] {
        let manifest = read(&process(&format!("logo = \"{logo}\""), "")).unwrap();
        assert_eq!(manifest.logo.as_deref(), Some(logo));
    }
    assert_eq!(read(&process("", "")).unwrap().logo, None);
    for logo in [
        "\"\"",
        "\"./logo.png\"",
        "\"../logo.png\"",
        "\"/logo.png\"",
        "\"logo.jpg\"",
        "\"logo\"",
        "1",
    ] {
        let problem = read(&process(&format!("logo = {logo}"), "")).unwrap_err();
        assert_eq!(
            (problem.code, problem.line),
            (Code::BadLogo, Some(4)),
            "{logo}"
        );
    }
}

/// 托盘入口（施工 TR-2，`package-pages.md`「清单多的几格」第 4 条）：`[tray] name` 读成分语言的名字；内置包也能写。
#[test]
fn a_tray_entry_is_read() {
    let manifest = read(&process(
        "",
        "\n[page]\ndir = \"page\"\n\n[tray]\nname = { en = \"Open bridge\", zh = \"打开桥\" }\n",
    ))
    .unwrap();
    let tray = manifest.tray.expect("读了托盘入口");
    assert_eq!(
        (
            tray.get("zh").map(String::as_str),
            tray.get("en").map(String::as_str)
        ),
        (Some("打开桥"), Some("Open bridge"))
    );
    assert_eq!(read(&process("", "")).unwrap().tray, None);
    let builtin = "[package]\nprotocol = [1, 1]\nname = { en = \"Memory\" }\n\n[page]\ndir = \"page\"\n\n[tray]\nname = { en = \"Memory\" }\n\n[builtin]\n";
    assert!(read(builtin).unwrap().tray.is_some());
}

/// 托盘入口只能打开自己的后台页（施工 TR-2）：写了 `name` 以外的格（指到命令、网址、别的包、页面）、没写 `[page]` 的报
/// `bad_tray`，报在那一行；少了 `name`、名字写错照各自的代码；别的种类写了 `wrong_kind`。
#[test]
fn a_tray_entry_only_opens_its_own_page() {
    let page = "\n[page]\ndir = \"page\"\n";
    for (extra, detail) in [
        ("page = \"other\"", "tray.page"),
        ("url = \"https://example.com\"", "tray.url"),
        ("command = \"rm\"", "tray.command"),
        ("package = \"other\"", "tray.package"),
    ] {
        let text = process(
            "",
            &format!("{page}\n[tray]\nname = {{ en = \"Open\" }}\n{extra}\n"),
        );
        let problem = read(&text).unwrap_err();
        assert_eq!(
            (problem.code, problem.detail.as_str(), problem.line),
            (Code::BadTray, detail, Some(17)),
            "{extra}"
        );
    }
    let without_page = read(&process("", "\n[tray]\nname = { en = \"Open\" }\n")).unwrap_err();
    assert_eq!(
        (without_page.code, without_page.detail.as_str()),
        (Code::BadTray, "tray"),
        "{without_page:?}"
    );
    let unnamed = read(&process("", &format!("{page}\n[tray]\n"))).unwrap_err();
    assert_eq!(unnamed.code, Code::MissingKey);
    let empty = read(&process(
        "",
        &format!("{page}\n[tray]\nname = {{ en = \"\" }}\n"),
    ))
    .unwrap_err();
    assert_eq!(empty.code, Code::EmptyPhrase);
    let mascot = "[package]\nname = { en = \"Cat\" }\n\n[mascot]\nmodel = \"cat.glb\"\n\n[tray]\nname = { en = \"Cat\" }\n";
    let problem = read(mascot).unwrap_err();
    assert_eq!(problem.code, Code::WrongKind, "{problem:?}");
}
