//! 同名覆盖的文件（施工 OV-1 上，设计 00 第五节「可自定义」）：工具说明的覆盖放在主人家目录
//! `tools/<工具>.toml`（施工 SU-3 下：系统区那一处拿掉）。这里只找、只读；读成什么样由策略那一层定
//! （`miyu_policy::overrides`）。读照配置文件的规矩（大小、UTF-8）。

use std::path::{Path, PathBuf};

use crate::config_file;

/// 每一层里放工具说明覆盖的目录名。
pub const TOOLS: &str = "tools";

/// 一个覆盖文件：哪件工具、在哪、读到的字（读不了的是为什么）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Written {
    /// 工具名：文件名去掉 `.toml`。
    pub tool: String,
    /// 文件的路径。
    pub path: PathBuf,
    /// 读到的字；读不了、太大、不是 UTF-8 的是为什么。
    pub text: Result<String, String>,
}

/// 目录 `dir` 里的工具说明覆盖：`<工具名>.toml`、是普通文件的，照工具名排。工具名照工具名的写法（英文字母、数字、`_`、`-`，
/// 1 到 64 个字），别的文件不算；没有这个目录的是空的。
pub fn tools_in(dir: &Path) -> Vec<Written> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut found: Vec<Written> = entries
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_file()))
        .filter_map(|entry| {
            let path = entry.path();
            let tool = path
                .file_name()?
                .to_str()?
                .strip_suffix(".toml")
                .filter(|tool| tool_name(tool))?
                .to_string();
            let text = match config_file::read(&path) {
                Ok(Some(read)) => Ok(read.text),
                Ok(None) => Err("not found".to_string()),
                Err(error) => Err(error.to_string()),
            };
            Some(Written { tool, path, text })
        })
        .collect();
    found.sort_by(|a, b| a.tool.cmp(&b.tool));
    found
}

/// 合工具名的写法。
fn tool_name(name: &str) -> bool {
    (1..=64).contains(&name.len())
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

#[cfg(test)]
mod tests;
