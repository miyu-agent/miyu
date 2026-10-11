//! 一件 MCP 工具（施工 X-3 上，`mcp.md`「怎么走」第四条）：调用时照服务这时连着的客户端发 `tools/call`，照清单的时限等。
//! 服务不在的（起着、退避着、停下了）、等着时进程退出了的说暂时不可用；到了时限没回的丢掉这次调用（客户端发取消），说没在
//! N 秒内答完；被掐掉的（人停了这一轮）同样丢掉、发取消。服务回 JSON-RPC 的错照它的原话交；写法不对、要补信息的照工具出了
//! 内部错误说。回来的内容换块在 `content.rs`。

use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;

use serde_json::{Value, json};

use miyu_kernel::block::{Block, Text};
use miyu_kernel::event::Said;
use miyu_kernel::template::Template;
use miyu_mcp::Failed;
use miyu_tool::{Call, Done, Progress, Running, Spec, Tool, Venues};

use super::{Shared, content};
use crate::Core;

/// 替服务写的几句的模板。
#[derive(Clone)]
pub(super) struct Texts {
    /// 暂时不可用（`core/tool-results/unavailable.txt`），字段 `name`。
    pub(super) unavailable: String,
    /// 没在时限里答完（`core/tool-results/timed-out.txt`），字段 `name`、`seconds`。
    pub(super) timed_out: String,
    /// 内部错误（`core/tool-results/crashed.txt`），字段 `name`。
    pub(super) crashed: String,
    /// 不给她看的一样东西（`core/mcp/not-shown.txt`），字段 `mime`、`bytes`。
    pub(super) not_shown: String,
    /// 文字截掉了（`core/mcp/cut.txt`），字段 `cut`。
    pub(super) cut: String,
}

impl Texts {
    /// 从资源目录读；读不出来的（安装坏了）没有。
    pub(super) fn load(core: &Core) -> Option<Texts> {
        let results = core.resources.core_texts().ok()?.tool_results;
        Some(Texts {
            unavailable: results.unavailable,
            timed_out: core.resources.tool_timed_out().ok()?,
            crashed: results.crashed,
            not_shown: core.resources.mcp_not_shown().ok()?,
            cut: core.resources.mcp_cut().ok()?,
        })
    }
}

/// 一件 MCP 工具。
pub(super) struct McpTool {
    /// 目录里的规格：名字是 `mcp__<服务名>__<工具名>`。
    pub(super) spec: Spec,
    /// 服务给的名字，原样：发 `tools/call` 用它。
    pub(super) original: String,
    /// 给哪种会话。
    pub(super) venues: Venues,
    /// 等多久。
    pub(super) timeout: Duration,
    /// 那个服务：这时连着的客户端。
    pub(super) shared: Arc<Shared>,
    /// 替它写的几句。
    pub(super) texts: Texts,
}

impl McpTool {
    /// 暂时不可用：说法同执行器替目录里没有的工具写的那一句。
    fn unavailable(&self) -> Done {
        let name = self.spec.name.as_str();
        Done {
            human: Some(Said::new("core/tool-results/unavailable").with("name", name)),
            ..failed(render(&self.texts.unavailable, &[("name", name)]))
        }
    }

    /// 没在时限里答完：秒数往上取整。
    fn timed_out(&self) -> Done {
        let name = self.spec.name.as_str();
        let seconds = self.timeout.as_millis().div_ceil(1000).to_string();
        Done {
            human: Some(
                Said::new("core/tool-results/timed-out")
                    .with("name", name)
                    .with("seconds", &seconds),
            ),
            ..failed(render(
                &self.texts.timed_out,
                &[("name", name), ("seconds", &seconds)],
            ))
        }
    }

    /// 服务答得不对（写法不对、要补信息）：照工具出了内部错误说。
    fn crashed(&self) -> Done {
        let name = self.spec.name.as_str();
        Done {
            human: Some(Said::new("core/tool-results/crashed").with("name", name)),
            ..failed(render(&self.texts.crashed, &[("name", name)]))
        }
    }
}

impl Tool for McpTool {
    fn spec(&self) -> &Spec {
        &self.spec
    }

    fn venues(&self) -> Option<Venues> {
        Some(self.venues)
    }

    fn run(&self, call: Call, _progress: Progress) -> Running<'_> {
        Box::pin(async move {
            // 目录过期的叫看管的任务再列一次（施工 X-3 下，`ttlMs`）：这一次照旧调。
            self.shared.touch();
            let Some(client) = self.shared.client() else {
                return self.unavailable();
            };
            let arguments: Value = serde_json::from_str(&call.args).unwrap_or_else(|_| json!({}));
            // 到点、被掐掉，这个调用的 future 都被丢掉：客户端那一刻发 `notifications/cancelled`。
            let called =
                tokio::time::timeout(self.timeout, client.call(&self.original, arguments)).await;
            match called {
                Err(_) => self.timed_out(),
                Ok(Ok(called)) => content::done(called, &self.texts),
                Ok(Err(Failed::Closed)) => self.unavailable(),
                Ok(Err(Failed::Rpc { message, .. })) => failed(message),
                Ok(Err(error)) => {
                    tracing::warn!(target: super::TARGET, tool = self.spec.name.as_str(), error = %error, "mcp call failed");
                    self.crashed()
                }
            }
        })
    }
}

/// 照模板 `template` 换进字段 `fields`；模板坏了的交回空的。
pub(super) fn render(template: &str, fields: &[(&str, &str)]) -> String {
    let fields: BTreeMap<&str, &str> = fields.iter().copied().collect();
    Template::parse(template)
        .ok()
        .and_then(|template| template.render(&fields).ok())
        .unwrap_or_default()
}

/// 出错，交回一段字。
fn failed(text: String) -> Done {
    Done {
        error: true,
        blocks: vec![Block::Text(Text { text })],
        ..Done::ok("")
    }
}
