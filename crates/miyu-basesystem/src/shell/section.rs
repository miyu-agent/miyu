//! `shell` 说明末尾讲脚本调工具的那一段（施工 M-4，设计 `39-脚本调工具.md` 第九节）：这个会话的工具面上有脚本里能调的工具时，
//! 组工具面的一方照 [`Section::render`] 接上（`Tool::clicall_section`）。字在 `software/basesystem/shell/call/`：
//!
//! - `describe.txt`：怎么用，`{tools}` 换成能调的名单；
//! - `example.txt`：`Example: {example}`，`{example}` 照这台机器的 shell 挑 `example-*.txt` 里的一行，核心起来时挑一次；都用不上的
//!   不带这一行；
//! - `shapes.txt`：`{shapes}` 换成有结构化结果的那几件的样子，一件都没有的不带这一行。
//!
//! 换进去的都是登记来的字（工具名、工具自己写的样子、资源文件），照原样，不转义：转了引号，样子就看不懂了（同 `--schema`）。

use std::collections::BTreeMap;
use std::ffi::OsStr;
use std::path::Path;

use miyu_kernel::template::Template;

use crate::load::{self, LoadError};

use super::program::{Kind, which};

/// 那一段的几句，和这台机器上挑好的例子。
pub(super) struct Section {
    describe: Template,
    example: Option<String>,
    shapes: Template,
}

/// 例子用哪一行：PowerShell 的用自带的；bash、zsh 的有 `jq` 用它，没有、有 `python3` 用 Python；都没有的不带（设计第九节第 2 条）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Example {
    PowerShell,
    Jq,
    Python,
}

impl Example {
    /// 照 shell 的种类 `kind` 和 `PATH`（`path`）挑。
    pub(super) fn pick(kind: Kind, path: Option<&OsStr>) -> Option<Example> {
        if !kind.posix() {
            return Some(Example::PowerShell);
        }
        if which(path, "jq").is_some() {
            return Some(Example::Jq);
        }
        which(path, "python3").map(|_| Example::Python)
    }

    /// 资源文件的名字。
    fn file(self) -> &'static str {
        match self {
            Example::PowerShell => "call/example-powershell",
            Example::Jq => "call/example-jq",
            Example::Python => "call/example-python",
        }
    }
}

impl Section {
    /// 照资源目录 `resources` 读，例子照 `example` 挑好。
    pub(super) fn load(resources: &Path, example: Option<Example>) -> Result<Section, LoadError> {
        let text = |name: &str, fields: &[&str]| load::text(resources, "shell", name, fields);
        let example = match example {
            Some(example) => {
                let line = load::raw(resources, "shell", example.file())?;
                let template = text("call/example", &["example"])?;
                Some(fill(&template, "example", line.trim_end()))
            }
            None => None,
        };
        Ok(Section {
            describe: text("call/describe", &["tools"])?,
            example,
            shapes: text("call/shapes", &["shapes"])?,
        })
    }

    /// 接在说明后面的那一段：`tools` 是能调的名单，`shapes` 是结果的样子（空的不带那一行）。一行一句，例子、样子各一行。
    pub(super) fn render(&self, tools: &str, shapes: &str) -> String {
        let mut lines = vec![fill(&self.describe, "tools", tools)];
        lines.extend(self.example.clone());
        if !shapes.is_empty() {
            lines.push(fill(&self.shapes, "shapes", shapes));
        }
        lines
            .iter()
            .map(|line| line.trim_end())
            .collect::<Vec<_>>()
            .join("\n")
    }
}

/// 照原样换进一个字段：读的时候试换过。
fn fill(template: &Template, name: &str, value: &str) -> String {
    template
        .fill(&BTreeMap::from([(name, value)]), str::to_string)
        .expect("读的时候试换过，字段都有")
}

#[cfg(test)]
mod tests;
