//! 通道上的一个请求怎么答（施工 M-3，设计 `39-脚本调工具.md` 第六节第 3、4 条）：了结的照结局定退出码和印什么，没收下的照
//! 原因说。退出码：0 成功，1 工具出错或被拒，2 用法不对，3 被打断或所属的命令已经结束，4 要人确认。

use miyu_kernel::event::NestedStatus;
use miyu_kernel::session::NestedRefusal;
use miyu_tool::{Nested, NestedPort, Settled};

use super::Texts;
use crate::load::say;

/// 退出码：用法不对。
pub(super) const USAGE: u8 = 2;

/// 答回去的：`miyu call` 照它印、退出。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Reply {
    pub(super) code: u8,
    pub(super) stdout: String,
    pub(super) stderr: String,
}

impl Reply {
    /// 只在标准错误上说一句。
    pub(super) fn said(code: u8, stderr: String) -> Reply {
        Reply {
            code,
            stdout: String::new(),
            stderr,
        }
    }
}

/// 调一次：等内核记过、落了盘的结局。
pub(super) async fn call(
    port: &dyn NestedPort,
    texts: &Texts,
    name: String,
    args: String,
) -> Reply {
    let nested = port.call(name.clone(), args).await;
    settle(port, texts, &name, nested)
}

/// 结局写成答回去的。
pub(super) fn settle(port: &dyn NestedPort, texts: &Texts, name: &str, nested: Nested) -> Reply {
    match nested {
        Nested::Refused(NestedRefusal::Gone) => Reply::said(3, say(&texts.ended, &[])),
        Nested::Refused(NestedRefusal::Unknown) => Reply::said(USAGE, unknown(port, texts, name)),
        Nested::Refused(NestedRefusal::NotAllowed) => {
            Reply::said(USAGE, not_allowed(port, texts, name))
        }
        Nested::Refused(NestedRefusal::NotAnObject) => {
            let mut stderr = say(&texts.bad_args, &[("name", name)]);
            if let Some(schema) = schema_text(port, texts, name) {
                stderr.push_str(&schema);
            }
            Reply::said(USAGE, stderr)
        }
        Nested::Settled(settled) => finished(texts, name, settled),
    }
}

/// 了结了的：跑完的印结果，别的说一句。
fn finished(texts: &Texts, name: &str, settled: Settled) -> Reply {
    match settled.status {
        NestedStatus::Ok => Reply {
            code: 0,
            // 有结构化结果的一行 JSON，中文不转义；没有的照原样的字。都以换行结尾。
            stdout: line(match settled.data {
                Some(data) => data.to_string(),
                None => settled.text,
            }),
            stderr: match settled.images_dropped {
                true => say(&texts.images, &[]),
                false => String::new(),
            },
        },
        NestedStatus::Error => Reply::said(1, line(settled.text)),
        NestedStatus::Denied => Reply::said(1, line(settled.reason.unwrap_or_default())),
        NestedStatus::NeedsApproval => Reply::said(4, say(&texts.approval, &[("name", name)])),
        // 被打断、父级先结束了，和不认识的结局：都当没跑完。
        _ => Reply::said(3, say(&texts.interrupted, &[])),
    }
}

/// `--schema`：这件的说明和参数格式；脚本里调不了的照不认识、调不了说。
pub(super) fn schema(port: &dyn NestedPort, texts: &Texts, name: &str) -> Reply {
    match schema_text(port, texts, name) {
        Some(text) => Reply {
            code: 0,
            stdout: text,
            stderr: String::new(),
        },
        None => match guess(port, name) {
            Some(_) => Reply::said(USAGE, unknown(port, texts, name)),
            None => Reply::said(USAGE, not_allowed(port, texts, name)),
        },
    }
}

/// 这件的说明和参数格式写成的字。不转义：两样都是工具面上的原样，她在 tools 数组里看到的也是原样，转了反而看不懂。
fn schema_text(port: &dyn NestedPort, texts: &Texts, name: &str) -> Option<String> {
    let schema = port.schema(name)?;
    let fields = std::collections::BTreeMap::from([
        ("name", name),
        ("description", schema.description.trim_end()),
        ("parameters", schema.parameters.as_str()),
    ]);
    texts.schema.fill(&fields, str::to_string).ok()
}

/// 不认识：名字差一两个字的指出来。
fn unknown(port: &dyn NestedPort, texts: &Texts, name: &str) -> String {
    let mut text = say(&texts.unknown, &[("name", name)]);
    if let Some(close) = guess(port, name) {
        text = format!(
            "{} {}",
            text.trim_end(),
            say(&texts.guess, &[("name", &close)])
        );
    }
    text
}

/// 调不了：列出能调的。
fn not_allowed(port: &dyn NestedPort, texts: &Texts, name: &str) -> String {
    let tools = port.callable().join(", ");
    say(&texts.not_allowed, &[("name", name), ("tools", &tools)])
}

/// 能调的里面和 `name` 只差一两个字的那一个：差得最少的，一样少的照名字先后。
fn guess(port: &dyn NestedPort, name: &str) -> Option<String> {
    port.callable()
        .into_iter()
        .map(|tool| (distance(&tool, name), tool))
        .filter(|(far, tool)| *far > 0 && *far <= 2 && tool.len() > 2)
        .min()
        .map(|(_, tool)| tool)
}

/// 编辑距离（照字）。
fn distance(a: &str, b: &str) -> usize {
    let b: Vec<char> = b.chars().collect();
    let mut row: Vec<usize> = (0..=b.len()).collect();
    for (i, x) in a.chars().enumerate() {
        let mut previous = row[0];
        row[0] = i + 1;
        for (j, y) in b.iter().enumerate() {
            let current = row[j + 1];
            row[j + 1] = (previous + usize::from(x != *y))
                .min(row[j] + 1)
                .min(current + 1);
            previous = current;
        }
    }
    row[b.len()]
}

/// 不是空的，就以换行结尾。
fn line(mut text: String) -> String {
    if !text.is_empty() && !text.ends_with('\n') {
        text.push('\n');
    }
    text
}

#[cfg(test)]
mod tests;
