//! 推给头的一块（施工 X-15 中，`widgets.md`「头」第 2、3 条）：装着、开着的扩展声明的全部块，照包的编号、清单里的先后；一块写成
//! `{"package", "id", "place", "title"?, "scope", "command"?, "source", "icon"?, "running", "content"?}`，字照连接的语言挑好。

use serde_json::{Value, json};

use miyu_config::package::{Manifest, Widget, WidgetPlace};

use miyu_kernel::id::AccountId;

use super::Key;
use crate::Core;
use crate::extensions::State;

/// 一个包声明的一块，连着画它要的包的几样。
#[derive(Debug, Clone)]
pub(super) struct Item {
    /// 包的编号。
    pub(super) package: String,
    /// 清单里的这一块。
    pub(super) widget: Widget,
    /// 包的名字。
    name: miyu_config::phrases::Phrases,
    /// 包的图标。
    icon: Option<String>,
    /// 每人一份的包（施工 IM-1 中下五）：看的那个人，内容、运行状态照他那一份；一份的包没有。
    pub(super) owner: Option<AccountId>,
}

impl Item {
    /// 这一块在会话 `session` 里（跟着会话的）、或者全局的那一份。
    pub(super) fn key(&self, session: Option<&str>) -> Key {
        Key {
            package: self.package.clone(),
            widget: self.widget.id.clone(),
            session: self
                .widget
                .per_session
                .then(|| session.map(str::to_string))
                .flatten(),
            account: self.owner.clone(),
        }
    }
}

/// 这一刻 `viewer` 的头看得到的块：装着、开着的扩展声明的；只要包 `only` 的就写上它。每人一份的包的块是他那一份的
/// （施工 IM-1 中下五）。
pub(super) fn declared(core: &Core, only: Option<&str>, viewer: &AccountId) -> Vec<Item> {
    let packages = core.system_packages();
    crate::extensions::processes(&packages)
        .filter(|(id, _)| only.is_none_or(|only| only == *id))
        .filter(|(id, manifest)| crate::extensions::switched_on(core, id, manifest))
        .flat_map(|(id, manifest)| items(id, manifest, viewer))
        .collect()
}

fn items(id: &str, manifest: &Manifest, viewer: &AccountId) -> Vec<Item> {
    let owner = crate::extensions::per_account(manifest).then(|| viewer.clone());
    manifest
        .widgets
        .iter()
        .map(|widget| Item {
            package: id.to_string(),
            widget: widget.clone(),
            name: manifest.name.clone(),
            icon: manifest.icon.clone(),
            owner: owner.clone(),
        })
        .collect()
}

/// 推给头的一块：`session` 是头正在看的会话（跟着会话的块照它取内容），`content` 是这一块在不在屏上（在的才带内容）。
pub(super) fn one(
    core: &Core,
    item: &Item,
    session: Option<&str>,
    content: bool,
    language: &str,
) -> Value {
    let widget = &item.widget;
    let shown = core.widgets.get(&item.key(session));
    let instance = crate::extensions::Instance {
        package: item.package.clone(),
        account: item.owner.clone(),
    };
    let running = matches!(
        core.extensions.status_of(&instance).state,
        State::Running { .. }
    );
    let mut one = json!({
        "package": item.package,
        "id": widget.id,
        "place": widget.place.as_str(),
        "scope": if widget.per_session { "session" } else { "global" },
        "source": crate::personas::pick(&item.name, language),
        "running": running,
    });
    let title = shown
        .as_ref()
        .and_then(|shown| shown.title.clone())
        .or_else(|| widget.title.as_ref().map(|title| json!(title)));
    if let Some(title) = title {
        one["title"] = miyu_config::widget::pick(&title, language);
    }
    if let Some(command) = &widget.command {
        one["command"] = json!(command);
    }
    if let Some(icon) = &item.icon {
        one["icon"] = json!(icon);
    }
    if content {
        one["content"] = shown.map_or(Value::Null, |shown| {
            miyu_config::widget::localize(&shown.content, language)
        });
    }
    one
}

/// 这一块在不在屏上：`showing` 里有它的位置；整页的是开着它。
pub(super) fn on_screen(
    item: &Item,
    showing: &[WidgetPlace],
    page: Option<&(String, String)>,
) -> bool {
    match item.widget.place {
        WidgetPlace::Page => page
            .is_some_and(|(package, widget)| *package == item.package && *widget == item.widget.id),
        place => showing.contains(&place),
    }
}
