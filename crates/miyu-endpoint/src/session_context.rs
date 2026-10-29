//! `session.context`（施工 CX-1 上）：这个会话这时的上下文一块块的，给头画「上下文明细」。
//!
//! - 问那个会话（`Handle::context`）：内核照有效历史组装一次，各块用和压缩线同一个本地估算器估；`used`、`anchor` 同
//!   终端底栏的口径（锚是最近一次主请求供应商报的，`basis` 说是不是照它起算的）。只在被问时算。
//! - 块的编号（`part`）稳定，给头认；`label` 照连接的语言（`core/human/<语言>.json` 的 `context/part/<编号>`），包、MCP
//!   服务的照原名。是 0 的块不交。
//! - system 拆成快照里单存原文的几块（技能、知识库、关于你、项目说明、扩展追加的）和其余的 `system.other`；工具照来处拆，
//!   只露名字的归 `tools.deferred`；对话照谁放进去的拆，记忆的常驻、召回和知识库想起的单列。
//! - 新会话预览、system 拆全在 CX-1 下：现在只收 `session`。

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::sync::Arc;

use serde::Deserialize;
use serde_json::{Value, json};

use miyu_config::Words;
use miyu_kernel::id::SessionId;
use miyu_kernel::session::Said;
use miyu_session::{Context, Spent, SystemPart};

use crate::Core;
use crate::config::methods::words;
use crate::hello::Peer;
use crate::refusal::Refusal;
use crate::session_tools::{new_session, package_of, source};

/// `session.context` 的参数：同 `session.tools`。
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Params {
    #[serde(default)]
    session: Option<String>,
    /// 同 `session.create`：不写的照 `persona.default`，写 `null` 的无人格。
    #[serde(default, deserialize_with = "crate::personas::written")]
    persona: Option<Option<String>>,
    /// 同 `session.create`：不写的照 `preset.default`，写 `null` 的是无工具（施工 P-16 下；CX-2 补：原来当成没写）。
    #[serde(default, deserialize_with = "crate::personas::written")]
    preset: Option<Option<String>>,
}

/// `session.context`：`{"window"?, "line"?, "used"?, "basis", "anchor"?, "estimated_total", "parts": [...]}`。
pub(crate) async fn context(
    core: &Arc<Core>,
    peer: Peer,
    params: Params,
) -> Result<Value, Refusal> {
    let context = match params.session {
        Some(_) if params.persona.is_some() || params.preset.is_some() => {
            return Err(Refusal::BAD_PARAMS);
        }
        Some(text) => {
            let id = SessionId::parse(&text).map_err(|_| Refusal::BAD_PARAMS)?;
            let found = core.sessions.get(core, &id).await?;
            found
                .handle
                .context()
                .await
                .map_err(|_| Refusal::INTERNAL)?
        }
        None => {
            let preset = crate::presets::opening(params.preset)?;
            let mut setup = new_session(core, peer, params.persona, preset).await?;
            miyu_session::preview_context(setup.create(core))
                .await
                .map_err(|_| Refusal::INTERNAL)?
        }
    };
    named(core, peer, |namer| shape(&context, namer))
}

/// `session.usage_parts` 的参数（施工 CX-3）。
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct SpentParams {
    session: String,
    /// 连它派的子会话一起算，同 `usage.query` 的 `tree`。
    #[serde(default)]
    tree: bool,
}

