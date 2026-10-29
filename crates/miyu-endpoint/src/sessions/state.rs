//! 包的会话状态（施工 TW-1，设计 `46-包的会话状态.md` 第三节）：工具结果带的、程序直接写的照同一个查法收；
//! `session.set_state` 程序直接写，`session.state` 读回自己的。
//!
//! - 一份是 `{"widgets"?, "note"?, "data"?}`，`null`、`{}` 是清掉。
//! - `widgets` 的键是这个包声明的 `state = true` 的块，值是 `{"content", "title"?}`：内容照那一块的位置查（`miyu_config::widget`）。
//! - `note` 照给模型看的字的规矩查（最多 [`NOTE_MOST`] 个字）；`data` 任意 JSON，写出来最多 [`DATA_MOST`] 字节。
//! - 写坏了整份不收。

use serde::Deserialize;
use serde_json::{Map, Value, json};

use miyu_config::package::{Manifest, WidgetPlace};
use miyu_kernel::event::{PackageState, State};
use miyu_kernel::id::{CommandId, SessionId};
use miyu_kernel::raw::RawJson;
use miyu_kernel::session::{Command, Outcome};

use crate::Core;
use crate::hello::Caller;
use crate::refusal::Refusal;

/// `note` 最多几个字。
const NOTE_MOST: usize = 2000;

/// `data` 写出来最多几个字节。
const DATA_MOST: usize = 16 * 1024;

/// 写坏了的是哪一种。
#[derive(Debug)]
pub(crate) enum Bad {
    /// 界面块的内容、标题不对：路径从 `widgets.<块>` 写起。
    Widget(miyu_config::widget::Bad),
    /// 别的格不对：哪一格。
    Field(String),
}

impl Bad {
    /// 记运行日志用的一个词。
    pub(crate) fn why(&self) -> String {
        match self {
            Bad::Widget(bad) => format!("{} {}", bad.path, bad.problem.as_str()),
            Bad::Field(field) => field.clone(),
        }
    }

    /// 交给程序的拒绝：内容不对的 `bad_widget_content`，别的 `bad_params` 带 `field`。
    fn refusal(&self) -> Refusal {
        match self {
            Bad::Widget(bad) => Refusal::bad_widget_content(bad),
            Bad::Field(field) => Refusal::bad_field(field),
        }
    }
}

/// 一个包声明的、查状态要用的：`state = true` 的块和它们的位置，整页的编号（`open` 动作照它查）。
#[derive(Debug, Clone, Default)]
pub(crate) struct Declared {
    widgets: Vec<(String, WidgetPlace)>,
    pages: Vec<String>,
}

impl Declared {
    /// 照装着的包 `package` 的清单取；没装、清单读不成的什么都没有。
    pub(crate) fn of(core: &Core, package: &str) -> Declared {
        let packages = core.system_packages();
        let manifest: Option<&Manifest> = packages
            .iter()
            .find(|found| found.id == package)
            .and_then(|found| found.read.as_ref().ok());
        let declared = manifest
            .map(|manifest| manifest.widgets.as_slice())
            .unwrap_or_default();
        Declared {
            widgets: declared
                .iter()
                .filter(|widget| widget.state)
                .map(|widget| (widget.id.clone(), widget.place))
                .collect(),
            pages: declared
                .iter()
                .filter(|widget| widget.place == WidgetPlace::Page)
                .map(|widget| widget.id.clone())
                .collect(),
        }
    }
}

/// 照包声明的 `declared` 查一份 `value`，读成内核的那一份。
pub(crate) fn read(declared: &Declared, value: &Value) -> Result<State, Bad> {
    let field = |name: &str| Bad::Field(format!("state.{name}"));
    let object = match value {
        Value::Null => return Ok(State::default()),
        Value::Object(object) => object,
        _ => return Err(Bad::Field("state".to_string())),
    };
    let mut state = State::default();
    for (key, item) in object {
        match key.as_str() {
            "widgets" => state.widgets = widgets(declared, item)?,
            "note" => {
                let note = item.as_str().ok_or_else(|| field("note"))?;
                miyu_policy::overrides::model_text(note, NOTE_MOST).map_err(|_| field("note"))?;
                state.note = Some(note.to_string());
            }
            "data" if item.is_null() => {}
            "data" => {
                let raw = RawJson::of(item).ok_or_else(|| field("data"))?;
                if raw.get().len() > DATA_MOST {
                    return Err(field("data"));
                }
                state.data = Some(raw);
            }
            other => return Err(field(other)),
        }
    }
    Ok(state)
}

