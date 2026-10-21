//! 包带了什么照表认（施工 F-8 上补，设计 `31-软件包.md` 第二节第 1、2 条）：清单不写种类。程序表 `[ui]`、`[process]`、
//! `[builtin]`、`[worker]` 至多一张，有哪一张就是哪一种；没有程序表、带了 MCP 服务（`[[mcp]]`）的是 `bundle`（施工 X-2）；
//! 再没有、有 `[mascot]` 的是吉祥物包；都没有的也是 `bundle`：只带技能、脚本的包（施工 X-13 上），清单里写不出它们，带的是
//! 包文件夹里的 `skills/`、`scripts/`。写了 `[command]` 或协议版本、却没有程序表的少了程序表（说协议的是程序）。哪种包能写哪几张表照 [`tables`]，吉祥物
//! 哪一种都能带。

use toml_edit::Table;

use super::{Code, PackageKind, Problem, Reader};

/// 程序表和它对着的种类，照这个先后报。
const PROGRAMS: [(&str, PackageKind); 4] = [
    ("ui", PackageKind::Ui),
    ("process", PackageKind::Process),
    ("builtin", PackageKind::Builtin),
    ("worker", PackageKind::Worker),
];

/// 照清单 `root` 里有哪几张表认出种类。
pub(super) fn of(reader: &Reader<'_>, root: &Table) -> Result<PackageKind, Problem> {
    let found: Vec<(&str, PackageKind)> = PROGRAMS
        .into_iter()
        .filter(|(name, _)| root.contains_key(name))
        .collect();
    match found.as_slice() {
        [(_, kind)] => Ok(*kind),
        [] if root.contains_key("mcp") => Ok(PackageKind::Bundle),
        [] if root.contains_key("mascot") => Ok(PackageKind::Mascot),
        [] if root.contains_key("theme") => Ok(PackageKind::Theme),
        [] if root.contains_key("command") || speaks_protocol(root) => Err(reader.problem(
            None,
            Code::MissingKey,
            "[ui] / [process] / [builtin] / [worker]",
            "a package with [command] or a protocol carries a program ([ui], [process], [builtin] or [worker])"
                .to_string(),
        )),
        [] => Ok(PackageKind::Bundle),
        [(first, _), (second, _), ..] => Err(reader.problem(
            Some(&root[second]),
            Code::TwoPrograms,
            &format!("[{first}], [{second}]"),
            format!("a package carries one program: [{first}] and [{second}] can't both be here"),
        )),
    }
}

/// `[package]` 里写了协议版本。
fn speaks_protocol(root: &Table) -> bool {
    root.get("package")
        .and_then(toml_edit::Item::as_table_like)
        .is_some_and(|package| package.contains_key("protocol"))
}

/// 这种包能写哪几张表（施工 F-1；F-8 上补起吉祥物哪一种都能带）：别的写了报 `wrong_kind`。
pub(super) fn tables(kind: PackageKind) -> &'static [&'static str] {
    match kind {
        PackageKind::Ui => &[
            "package",
            "command",
            "ui",
            "check",
            "settings",
            "depends",
            "recommends",
            "mascot",
        ],
        PackageKind::Process => &[
            "package",
            "command",
            "process",
            "check",
            "settings",
            "features",
            "connection",
            "depends",
            "recommends",
            "page",
            "mascot",
            "mcp",
            "slash",
            "search",
            "widget",
            "export",
            "content",
        ],
        PackageKind::Builtin => &[
            "package",
            "builtin",
            "features",
            "depends",
            "recommends",
            "page",
            "mascot",
        ],
        PackageKind::Worker => &["package", "worker", "mascot", "depends", "recommends"],
        PackageKind::Mascot => &["package", "mascot", "depends", "recommends"],
        // 主题包（施工 F-16）：只有主题，不带吉祥物、程序。
        PackageKind::Theme => &["package", "theme", "depends", "recommends"],
        PackageKind::Bundle => &[
            "package",
            "mcp",
            "mascot",
            "content",
            "depends",
            "recommends",
        ],
    }
}
