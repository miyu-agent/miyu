//! `session.context`（施工 CX-1 上）：这个会话这时的上下文一块块的，给头画「上下文明细」。
//!
//! - 问那个会话（`Handle::context`）：内核照有效历史组装一次，各块用和压缩线同一个本地估算器估；`used`、`anchor` 同
//!   终端底栏的口径（锚是最近一次主请求供应商报的，`basis` 说是不是照它起算的）。只在被问时算。
//! - 块的编号（`part`）稳定，给头认；`label` 照连接的语言（`core/human/<语言>.json` 的 `context/part/<编号>`），包、MCP
//!   服务的照原名。是 0 的块不交。
//! - system 拆成快照里单存原文的几块（技能、知识库、关于你、项目说明、扩展追加的）和其余的 `system.other`；工具照来处拆，
//!   只露名字的归 `tools.deferred`；对话照谁放进去的拆，记忆的常驻、召回和知识库想起的单列。
//! - 新会话预览、system 拆全在 CX-1 下：现在只收 `session`。

use std::sync::Arc;

use serde::Deserialize;
use serde_json::{Value, json};

use miyu_config::Words;
use miyu_kernel::id::SessionId;
use miyu_kernel::session::Said;
use miyu_session::{Context, SystemPart};

use crate::Core;
use crate::config::methods::words;
use crate::hello::Peer;
use crate::refusal::Refusal;
use crate::session_tools::{package_of, source};

/// `session.context` 的参数。
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Params {
    session: String,
}

/// `session.context`：`{"window"?, "line"?, "used"?, "basis", "anchor"?, "estimated_total", "parts": [...]}`。
pub(crate) async fn context(
    core: &Arc<Core>,
    peer: Peer,
    params: Params,
) -> Result<Value, Refusal> {
    let id = SessionId::parse(&params.session).map_err(|_| Refusal::BAD_PARAMS)?;
    let found = core.sessions.get(core, &id).await?;
    let context = found
        .handle
        .context()
        .await
        .map_err(|_| Refusal::INTERNAL)?;
    let language = peer.language.to_string();
    let human = words(core, &language)?;
    let word = |part: &str| {
        Words::sentence(&human, &format!("context/part/{part}"), &[])
            .unwrap_or_else(|| part.to_string())
    };
    let tool = |name: &str| {
        let (source, _) = source(core, &language, name);
        let text = |key: &str| source[key].as_str().unwrap_or_default().to_string();
        match text("kind").as_str() {
            "mcp" => (format!("tools.mcp:{}", text("id")), Some(text("name"))),
            "package" => (format!("tools.package:{}", text("id")), Some(text("name"))),
            "script" => ("tools.scripts".to_string(), None),
            _ => ("tools.builtin".to_string(), None),
        }
    };
    let appended = |package: &str| package_of(core, &language, package).1;
    Ok(shape(
        &context,
        &Namer {
            word: &word,
            tool: &tool,
            appended: &appended,
        },
    ))
}

/// 起名字的几样：编号的那句话、一件工具归哪块（编号、原名）、扩展包的名字。
pub(crate) struct Namer<'a> {
    pub(crate) word: &'a dyn Fn(&str) -> String,
    pub(crate) tool: &'a dyn Fn(&str) -> (String, Option<String>),
    pub(crate) appended: &'a dyn Fn(&str) -> String,
}

/// 一块：编号、原名（有的照它，没有的照编号查那句话）、token 数、下面的几块。
struct Part {
    id: String,
    name: Option<String>,
    tokens: u64,
    children: Vec<Part>,
}

impl Part {
    fn leaf(tokens: u64, id: impl Into<String>, name: Option<String>) -> Part {
        Part {
            id: id.into(),
            name,
            tokens,
            children: Vec::new(),
        }
    }

    /// 几块并成一块：是 0 的去掉，加起来。
    fn of(children: Vec<Part>, id: &str) -> Part {
        let children: Vec<Part> = children
            .into_iter()
            .filter(|child| child.tokens > 0)
            .collect();
        Part {
            id: id.to_string(),
            name: None,
            tokens: children.iter().map(|child| child.tokens).sum(),
            children,
        }
    }

