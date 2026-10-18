//! `persona.toml` 里 `[persona]` 以外的几张表（施工 P-6 从 `persona.rs` 挪出来守 500 行上限）：`[memory]`（施工 R-3 下）、
//! `[knowledge]`（施工 K-1 下）、`[appearance]`（施工 P-6）。

use toml_edit::Item;

use super::{Code, Problem, problem};
use crate::memory::MemoryScope;

/// `[memory]`：只有 `scope`，`persona` 或 `session`。`off` 不在这里：不开记忆是开会话时、预设的事。
pub(super) fn read_memory(
    item: &Item,
    at: &dyn Fn(&Item) -> Option<usize>,
) -> Result<Option<MemoryScope>, Problem> {
    let Some(table) = item.as_table_like() else {
        return Err(problem(
            at(item),
            Code::NotATable,
            "memory",
            "memory must be a table".to_string(),
        ));
    };
    let mut scope = None;
    for (key, item) in table.iter() {
        if key != "scope" {
            return Err(problem(
                at(item),
                Code::UnknownKey,
                &format!("memory.{key}"),
                format!("unknown key memory.{key}"),
            ));
        }
        scope = match item.as_str().and_then(MemoryScope::parse) {
            Some(MemoryScope::Off) | None => {
                return Err(problem(
                    at(item),
                    Code::BadMemoryScope,
                    "memory.scope",
                    "memory.scope must be persona or session".to_string(),
                ));
            }
            found => found,
        };
    }
    Ok(scope)
}

/// `[knowledge]`：只有 `bases`，一列知识库的编号（施工 K-1 下，19 N11）。编号的写法同 `miyu_store::layers::valid`（小写字母
/// 开头，小写字母、数字、`-`、`_`，最多 64 个）：这一层不碰存储，照写一遍。列着的库在不在不查：查的时候不在的当没有。
pub(super) fn read_knowledge(
    item: &Item,
    at: &dyn Fn(&Item) -> Option<usize>,
) -> Result<Option<Vec<String>>, Problem> {
    let Some(table) = item.as_table_like() else {
        return Err(problem(
            at(item),
            Code::NotATable,
            "knowledge",
            "knowledge must be a table".to_string(),
        ));
    };
    let mut bases = None;
    for (key, item) in table.iter() {
        if key != "bases" {
            return Err(problem(
                at(item),
                Code::UnknownKey,
                &format!("knowledge.{key}"),
                format!("unknown key knowledge.{key}"),
            ));
        }
        let ids: Option<Vec<String>> = item.as_array().and_then(|array| {
            array
                .iter()
                .map(|value| value.as_str().filter(|id| base_id(id)).map(str::to_string))
                .collect()
        });
        let Some(ids) = ids else {
            return Err(problem(
                at(item),
                Code::BadKnowledgeBases,
                "knowledge.bases",
                "knowledge.bases must be a list of knowledge base ids".to_string(),
            ));
        };
        bases = Some(ids);
    }
    Ok(bases)
}

/// 知识库编号的写法。
fn base_id(id: &str) -> bool {
    id.len() <= 64
        && id.starts_with(|c: char| c.is_ascii_lowercase())
        && id
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_')
}

/// `[appearance]` 读出来的几样：没写的是没有。
#[derive(Debug, Default)]
pub(super) struct Appearance {
    pub(super) seed: Option<String>,
    pub(super) background_opacity: Option<u8>,
    pub(super) background_focus: Option<[u8; 2]>,
    pub(super) background_blur: Option<u8>,
}

/// `[appearance]`：`seed`（`#` 加六位十六进制，交回小写的），`background_opacity`（0 到 100 的整数），`background_focus`
/// （两个 0 到 100 的整数，施工 P-7），`background_blur`（1 到 100 的整数，施工 P-7 补）。别的键 `unknown_key`。
pub(super) fn read_appearance(
    item: &Item,
    at: &dyn Fn(&Item) -> Option<usize>,
) -> Result<Appearance, Problem> {
    let Some(table) = item.as_table_like() else {
        return Err(problem(
            at(item),
            Code::NotATable,
            "appearance",
            "appearance must be a table".to_string(),
        ));
    };
    let mut appearance = Appearance::default();
    for (key, item) in table.iter() {
        match key {
            "seed" => {
                let Some(color) = item.as_str().and_then(color) else {
                    return Err(problem(
                        at(item),
                        Code::BadSeed,
                        "appearance.seed",
                        "appearance.seed must be a color like #3368c0".to_string(),
                    ));
                };
                appearance.seed = Some(color);
            }
            "background_opacity" => {
                let Some(percent) = item.as_integer().and_then(percent) else {
                    return Err(problem(
                        at(item),
                        Code::BadBackgroundOpacity,
                        "appearance.background_opacity",
                        "appearance.background_opacity must be a whole number from 0 to 100"
                            .to_string(),
                    ));
                };
                appearance.background_opacity = Some(percent);
            }
            "background_focus" => {
                let focus = item.as_array().and_then(|array| {
                    let mut numbers = array
                        .iter()
                        .map(|value| value.as_integer().and_then(percent));
                    match (numbers.next(), numbers.next(), numbers.next()) {
                        (Some(Some(x)), Some(Some(y)), None) => Some([x, y]),
                        _ => None,
                    }
                });
                let Some(focus) = focus else {
                    return Err(problem(
                        at(item),
                        Code::BadBackgroundFocus,
                        "appearance.background_focus",
                        "appearance.background_focus must be [x, y], each from 0 to 100"
                            .to_string(),
                    ));
                };
                appearance.background_focus = Some(focus);
            }
            "background_blur" => {
                let Some(blur) = item.as_integer().and_then(blur) else {
                    return Err(problem(
                        at(item),
                        Code::BadBackgroundBlur,
                        "appearance.background_blur",
                        "appearance.background_blur must be a whole number from 1 to 100"
                            .to_string(),
                    ));
                };
                appearance.background_blur = Some(blur);
            }
            _ => {
                return Err(problem(
                    at(item),
                    Code::UnknownKey,
                    &format!("appearance.{key}"),
                    format!("unknown key appearance.{key}"),
                ));
            }
        }
    }
    Ok(appearance)
}

/// 0 到 100 的百分比；别的没有。
pub fn percent(number: i64) -> Option<u8> {
    u8::try_from(number).ok().filter(|percent| *percent <= 100)
}

/// 模糊多少：1 到 100；0 不算（不模糊就是不写），别的也没有。
pub fn blur(number: i64) -> Option<u8> {
    percent(number).filter(|blur| *blur > 0)
}

/// `#rrggbb` 的颜色，大小写都认，交回小写的；别的写法没有（施工 P-6：头照它比对，一律小写）。
pub fn color(text: &str) -> Option<String> {
    let hex = text.strip_prefix('#')?;
    (hex.len() == 6 && hex.bytes().all(|byte| byte.is_ascii_hexdigit()))
        .then(|| format!("#{}", hex.to_ascii_lowercase()))
}

#[cfg(test)]
mod tests;
