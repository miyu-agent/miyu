//! 点开那次 `shell`、那个任务时的整份嵌套调用（施工 M-5 上，`docs/blueprint/view.md`「`view.detail`」）：照日志里了结的
//! `tool.nested` 和会话这一刻在跑的，挑出挂在这个父级下面的，照序号排。纯逻辑：日志、在跑的、字都由外面交进来。

use std::collections::BTreeMap;

use serde::Serialize;
use serde_json::Value;

use miyu_kernel::event::{Body as EventBody, Event};
use miyu_kernel::id::{ContentHash, Parent};
use miyu_kernel::session::NestedRunning;
use miyu_kernel::time::Timestamp;

use crate::entry::{Title, ToolState};
use crate::title;
use crate::words::Texts;

/// 列表里的一项。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct NestedItem {
    /// 序号。
    pub n: u32,
    /// 工具名。
    pub name: String,
    /// 显示名和对象，照连接的语言。
    pub title: Title,
    /// 参数原文。
    pub args: String,
    /// `running`，或者了结的 `ok`、`error`、`denied`、`needs_approval`、`cancelled`。
    pub state: String,
    /// 在跑的：派去跑的时刻。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub started: Option<Timestamp>,
    /// 执行用了多久。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub took_ms: Option<u64>,
    /// 拒绝、出错的那一句。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    /// 交给脚本的那份结果：头照 `blob.get` 取。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Stored>,
}

/// 一份存下来的结果。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Stored {
    /// 内容哈希。
    pub blob: ContentHash,
    /// 多少字节。
    pub bytes: u64,
}

/// 挂在 `parent` 下面的嵌套调用：日志 `events` 里了结的加上 `running` 在跑的，照序号；写了 `after` 的只交序号比它大的，
/// 和它以前还在跑的。
#[must_use]
pub fn listed(
    parent: &Parent,
    events: &[Event],
    running: &[NestedRunning],
    after: Option<u32>,
    texts: &Texts,
) -> Vec<NestedItem> {
    let mut items: BTreeMap<u32, NestedItem> = BTreeMap::new();
    for event in events {
        let EventBody::ToolNested(nested) = &event.body else {
            continue;
        };
        if nested.id.parent() != parent {
            continue;
        }
        items.insert(
            nested.id.n(),
            NestedItem {
                n: nested.id.n(),
                name: nested.name.clone(),
                title: titled(&nested.name, &nested.args, ToolState::Ok, texts),
                args: nested.args.clone(),
                state: nested.status.as_str().to_string(),
                started: None,
                took_ms: nested.duration_ms,
                reason: nested.reason.clone(),
                result: nested
                    .result
                    .clone()
                    .zip(nested.bytes)
                    .map(|(blob, bytes)| Stored { blob, bytes }),
            },
        );
    }
    for call in running.iter().filter(|call| call.id.parent() == parent) {
        items.entry(call.id.n()).or_insert_with(|| NestedItem {
            n: call.id.n(),
            name: call.name.clone(),
            title: titled(&call.name, &call.args, ToolState::Running, texts),
            args: call.args.clone(),
            state: "running".to_string(),
            started: Some(call.at),
            took_ms: None,
            reason: None,
            result: None,
        });
    }
    items
        .into_values()
        .filter(|item| after.is_none_or(|after| item.n > after || item.state == "running"))
        .collect()
}

/// 一项的标题：同工具那一步的标题，结果那一句不写。
fn titled(name: &str, args: &str, state: ToolState, texts: &Texts) -> Title {
    let parsed: Value = serde_json::from_str(args).unwrap_or(Value::Null);
    title::of(name, &parsed, state, None, &BTreeMap::new(), texts)
}
