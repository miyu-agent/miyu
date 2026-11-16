//! 写目录的几项多查一道（施工 PT-2 外起；施工 PV-RM 去掉私密目录以后只剩别的技能目录 `skills.external.dirs`）：每一个目录要写成
//! `~`、`~/…` 或绝对路径（[`external::well_written`]），写错的照原样说。`config.set` 时查，`miyu check`、`config.check` 也查
//! 手改的文件（[`external_dirs`]）。类型照旧由清单查。

use miyu_config::Value;
use miyu_config::problem::{Code, Problem};
use miyu_config::{Layer, parse::Parsed};
use miyu_store::skills::external;

/// 这一项 `key` 写成 `value` 时的问题：哪一种、写错的是哪一个（照原样）。不是这一项、没问题的没有。
pub(super) fn problem(key: &str, value: &Value) -> Option<(Code, String)> {
    let Value::List(dirs) = value else {
        return None;
    };
    if key != external::DIRS {
        return None;
    }
    dirs.iter()
        .filter_map(|dir| match dir {
            Value::Text(text) => Some(text.as_ref()),
            _ => None,
        })
        .find(|text| !external::well_written(text))
        .map(|text| (Code::DirPath, text.to_string()))
}

/// 当成 `layer` 那一层读成的 `parsed` 里别的技能目录写错了的（施工 PT-2 外）：手改的文件在 `miyu check`、`config.check` 时就报，
/// 不等读的时候（读的时候认不了的静默跳过）。
pub(super) fn external_dirs(parsed: &Parsed, layer: Layer) -> Vec<Problem> {
    parsed
        .entries
        .iter()
        .filter(|(key, _)| key.as_str() == external::DIRS)
        .filter_map(|(key, entry)| {
            let (code, got) = problem(key, &entry.value)?;
            Some(Problem::item(code, layer, key, entry.at, &got))
        })
        .collect()
}

#[cfg(test)]
mod tests;
