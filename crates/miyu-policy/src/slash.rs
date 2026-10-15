//! 斜杠命令模板（施工 X-14 上，`docs/blueprint/slash.md`「模板」）：一个 Markdown 文件，文件名是命令名，正文照参数展开成
//! 一段话发给 AI，照 Claude Code 的 custom slash commands。开头可以有一段 `---` 包起来的 YAML：
//!
//! - `description`：一句说明，命令菜单里给人看的（算 `en`，也是别的语言没写时退回的那一份）；
//! - `argument-hint`：名字后面跟什么的提示；
//! - `metadata` 里的 `summary_zh`、`summary_ja`：另两种语言的说明（同技能的写法，`skill.rs`）。
//!
//! Claude Code 别的键（`allowed-tools`、`model`……）认、不用。没有开头的整份都是正文。纯逻辑。

use std::collections::BTreeMap;
use std::fmt;

use yaml_rust2::{Yaml, YamlLoader};

/// 名字最多几个字：同子命令名（`[command] name`）。
const NAME_CHARS: usize = 32;

/// 模板命令算的那个功能的编号的开头（同技能的 `skill-<名字>`）。
const FEATURE_PREFIX: &str = "command-";

/// 正文里换成整段参数的那个记号。
const ARGUMENTS: &str = "$ARGUMENTS";

/// 模板命令 `name` 算的功能的编号：预设照它开关。
pub fn feature_id(name: &str) -> String {
    format!("{FEATURE_PREFIX}{name}")
}

/// 命令名合不合写法：小写字母开头，只用小写字母、数字、`-`，最多 32 个（同子命令名）。
pub fn valid_name(name: &str) -> bool {
    name.chars().next().is_some_and(|c| c.is_ascii_lowercase())
        && name.chars().count() <= NAME_CHARS
        && name
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

/// 读好的模板。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Template {
    /// 一句说明，照语言：`en`、`zh`、`ja`，没写的没有。
    pub summary: BTreeMap<String, String>,
    /// 名字后面跟什么的提示，照原样；没写的没有。
    pub argument: Option<String>,
    /// 正文：开头那一段后面的全部；没有开头的是整份。
    pub body: String,
}

/// 哪里不对。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TemplateProblem {
    /// 开头那一段的 YAML 读不懂：解析器的原话。
    Yaml(String),
    /// 开头那一段不是一张表。
    NotAMap,
}

impl fmt::Display for TemplateProblem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TemplateProblem::Yaml(why) => write!(f, "front matter is not valid YAML: {why}"),
            TemplateProblem::NotAMap => f.write_str("front matter is not a mapping"),
        }
    }
}

/// 读一份模板的字 `text`。
///
/// # Errors
///
/// 开头那一段读不懂、不是一张表：见 [`TemplateProblem`]。
pub fn read(text: &str) -> Result<Template, TemplateProblem> {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let Some((front, body)) = split(text) else {
        return Ok(Template {
            summary: BTreeMap::new(),
            argument: None,
            body: text.to_string(),
        });
    };
    let docs = YamlLoader::load_from_str(front)
        .map_err(|error| TemplateProblem::Yaml(error.to_string()))?;
    let doc = match docs.first() {
        None => &Yaml::Null,
        Some(doc @ Yaml::Hash(_)) => doc,
        Some(_) => return Err(TemplateProblem::NotAMap),
    };
    let mut summary = BTreeMap::new();
    if let Some(text) = said(&doc["description"]) {
        summary.insert("en".to_string(), text);
    }
    for (key, language) in [("summary_zh", "zh"), ("summary_ja", "ja")] {
        if let Some(text) = said(&doc["metadata"][key]) {
            summary.insert(language.to_string(), text);
        }
    }
    Ok(Template {
        summary,
        argument: said(&doc["argument-hint"]),
        body: body.to_string(),
    })
}

/// 照参数展开（照 Claude Code）：`$ARGUMENTS[<第几个>]` 换成照空白切开的第几个（从 0 数，没有的换成空的），`$ARGUMENTS`
/// 换成整段 `arguments`（去掉前后空白）；正文里一处都没写、又跟了字的，末尾空一行接上那段字（Claude Code 另加一个
/// `ARGUMENTS:` 标签，这里不加：给模型的字非必要不加）。展开完去掉前后空白。
pub fn expand(body: &str, arguments: &str) -> String {
    let arguments = arguments.trim();
    let parts: Vec<&str> = arguments.split_whitespace().collect();
    let mut out = String::with_capacity(body.len() + arguments.len());
    let mut used = false;
    let mut rest = body;
    while let Some(at) = rest.find(ARGUMENTS) {
        out.push_str(&rest[..at]);
        used = true;
        let after = &rest[at + ARGUMENTS.len()..];
        match indexed(after) {
            Some((index, taken)) => {
                out.push_str(parts.get(index).copied().unwrap_or_default());
                rest = &after[taken..];
            }
            None => {
                out.push_str(arguments);
                rest = after;
            }
        }
    }
    out.push_str(rest);
    if !used && !arguments.is_empty() {
        out = format!("{}\n\n{arguments}", out.trim_end());
    }
    out.trim().to_string()
}

/// `$ARGUMENTS` 后面紧跟着的 `[<数字>]`：第几个、占了几个字节；不是的没有。
fn indexed(after: &str) -> Option<(usize, usize)> {
    let inside = after.strip_prefix('[')?;
    let close = inside.find(']')?;
    let digits = &inside[..close];
    if digits.is_empty() || !digits.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    Some((digits.parse().ok()?, close + 2))
}

/// 开头 `---` 和下一个 `---` 之间的字，和后面的正文；没有开头那一段的没有。行尾的 `\r` 不算。
fn split(text: &str) -> Option<(&str, &str)> {
    let mut lines = text.split_inclusive('\n');
    let first = lines.next()?;
    if first.trim_end() != "---" {
        return None;
    }
    let start = first.len();
    let mut end = start;
    for line in lines {
        if line.trim_end() == "---" {
            return Some((text.get(start..end)?, text.get(end + line.len()..)?));
        }
        end += line.len();
    }
    None
}

/// 一格给人看的字：去掉前后空白，空的、不是字的没有。
fn said(value: &Yaml) -> Option<String> {
    value
        .as_str()
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .map(str::to_string)
}

#[cfg(test)]
mod tests;
