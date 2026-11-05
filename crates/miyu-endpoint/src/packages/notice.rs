//! 看一眼里的「注意」（施工 PK-3 上，设计 `61-装包说清来源和能力.md` 第三节第 2 条、定了的 3）：社区的、本机文件的包写「没经过
//! 官方审核」；里面带程序、MCP 服务、脚本的再写「它带的程序会以你的身份在这台机器上跑，不在沙盒里」。字是核心的
//! （`resources/core/human/<语言>.json` 的 `packages/notice/*`），照连接的语言挑好、一句一个字符串交给头，头只管排版。

use miyu_config::Words;
use miyu_source::index::Entry;
use serde_json::{Value, json};

use crate::Core;

/// 「没经过官方审核」：只有一个包的；写出是哪几个的。
const UNREVIEWED: &str = "packages/notice/unreviewed";
const UNREVIEWED_NAMED: &str = "packages/notice/unreviewed/named";
/// 「它带的程序会以你的身份在这台机器上跑，不在沙盒里」：只有一个包的；写出是哪几个的。扩展还没关进沙盒（`extensions.md`
/// 「还没有的」）才这么写：**设计 62（扩展沙盒）落地后改这两句**，别的不用动。
const UNSANDBOXED: &str = "packages/notice/unsandboxed";
const UNSANDBOXED_NAMED: &str = "packages/notice/unsandboxed/named";
/// 几个编号之间的连接符。
const SEPARATOR: &str = "packages/notice/separator";

/// 看一眼里的一个包：编号、`source`（`official`、`community`、`file`……）、带没带会跑的东西。
pub(super) struct Item<'a> {
    pub(super) id: &'a str,
    pub(super) source: &'a str,
    pub(super) runs: bool,
}

/// 没经过官方审核的来源：社区的、本机文件的。
fn unreviewed(source: &str) -> bool {
    matches!(source, "community" | "file")
}

/// 一份清单的种类 `kind`（`process`、`ui`……，索引里以前生成的没有）、带的几样 `has` 里有没有会以你的身份跑的：自己的程序（编进
/// 核心的 `builtin` 不算）、子命令、MCP 服务、脚本。
pub(super) fn runs(kind: Option<&str>, has: impl Fn(&str) -> bool) -> bool {
    if kind == Some("builtin") {
        return false;
    }
    matches!(kind, Some("process" | "ui" | "worker"))
        || ["command", "mcp", "scripts"].into_iter().any(has)
}

/// 照索引里的一条认：种类、`provides` 里的子命令、MCP 服务、脚本。
pub(super) fn entry_runs(entry: &Entry) -> bool {
    runs(entry.kind.as_deref(), |kind| {
        entry.provides.contains_key(kind)
    })
}

/// 这一次要装的几个包 `items` 里要说的「注意」，照连接的语言 `language`，一句一个字符串；没有社区的、本机文件的是 `None`。
/// `alone` 是这一次只装一个包：说「这个包」，不然写出是哪几个。
pub(super) fn notice(
    core: &Core,
    language: &str,
    items: &[Item<'_>],
    alone: bool,
) -> Option<Value> {
    let flagged: Vec<&Item<'_>> = items.iter().filter(|one| unreviewed(one.source)).collect();
    if flagged.is_empty() {
        return None;
    }
    let words = crate::config::methods::words(core, language).ok()?;
    let say = |(one, named): (&str, &str), ids: &[&str]| -> Option<String> {
        if alone {
            return words.sentence(one, &[]);
        }
        let separator = words
            .sentence(SEPARATOR, &[])
            .unwrap_or_else(|| ", ".to_string());
        words.sentence(named, &[("packages", &ids.join(&separator))])
    };
    let ids: Vec<&str> = flagged.iter().map(|one| one.id).collect();
    let running: Vec<&str> = flagged
        .iter()
        .filter(|one| one.runs)
        .map(|one| one.id)
        .collect();
    let mut said = vec![say((UNREVIEWED, UNREVIEWED_NAMED), &ids)?];
    if !running.is_empty() {
        said.push(say((UNSANDBOXED, UNSANDBOXED_NAMED), &running)?);
    }
    Some(json!(said))
}
