//! `miyu pkg` 印的那一行、装的是包目录还是编号（施工 T-3，`docs/blueprint/cli/pkg.md`）。

use serde_json::json;

use super::{Pkg, PkgCommand, installing, refused, shown};
use crate::diagnostic::{Level, Problem};
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
            "bad         （清单有误：第一行不是 TOML）",
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
    // `-Qi` 只查装着的、`-Si` 只查软件源、`info` 不写（施工 IN-2）。
    let info = |args: &[&str]| match one(args) {
        PkgCommand::Info { package, source } => (package, source),
        other => panic!("{other:?}"),
    };
    assert_eq!(info(&["pkg", "-Qi", "x"]), ("x".to_string(), Some(false)));
    assert_eq!(info(&["pkg", "-Si", "x"]), ("x".to_string(), Some(true)));
    assert_eq!(info(&["pkg", "info", "x"]), ("x".to_string(), None));
    assert!(matches!(one(&["pkg", "-Ql", "x"]), PkgCommand::Files { package } if package == "x"));
    assert!(matches!(one(&["pkg", "-Qo", "/a"]), PkgCommand::Owns { path } if path == "/a"));
    assert!(matches!(
        one(&["pkg", "-Qk"]),
        PkgCommand::Check { package: None }
    ));
    assert!(
        matches!(one(&["pkg", "-Qk", "x"]), PkgCommand::Check { package: Some(id) } if id == "x")
    );
    for wrong in [
        &["pkg", "-Qi"][..],
        &["pkg", "-Qil", "x"],
        &["pkg", "-Q", "x"],
        &["pkg", "-Qo"],
        &["pkg", "-Si"],
        &["pkg", "-Si", "x", "y"],
        &["pkg", "-Ssi", "x"],
    ] {
        assert!(parsed(wrong).is_err(), "{wrong:?}");
    }
}

#[test]
fn a_conflict_says_cannot_install_and_one_line_each() {
    // 施工 F-8 六补：照 pacman 的「冲突的文件」一处一行；CL-2 补起一处一条报错，说装的是哪一个。
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
    let said = |language: Language| -> Vec<String> {
        refused(&error, Some("./pudding"), &language)
            .iter()
            .flat_map(|problem| problem.lines(language, Level::Error))
            .collect()
    };
    assert_eq!(
        said(Language::Chinese),
        [
            "miyu: 无法安装 ./pudding：工具 read 已在 basesystem 中",
            "miyu: 无法安装 ./pudding：技能 pdf 已在 kit 中",
            "miyu: 无法安装 ./pudding：MCP 服务 lone 已在 svc 中",
            "miyu: 无法安装 ./pudding：斜杠命令 review 已在 kit 中",
            "miyu: 无法安装 ./pudding：人格 dog 已在 数据根 中",
        ]
    );
    assert_eq!(
        said(Language::English),
        [
            "miyu: cannot install ./pudding: tool read exists in basesystem",
            "miyu: cannot install ./pudding: skill pdf exists in kit",
            "miyu: cannot install ./pudding: MCP server lone exists in svc",
            "miyu: cannot install ./pudding: slash command review exists in kit",
            "miyu: cannot install ./pudding: persona dog exists in the data root",
        ]
    );
}

#[test]
fn an_invalid_manifest_says_which_line() {
    // 设计 47 第四节「装包失败」：`cannot install <包>: package.toml:<行>: <哪里不对>`。
    let error = json!({
        "message": "Invalid package.",
        "data": {"reason": "package_invalid", "problem": "unknown key 'process.system_account'", "line": 29}
    });
    assert_eq!(
        refused(&error, Some("foo"), &Language::English),
        [Problem::new(
            "cannot install foo: package.toml:29: unknown key 'process.system_account'"
        )]
    );
    // 别的拒绝照核心的原话，只有一条。
    let other = json!({"message": "软件包不存在。", "data": {"reason": "unknown_package"}});
    assert_eq!(
        refused(&other, None, &Language::Chinese),
        [Problem::new("软件包不存在")]
    );
}
