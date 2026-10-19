//! 边收边读合并交回来的（施工 R-21，`docs/blueprint/memory.md` 第七条第 12 款）：模型一边写，一边交给 [`Watch`]，写完一条改的、
//! 一条作废的就交出来一条，摘要交写到第几个字。头照它实时画「窥探梦境」（2026-10-19 项目主人：「能实时把当前处理的条目显示
//! 出来」）。纯逻辑。
//!
//! - 只看写完了的：`revised`、`retired` 列表里括号配得上的一个对象读成了才算一条；单条照 [`super::decisions`] 的规矩核对
//!   （编号是交进去的、字数、种类、`by`），不合的不交。几条之间的核对（`by` 那一条也要作废的）要看全部，这里不做：交出来的是
//!   她这一刻写的，落的照全部核对，可能少几条。
//! - 不认得 JSON 的写法（前后多说的话、代码块的围栏）不碍事：照键名找。字里碰巧有同样的键名的，可能早一点、晚一点开始交，落的
//!   照整段读。
//! - 换了端点重来（[`Watch::restart`]）：从头读，已经交过的条目不再交；摘要是重写的，字数从零交起。

use std::collections::BTreeSet;

use serde_json::Value;

use super::{Retire, Revised, retire, revise};
use crate::memory::MemoryId;

/// 交出来的一样。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Seen {
    /// 写完了一条改的。
    Revised(Revised),
    /// 写完了一条作废的。
    Retired(Retire),
    /// 摘要写到了第几个字（只在多了 [`SUMMARY_STEP`] 个字以上、或者写完了的时候交）。
    Summary {
        /// 写了几个字。
        chars: usize,
        /// 写完了。
        done: bool,
    },
}

/// 摘要的字数至少多这么多才交一次：一个字一次太碎。
pub const SUMMARY_STEP: usize = 20;

/// 一次合并交回来的，边收边读。
#[derive(Debug, Clone)]
pub struct Watch {
    given: BTreeSet<MemoryId>,
    max_chars: usize,
    text: String,
    /// 交过的：同一条不交两次（重来以后也是）。
    revised: BTreeSet<MemoryId>,
    retired: BTreeSet<MemoryId>,
    /// 摘要交到了第几个字、交过写完了没有。
    summary: (usize, bool),
}

impl Watch {
    /// 这一次交进去的是 `given` 这几条，一条最多 `max_chars` 个字（同 [`super::decisions`]）。
    #[must_use]
    pub fn new(given: BTreeSet<MemoryId>, max_chars: usize) -> Watch {
        Watch {
            given,
            max_chars,
            text: String::new(),
            revised: BTreeSet::new(),
            retired: BTreeSet::new(),
            summary: (0, false),
        }
    }

    /// 又收到一段 `delta`：交回这一下新写完的，照写的先后。
    pub fn feed(&mut self, delta: &str) -> Vec<Seen> {
        self.text.push_str(delta);
        let mut seen = Vec::new();
        for item in items(&self.text, "revised") {
            if let Some(one) = revise(&item, &self.given, self.max_chars)
                && self.revised.insert(one.id)
            {
                seen.push(Seen::Revised(one));
            }
        }
        for item in items(&self.text, "retired") {
            if let Some(one) = retire(&item, &self.given)
                && self.retired.insert(one.id)
            {
                seen.push(Seen::Retired(one));
            }
        }
        if let Some((chars, done)) = summary(&self.text) {
            let (told, finished) = self.summary;
            if !finished && (done || chars >= told + SUMMARY_STEP) {
                self.summary = (chars, done);
                seen.push(Seen::Summary { chars, done });
            }
        }
        seen
    }

    /// 换了端点重来：收到的从头算，交过的条目不再交；摘要从头写，字数也从零算。
    pub fn restart(&mut self) {
        self.text.clear();
        self.summary = (0, false);
    }
}

/// 键 `name` 的值开头在哪（冒号后面第一个不是空白的字）：照 `"name"` 后面跟着冒号找，第一个算。
fn value_at(text: &str, name: &str) -> Option<usize> {
    let key = format!("\"{name}\"");
    let mut from = 0;
    while let Some(at) = text[from..].find(&key) {
        let after = from + at + key.len();
        let rest = text[after..].trim_start();
        if let Some(value) = rest.strip_prefix(':') {
            let skipped = value.len() - value.trim_start().len();
            return Some(text.len() - value.len() + skipped);
        }
        from = after;
    }
    None
}

/// 列表 `name` 里写完了的几个对象，照先后；列表还没开始的、不是列表的是空的。
fn items(text: &str, name: &str) -> Vec<Value> {
    let Some(start) = value_at(text, name) else {
        return Vec::new();
    };
    let Some(rest) = text[start..].strip_prefix('[') else {
        return Vec::new();
    };
    let mut found = Vec::new();
    let mut at = 0;
    loop {
        let skipped = rest[at..].len()
            - rest[at..]
                .trim_start_matches([' ', '\n', '\r', '\t', ','])
                .len();
        at += skipped;
        if !rest[at..].starts_with('{') {
            return found;
        }
        let Some(end) = closing(&rest[at..]) else {
            return found;
        };
        match serde_json::from_str(&rest[at..at + end]) {
            Ok(value) => found.push(value),
            Err(_) => return found,
        }
        at += end;
    }
}

