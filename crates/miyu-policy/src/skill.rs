//! 技能 `SKILL.md` 的开头（施工 X-10 上，`docs/blueprint/skills.md`「技能」第 1 条）：照公开的 Agent Skills 格式读。第一行是
//! `---`，到下一个只有 `---` 的行之间是 YAML：
//!
//! - `name`：1 到 64 个字，只用小写字母、数字、`-`，不在头尾、不连着两个，和技能的文件夹同名；
//! - `description`：给模型看的一句，不能空，超过 [`DESCRIPTION_CHARS`] 个字的截掉（格式的上限）；
//! - `metadata` 里的 `display_name`、`summary`：给人看的名字、一句说明（双槽：给模型的和给人的分开）；`_zh`、`_ja` 结尾的是那两种
//!   语言的（施工 X-13 下）。
//!
//! 别的格子（`license`、`compatibility`、`allowed-tools`……）照格式认、不用。纯逻辑：进来的是文件夹的名字和文件里的字，出去的是
//! 读好的样子（带几条警告），或者哪里不对。
//!
//! 宽松地读（施工 PT-2 上，照 agentskills.io 接入指南的「Lenient validation」）：别家写的技能常有小毛病，能用的照收、记警告
//! （[`SkillWarning`]），只有真读不成的才不收（[`SkillProblem`]）：
//!
//! - 名字和文件夹不同名的照收、照写的名字用；名字没写、写错的照文件夹的名字用；两个都不合写法的才不收。
//! - YAML 读不懂的，把没加引号、值里带冒号的单行（别家技能最常见的写法：`description: Use when: …`）当一整句字重读一次，
//!   还读不懂才不收。
//! - 说明超了上限的截掉，`compatibility` 超过 [`COMPATIBILITY_CHARS`] 个字、`metadata` 不是字到字的表的，照收、记警告。

use std::collections::BTreeMap;

use std::fmt;

use yaml_rust2::{Yaml, YamlLoader};

/// 说明最多几个字（Unicode 字符）：格式的上限，超过的截到这么多。
pub const DESCRIPTION_CHARS: usize = 1024;

/// 名字最多几个字。
const NAME_CHARS: usize = 64;

/// `compatibility` 最多几个字（格式的上限）。
pub const COMPATIBILITY_CHARS: usize = 500;

/// 技能算的那个功能的编号的开头（施工 X-11 上，设计 `30-插件框架.md` 第三节第 2 条）：`skill-<名字>`，同 MCP 服务的
/// `mcp-<服务名>`。
const FEATURE_PREFIX: &str = "skill-";

/// 技能 `name` 算的功能的编号（施工 X-11 上）：预设照它开关。
pub fn feature_id(name: &str) -> String {
    format!("{FEATURE_PREFIX}{name}")
}

/// 读好的开头。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkillHead {
    /// 名字：和文件夹同名。
    pub name: String,
    /// 给模型看的一句：并成一行，截过的。
    pub description: String,
    /// 给人看的名字，照语言（施工 X-13 下）：`metadata` 的 `display_name` 算 `en`（也是别的语言没写时退回的那一份），
    /// `display_name_zh`、`display_name_ja` 是另两种。格式的 `metadata` 只收字到字的表，不嵌套，语言写进键里。
    pub display_name: BTreeMap<String, String>,
    /// 给人看的一句说明，照语言：`summary`、`summary_zh`、`summary_ja`，同上。
    pub summary: BTreeMap<String, String>,
    /// 照收了、但不合格式的地方（施工 PT-2 上）：`miyu check`、`skill.list` 报给人看。
    pub warnings: Vec<SkillWarning>,
}

