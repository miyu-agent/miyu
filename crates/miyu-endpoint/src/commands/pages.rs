//! 整页的命令（施工 X-15 下，`docs/blueprint/widgets.md`「整页的命令」）：清单 `[[widget]]` 里整页写的 `command`，打了只叫头打开
//! 那一页：不碰会话、不记日志、不归预设管。装着、开着的扩展的才算；场所会话里不认（通讯平台开不了页）。自己放的同名模板盖住它。

use serde_json::{Value, json};

use miyu_config::phrases::Phrases;

use crate::Core;

/// 一条整页的命令。
pub(crate) struct Page {
    /// 命令名。
    pub(crate) name: String,
    package: String,
    widget: String,
    title: Option<Phrases>,
}

/// 这一刻认的：装着、开着的扩展的整页里写了命令名的（清单只许整页写），照包的编号、清单里的先后。
pub(crate) fn found(core: &Core) -> Vec<Page> {
    let packages = core.system_packages();
    crate::extensions::processes(&packages)
        .filter(|(id, manifest)| crate::extensions::switched_on(core, id, manifest))
        .flat_map(|(id, manifest)| {
            manifest.widgets.iter().filter_map(move |widget| {
                Some(Page {
                    name: widget.command.clone()?,
                    package: id.to_string(),
                    widget: widget.id.clone(),
                    title: widget.title.clone(),
                })
            })
        })
        .collect()
}

impl Page {
    /// 哪个包的整页（施工 U23 末下：关了的包不列、不开）。
    pub(crate) fn package(&self) -> &str {
        &self.package
    }
}

/// 叫 `name` 的那一条。
pub(crate) fn named(core: &Core, name: &str) -> Option<Page> {
    found(core).into_iter().find(|page| page.name == name)
}

fn opens(page: &Page) -> Value {
    json!({"package": page.package, "widget": page.widget})
}

/// 命令菜单里的一项：说明是整页的标题，照语言挑。
pub(crate) fn listed(page: &Page, language: &str) -> Value {
    let summary = page
        .title
        .as_ref()
        .and_then(|title| crate::personas::pick(title, language));
    json!({"name": page.name, "aliases": [], "summary": summary, "opens": opens(page)})
}

/// `command.run` 的回应：头照 `opens` 打开那一页。
pub(crate) fn opened(page: &Page) -> Value {
    json!({"command": page.name, "opens": opens(page), "said": ""})
}
