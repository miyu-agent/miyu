//! 驱动要人确认的参数怎么读、怎么答（施工 ACP-8，设计 73 第四节第 7 条，蓝图 `session/driver-ask.md`）：`driver.approve` 的参数读成
//! 内核要的那几格和给头看的 `detail`，内核的结论写回给驱动的 `result`。纯函数，执行器（`route/extension/approve.rs`）、actor
//! （`actor/driver_asks.rs`）都照它。
//!
//! - `detail`：带 `command` 的照跑命令的确认写（`{driver, tool, title, command}`），别的照包的守卫问人写（`{driver, guard, tool,
//!   reason}`）：头照这两种已有的样子画，不用改。标题截到 200 个字、说明和命令截到 4000 个字，截了的末尾接「…」。
//! - 回的 `option`：允许照对方给的「允许这一次」，没有的照「总是允许」；拒绝照「拒绝这一次」，没有的照「总是拒绝」；对方没给
//!   对得上的不写。Miyu 只有「允许这一次」「拒绝」两种，不替对方记住。

use serde_json::{Map, Value, json};

use miyu_kernel::raw::RawJson;
use miyu_kernel::session::{AutoDeny, DriverAnswer};
use miyu_kernel::tool::Access;

/// 标题最多几个字。
const TITLE_CHARS: usize = 200;
/// 说明、命令最多几个字。
const TEXT_CHARS: usize = 4000;
/// 对方最多给几个选项。
const OPTIONS: usize = 16;

/// 对方给的一个选项：编号、种类（ACP 的 `PermissionOptionKind`）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Choice {
    id: String,
    kind: String,
}

/// 读好的一问。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Asked {
    /// 画在第几块供应商侧工具上。
    pub(crate) index: u32,
    /// 要的是哪一类访问。
    pub(crate) access: Access,
    /// 给头看的。
    pub(crate) detail: RawJson,
    /// 对方给的选项。
    pub(crate) choices: Vec<Choice>,
}

/// 读 `driver.approve` 的参数：驱动是 `driver`（`<包>/<驱动名>`），包是 `package`。写得不对的交回一句英文的原因。
pub(crate) fn read(params: &Value, package: &str, driver: &str) -> Result<Asked, String> {
    let index = params
        .get("index")
        .and_then(Value::as_u64)
        .and_then(|index| u32::try_from(index).ok())
        .ok_or("index must be the number of a provider-side tool block of this call")?;
    let access = match params.get("access").and_then(Value::as_str) {
        Some("read") => Access::Read,
        Some("write") => Access::Write,
        Some("execute") => Access::Execute,
        Some("network") => Access::Network,
        _ => return Err("access must be read, write, execute or network".to_string()),
    };
    let title = text(params, "title", TITLE_CHARS)?.ok_or("title must be a non-empty string")?;
    let choices = choices(params.get("options"))?;
    let mut detail = Map::new();
    detail.insert("driver".to_string(), json!(driver));
    detail.insert("tool".to_string(), json!(title));
    match text(params, "command", TEXT_CHARS)? {
        Some(command) => {
            detail.insert("title".to_string(), json!(title));
            detail.insert("command".to_string(), json!(command));
        }
        None => {
            detail.insert("guard".to_string(), json!(package));
            if let Some(said) = text(params, "text", TEXT_CHARS)? {
                detail.insert("reason".to_string(), json!(said));
            }
        }
    }
    let detail = RawJson::of(&Value::Object(detail)).ok_or("detail is not JSON")?;
    Ok(Asked {
        index,
        access,
        detail,
        choices,
    })
}

/// 内核的结论写成回驱动的 `result`；那一块不是供应商侧工具的交回一句英文的原因（端口回 `bad_params`）。
pub(crate) fn answered(answer: &DriverAnswer, choices: &[Choice]) -> Result<Value, String> {
    let (decision, option) = match answer {
        DriverAnswer::Allowed => ("once", picked(choices, &["allow_once", "allow_always"])),
        DriverAnswer::Denied { .. } | DriverAnswer::Auto(_) => {
            ("deny", picked(choices, &["reject_once", "reject_always"]))
        }
        DriverAnswer::Cancelled => return Ok(json!({"decision": "cancelled"})),
        DriverAnswer::NotHosted => {
            return Err(
                "index is not a provider-side tool block this call has started".to_string(),
            );
        }
    };
    let mut result = json!({"decision": decision});
    if let Some(option) = option {
        result["option"] = json!(option);
    }
    match answer {
        DriverAnswer::Denied {
            reason: Some(reason),
        } => result["reason"] = json!(reason),
        DriverAnswer::Auto(auto) => result["auto"] = json!(auto.as_str()),
        _ => {}
    }
    Ok(result)
}

/// 没问会话就拒的（辅助请求里问的）：照 [`answered`] 写，参数写得不对的照样交回原因。
pub(crate) fn refused(params: &Value) -> Result<Value, String> {
    let choices = choices(params.get("options"))?;
    answered(&DriverAnswer::Auto(AutoDeny::Aside), &choices)
}

/// 照 `kinds` 的先后挑对方给的第一个对得上的选项。
fn picked<'a>(choices: &'a [Choice], kinds: &[&str]) -> Option<&'a str> {
    kinds.iter().find_map(|kind| {
        choices
            .iter()
            .find(|choice| choice.kind == *kind)
            .map(|choice| choice.id.as_str())
    })
}

/// 对方给的选项：一个数组，每项有 `id`（不空的字）、`kind`（四种之一），至多 [`OPTIONS`] 项。
fn choices(options: Option<&Value>) -> Result<Vec<Choice>, String> {
    let bad = || {
        "options must be 1 to 16 objects with an id and a kind of allow_once, allow_always, \
         reject_once or reject_always"
            .to_string()
    };
    let options = options.and_then(Value::as_array).ok_or_else(bad)?;
    if options.is_empty() || options.len() > OPTIONS {
        return Err(bad());
    }
    options
        .iter()
        .map(|option| {
            let id = option
                .get("id")
                .and_then(Value::as_str)
                .filter(|id| !id.is_empty());
            let kind = option.get("kind").and_then(Value::as_str).filter(|kind| {
                matches!(
                    *kind,
                    "allow_once" | "allow_always" | "reject_once" | "reject_always"
                )
            });
            match (id, kind) {
                (Some(id), Some(kind)) => Ok(Choice {
                    id: id.to_string(),
                    kind: kind.to_string(),
                }),
                _ => Err(bad()),
            }
        })
        .collect()
}

/// 参数里的一段字 `key`：没写的、空白的是 `None`，不是字的说不对；超过 `cap` 个字的截了接「…」。
fn text(params: &Value, key: &str, cap: usize) -> Result<Option<String>, String> {
    match params.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(text)) if text.trim().is_empty() => Ok(None),
        Some(Value::String(text)) if text.chars().count() > cap => {
            let mut cut: String = text.chars().take(cap).collect();
            cut.push('…');
            Ok(Some(cut))
        }
        Some(Value::String(text)) => Ok(Some(text.clone())),
        Some(_) => Err(format!("{key} must be a string")),
    }
}

#[cfg(test)]
mod tests;
