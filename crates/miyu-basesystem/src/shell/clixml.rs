//! PowerShell 的 CLIXML 换回纯文本（施工 SW-4）：Windows PowerShell 用 `-EncodedCommand` 起、标准错误不是终端时，出的错写成
//! `#< CLIXML` 开头的一段 XML（`<Objs …><S S="Error">…</S>…</Objs>`），`-OutputFormat Text` 也挡不住；原样交给她是一团看不懂的
//! 标记（2026-10-30 桌面端在 Win11 虚拟机上看到）。这里认出这一段，只留错误、警告那几行的原文，还原 XML 的转义和
//! PowerShell 的 `_xHHHH_` 转义（换行写成 `_x000D__x000A_`）；进度这类别的记录丢掉。没有这一段的照原样交回。

use std::borrow::Cow;

/// 这一段的开头。
const MARK: &str = "#< CLIXML";

/// 这一段的收尾。
const END: &str = "</Objs>";

/// `text` 里每一段 CLIXML 换成它的错误、警告原文；没有的照原样。
pub(super) fn plain(text: &str) -> Cow<'_, str> {
    if !text.contains(MARK) {
        return Cow::Borrowed(text);
    }
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(at) = rest.find(MARK) {
        out.push_str(&rest[..at]);
        let block = &rest[at + MARK.len()..];
        let Some(end) = block.find(END) else {
            // 截在半路的（输出太长被截了）：剩下的照原样。
            out.push_str(&rest[at..]);
            return Cow::Owned(out);
        };
        let said = messages(&block[..end]);
        out.push_str(&said);
        if !said.is_empty() && !said.ends_with('\n') {
            out.push('\n');
        }
        rest = block[end + END.len()..].trim_start_matches(['\r', '\n']);
    }
    out.push_str(rest);
    Cow::Owned(out)
}

/// 一段 `<Objs>` 里错误、警告那几条 `<S S="…">…</S>` 的原文，照先后接起来。
fn messages(block: &str) -> String {
    let mut out = String::new();
    let mut rest = block;
    while let Some(at) = rest.find("<S S=\"") {
        let after = &rest[at + 6..];
        let Some(quote) = after.find('"') else { break };
        let stream = &after[..quote];
        let Some(open_end) = after.find('>') else {
            break;
        };
        let body = &after[open_end + 1..];
        let Some(close) = body.find("</S>") else {
            break;
        };
        if matches!(stream, "Error" | "Warning" | "warning" | "error") {
            out.push_str(&unescape(&body[..close]));
        }
        rest = &body[close + 4..];
    }
    out
}

/// 还原 `_xHHHH_`（PowerShell 写进 XML 的字符）和 XML 的五种实体。
fn unescape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(at) = rest.find(['_', '&']) {
        out.push_str(&rest[..at]);
        let tail = &rest[at..];
        if let Some((ch, used)) = powershell(tail).or_else(|| entity(tail)) {
            if ch != '\r' {
                out.push(ch);
            }
            rest = &tail[used..];
        } else {
            out.push_str(&tail[..1]);
            rest = &tail[1..];
        }
    }
    out.push_str(rest);
    out
}

/// `_x000A_` 这样的一个：交回字和用掉几个字节。
fn powershell(text: &str) -> Option<(char, usize)> {
    let hex = text.strip_prefix("_x")?.get(..5)?;
    let (digits, end) = hex.split_at(4);
    if end != "_" {
        return None;
    }
    let code = u32::from_str_radix(digits, 16).ok()?;
    Some((char::from_u32(code)?, 7))
}

/// `&lt;` 这样的一个：交回字和用掉几个字节。
fn entity(text: &str) -> Option<(char, usize)> {
    [
        ("&lt;", '<'),
        ("&gt;", '>'),
        ("&amp;", '&'),
        ("&quot;", '"'),
        ("&apos;", '\''),
    ]
    .into_iter()
    .find_map(|(name, ch)| text.starts_with(name).then_some((ch, name.len())))
}

#[cfg(test)]
mod tests;
