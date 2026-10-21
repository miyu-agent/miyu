//! 改私密目录时多查一道（施工 5-15 下，`docs/designs/11-权限与沙盒.md` 第四节）：`sandbox.private.add` 里每一个目录要写成
//! `~/…` 或绝对路径；家目录本身和它的上级不收（藏了它什么命令都跑不了，2026-10-20 项目主人定），说清原因。
//! `sandbox.private.remove` 只收这台机器的出厂清单，`config.schema` 照它给选项（施工 5-15 下补：清单里登记成文字的列表，生成的
//! 文件哪个系统都一样）。类型照旧由清单查。

use std::path::Path;

use miyu_config::Value;
use miyu_config::problem::Code;

/// 这一项 `key` 写成 `value` 时的问题：哪一种、写错的是哪一个（照原样）。不是这一项、没问题的没有。家目录不知道（`home`）
/// 的只查写法。
pub(super) fn problem(key: &str, value: &Value, home: Option<&Path>) -> Option<(Code, String)> {
    let Value::List(dirs) = value else {
        return None;
    };
    if key == miyu_tool::private::REMOVE {
        return dirs.iter().find_map(|dir| match dir {
            Value::Text(text) if !miyu_fs::SHIPPED_PRIVATE.contains(&text.as_ref()) => {
                Some((Code::PrivateNotShipped, text.to_string()))
            }
            _ => None,
        });
    }
    if key != miyu_tool::private::ADD {
        return None;
    }
    dirs.iter().find_map(|dir| {
        let Value::Text(text) = dir else {
            return None;
        };
        let written = miyu_fs::written(home.unwrap_or(Path::new("/")), text);
        match written {
            None => Some((Code::PrivatePath, text.to_string())),
            Some(dir) if home.is_some_and(|home| miyu_fs::too_broad(home, &dir)) => {
                Some((Code::PrivateTooBroad, text.to_string()))
            }
            Some(_) => None,
        }
    })
}

/// `config.schema` 里这一项在这个系统上不画（施工 5-16 下）：沙盒强度只在 Linux 上起作用。
pub(super) fn hidden_here(key: &str) -> bool {
    key == "sandbox.strength" && !cfg!(target_os = "linux")
}

/// `config.schema` 里这一项怎么画勾（施工 5-15 再补，2026-10-21 项目主人验收时定）：去掉的出厂项交 `unlisted`，勾着的是不在值
/// 里的（照旧私密），头照它反着画，不认键名；别的没有（勾着的是值里的）。
pub(super) fn ticks(key: &str) -> Option<&'static str> {
    (key == miyu_tool::private::REMOVE).then_some("unlisted")
}

/// 照 [`ticks`] 给 `config.schema` 里的这一项 `map` 写上 `ticks`，没有的不写。
pub(super) fn tick(key: &str, map: &mut serde_json::Map<String, serde_json::Value>) {
    if let Some(ticks) = ticks(key) {
        map.insert("ticks".to_string(), serde_json::json!(ticks));
    }
}

/// `config.schema` 里这一项的选项：去掉的出厂项交这台机器的出厂清单，别的没有。
pub(super) fn options(key: &str) -> Option<&'static [&'static str]> {
    (key == miyu_tool::private::REMOVE).then_some(miyu_fs::SHIPPED_PRIVATE)
}

#[cfg(test)]
mod tests;
