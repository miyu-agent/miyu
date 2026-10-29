//! 改「隐私与安全」那一页的目录时多查一道（施工 5-15 下，`docs/designs/11-权限与沙盒.md` 第四节）：`sandbox.private.add` 里每一个
//! 目录要写成 `~/…` 或绝对路径；家目录本身和它的上级不收（藏了它什么命令都跑不了，2026-10-20 项目主人定），说清原因。
//! `sandbox.private.remove` 只收这台机器的出厂清单，`config.schema` 照它给选项（施工 5-15 下补：清单里登记成文字的列表，生成的
//! 文件哪个系统都一样）。受保护的路径（只禁写，施工 5-17 下）同一个办法：`sandbox.readonly.add` 多收工作区里的相对路径（不带
//! `..`），家目录和它的上级不收；`sandbox.readonly.remove` 只收出厂清单。类型照旧由清单查。别的技能目录
//! （`skills.external.dirs`，施工 PT-2 外）同私密目录的写法，家目录本身也收；`miyu check`、`config.check` 也查它（[`external_dirs`]）。

use std::path::Path;

use miyu_config::Value;
use miyu_config::problem::{Code, Problem};
use miyu_config::{Layer, parse::Parsed};
use miyu_store::skills::external;

/// 这一项 `key` 写成 `value` 时的问题：哪一种、写错的是哪一个（照原样）。不是这几项、没问题的没有。家目录不知道（`home`）
/// 的只查写法。
pub(super) fn problem(key: &str, value: &Value, home: Option<&Path>) -> Option<(Code, String)> {
    let Value::List(dirs) = value else {
        return None;
    };
    let texts: Vec<&str> = dirs
        .iter()
        .filter_map(|dir| match dir {
            Value::Text(text) => Some(text.as_ref()),
            _ => None,
        })
        .collect();
    let unshipped = |shipped: &[&str], code: Code| {
        texts
            .iter()
            .find(|text| !shipped.contains(text))
            .map(|text| (code, (*text).to_string()))
    };
    if key == miyu_tool::private::REMOVE {
        unshipped(miyu_fs::SHIPPED_PRIVATE, Code::PrivateNotShipped)
    } else if key == miyu_tool::readonly::REMOVE {
        unshipped(miyu_fs::SHIPPED_READONLY, Code::ReadonlyNotShipped)
    } else if key == miyu_tool::private::ADD {
        texts.iter().find_map(|text| private_added(text, home))
    } else if key == miyu_tool::readonly::ADD {
        texts.iter().find_map(|text| readonly_added(text, home))
    } else if key == external::DIRS {
        texts
            .iter()
            .find(|text| !external::well_written(text))
            .map(|text| (Code::PrivatePath, (*text).to_string()))
    } else {
        None
    }
}

/// 当成 `layer` 那一层读成的 `parsed` 里别的技能目录写错了的（施工 PT-2 外）：手改的文件在 `miyu check`、`config.check` 时就报，
/// 不等读的时候（读的时候认不了的静默跳过）。私密目录那几项照旧只在 `config.set` 时查。
pub(super) fn external_dirs(parsed: &Parsed, layer: Layer) -> Vec<Problem> {
    parsed
        .entries
        .iter()
        .filter(|(key, _)| key.as_str() == external::DIRS)
        .filter_map(|(key, entry)| {
            let (code, got) = problem(key, &entry.value, None)?;
            Some(Problem::item(code, layer, key, entry.at, &got))
        })
        .collect()
}

/// 人加的一个私密目录 `text` 的问题：不是 `~/…`、绝对路径的写法不对；家目录本身和它的上级太宽。
fn private_added(text: &str, home: Option<&Path>) -> Option<(Code, String)> {
    match miyu_fs::written(home.unwrap_or(Path::new("/")), text) {
        None => Some((Code::PrivatePath, text.to_string())),
        Some(dir) if home.is_some_and(|home| miyu_fs::too_broad(home, &dir)) => {
            Some((Code::PrivateTooBroad, text.to_string()))
        }
        Some(_) => None,
    }
}

/// 人加的一个受保护的路径 `text` 的问题（施工 5-17 下）：`~/…`、绝对路径、不带 `..` 的相对路径以外的写法不对；家目录本身和它的
/// 上级太宽。家目录不知道的，`~` 开头的照家目录是 `/` 查写法。
fn readonly_added(text: &str, home: Option<&Path>) -> Option<(Code, String)> {
    let assumed = home.or(Some(Path::new("/")));
    match miyu_fs::readonly_entry(assumed, text) {
        None => Some((Code::ReadonlyPath, text.to_string())),
        Some(miyu_fs::ReadonlyEntry::Path(dir))
            if home.is_some_and(|home| miyu_fs::too_broad(home, &dir)) =>
        {
            Some((Code::ReadonlyTooBroad, text.to_string()))
        }
        Some(_) => None,
    }
}

/// `config.schema` 里这一项在这个系统上不画（施工 5-16 下）：沙盒强度只在 Linux 上起作用。
pub(super) fn hidden_here(key: &str) -> bool {
    key == "sandbox.strength" && !cfg!(target_os = "linux")
}

/// `config.schema` 里这一项怎么画勾（施工 5-15 再补，2026-10-21 项目主人验收时定）：去掉的出厂项交 `unlisted`，勾着的是不在值
/// 里的（照旧私密、照旧受保护，施工 5-17 下），头照它反着画，不认键名；别的没有（勾着的是值里的）。
pub(super) fn ticks(key: &str) -> Option<&'static str> {
    (key == miyu_tool::private::REMOVE || key == miyu_tool::readonly::REMOVE).then_some("unlisted")
}

/// 照 [`ticks`] 给 `config.schema` 里的这一项 `map` 写上 `ticks`，没有的不写。
pub(super) fn tick(key: &str, map: &mut serde_json::Map<String, serde_json::Value>) {
    if let Some(ticks) = ticks(key) {
        map.insert("ticks".to_string(), serde_json::json!(ticks));
    }
}

/// `config.schema` 里这一项的选项：去掉的出厂项交这台机器的出厂清单（私密目录的、受保护路径的），别的没有。
pub(super) fn options(key: &str) -> Option<&'static [&'static str]> {
    if key == miyu_tool::private::REMOVE {
        Some(miyu_fs::SHIPPED_PRIVATE)
    } else if key == miyu_tool::readonly::REMOVE {
        Some(miyu_fs::SHIPPED_READONLY)
    } else {
        None
    }
}

#[cfg(test)]
mod tests;
