//! 从资源目录读工具的字（`26-提示词.md` 第八节）：`software/<软件包>/tools/<工具>.json` 是给模型看的说明和
//! 参数格式，`software/<软件包>/<工具>/<名字>.txt` 是输出里给她看的几句（施工 4-4 下）。原来在基础系统里、软件包名写死，
//! 施工 R-3 中挪到这里、多一个软件包名：记忆这个软件包照同一套读（同一件事不写两遍）。
//!
//! 自己带着字的软件包（接入QQ，施工 S-6 上）不在资源目录的 `software/` 下：字放在包文件夹的 `data/` 里，照 [`spec_in`]、
//! [`text_in`] 读，给的是那个目录本身，下面的 `tools/`、`<工具>/` 和 `software/<软件包>/` 下的一个样子。

use std::collections::BTreeMap;
use std::fmt;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::Spec;
use miyu_kernel::raw::RawJson;
use miyu_kernel::template::Template;
use miyu_kernel::tool::Access;

/// 一份字读不出来，或者写法不对。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoadError {
    /// 哪一份。
    pub file: PathBuf,
    /// 为什么。
    pub why: String,
}

impl fmt::Display for LoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.file.display(), self.why)
    }
}

impl std::error::Error for LoadError {}

/// `tools/<工具>.json` 的样子。
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SpecFile {
    description: String,
    parameters: RawJson,
    /// 要先加载（施工 L-1）：出厂的都不写，全量。
    #[serde(default)]
    need_load: bool,
    /// 脚本里能调（施工 M-2）：出厂只有几件写 `true`，没写的关。
    #[serde(default)]
    clicall: bool,
    /// 结构化结果的样子（施工 M-4）：只有交结构化结果的写。
    #[serde(default)]
    result: Option<String>,
}

/// 读软件包 `package` 的工具 `name` 的说明和参数格式，访问类别是 `access`。
///
/// # Errors
///
/// 读不出来；不是 `{description, parameters}` 的样子。
pub fn spec(
    resources: &Path,
    package: &str,
    name: &str,
    access: Access,
) -> Result<Spec, LoadError> {
    spec_in(&package_dir(resources, package), name, access)
}

/// 同 [`spec`]，字在目录 `dir` 里（`dir/tools/<工具>.json`）：包文件夹自己带的字（施工 S-6 上）。
///
/// # Errors
///
/// 同 [`spec`]。
pub fn spec_in(dir: &Path, name: &str, access: Access) -> Result<Spec, LoadError> {
    let file = spec_file(dir, name);
    let text = read(&file)?;
    let parsed: SpecFile = serde_json::from_str(&text).map_err(|error| LoadError {
        file: file.clone(),
        why: error.to_string(),
    })?;
    Ok(Spec {
        name: name.to_string(),
        description: parsed.description,
        parameters: parsed.parameters,
        access,
        need_load: parsed.need_load,
        clicall: parsed.clicall,
        result: parsed.result,
    })
}

/// 同 [`spec`]，说明是一段模板，换进 `fields`：说明里要写这台机器上的东西的（`shell` 写用的是哪种 shell，施工
/// 4-8）。核心起来时换一次，会话里不变。别的工具的说明不当模板读：里面的花括号是字面的。
///
/// # Errors
///
/// 同 [`spec`]；说明不是合写法的模板，或者要了没给的字段。
pub fn spec_filled(
    resources: &Path,
    package: &str,
    name: &str,
    access: Access,
    fields: &[(&str, &str)],
) -> Result<Spec, LoadError> {
    let mut spec = spec(resources, package, name, access)?;
    let bad = |why: String| LoadError {
        file: spec_file(&package_dir(resources, package), name),
        why,
    };
    let template = Template::parse(&spec.description).map_err(|error| bad(error.to_string()))?;
    let fields: BTreeMap<&str, &str> = fields.iter().copied().collect();
    spec.description = template
        .render(&fields)
        .map_err(|error| bad(error.to_string()))?;
    Ok(spec)
}

/// 读软件包 `package` 的工具 `tool` 输出里的一句 `name`，拿 `fields` 里的每个字段试换一次。
///
/// # Errors
///
/// 读不出来；不是合写法的模板，或者要了 `fields` 以外的字段。
pub fn text(
    resources: &Path,
    package: &str,
    tool: &str,
    name: &str,
    fields: &[&str],
) -> Result<Template, LoadError> {
    text_in(&package_dir(resources, package), tool, name, fields)
}

/// 同 [`text`]，字在目录 `dir` 里（`dir/<工具>/<名字>.txt`）：包文件夹自己带的字（施工 S-6 上）。
///
/// # Errors
///
/// 同 [`text`]。
pub fn text_in(dir: &Path, tool: &str, name: &str, fields: &[&str]) -> Result<Template, LoadError> {
    let file = dir.join(tool).join(format!("{name}.txt"));
    let source = read(&file)?;
    let bad = |why: String| LoadError {
        file: file.clone(),
        why,
    };
    let template = Template::parse(&source).map_err(|error| bad(error.to_string()))?;
    let trial: BTreeMap<&str, &str> = fields.iter().map(|field| (*field, "")).collect();
    template
        .render(&trial)
        .map_err(|error| bad(error.to_string()))?;
    Ok(template)
}

/// 读软件包 `package` 的工具 `tool` 的一份字 `name` 的原文，不当模板读（施工 M-4：`shell` 说明里的例子，里面有花括号）。
///
/// # Errors
///
/// 读不出来。
pub fn raw(resources: &Path, package: &str, tool: &str, name: &str) -> Result<String, LoadError> {
    read(
        &package_dir(resources, package)
            .join(tool)
            .join(format!("{name}.txt")),
    )
}

/// 照模板 `template` 换进字段 `fields`。
///
/// # Panics
///
/// 实际不会：造的时候试换过，字段都有。
pub fn say(template: &Template, fields: &[(&str, &str)]) -> String {
    let fields: BTreeMap<&str, &str> = fields.iter().copied().collect();
    template.render(&fields).expect("造的时候试换过，字段都有")
}

/// 资源目录 `resources` 里软件包 `package` 的字在哪个目录：`software/<软件包>/`。
fn package_dir(resources: &Path, package: &str) -> PathBuf {
    resources.join("software").join(package)
}

/// 字在 `dir` 里的工具 `name` 的说明在哪。
fn spec_file(dir: &Path, name: &str) -> PathBuf {
    dir.join("tools").join(format!("{name}.json"))
}

/// 读一份文件。
fn read(file: &Path) -> Result<String, LoadError> {
    std::fs::read_to_string(file).map_err(|error| LoadError {
        file: file.to_path_buf(),
        why: error.to_string(),
    })
}

#[cfg(test)]
mod tests;
