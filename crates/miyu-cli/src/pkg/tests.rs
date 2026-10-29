//! `miyu pkg` 印的那一行、装的是包目录还是编号（施工 T-3，`docs/blueprint/cli/pkg.md`）。

use serde_json::json;

use super::{Pkg, PkgCommand, installing, refused, shown};
use crate::language::Language;

#[test]
fn a_package_is_one_aligned_line_and_only_what_people_need() {
    let packages = [
        json!({"package": "basesystem", "layer": "shipped", "kind": "builtin", "name": "基础系统", "version": "1"}),
        json!({"package": "net", "layer": "shipped", "kind": "builtin", "name": "联网", "removed": true}),
        json!({"package": "bad", "layer": "home", "code": "bad_toml", "problem": "第一行不是 TOML"}),
    ];
    assert_eq!(
        shown::listed(&packages, &Language::Chinese),
        [
            "bad         （写错了：第一行不是 TOML）",
            "basesystem  基础系统",
            "net         联网（已卸载）",
        ],
        "照编号排"
    );
    assert_eq!(
        shown::listed(&packages[1..2], &Language::English),
        ["net  联网 (removed)"]
    );
}

#[test]
fn a_path_is_a_package_folder_and_a_bare_word_is_a_package() {
    // 真的绝对路径：Windows 上 `/work` 不算绝对的，拼出来的样子两边不一样。
    let temp = std::env::temp_dir();
    let cwd = temp.join("work");
    let cwd = cwd.as_path();
    assert_eq!(installing("./x", cwd), json!({"path": cwd.join("x")}));
    assert_eq!(installing("x/", cwd), json!({"path": cwd.join("x")}));
    assert_eq!(installing(".", cwd), json!({"path": cwd}));
    assert_eq!(installing("../y", cwd), json!({"path": temp.join("y")}));
    assert_eq!(
        installing("package.toml", cwd),
        json!({"path": cwd.join("package.toml")})
    );
    let elsewhere = temp.join("abs").join("y");
    assert_eq!(
        installing(&elsewhere.to_string_lossy(), cwd),
        json!({"path": elsewhere})
    );
    assert_eq!(installing("net", cwd), json!({"package": "net"}));
}

/// 照命令行读参数，交回换成正式写法以后的子命令。
fn parsed(args: &[&str]) -> Result<PkgCommand, clap::Error> {
    #[derive(clap::Parser)]
    struct Line {
        #[command(flatten)]
        pkg: Pkg,
    }
    let line = <Line as clap::Parser>::try_parse_from(args)?;
    Ok(line.pkg.command.unwrap_or(PkgCommand::List).plain())
}

#[test]
fn pacman_options_are_the_plain_commands() {
    let one = |args: &[&str]| parsed(args).expect("认得");
    assert!(matches!(one(&["pkg", "-U", "./x"]), PkgCommand::Install { what } if what == "./x"));
    assert!(matches!(one(&["pkg", "-R", "x"]), PkgCommand::Remove { package } if package == "x"));
    assert!(matches!(one(&["pkg", "-Q"]), PkgCommand::List));
    assert!(matches!(one(&["pkg", "-Qi", "x"]), PkgCommand::Info { package } if package == "x"));
    assert!(matches!(one(&["pkg", "-Ql", "x"]), PkgCommand::Files { package } if package == "x"));
    assert!(matches!(one(&["pkg", "-Qo", "/a"]), PkgCommand::Owns { path } if path == "/a"));
    assert!(matches!(
        one(&["pkg", "-Qk"]),
        PkgCommand::Check { package: None }
    ));
    assert!(
        matches!(one(&["pkg", "-Qk", "x"]), PkgCommand::Check { package: Some(id) } if id == "x")
    );
    assert!(matches!(one(&["pkg", "info", "x"]), PkgCommand::Info { package } if package == "x"));
    for wrong in [
        &["pkg", "-Qi"][..],
        &["pkg", "-Qil", "x"],
        &["pkg", "-Q", "x"],
        &["pkg", "-Qo"],
    ] {
        assert!(parsed(wrong).is_err(), "{wrong:?}");
    }
}

#[test]
fn a_conflict_says_cannot_install_and_one_line_each() {
    // 施工 F-8 六补：照 pacman 的「冲突的文件」，第一行说装不上，接着一处一行。
    let error = json!({
        "message": "名称已被占用，无法安装。",
        "data": {"reason": "package_conflict", "conflicts": [
            {"kind": "tool", "name": "read", "package": "basesystem"},
            {"kind": "skill", "name": "pdf", "package": "kit"},
            {"kind": "mcp", "name": "lone", "package": "svc"},
            {"kind": "slash", "name": "review", "package": "kit"},
            {"kind": "persona", "name": "dog", "layer": "home"},
        ]}
    });
    assert_eq!(
        refused(&error, &Language::Chinese),
        "装不上：名字已经有了\n工具 read 已在 basesystem 中\n技能 pdf 已在 kit 中\nMCP 服务 lone 已在 svc 中\n斜杠命令 review 已在 kit 中\n人格 dog 已在 你的数据根 中"
    );
    assert_eq!(
        refused(&error, &Language::English),
        "Cannot install: names already in use\ntool read exists in basesystem\nskill pdf exists in kit\nMCP server lone exists in svc\nslash command review exists in kit\npersona dog exists in your data root"
    );
}
