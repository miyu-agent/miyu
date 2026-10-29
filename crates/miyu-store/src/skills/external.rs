//! 读别的目录里的技能（施工 PT-2 外，`docs/blueprint/skills.md`「找」，2026-10-29 项目主人定）：总开关
//! `skills.external.enabled`（出厂关），一张目录表 `skills.external.dirs`（出厂一项 `~/.agents/skills`，agentskills.io 的约定：
//! Codex、opencode 这些装的技能放这里）。只写在 Miyu 的配置里，项目配置不收；下一轮生效，同技能本来的时机（造会话找一次，
//! 回合开始再找一遍）。
//!
//! 目录的写法同私密目录（[`miyu_fs::written`]）：`~`、`~/…` 照家目录拼，绝对路径照原样，别的写法（相对路径）不认，
//! `config.set`、`miyu check` 时就报。关着的一个目录都不交，找技能不碰这些目录。

use std::borrow::Cow;
use std::path::{Path, PathBuf};

use miyu_config::{Applies, Control, Item, Kind, Layer, Setting, Ui, Value, Values};

/// 总开关。
pub const ENABLED: &str = "skills.external.enabled";

/// 目录表。
pub const DIRS: &str = "skills.external.dirs";

/// 目录表出厂的那一项。
pub const SHIPPED: &str = "~/.agents/skills";

/// 一个目录的写法最多几个字（同私密目录）。
const LONGEST: usize = 4096;

/// 设置页「高级」那一页、技能那一组（网页另画在技能页顶上，照键名取）。
const UI: Ui = Ui {
    page: "advanced",
    group: "skills",
    package: None,
    common: false,
    control: Control::List,
    hidden: false,
};

/// 这两项：只写在 Miyu 的配置里，下一轮生效；开关排在组里最上面。
pub const ITEMS: &[Item] = &[
    Item {
        key: ENABLED,
        kind: Kind::Bool,
        default: Some(Value::Bool(false)),
        layers: &[Layer::System],
        tighten: None,
        env: None,
        applies: Applies::NextTurn,
        ui: Ui {
            control: Control::Toggle,
            ..UI
        },
    },
    Item {
        key: DIRS,
        kind: Kind::List(&Kind::Text { max: LONGEST }),
        default: Some(Value::List(Cow::Borrowed(&[Value::Text(Cow::Borrowed(
            SHIPPED,
        ))]))),
        layers: &[Layer::System],
        tighten: None,
        env: None,
        applies: Applies::NextTurn,
        ui: UI,
    },
];

/// 一个读技能的别的目录。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct External {
    /// 配置里写的原文（`skill.list` 的 `dir`）。
    pub text: String,
    /// 照家目录拼好的位置。
    pub dir: PathBuf,
}

/// 照这一轮配置的最终值 `values` 拼出要读的别的目录，照表里的先后，重了的留前一个：关着的是空的；写法不认的、`~` 开头而没有
/// 家目录 `home` 的跳过。
pub fn of(values: &Values, home: Option<&Path>) -> Vec<External> {
    if <Option<bool>>::read(values.get(ENABLED)) != Some(true) {
        return Vec::new();
    }
    let written =
        <Option<Vec<String>>>::read(values.get(DIRS)).unwrap_or_else(|| vec![SHIPPED.to_string()]);
    let mut external: Vec<External> = Vec::new();
    for text in written {
        let Some(dir) = dir_of(&text, home) else {
            continue;
        };
        if external.iter().all(|one| one.dir != dir) {
            external.push(External { text, dir });
        }
    }
    external
}

/// 目录表里一项的写法 `text` 认不认（`config.set`、`miyu check` 照它查）：`~`、`~/…`、绝对路径认。
pub fn well_written(text: &str) -> bool {
    text == "~" || text.starts_with("~/") || Path::new(text).is_absolute()
}

/// 一项写法 `text` 照家目录 `home` 拼成的位置。
fn dir_of(text: &str, home: Option<&Path>) -> Option<PathBuf> {
    match home {
        Some(home) => miyu_fs::written(home, text),
        None => {
            let path = Path::new(text);
            path.is_absolute().then(|| path.to_path_buf())
        }
    }
}

#[cfg(test)]
mod tests;
