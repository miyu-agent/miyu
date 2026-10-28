//! 程序推内容（施工 X-15 中，`widgets.md`「程序发给核心」）：`widget.set`、`widget.clear`，只在核心拉起的那个连接上。块要在这个
//! 包的清单里；全局的不写会话，跟着会话的要写；内容、换掉的标题照 `miyu_config::widget` 查，写法不对的整块不收、原来的不动。

use serde::Deserialize;
use serde_json::{Value, json};

use miyu_config::package::{Manifest, WidgetPlace};
use miyu_kernel::id::SessionId;

use super::{Key, Shown};
use crate::Core;
use crate::hello::Caller;
use crate::refusal::Refusal;

/// `widget.set` 的参数。
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct SetParams {
    widget: String,
    #[serde(default)]
    session: Option<String>,
    #[serde(default)]
    title: Option<Value>,
    content: Value,
}

/// `widget.clear` 的参数。
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ClearParams {
    widget: String,
    #[serde(default)]
    session: Option<String>,
}

/// `widget.set`：查过记下，交回 `{"kept"}`：跟着会话的、这个会话没人在看的不留。
pub(crate) fn set(core: &Core, caller: &Caller, params: SetParams) -> Result<Value, Refusal> {
    let (key, place, pages) = locate(core, caller, &params.widget, params.session)?;
    let pages: Vec<&str> = pages.iter().map(String::as_str).collect();
    let checked = miyu_config::widget::check(place, &pages, &params.content).and_then(|()| {
        params
            .title
            .as_ref()
            .map_or(Ok(()), miyu_config::widget::check_title)
    });
    if let Err(bad) = checked {
        tracing::warn!(target: "miyu::endpoint", package = key.package.as_str(), widget = key.widget.as_str(), problem = bad.problem.as_str(), "widget content rejected");
        return Err(Refusal::bad_widget_content(&bad));
    }
    let shown = Shown {
        content: params.content,
        title: params.title,
    };
    Ok(json!({"kept": core.widgets.put(key, shown)}))
}

/// `widget.clear`：回到没有内容。
pub(crate) fn clear(core: &Core, caller: &Caller, params: ClearParams) -> Result<Value, Refusal> {
    let (key, _, _) = locate(core, caller, &params.widget, params.session)?;
    core.widgets.clear(key);
    Ok(json!({}))
}

/// 找这个连接的包声明的那一块：交回它的 [`Key`]、位置、这个包的整页的编号。
fn locate(
    core: &Core,
    caller: &Caller,
    widget: &str,
    session: Option<String>,
) -> Result<(Key, WidgetPlace, Vec<String>), Refusal> {
    let Some(package) = caller.package.as_deref() else {
        return Err(Refusal::NOT_AN_EXTENSION);
    };
    let packages = core.system_packages();
    let manifest: Option<&Manifest> = packages
        .iter()
        .find(|found| found.id == package)
        .and_then(|found| found.read.as_ref().ok());
    let declared = manifest
        .and_then(|manifest| manifest.widgets.iter().find(|one| one.id == widget))
        .ok_or_else(|| Refusal::unknown_widget(widget))?;
    match (&session, declared.per_session) {
        (Some(id), true) if SessionId::parse(id).is_ok() => {}
        (None, false) => {}
        _ => return Err(Refusal::BAD_PARAMS),
    }
    let pages = manifest
        .map(|manifest| {
            manifest
                .widgets
                .iter()
                .filter(|one| one.place == WidgetPlace::Page)
                .map(|one| one.id.clone())
                .collect()
        })
        .unwrap_or_default();
    let key = Key {
        package: package.to_string(),
        widget: widget.to_string(),
        session,
    };
    Ok((key, declared.place, pages))
}