/// 照收了、但不合格式的地方（施工 PT-2 上）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SkillWarning {
    /// 写的名字和文件夹不同名：照写的名字用。
    NameMismatch {
        /// 写的名字。
        name: String,
        /// 文件夹的名字。
        dir: String,
    },
    /// 名字没写，或者不合写法（写了的是这一句）：照文件夹的名字用。
    NameFromFolder {
        /// 写了的那个不合写法的名字；没写的没有。
        written: Option<String>,
        /// 文件夹的名字，照它用。
        dir: String,
    },
    /// YAML 本来读不懂，照「没加引号的值当一整句字」重读才读成。
    LenientYaml,
    /// 说明超过 [`DESCRIPTION_CHARS`] 个字，截掉了。
    DescriptionCut,
    /// `compatibility` 不是字，或者超过 [`COMPATIBILITY_CHARS`] 个字。
    Compatibility,
    /// `metadata` 不是字到字的表。
    Metadata,
}

impl SkillWarning {
    /// 代码：`miyu check` 的问题码、`skill.list` 的 `warnings` 里的 `code`，给人看的那一句照它在字表里找。
    pub fn code(&self) -> &'static str {
        match self {
            SkillWarning::NameMismatch { .. } => "skill_name_mismatch",
            SkillWarning::NameFromFolder { .. } => "skill_name_from_folder",
            SkillWarning::LenientYaml => "skill_yaml_lenient",
            SkillWarning::DescriptionCut => "skill_description_cut",
            SkillWarning::Compatibility => "skill_compatibility",
            SkillWarning::Metadata => "skill_metadata",
        }
    }

    /// 填进那一句的细节：名字那两种是照哪个名字用的，别的没有。
    pub fn detail(&self) -> String {
        match self {
            SkillWarning::NameMismatch { name, .. } => name.clone(),
            SkillWarning::NameFromFolder { dir, .. } => dir.clone(),
            _ => String::new(),
        }
    }
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
    /// `name` 不合写法，文件夹的名字也不合（施工 PT-2 上：只有一个不合的照另一个用）。
    BadName(String),
    /// 没写 `description`，或者是空的。
    NoDescription,
}

impl fmt::Display for SkillProblem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SkillProblem::NoFrontmatter => f.write_str("no front matter between --- lines"),
            SkillProblem::Yaml(why) => write!(f, "front matter is not valid YAML: {why}"),
            SkillProblem::NoName => {
                f.write_str("name is missing and the folder name is not a valid skill name")
            }
            SkillProblem::BadName(name) => write!(
                f,
                "name {name:?} must be 1 to 64 lowercase letters, digits and single hyphens (the folder name is not valid either)"
            ),
            SkillProblem::NoDescription => f.write_str("description is missing or empty"),
        }
    }
}

/// 读文件夹 `dir` 里那份 `SKILL.md` 的字 `text`：照收了、不合格式的地方记在 [`SkillHead::warnings`]。
///
/// # Errors
///
/// 开头没有 YAML 那一段、宽松重读也读不懂、名字和文件夹的名字都不合写法、没有说明：见 [`SkillProblem`]。
pub fn read(dir: &str, text: &str) -> Result<SkillHead, SkillProblem> {
    let front = front_matter(text).ok_or(SkillProblem::NoFrontmatter)?;
    let mut warnings = Vec::new();
    let docs = match YamlLoader::load_from_str(front) {
        Ok(docs) => docs,
        Err(error) => {
            let retried = lenient(front)
                .and_then(|fixed| YamlLoader::load_from_str(&fixed).ok())
                .ok_or_else(|| SkillProblem::Yaml(error.to_string()))?;
            warnings.push(SkillWarning::LenientYaml);
            retried
        }
    };
    let empty = Yaml::Null;
    let doc = docs.first().unwrap_or(&empty);
    let name = name(doc["name"].as_str().map(str::trim), dir, &mut warnings)?;
    let description = doc["description"]
        .as_str()
        .map(str::trim)
        .unwrap_or_default();
    if description.is_empty() {
        return Err(SkillProblem::NoDescription);
    }
    let description = one_line(description);
    if description.chars().count() > DESCRIPTION_CHARS {
        warnings.push(SkillWarning::DescriptionCut);
    }
    let compatibility = &doc["compatibility"];
    let fits = compatibility
        .as_str()
        .is_some_and(|text| text.chars().count() <= COMPATIBILITY_CHARS);
    if !compatibility.is_badvalue() && !fits {
        warnings.push(SkillWarning::Compatibility);
    }
    let metadata = &doc["metadata"];
    if !metadata.is_badvalue() && !text_map(metadata) {
        warnings.push(SkillWarning::Metadata);
    }
    Ok(SkillHead {
        name,
        description: description.chars().take(DESCRIPTION_CHARS).collect(),
        display_name: phrases(metadata, "display_name"),
        summary: phrases(metadata, "summary"),
        warnings,
    })
}

