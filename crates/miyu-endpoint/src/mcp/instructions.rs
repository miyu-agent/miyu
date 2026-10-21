//! MCP 服务交的说明放进 system 尾部（施工 X-39，`docs/blueprint/mcp.md`「怎么走」第三条第 5 条）：一个服务一段，键是
//! [`miyu_session::mcp_key`]，外壳照 `core/mcp/instructions.txt`。原文截到 [`APPEND_MAX`] 个字符（同扩展经 `system.append`
//! 放的上限）；原文里出现外壳的结束标签的，那一处的 `<` 写成 `\u003c`（同技能列表的转义写法），伪造不出标签外的字，别的照原样
//! （它就是说明）：不经模板换（模板换进去的字段一律照 JSON 转成一行，08 C7），外壳照 `{text}` 切成前后两段，服务名照模板换进
//! 前一段，原文接在中间。会话照这一刻的工具面挑哪几段带、什么时候换（`miyu_session` 的 `appended`）。

use std::collections::BTreeMap;

use miyu_policy::APPEND_MAX;

use super::TARGET;
use super::tool::render;
use crate::Core;

/// 外壳的结束标签的开头（大小写都算）。
const CLOSE: &str = "</mcp-instructions";

/// 外壳里原文的位置：照它切成前后两段。
const TEXT: &str = "{text}";

/// 每个交了说明的服务一段：键到拼好的那一段。外壳读不出来的（安装坏了）一段都没有，记一行。
pub(crate) fn appended(core: &Core) -> BTreeMap<String, Option<String>> {
    let given = core.mcp.instructions();
    if given.is_empty() {
        return BTreeMap::new();
    }
    let shell = core
        .resources
        .mcp_instructions()
        .map_err(|error| error.to_string())
        .and_then(|template| {
            template
                .split_once(TEXT)
                .map(|(head, tail)| (head.to_string(), tail.to_string()))
                .ok_or_else(|| format!("no {TEXT} in it"))
        });
    let (head, tail) = match shell {
        Ok(shell) => shell,
        Err(error) => {
            tracing::error!(target: TARGET, error = error.as_str(), "mcp instructions template unreadable");
            return BTreeMap::new();
        }
    };
    given
        .into_iter()
        .map(|(service, text)| {
            let opened = render(&head, &[("name", &service)]);
            let block = format!("{opened}{}{}", guarded(&cut(&text)), render(&tail, &[]));
            (
                miyu_session::mcp_key(&service),
                Some(block.trim_end().to_string()),
            )
        })
        .collect()
}

/// 截到 [`APPEND_MAX`] 个字符。
fn cut(text: &str) -> String {
    text.chars().take(APPEND_MAX).collect()
}

/// 原文里外壳的结束标签（大小写都算）：那一处的 `<` 写成 `\u003c`。只动 ASCII，字节的位置照旧对得上。
fn guarded(text: &str) -> String {
    let lower = text.to_ascii_lowercase();
    let mut out = String::with_capacity(text.len());
    let mut at = 0;
    while let Some(found) = lower[at..].find(CLOSE) {
        let start = at + found;
        out.push_str(&text[at..start]);
        out.push_str("\\u003c");
        at = start + 1;
    }
    out.push_str(&text[at..]);
    out
}

#[cfg(test)]
mod tests;
