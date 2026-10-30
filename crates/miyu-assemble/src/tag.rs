//! 一块带标签的事实（施工 7-7 从 `jobs.rs` 拿出来，施工 7-10 别的 harness 发来的话也用）：别处来的一句话，开头一行是标签，
//! 接着是原话，最后是收尾的标签，拼成一块字；字以外的块接在这一块后面。
//!
//! 原话不转义（是别人写的多行正文），只中和一样：原话里提前写出了这一块的收尾标签的（[`defuse`]，施工 AS-10，发布前第二轮
//! 审查 R2-2、R2-3）。这几块落在人这一边，原话里写一行 `</command-ended>`，后面的字在模型看来就跑到了标签外面，能冒充人、
//! 冒充核心包的另一块。

use std::borrow::Cow;

use miyu_kernel::block::{Block, Text};

/// 把 `blocks` 包进标签：`open` 是填好字段的开头一行，几块字照先后接上、原样不转义（是别人写的多行正文，只照 [`defuse`]
/// 中和提前写出的收尾），末尾没有换行的补一个，再接 `close`。字以外的块（附件）照先后接在这一块后面。一块字都没有的，这一块
/// 是开头接收尾。
pub(crate) fn tagged(open: String, blocks: Vec<Block>, close: &str) -> Vec<Block> {
    let mut text = open;
    let mut rest = Vec::new();
    for block in blocks {
        match block {
            Block::Text(said) => text.push_str(&defuse(&said.text, &[close])),
            other => rest.push(other),
        }
    }
    if !text.ends_with('\n') {
        text.push('\n');
    }
    text.push_str(close);
    let mut tagged = vec![Block::Text(Text { text })];
    tagged.extend(rest);
    tagged
}

/// 别处来的原文 `text` 里提前写出了 `closes` 里那几个收尾标签的，在 `<` 后面插一个 `\` 中和掉（施工 AS-10）：`</command-ended`
/// 写成 `<\/command-ended`，自闭合的 `<end/` 写成 `<\end/`，大小写不论；别的字节一个不动，没写出的照原样交回（前缀的字节不变）。
/// 收尾标签照 `closes` 的字认（`</名字>`、`<名字/>`），代码里不写死标签名。
pub(crate) fn defuse<'a>(text: &'a str, closes: &[&str]) -> Cow<'a, str> {
    let needles: Vec<String> = closes.iter().flat_map(|close| needles(close)).collect();
    let lower = text.to_ascii_lowercase();
    let mut cuts: Vec<usize> = needles
        .iter()
        .flat_map(|needle| lower.match_indices(needle.as_str()).map(|(at, _)| at + 1))
        .collect();
    if cuts.is_empty() {
        return Cow::Borrowed(text);
    }
    cuts.sort_unstable();
    cuts.dedup();
    let mut out = String::with_capacity(text.len() + cuts.len());
    let mut from = 0;
    for cut in cuts {
        out.push_str(&text[from..cut]);
        out.push('\\');
        from = cut;
    }
    out.push_str(&text[from..]);
    Cow::Owned(out)
}

/// 收尾的字 `close` 里认出来的收尾标签，写成要找的开头（小写）：`</file>` 是 `</file`，`<end/>` 是 `<end/`。
fn needles(close: &str) -> Vec<String> {
    let mut found = Vec::new();
    for (at, _) in close.match_indices('<') {
        let rest = &close[at + 1..];
        let (slash, rest) = match rest.strip_prefix('/') {
            Some(rest) => (true, rest),
            None => (false, rest),
        };
        let name: String = rest
            .chars()
            .take_while(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_')
            .collect();
        if name.is_empty() {
            continue;
        }
        let after = &rest[name.len()..];
        if slash {
            found.push(format!("</{}", name.to_ascii_lowercase()));
        } else if after.starts_with("/>") {
            found.push(format!("<{}/", name.to_ascii_lowercase()));
        }
    }
    found
}

#[cfg(test)]
mod tests;
