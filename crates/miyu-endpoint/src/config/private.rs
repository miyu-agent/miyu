//! 改私密目录时多查一道（施工 5-15 下，`docs/designs/11-权限与沙盒.md` 第四节）：`sandbox.private.add` 里每一个目录要写成
//! `~/…` 或绝对路径；家目录本身和它的上级不收（藏了它什么命令都跑不了，2026-10-20 项目主人定），说清原因。类型照旧由清单查。

use std::path::Path;

use miyu_config::Value;
use miyu_config::problem::Code;

/// 这一项 `key` 写成 `value` 时的问题：哪一种、写错的是哪一个（照原样）。不是这一项、没问题的没有。家目录不知道（`home`）
/// 的只查写法。
pub(super) fn problem(key: &str, value: &Value, home: Option<&Path>) -> Option<(Code, String)> {
    if key != miyu_tool::private::ADD {
        return None;
    }
    let Value::List(dirs) = value else {
        return None;
    };
    dirs.iter().find_map(|dir| {
        let Value::Text(text) = dir else {
            return None;
        };
        let written = miyu_fs::written(home.unwrap_or(Path::new("/")), text);
        match written {
            None => Some((Code::BadFormat, text.to_string())),
            Some(dir) if home.is_some_and(|home| miyu_fs::too_broad(home, &dir)) => {
                Some((Code::PrivateTooBroad, text.to_string()))
            }
            Some(_) => None,
        }
    })
}

#[cfg(test)]
mod tests;