/// `widgets` 那一格：每一块要是这个包声明的 `state = true` 的，内容照它的位置查。空的当没写。
fn widgets(declared: &Declared, value: &Value) -> Result<Option<RawJson>, Bad> {
    let field = |name: &str| Bad::Field(format!("state.widgets{name}"));
    let blocks = match value {
        Value::Null => return Ok(None),
        Value::Object(blocks) if blocks.is_empty() => return Ok(None),
        Value::Object(blocks) => blocks,
        _ => return Err(field("")),
    };
    let pages: Vec<&str> = declared.pages.iter().map(String::as_str).collect();
    for (id, block) in blocks {
        let place = declared
            .widgets
            .iter()
            .find(|(declared, _)| declared == id)
            .map(|(_, place)| *place)
            .ok_or_else(|| field(&format!(".{id}")))?;
        let block: &Map<String, Value> =
            block.as_object().ok_or_else(|| field(&format!(".{id}")))?;
        if let Some(other) = block
            .keys()
            .find(|key| !matches!(key.as_str(), "content" | "title"))
        {
            return Err(field(&format!(".{id}.{other}")));
        }
        let content = block
            .get("content")
            .ok_or_else(|| field(&format!(".{id}.content")))?;
        let prefixed = |mut bad: miyu_config::widget::Bad| {
            bad.path = format!("widgets.{id}.{}", bad.path);
            Bad::Widget(bad)
        };
        miyu_config::widget::check(place, &pages, content).map_err(prefixed)?;
        if let Some(title) = block.get("title") {
            miyu_config::widget::check_title(title).map_err(prefixed)?;
        }
    }
    Ok(RawJson::of(value))
}

/// `session.set_state` 的参数。
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct SetParams {
    session: String,
    state: Value,
}

/// `session.state` 的参数。
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ReadParams {
    session: String,
}

/// `session.set_state`：程序直接换上它在这个会话里的一份，落了盘回 `{}`。不开回合。
pub(crate) async fn set(
    core: &std::sync::Arc<Core>,
    caller: &Caller,
    id: &CommandId,
    params: SetParams,
) -> Result<Value, Refusal> {
    let package = caller.package.as_deref().ok_or(Refusal::NOT_AN_EXTENSION)?;
    let session = SessionId::parse(&params.session).map_err(|_| Refusal::bad_field("session"))?;
    let state = read(&Declared::of(core, package), &params.state).map_err(|bad| bad.refusal())?;
    let found = core.sessions.get(core, &session).await?;
    let written = PackageState {
        package: package.to_string(),
        state,
    };
    let by = crate::sessions::person();
    match found
        .handle
        .command(id.clone(), by, Command::SetState(written))
        .await
    {
        Ok(Outcome::Rejected { reason, about }) => Err(Refusal::kernel_about(reason, &about)),
        Ok(_) => Ok(json!({})),
        Err(_) => Err(Refusal::STOPPED),
    }
}

/// `session.state`：这个包在这个会话里当前的一份，没有的是 `null`。
pub(crate) async fn get(
    core: &std::sync::Arc<Core>,
    caller: &Caller,
    params: ReadParams,
) -> Result<Value, Refusal> {
    let package = caller.package.as_deref().ok_or(Refusal::NOT_AN_EXTENSION)?;
    let session = SessionId::parse(&params.session).map_err(|_| Refusal::bad_field("session"))?;
    let found = core.sessions.get(core, &session).await?;
    Ok(json!({"state": found.handle.package_state(package)}))
}

#[cfg(test)]
mod tests;
