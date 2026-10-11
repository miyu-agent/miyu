//! 技能 `SKILL.md` 的开头（施工 X-10 上，`docs/blueprint/skills.md`「技能」第 1 条）：照公开的 Agent Skills 格式读。第一行是
//! `---`，到下一个只有 `---` 的行之间是 YAML：
//!
//! - `name`：1 到 64 个字，只用小写字母、数字、`-`，不在头尾、不连着两个，和技能的文件夹同名；
//! - `description`：给模型看的一句，不能空，超过 [`DESCRIPTION_CHARS`] 个字的截掉（格式的上限）；
//! - `metadata` 里的 `display_name`、`summary`：给人看的名字、一句说明（双槽：给模型的和给人的分开）。
//!
//! 别的格子（`license`、`compatibility`、`allowed-tools`……）照格式认、不用。纯逻辑：进来的是文件夹的名字和文件里的字，出去的是
//! 读好的样子，或者哪里不对。

use std::fmt;

use yaml_rust2::{Yaml, YamlLoader};

/// 说明最多几个字（Unicode 字符）：格式的上限，超过的截到这么多。
pub const DESCRIPTION_CHARS: usize = 1024;

/// 名字最多几个字。
const NAME_CHARS: usize = 64;

/// 读好的开头。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkillHead {
    /// 名字：和文件夹同名。
    pub name: String,
    /// 给模型看的一句：并成一行，截过的。
    pub description: String,
    /// 给人看的名字；没写的没有。
    pub display_name: Option<String>,
    /// 给人看的一句说明；没写的没有。
    pub summary: Option<String>,
}

/// 哪里不对。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SkillProblem {
    /// 开头没有 `---` 包着的一段。
    NoFrontmatter,
    /// YAML 读不懂：解析器的原话。
    Yaml(String),
    /// 没写 `name`，或者不是一句字。
    NoName,
    /// `name` 不合写法。
    BadName(String),
    /// `name` 和文件夹的名字不一样。
    NameMismatch {
        /// 写的名字。
        name: String,
        /// 文件夹的名字。
        dir: String,
    },
    /// 没写 `description`，或者是空的。
    NoDescription,
}

impl fmt::Display for SkillProblem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SkillProblem::NoFrontmatter => f.write_str("no front matter between --- lines"),
            SkillProblem::Yaml(why) => write!(f, "front matter is not valid YAML: {why}"),
            SkillProblem::NoName => f.write_str("name is missing"),
            SkillProblem::BadName(name) => write!(
                f,
                "name {name:?} must be 1 to 64 lowercase letters, digits and single hyphens"
            ),
            SkillProblem::NameMismatch { name, dir } => {
                write!(f, "name {name:?} does not match its folder {dir:?}")
            }
            SkillProblem::NoDescription => f.write_str("description is missing or empty"),
        }
    }
}

/// 读文件夹 `dir` 里那份 `SKILL.md` 的字 `text`。
///
/// # Errors
///
/// 开头没有 YAML 那一段、YAML 读不懂、名字不合写法或者和文件夹不同名、没有说明：见 [`SkillProblem`]。
pub fn read(dir: &str, text: &str) -> Result<SkillHead, SkillProblem> {
    let front = front_matter(text).ok_or(SkillProblem::NoFrontmatter)?;
    let docs =
        YamlLoader::load_from_str(front).map_err(|error| SkillProblem::Yaml(error.to_string()))?;
    let doc = docs.first().ok_or(SkillProblem::NoName)?;
    let name = doc["name"].as_str().ok_or(SkillProblem::NoName)?.trim();
    if !valid_name(name) {
        return Err(SkillProblem::BadName(name.to_string()));
    }
    if name != dir {
        return Err(SkillProblem::NameMismatch {
            name: name.to_string(),
            dir: dir.to_string(),
        });
    }
    let description = doc["description"]
        .as_str()
        .map(str::trim)
        .unwrap_or_default();
    if description.is_empty() {
        return Err(SkillProblem::NoDescription);
    }
    let metadata = &doc["metadata"];
    Ok(SkillHead {
        name: name.to_string(),
        description: one_line(description)
            .chars()
            .take(DESCRIPTION_CHARS)
            .collect(),
        display_name: said(&metadata["display_name"]),
        summary: said(&metadata["summary"]),
    })
}

/// 开头 `---` 和下一个 `---` 之间的字；没有的没有。文件开头的 BOM 不算，行尾的 `\r` 不算。
fn front_matter(text: &str) -> Option<&str> {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let mut lines = text.split_inclusive('\n');
    let first = lines.next()?;
    if first.trim_end() != "---" {
        return None;
    }
    let start = first.len();
    let mut end = start;
    for line in lines {
        if line.trim_end() == "---" {
            return text.get(start..end);
        }
        end += line.len();
    }
    None
}

/// 名字合不合写法：1 到 64 个字，只用 `a-z`、`0-9`、`-`，不在头尾、不连着两个。
fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && name.chars().count() <= NAME_CHARS
        && name
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
        && !name.starts_with('-')
        && !name.ends_with('-')
        && !name.contains("--")
}

/// 说明并成一行：连着的空白（换行、制表符这些）换成一个空格。列出来时一个技能一行，换行照模板转义成 `\n` 不好读。
fn one_line(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// `metadata` 里一格给人看的字：去掉前后空白，空的、不是字的没有。
fn said(value: &Yaml) -> Option<String> {
    value
        .as_str()
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .map(str::to_string)
}

#[cfg(test)]
mod tests;
