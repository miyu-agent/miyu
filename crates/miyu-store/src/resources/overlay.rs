//! 提示词的同名覆盖（施工 OV-1 中，设计 00 第五节「可自定义」、`26-提示词.md` J9）：随核心附带的给模型看的字（资源目录里的
//! `.txt`、`.md`）可以在 `prompts/<同样的路径>` 写一份同名的盖掉。
//!
//! 读的时候（[`ResourceRoot::read`]）从上往下找，第一份合规矩的就用它：
//! - 出厂的得有这一份（覆盖只改已有的字，不加新的）；
//! - 不是空的，除了换行、制表没有控制字符，CJK 的字不到一半（给模型看的字一律英文，`26-提示词.md` J3）；
//! - `{名字}` 这样的占位和出厂的一模一样：少了的换不进去，多了的原样留着发给模型。
//!
//! 不合的跳过、照下一层，最后是出厂的；`check` 照 [`ResourceRoot::overlay_problems`] 报。

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use super::ResourceRoot;

/// 每一层里放提示词覆盖的目录名。
pub const PROMPTS: &str = "prompts";

impl ResourceRoot {
    /// 压上几层覆盖 `layers`（从上往下排：自己的、系统的）。只换给模型看的字，资源目录本身照旧。
    #[must_use]
    pub fn overlaid(&self, layers: Vec<PathBuf>) -> ResourceRoot {
        ResourceRoot {
            path: self.path.clone(),
            overlays: layers,
            // 覆盖不碰给人看的字：读好的字照旧共用（施工 V-30）。
            shelf: self.shelf.clone(),
        }
    }

    /// `parts` 那一份字：几层覆盖里第一份合规矩的，都没有的是出厂的 `shipped`。
    pub(super) fn overlaid_text(&self, parts: &[&str], shipped: String) -> String {
        if !overridable(parts) {
            return shipped;
        }
        self.overlays
            .iter()
            .map(|layer| {
                parts
                    .iter()
                    .fold(layer.clone(), |path, part| path.join(part))
            })
            .filter_map(|path| std::fs::read_to_string(path).ok())
            .find(|text| why_bad(text, &shipped).is_none())
            .unwrap_or(shipped)
    }

    /// 几层覆盖里不合规矩的文件：路径和为什么（英文一句，给写的人看）。出厂没有这一份的、不是 `.txt`、`.md` 的也算。
    pub fn overlay_problems(&self) -> Vec<(PathBuf, String)> {
        let mut problems = Vec::new();
        for layer in &self.overlays {
            for path in files(layer) {
                let Ok(relative) = path.strip_prefix(layer) else {
                    continue;
                };
                let shipped = self.path.join(relative);
                let why = match (
                    is_text(&path),
                    std::fs::read_to_string(&shipped),
                    std::fs::read_to_string(&path),
                ) {
                    (false, _, _) => Some("only .txt and .md files can be overridden".to_string()),
                    (true, Err(_), _) => Some("nothing shipped has this path".to_string()),
                    (true, Ok(_), Err(error)) => Some(error.to_string()),
                    (true, Ok(shipped), Ok(text)) => why_bad(&text, &shipped),
                };
                if let Some(why) = why {
                    problems.push((path, why));
                }
            }
        }
        problems
    }
}

/// 照路径能不能盖：`.txt`、`.md`。
fn overridable(parts: &[&str]) -> bool {
    parts
        .last()
        .is_some_and(|name| name.ends_with(".txt") || name.ends_with(".md"))
}

/// 文件 `path` 是不是 `.txt`、`.md`。
fn is_text(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension == "txt" || extension == "md")
}

/// `text` 盖 `shipped` 合不合规矩：不合的交回为什么。
fn why_bad(text: &str, shipped: &str) -> Option<String> {
    if text.trim().is_empty() {
        return Some("is empty".to_string());
    }
    if text
        .chars()
        .any(|c| c.is_control() && c != '\n' && c != '\t' && c != '\r')
    {
        return Some("has control characters".to_string());
    }
    let count = text.chars().count();
    let cjk = text.chars().filter(|c| is_cjk(*c)).count();
    if cjk * 2 >= count {
        return Some("is not English".to_string());
    }
    let (written, wanted) = (placeholders(text), placeholders(shipped));
    if written != wanted {
        let names = |set: &BTreeSet<String>| set.iter().cloned().collect::<Vec<_>>().join(", ");
        return Some(format!(
            "placeholders must be exactly {{{}}}, found {{{}}}",
            names(&wanted),
            names(&written)
        ));
    }
    None
}

/// `text` 里的 `{名字}`：名字是英文小写字母、数字、`_`、`-`。
fn placeholders(text: &str) -> BTreeSet<String> {
    let mut found = BTreeSet::new();
    let mut rest = text;
    while let Some(start) = rest.find('{') {
        rest = &rest[start + 1..];
        let Some(end) = rest.find('}') else {
            break;
        };
        let name = &rest[..end];
        if !name.is_empty()
            && name
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '-')
        {
            found.insert(name.to_string());
        }
    }
    found
}

/// 中日韩的字。
fn is_cjk(c: char) -> bool {
    matches!(c, '\u{3040}'..='\u{30ff}' | '\u{3400}'..='\u{4dbf}' | '\u{4e00}'..='\u{9fff}' | '\u{ac00}'..='\u{d7af}' | '\u{f900}'..='\u{faff}')
}

/// 目录 `dir` 下所有普通文件（往下找），照路径排；没有的是空的。
fn files(dir: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    let mut pending = vec![dir.to_path_buf()];
    while let Some(next) = pending.pop() {
        let Ok(entries) = std::fs::read_dir(&next) else {
            continue;
        };
        for entry in entries.filter_map(Result::ok) {
            match entry.file_type() {
                Ok(kind) if kind.is_dir() => pending.push(entry.path()),
                Ok(kind) if kind.is_file() => found.push(entry.path()),
                _ => {}
            }
        }
    }
    found.sort();
    found
}

#[cfg(test)]
mod tests;