/// `session.usage_parts`（施工 CX-3）：`{"requests", "input", "output", "parts": [...]}`，见 [`spent_shape`]。
pub(crate) async fn usage_parts(
    core: &Arc<Core>,
    peer: Peer,
    params: SpentParams,
) -> Result<Value, Refusal> {
    let id = SessionId::parse(&params.session).map_err(|_| Refusal::BAD_PARAMS)?;
    // 没有这个会话的照 `session_not_found`，再找它派的子会话。
    let found = core.sessions.get(core, &id).await?;
    let mut spent = vec![
        found
            .handle
            .usage_parts()
            .await
            .map_err(|_| Refusal::INTERNAL)?,
    ];
    if params.tree {
        for child in crate::usage::tree(core, &id).await {
            if child == id {
                continue;
            }
            // 读不出来的子会话（删了、坏了）不算，总数照读得出的。
            if let Ok(found) = core.sessions.get(core, &child).await
                && let Ok(one) = found.handle.usage_parts().await
            {
                spent.push(one);
            }
        }
    }
    named(core, peer, |namer| spent_shape(&spent, namer))
}

/// 照连接的语言备好起名字的几样，交给 `f`：块的那句话、工具归哪块（问过的记下）、扩展包的名字。
/// 工具归哪块照看得到的包认（施工 U23 中下）。
fn named(core: &Arc<Core>, peer: Peer, f: impl FnOnce(&Namer) -> Value) -> Result<Value, Refusal> {
    let language = peer.language.to_string();
    let human = words(core, &language)?;
    let word = |part: &str| {
        Words::sentence(&human, &format!("context/part/{part}"), &[])
            .unwrap_or_else(|| part.to_string())
    };
    let known: RefCell<BTreeMap<String, (String, Option<String>)>> = RefCell::default();
    let tool = |name: &str| {
        if let Some(found) = known.borrow().get(name) {
            return found.clone();
        }
        let (source, _) = source(core, &language, name);
        let text = |key: &str| source[key].as_str().unwrap_or_default().to_string();
        let found = match text("kind").as_str() {
            "mcp" => (format!("tools.mcp:{}", text("id")), Some(text("name"))),
            "package" => (format!("tools.package:{}", text("id")), Some(text("name"))),
            "script" => ("tools.scripts".to_string(), None),
            _ => ("tools.builtin".to_string(), None),
        };
        known.borrow_mut().insert(name.to_string(), found.clone());
        found
    };
    let appended = |package: &str| package_of(core, &language, package).1;
    Ok(f(&Namer {
        word: &word,
        tool: &tool,
        appended: &appended,
    }))
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
        // 摊完成了 0 的不交。
        let children: Vec<Value> = self
            .children
            .iter()
            .filter(|child| child.tokens > 0)
            .map(|child| child.json(namer))
            .collect();
        if !children.is_empty() {
            part["children"] = Value::Array(children);
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

/// 照 `used` 摊（施工 CX-1 下，主会话定：一份逻辑，头不各写一遍）：叶子照 `used / 各块的和` 摊，向下取整以后差的那几个
/// 照小数部分从大到小一个个补（一样大的照先后），加起来正好是 `used`；上一层再照下面的加起来。
fn scale(parts: &mut [Part], used: u64) {
    fn leaves<'a>(parts: &'a mut [Part], into: &mut Vec<&'a mut Part>) {
        for part in parts {
            if part.children.is_empty() {
                into.push(part);
            } else {
                leaves(&mut part.children, into);
            }
        }
    }
    fn total(part: &mut Part) -> u64 {
        if !part.children.is_empty() {
            part.tokens = part.children.iter_mut().map(total).sum();
        }
        part.tokens
    }
    let mut all = Vec::new();
    leaves(parts, &mut all);
    let sum: u128 = all.iter().map(|part| u128::from(part.tokens)).sum();
    if sum == 0 {
        return;
    }
    let used = u128::from(used);
    let mut rests = Vec::with_capacity(all.len());
    let mut given: u128 = 0;
    for (at, part) in all.iter_mut().enumerate() {
        let exact = u128::from(part.tokens) * used;
        let floor = exact / sum;
        given += floor;
        rests.push((exact % sum, at));
        part.tokens = u64::try_from(floor).unwrap_or(u64::MAX);
    }
    rests.sort_by(|one, other| other.0.cmp(&one.0).then(one.1.cmp(&other.1)));
    let short = usize::try_from(used.saturating_sub(given)).unwrap_or(usize::MAX);
    for (_, at) in rests.into_iter().take(short) {
        all[at].tokens = all[at].tokens.saturating_add(1);
    }
    for part in parts {
        total(part);
    }
}

/// 一块块的，没摊（施工 CX-3 从 [`shape`] 分出来，累计用量也照它）。
fn tree(context: &Context, namer: &Namer) -> Vec<Part> {
    let breakdown = &context.breakdown;
    let system = context
        .system
        .iter()
        .map(|(part, tokens)| match part {
            SystemPart::Persona => Part::leaf(*tokens, "system.persona", None),
            SystemPart::Venue => Part::leaf(*tokens, "system.venue", None),
            SystemPart::Group => Part::leaf(*tokens, "system.group", None),
            SystemPart::Core => Part::leaf(*tokens, "system.core", None),
            SystemPart::Clicall => Part::leaf(*tokens, "system.clicall", None),
            SystemPart::Show => Part::leaf(*tokens, "system.show", None),
            SystemPart::StyleLock => Part::leaf(*tokens, "system.style_lock", None),
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
        let (id, shown) = (namer.tool)(name);
        add(&mut tools, id, shown, *tokens);
    }
    let mut messages = Vec::new();
    let mut memory = Vec::new();
    for (said, tokens) in &breakdown.said {
        let id = match said {
            Said::User => "messages.user",
            Said::Assistant => "messages.assistant",
            Said::ToolResult => "messages.tool_results",
            Said::Report => "messages.reports",
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
                    // 仓库里的 AGENTS.md（施工 P-14）：一份能有 32 KiB，单列。
                    (Some("project-docs"), _) => "messages.project_docs",
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
    vec![
        Part::of(system, "system"),
        Part::of(tools, "tools"),
        Part::leaf(breakdown.demos, "demos", None),
        Part::leaf(breakdown.summary, "summary", None),
        Part::of(messages, "messages"),
    ]
}

/// 协议上的样子。
pub(crate) fn shape(context: &Context, namer: &Namer) -> Value {
    let breakdown = &context.breakdown;
    let mut parts = tree(context, namer);
    if let Some(used) = breakdown.used {
        scale(&mut parts, used);
    }
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

/// 累计用量照上下文的块摊（施工 CX-3，`session.usage_parts`）：每次主请求的块照它报的输入摊（叶子照比例、余数照小数
/// 部分补，同 [`shape`]），照编号加起来；输出单成最后一块 `output`。`{"requests", "input", "output", "parts"}`，块的写法同
/// `session.context`，加起来是 `input + output`。
pub(crate) fn spent_shape(spent: &[Spent], namer: &Namer) -> Value {
    let mut total: Vec<Part> = Vec::new();
    let (mut requests, mut input, mut output) = (0u64, 0u64, 0u64);
    for one in spent {
        requests = requests.saturating_add(one.requests);
        output = output.saturating_add(one.output);
        for charged in &one.charged {
            input = input.saturating_add(charged.input);
            let mut parts = tree(&charged.context, namer);
            scale(&mut parts, charged.input);
            merge(&mut total, parts);
        }
    }
    total.push(Part::leaf(output, "output", None));
    let parts: Vec<Value> = total
        .iter()
        .filter(|part| part.tokens > 0)
        .map(|part| part.json(namer))
        .collect();
    json!({"requests": requests, "input": input, "output": output, "parts": parts})
}

/// 照编号加进 `into`：有的加上、下面的接着并；没有的照先后接在后面。
fn merge(into: &mut Vec<Part>, parts: Vec<Part>) {
    for part in parts {
        match into.iter_mut().find(|known| known.id == part.id) {
            Some(known) => {
                known.tokens = known.tokens.saturating_add(part.tokens);
                merge(&mut known.children, part.children);
            }
            None => into.push(part),
        }
    }
}

#[cfg(test)]
mod tests;
