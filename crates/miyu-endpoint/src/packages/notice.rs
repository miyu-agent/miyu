//! 看一眼里的「注意」（施工 PK-3 上起，设计 `61-装包说清来源和能力.md` 第三节第 2 条、定了的 3；施工 CM-1 改字）：社区的包写
//! 「是社区包，没有经过官方审核，可能存在风险」，本机文件装的写「来自本机文件，没有经过官方审核，可能存在风险」：本机文件不是从
//! 社区收录来的，不能叫社区包。原来另有一句「它带的程序会以你的身份在这台机器上跑，不在沙盒里」，扩展沙盒（设计 62）暂缓、现在
//! 没有那层沙盒，施工 CM-1 连同认「带没带会跑的东西」的代码一起去掉（2026-11-12 项目主人定）。字是核心的
//! （`resources/core/human/<语言>.json` 的 `packages/notice/*`），照连接的语言挑好、一句一个字符串交给头，头只管排版。

use miyu_config::Words;
use serde_json::{Value, json};

use crate::Core;

/// 社区的：只有一个包的；写出是哪几个的。
const COMMUNITY: (&str, &str) = (
    "packages/notice/unreviewed",
    "packages/notice/unreviewed/named",
);
/// 本机文件的：只有一个包的；写出是哪几个的。
const FILE: (&str, &str) = ("packages/notice/file", "packages/notice/file/named");
/// 几个编号之间的连接符。
const SEPARATOR: &str = "packages/notice/separator";

/// 看一眼里的一个包：编号、`source`（`official`、`community`、`file`……）。
pub(super) struct Item<'a> {
    pub(super) id: &'a str,
    pub(super) source: &'a str,
}

/// 这一次要装的几个包 `items` 里要说的「注意」，照连接的语言 `language`，一句一个字符串；没有社区的、本机文件的是 `None`。
/// `alone` 是这一次只装一个包：说「这个包」，不然写出是哪几个。
pub(super) fn notice(
    core: &Core,
    language: &str,
    items: &[Item<'_>],
    alone: bool,
) -> Option<Value> {
    if !items
        .iter()
        .any(|one| matches!(one.source, "community" | "file"))
    {
        return None;
    }
    let words = crate::config::methods::words(core, language).ok()?;
    said(items, alone, |key, args| words.sentence(key, args)).map(|said| json!(said))
}

/// [`notice`] 的句子怎么拼（纯的，测试照它算）：社区的一句、本机文件的一句，各自写出是哪几个，先社区后本机文件；`sentence`
/// 照 key 和参数取一句。两样都没有的、哪一句取不到的是 `None`。
fn said(
    items: &[Item<'_>],
    alone: bool,
    sentence: impl Fn(&str, &[(&str, &str)]) -> Option<String>,
) -> Option<Vec<String>> {
    let mut said = Vec::new();
    for (source, (one, named)) in [("community", COMMUNITY), ("file", FILE)] {
        let ids: Vec<&str> = items
            .iter()
            .filter(|item| item.source == source)
            .map(|item| item.id)
            .collect();
        if ids.is_empty() {
            continue;
        }
        if alone {
            said.push(sentence(one, &[])?);
            continue;
        }
        let separator = sentence(SEPARATOR, &[]).unwrap_or_else(|| ", ".to_string());
        said.push(sentence(named, &[("packages", &ids.join(&separator))])?);
    }
    (!said.is_empty()).then_some(said)
}

#[cfg(test)]
mod tests;