/// 照写的名字 `written`、文件夹的名字 `dir` 定技能叫什么（施工 PT-2 上）：写的合写法照它（和文件夹不同名的记警告）；没写、
/// 写错的照文件夹的（记警告）；两个都不合的不收。
fn name(
    written: Option<&str>,
    dir: &str,
    warnings: &mut Vec<SkillWarning>,
) -> Result<String, SkillProblem> {
    match written {
        Some(name) if valid_name(name) => {
            if name != dir {
                warnings.push(SkillWarning::NameMismatch {
                    name: name.to_string(),
                    dir: dir.to_string(),
                });
            }
            Ok(name.to_string())
        }
        _ if valid_name(dir) => {
            warnings.push(SkillWarning::NameFromFolder {
                written: written.map(str::to_string),
                dir: dir.to_string(),
            });
            Ok(dir.to_string())
        }
        Some(name) => Err(SkillProblem::BadName(name.to_string())),
        None => Err(SkillProblem::NoName),
    }
}

/// 宽松重读（施工 PT-2 上）：顶格的 `键: 值` 里，值没加引号、自己又带着 `: ` 或者以 `:` 结尾的（YAML 会当成又一层键值），
/// 整个值加上双引号当一句字。改了的交回改好的；一行都没改的没有（重读也没用）。
fn lenient(front: &str) -> Option<String> {
    let mut changed = false;
    let lines: Vec<String> = front
        .lines()
        .map(|line| match quoted(line) {
            Some(fixed) => {
                changed = true;
                fixed
            }
            None => line.to_string(),
        })
        .collect();
    changed.then(|| lines.join("\n"))
}

/// 一行要加引号的：加好的那一行；不用加的没有。
fn quoted(line: &str) -> Option<String> {
    if line.starts_with([' ', '\t', '#', '-']) {
        return None;
    }
    let (key, value) = line.split_once(": ")?;
    let value = value.trim();
    let plain = key
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-');
    let opens_something = value.starts_with(['"', '\'', '|', '>', '[', '{', '&', '*', '!', '#']);
    let colon = value.contains(": ") || value.ends_with(':');
    (plain && !key.is_empty() && !opens_something && colon).then(|| {
        let escaped = value.replace('\\', "\\\\").replace('"', "\\\"");
        format!("{key}: \"{escaped}\"")
    })
}

/// 字到字的表（`metadata` 的格式）：是表，键和值都是字。
fn text_map(value: &Yaml) -> bool {
    value.as_hash().is_some_and(|hash| {
        hash.iter()
            .all(|(key, value)| key.as_str().is_some() && value.as_str().is_some())
    })
}

/// `metadata` 里照语言写的一格：`<键>` 是 `en`，`<键>_zh`、`<键>_ja` 是另两种；没写的、空的不记。
fn phrases(metadata: &Yaml, key: &str) -> BTreeMap<String, String> {
    [("", "en"), ("_zh", "zh"), ("_ja", "ja")]
        .into_iter()
        .filter_map(|(suffix, language)| {
            said(&metadata[format!("{key}{suffix}").as_str()])
                .map(|text| (language.to_string(), text))
        })
        .collect()
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