    fn json(&self, namer: &Namer) -> Value {
        let label = self.name.clone().unwrap_or_else(|| (namer.word)(&self.id));
        let mut part = json!({"part": self.id, "label": label, "tokens": self.tokens});
        if !self.children.is_empty() {
            part["children"] = self
                .children
                .iter()
                .map(|child| child.json(namer))
                .collect();
        }
        part
    }
}

/// 加进同编号的那块，没有的照先后接在后面。
fn add(parts: &mut Vec<Part>, id: String, name: Option<String>, tokens: u64) {
    match parts.iter_mut().find(|part| part.id == id) {
        Some(part) => part.tokens = part.tokens.saturating_add(tokens),
        None => parts.push(Part::leaf(tokens, id, name)),
    }
}

/// 协议上的样子。
pub(crate) fn shape(context: &Context, namer: &Namer) -> Value {
    let breakdown = &context.breakdown;
    let system = context
        .system
        .iter()
        .map(|(part, tokens)| match part {
            SystemPart::Skills => Part::leaf(*tokens, "system.skills", None),
            SystemPart::Knowledge => Part::leaf(*tokens, "system.knowledge", None),
            SystemPart::Profile => Part::leaf(*tokens, "system.profile", None),
            SystemPart::Project => Part::leaf(*tokens, "system.project", None),
            SystemPart::Appended(package) => Part::leaf(
                *tokens,
                format!("system.appended:{package}"),
                Some((namer.appended)(package)),
            ),
            SystemPart::Other => Part::leaf(*tokens, "system.other", None),
        })
        .collect();
    let mut tools = Vec::new();
    for (name, tokens) in &breakdown.tools {
        let deferred = context
            .offered
            .iter()
            .any(|offered| offered.name == *name && !offered.loaded);
        let (id, shown) = match deferred {
            true => ("tools.deferred".to_string(), None),
            false => (namer.tool)(name),
        };
        add(&mut tools, id, shown, *tokens);
    }
    let mut messages = Vec::new();
    let mut memory = Vec::new();
    for (said, tokens) in &breakdown.said {
        let id = match said {
            Said::User => "messages.user",
            Said::Assistant => "messages.assistant",
            Said::ToolResult => "messages.tool_results",
            Said::Fact { module, kind } => {
                let module = module.as_ref().map(|module| module.as_str());
                match (module, kind.as_str()) {
                    (Some("memory"), "memory") => {
                        add(
                            &mut memory,
                            "messages.memory.resident".into(),
                            None,
                            *tokens,
                        );
                        continue;
                    }
                    (Some("memory"), "recall") => {
                        add(&mut memory, "messages.memory.recall".into(), None, *tokens);
                        continue;
                    }
                    (Some("knowledge"), "recall") => "messages.knowledge",
                    _ => "messages.facts",
                }
            }
        };
        add(&mut messages, id.to_string(), None, *tokens);
    }
    // 记忆排在人说的、她说的后面，事实前面。
    let at = messages
        .iter()
        .position(|part| !["messages.user", "messages.assistant"].contains(&part.id.as_str()))
        .unwrap_or(messages.len());
    messages.insert(at, Part::of(memory, "messages.memory"));
    let parts = [
        Part::of(system, "system"),
        Part::of(tools, "tools"),
        Part::leaf(breakdown.demos, "demos", None),
        Part::leaf(breakdown.summary, "summary", None),
        Part::of(messages, "messages"),
    ];
    let parts: Vec<Value> = parts
        .iter()
        .filter(|part| part.tokens > 0)
        .map(|part| part.json(namer))
        .collect();
    let mut answer = json!({
        "basis": if breakdown.anchor.is_some() { "measured" } else { "estimated" },
        "estimated_total": breakdown.estimated_total(),
        "parts": parts,
    });
    let fields = [
        ("window", breakdown.limits.window),
        ("line", breakdown.limits.compaction_line),
        ("used", breakdown.used),
        ("anchor", breakdown.anchor),
    ];
    for (name, value) in fields {
        if let Some(value) = value {
            answer[name] = json!(value);
        }
    }
    answer
}

#[cfg(test)]
mod tests;
