//! 读好的配置项拼成配置清单的项（施工 9-1 下；施工 PC-1 从 `settings.rs` 分出来，行数）。

use super::{PAGE, Setting, SettingKind};
use crate::item::{Control, Item, Kind, Ui};

/// 包 `package` 的配置项拼成配置清单的项：键 `<前缀>.<名字>`（`prefix`，施工 PN-1，[`crate::package::Manifest::prefix_of`]），挂在
/// 「软件包」那一页（[`PAGE`]）、这个包那一组。字留在进程里，一直用到退出：只在核心起来、装卸时调。
pub fn items(package: &str, prefix: &str, settings: &[Setting]) -> Vec<Item> {
    let group: &'static str = leak(package);
    settings
        .iter()
        .map(|setting| Item {
            key: leak(&format!("{prefix}.{}", setting.name)),
            kind: leaked(&setting.kind),
            default: setting.default.clone(),
            layers: Box::leak(setting.layers.clone().into_boxed_slice()),
            tighten: None,
            env: None,
            applies: setting.applies,
            ui: Ui {
                page: PAGE,
                group,
                package: Some(group),
                common: false,
                control: setting
                    .slider
                    .map_or_else(|| setting.kind.control(), |step| Control::Slider { step }),
                hidden: setting.hidden,
            },
        })
        .collect()
}

/// 换成配置清单的类型：选项的字、列表的元素留在进程里。
fn leaked(kind: &SettingKind) -> Kind {
    match kind {
        SettingKind::Option(choices) => Kind::Option(Box::leak(
            choices
                .iter()
                .map(|choice| leak(&choice.value))
                .collect::<Vec<_>>()
                .into_boxed_slice(),
        )),
        SettingKind::List(element) => Kind::List(Box::leak(Box::new(leaked(element)))),
        plain => plain.plain().unwrap_or(Kind::Bool),
    }
}

/// 一段字留在进程里。
fn leak(text: &str) -> &'static str {
    Box::leak(text.to_string().into_boxed_str())
}
