//! 标题那一句（`docs/blueprint/view.md`「标题那一句」，终端蓝图 `tui.md`「时间线」第 3、6–9 条）：显示名、对象、
//! 别的会话的短编号、结果那一句，照连接的语言。

use std::collections::BTreeMap;

use serde_json::Value;

use miyu_kernel::event::Said;
use miyu_kernel::id::SessionId;

use crate::entry::{Title, ToolState};
use crate::words::{self, Texts, ToolKind, keys};

/// 结果的 `human` 里「经谁做的」那一格（施工 N-2 再补，项目主人 2026-10-19 定：网络搜索的来源挑到名字后面）。
pub const VIA: &str = "via";

/// 一步工具的标题。`said` 是结果那一句的说法，结果还没到的没有；`named` 是结果里记下的给人看的名字（施工 9-8 六补），
/// 还没到的、没记的是空的。名字先照记下的挑连接的语言，再照核心的字、这一刻工具目录里的，都没有的照工具名。
pub(crate) fn of(
    name: &str,
    args: &Value,
    state: ToolState,
    said: Option<&Said>,
    named: &BTreeMap<String, String>,
    texts: &Texts,
) -> Title {
    let words = texts.local.as_ref();
    let face = words.face(name);
    let shown = words::pick(named, &texts.language)
        .or_else(|| face.as_ref().map(|f| f.name.clone()))
        .or_else(|| texts.tools.get(name).cloned())
        .unwrap_or_else(|| name.to_string());
    if state == ToolState::Preparing {
        let name = words::say(words, keys::PREPARE, &[("name", shown.clone())]).unwrap_or(shown);
        return Title {
            name,
            ..Title::default()
        };
    }
    let kind = texts.kinds.of(name);
    let arg = |key: &str| {
        args.get(key)
            .and_then(Value::as_str)
            .filter(|v| !v.is_empty())
    };
    let object = match kind {
        Some(ToolKind::Command | ToolKind::Agent) => arg("description").map(str::to_string),
        Some(ToolKind::Message) => arg("to").map(|to| recipient(to, texts)),
        // 对象跟哪个参数：核心的字里写的，没有的照这一刻工具目录里那件自己说的（施工 9-8 十一补：扩展、MCP、脚本的工具）。
        _ => face
            .as_ref()
            .and_then(|f| f.subject.as_deref())
            .or_else(|| texts.subjects.get(name).map(String::as_str))
            .and_then(arg)
            .map(|value| home_short(value, texts.home.as_deref())),
    };
    let session = texts
        .kinds
        .session_arg
        .as_deref()
        .and_then(arg)
        .map(|id| on_session(id, texts));
    let via = said
        .and_then(|s| s.fields.get(VIA))
        .filter(|via| !via.is_empty())
        .cloned();
    // 别的工具后面跟结果那一句；命令有预览、编辑有加减的行数、子代理有任务编号，不写；留言送到了的和对象重了，不写。
    // 记住（9-8 十补只为收起那一行单算）照别的工具接「记成 m14」。
    // 被拒的不分工具都写（施工 HK-1补）：守卫、权限策略、只读、人拒的为什么，不然人只能点开看给模型的那一句。
    let said = match kind {
        _ if state == ToolState::Denied => said.and_then(|s| words.say(s)),
        None | Some(ToolKind::Remember | ToolKind::Search | ToolKind::Fetch) => {
            said.and_then(|s| words.say(s))
        }
        Some(ToolKind::Message) if state.failed() => said.and_then(|s| words.say(s)),
        _ => None,
    };
    Title {
        name: shown,
        object,
        session,
        said,
        via,
    }
}

/// 供应商侧工具那一步的标题（施工 HT-3，`docs/blueprint/view.md`「标题那一句」）：`core/view.json` 的 `hosted` 表对得上的照
/// 核心那件工具的显示名、对象照表里说的参数；对不上的写原名、没有对象。`via` 是谁跑的（写这条回复的供应商，照
/// [`via`] 挑好的），准备中也带。没有结果那一句：供应商不报给人看的一句。
pub(crate) fn hosted(
    name: &str,
    args: &Value,
    state: ToolState,
    via: Option<String>,
    texts: &Texts,
) -> Title {
    let words = texts.local.as_ref();
    let mapped = texts.kinds.hosted_as(name);
    let shown = mapped.map_or_else(
        || name.to_string(),
        |mapped| {
            words
                .face(&mapped.core)
                .map(|face| face.name)
                .or_else(|| texts.tools.get(&mapped.core).cloned())
                .unwrap_or_else(|| mapped.core.clone())
        },
    );
    if state == ToolState::Preparing {
        let name = words::say(words, keys::PREPARE, &[("name", shown.clone())]).unwrap_or(shown);
        return Title {
            name,
            via,
            ..Title::default()
        };
    }
    let object = mapped
        .and_then(|mapped| mapped.subject.as_deref())
        .and_then(|key| args.get(key))
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .map(|value| home_short(value, texts.home.as_deref()));
    Title {
        name: shown,
        object,
        via,
        ..Title::default()
    }
}

/// 供应商 `provider` 给人看的名字：交进来的显示名，没有的照编号。
pub(crate) fn via(provider: &str, texts: &Texts) -> String {
    texts
        .providers
        .get(provider)
        .cloned()
        .unwrap_or_else(|| provider.to_string())
}

/// 留言发给谁：子代理写任务编号，父会话写「父会话」，别的会话写「会话 短编号」。
fn recipient(to: &str, texts: &Texts) -> String {
    if to == "parent" {
        return words::say(texts.local.as_ref(), keys::PARENT, &[])
            .unwrap_or_else(|| to.to_string());
    }
    match SessionId::parse(to) {
        Ok(_) => on_session(to, texts),
        Err(_) => to.to_string(),
    }
}

/// 「会话 短编号」：短编号是整个编号的最后 8 个字。
fn on_session(id: &str, texts: &Texts) -> String {
    let short = short(id);
    words::say(
        texts.local.as_ref(),
        keys::ON_SESSION,
        &[("id", short.clone())],
    )
    .unwrap_or(short)
}

/// 短编号：整个编号的最后 8 个字（核心 C-1 定的）。
pub(crate) fn short(id: &str) -> String {
    let skip = id.chars().count().saturating_sub(8);
    id.chars().skip(skip).collect()
}

/// 在家目录底下的路径写成 `~/…`。
fn home_short(path: &str, home: Option<&str>) -> String {
    let Some(home) = home
        .map(|h| h.trim_end_matches('/'))
        .filter(|h| !h.is_empty())
    else {
        return path.to_string();
    };
    match path.strip_prefix(home) {
        Some("") => "~".to_string(),
        Some(rest) if rest.starts_with('/') => format!("~{rest}"),
        _ => path.to_string(),
    }
}
