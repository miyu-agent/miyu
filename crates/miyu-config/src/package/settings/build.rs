//! 读好的配置项拼成配置清单的项（施工 9-1 下；施工 PC-1 从 `settings.rs` 分出来，行数）。

use super::{PAGE, Setting, SettingKind};
use crate::item::{Control, Item, Kind, Record, Ui};

/// 包 `package` 的配置项拼成配置清单的项：键 `<前缀>.<名字>`（`prefix`，施工 PN-1，[`crate::package::Manifest::prefix_of`]），挂在
/// 「软件包」那一页（[`PAGE`]）、这个包那一组。字留在进程里，一直用到退出：只在核心起来、装卸时调。
pub fn items(package: &str, prefix: &str, settings: &[Setting]) -> Vec<Item> {
    let group: &'static str = leak(package);
    settings
        .iter()
        .map(|setting| item(group, &format!("{prefix}.{}", setting.name), setting))
        .collect()
}

/// 一项（或者一组字段里的一个字段，键是字段名，施工 PC-2）拼成配置清单的项，挂在包 `group` 那一组。
fn item(group: &'static str, key: &str, setting: &Setting) -> Item {
    Item {
        key: leak(key),
        kind: leaked(group, &setting.kind),
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
    }
}

/// 换成配置清单的类型：选项的字、列表的元素、一组字段的字段（施工 PC-2）留在进程里。
fn leaked(group: &'static str, kind: &SettingKind) -> Kind {
    match kind {
        SettingKind::Option(choices) => Kind::Option(Box::leak(
            choices
                .iter()
                .map(|choice| leak(&choice.value))
                .collect::<Vec<_>>()
                .into_boxed_slice(),
        )),
        SettingKind::List(element) => Kind::List(Box::leak(Box::new(leaked(group, element)))),
        SettingKind::Record { fields, title } => Kind::Record(Box::leak(Box::new(Record {
            fields: Box::leak(
                fields
                    .iter()
                    .map(|field| item(group, &field.name, field))
                    .collect::<Vec<_>>()
                    .into_boxed_slice(),
            ),
            title: title.as_deref().map(leak),
        }))),
        plain => plain.plain().unwrap_or(Kind::Bool),
    }
}

/// 一段字留在进程里。
fn leak(text: &str) -> &'static str {
    Box::leak(text.to_string().into_boxed_str())
}