/// 从 `{` 起，配上它的那个 `}` 后面一个字节的位置；还没写完的没有。字符串里的括号不算。
fn closing(text: &str) -> Option<usize> {
    let (mut depth, mut quoted, mut escaped) = (0_usize, false, false);
    for (at, byte) in text.bytes().enumerate() {
        if quoted {
            match (escaped, byte) {
                (true, _) => escaped = false,
                (false, b'\\') => escaped = true,
                (false, b'"') => quoted = false,
                _ => {}
            }
            continue;
        }
        match byte {
            b'"' => quoted = true,
            b'{' => depth += 1,
            b'}' => {
                depth = depth.checked_sub(1)?;
                if depth == 0 {
                    return Some(at + 1);
                }
            }
            _ => {}
        }
    }
    None
}

/// 摘要写到了第几个字、写完了没有：还没开始写的、不是字的没有。转义算一个字。
fn summary(text: &str) -> Option<(usize, bool)> {
    let start = value_at(text, "summary")?;
    let rest = text[start..].strip_prefix('"')?;
    let (mut chars, mut escaped) = (0, false);
    for c in rest.chars() {
        match (escaped, c) {
            (true, _) => {
                escaped = false;
                chars += 1;
            }
            (false, '\\') => escaped = true,
            (false, '"') => return Some((chars, true)),
            _ => chars += 1,
        }
    }
    Some((chars, false))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::merge::Kind;

    fn id(n: u64) -> MemoryId {
        MemoryId::parse(&format!("m{n}")).expect("合写法")
    }

    fn watch() -> Watch {
        Watch::new((1..=5).map(id).collect(), 120)
    }

    /// 一个字一个字地喂，交回全部交出来的。
    fn trickle(watch: &mut Watch, text: &str) -> Vec<Seen> {
        let mut seen = Vec::new();
        let mut buffer = [0; 4];
        for c in text.chars() {
            seen.extend(watch.feed(c.encode_utf8(&mut buffer)));
        }
        seen
    }

    const ANSWER: &str = r#"```json
{"revised": [{"id": "m1", "text": "用户 2026-10-16 去复查膝盖。"}, {"id": "m9", "text": "不是交进去的"}],
 "retired": [{"id": "m2", "kind": "repeat", "by": "m3", "why": "和 m3 一样 {括号} \"引号\""},
             {"id": "m4", "kind": "worthless", "why": "一次性的"},
             {"id": "m5", "kind": "outdated", "by": "m1", "why": "by 比它早，不合"}],
 "summary": "用户养了一只橘猫，叫团子，最近在学日语，准备明年考 N2。"}
```"#;

    #[test]
    fn each_item_comes_out_once_it_is_written() {
        let seen = trickle(&mut watch(), ANSWER);
        let items: Vec<&Seen> = seen
            .iter()
            .filter(|one| !matches!(one, Seen::Summary { .. }))
            .collect();
        assert_eq!(
            items,
            [
                &Seen::Revised(Revised {
                    id: id(1),
                    text: "用户 2026-10-16 去复查膝盖。".into()
                }),
                &Seen::Retired(Retire {
                    id: id(2),
                    kind: Kind::Repeat,
                    by: Some(id(3)),
                    why: "和 m3 一样 {括号} \"引号\"".into()
                }),
                &Seen::Retired(Retire {
                    id: id(4),
                    kind: Kind::Worthless,
                    by: None,
                    why: "一次性的".into()
                }),
            ],
            "单条不合的不交（m9 不是交进去的、m5 的 by 比它早），字里的括号、引号不算"
        );
        let summaries: Vec<(usize, bool)> = seen
            .iter()
            .filter_map(|one| match one {
                Seen::Summary { chars, done } => Some((*chars, *done)),
                _ => None,
            })
            .collect();
        assert_eq!(
            summaries,
            [(20, false), (29, true)],
            "多二十个字交一次，写完交一次"
        );
    }

    #[test]
    fn nothing_comes_out_twice_even_after_a_restart() {
        let mut watch = watch();
        let first = watch.feed(ANSWER);
        assert_eq!(first.len(), 4, "{first:?}");
        assert!(watch.feed("").is_empty(), "没有新的");
        watch.restart();
        let again = watch.feed(ANSWER);
        assert_eq!(
            again,
            [Seen::Summary {
                chars: 29,
                done: true
            }],
            "重来以后同样的几条不再交；摘要是重写的，照交"
        );
    }

    #[test]
    fn half_written_items_wait() {
        let mut watch = watch();
        assert!(
            watch
                .feed(r#"{"revised": [{"id": "m1", "text": "写了一半"#)
                .is_empty()
        );
        assert_eq!(watch.feed(r#""}"#).len(), 1, "括号配上了才算");
        assert!(watch.feed(r#"], "retired": "不是列表""#).is_empty());
    }
}
