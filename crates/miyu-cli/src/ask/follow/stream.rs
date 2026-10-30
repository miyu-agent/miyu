//! 事件流（施工 AP-3，设计 50 第四节，`docs/blueprint/cli/ask.md`「样子：`--format jsonl`」）：`--format jsonl` 的，标准输出
//! 一行一个事件，边跑边出；标准错误照旧只印出错。形状是命令行自己定的一份，从核心的事件裁出来，不原样转发：核心的事件以后
//! 加格、改说法，这一份照旧。每一行是一个 JSON 对象，`type` 说是什么，格照名字的字母先后排（`serde_json` 的 `Map` 是
//! `BTreeMap`：工作区没开 `preserve_order`）。
//!
//! 先后：`session` 一次 → 每一轮 `turn.started`、`thinking`、`text`、`step`、`step.done`、`compaction`、`turn.ended` → 等子代理
//! 时的 `agent.reported` → `done` 一次。`warning` 照告警出来的地方。开好会话以前就出错的（握手、传附件、造会话被拒……），
//! 标准输出上什么都没有，原因在标准错误上。

use std::io::Write;

use serde_json::{Value, json};

use super::{Follow, Output, Screen, steps};
use crate::ask::exit;
use crate::diagnostic::Problem;
use crate::shown::write;

/// 往 `out` 上写一行事件：`type` 是 `kind`，别的格照 `fields`（一个对象）。
pub(crate) fn emit(out: &mut dyn Write, kind: &str, mut fields: Value) {
    fields["type"] = json!(kind);
    write(out, &format!("{fields}\n"));
}

impl Follow<'_> {
    /// 写事件流的：`--format jsonl`。
    pub(super) fn streams(&self) -> bool {
        self.plan.format == Output::Jsonl
    }

    /// 写事件流的，往标准输出上写一行事件。
    fn stream(&self, screen: &mut Screen<'_>, kind: &str, fields: Value) {
        if self.streams() {
            emit(screen.out, kind, fields);
        }
    }

    /// 开始跟第 `turn` 轮。
    pub(super) fn stream_started(&self, turn: Option<u64>, screen: &mut Screen<'_>) {
        self.stream(screen, "turn.started", json!({ "turn": turn }));
    }

    /// 一段增量：思考的是 `thinking`，回答的是 `text`，别的（工具调用的参数）不写。
    pub(super) fn stream_delta(&self, kind: Option<&str>, text: &str, screen: &mut Screen<'_>) {
        let kind = match kind {
            Some("reasoning") => "thinking",
            Some("text") => "text",
            _ => return,
        };
        self.stream(screen, kind, json!({ "text": text }));
    }

    /// 一次回复（`message.assistant` 的 `body`，里面的调用已经记进了 `steps`）：每一次工具调用一步开始。
    pub(super) fn stream_steps(&self, body: &Value, screen: &mut Screen<'_>) {
        if !self.streams() {
            return;
        }
        for block in body["blocks"].as_array().into_iter().flatten() {
            let Some(id) = block["call_id"]
                .as_str()
                .filter(|_| block["type"] == "tool_call")
            else {
                continue;
            };
            let Some((tool, title)) = self.steps.opened(id, self.plan, &self.cwd) else {
                continue;
            };
            let fields = json!({"id": id, "title": title, "tool": tool});
            self.stream(screen, "step", fields);
        }
    }

    /// 一次结果（`tool.result` 的 `body`）：一步结束。`ok` 是状态是不是 `ok`；结果那一句换得成字的写 `summary`。
    pub(super) fn stream_result(&self, body: &Value, screen: &mut Screen<'_>) {
        if !self.streams() {
            return;
        }
        let mut fields = json!({
            "id": body["call_id"].as_str().unwrap_or_default(),
            "ok": body["status"] == "ok",
        });
        if let Some(summary) = steps::summary(body, self.plan) {
            fields["summary"] = json!(summary);
        }
        self.stream(screen, "step.done", fields);
    }

    /// 等的子代理 `agent` 报回来了，`body` 是 `child.reported` 的：它最后说的。
    pub(super) fn stream_reported(&self, agent: &str, body: &Value, screen: &mut Screen<'_>) {
        let text = body["text"].as_str().unwrap_or_default();
        self.stream(
            screen,
            "agent.reported",
            json!({"agent": agent, "text": text}),
        );
    }

    /// 压缩了：压之前、压完的 token 数（估算）。
    pub(super) fn stream_compacted(&self, before: u64, after: u64, screen: &mut Screen<'_>) {
        self.stream(
            screen,
            "compaction",
            json!({"after": after, "before": before}),
        );
    }

    /// 一句告警：`message` 是告警那一句（不带 `miyu: 警告：`），有建议的另带 `hints`。
    pub(super) fn stream_warning(&self, problem: &Problem, screen: &mut Screen<'_>) {
        let mut fields = json!({ "message": problem.what() });
        if !problem.hints().is_empty() {
            fields["hints"] = json!(problem.hints());
        }
        self.stream(screen, "warning", fields);
    }

    /// 第 `turn` 轮结束了，原因是 `reason`：这一轮的全部回答、用量，最后一次请求出错的带 `error`。
    pub(super) fn stream_ended(&self, turn: Option<u64>, reason: &str, screen: &mut Screen<'_>) {
        let mut fields = json!({
            "reason": reason,
            "text": self.answer,
            "turn": turn,
            "usage": self.usage.json(),
        });
        if let Some(failure) = &self.failure {
            fields["error"] = json!({"class": failure.class, "message": failure.message});
        }
        self.stream(screen, "turn.ended", fields);
    }

    /// 都结束了，退出码 `code`：最后一行。不是 0 的带 `reason`：为什么（[`Follow::why`]）；没记下的，按了两次 Ctrl+C 的是
    /// `interrupted`，别的是核心断开了（`disconnected`）。
    pub(crate) fn stream_done(&self, code: u8, screen: &mut Screen<'_>) {
        let mut fields = json!({"exit": code, "session": self.session});
        if code != exit::OK {
            let why = match (&self.why, code) {
                (Some(why), _) => why.as_str(),
                (None, exit::INTERRUPTED) => "interrupted",
                (None, _) => "disconnected",
            };
            fields["reason"] = json!(why);
        }
        self.stream(screen, "done", fields);
    }
}
