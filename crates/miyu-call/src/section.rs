//! 跑命令的工具说明末尾讲脚本调工具的那一段（施工 M-4，设计 `39-脚本调工具.md` 第九节；施工 XG-1 下起内置 `shell`、扩展声明了
//! `runs_commands` 的共用，设计 63 第六节第 2 条）：这个会话的工具面上有脚本里能调的工具时，组工具面的一方照 [`Section::render`]
//! 接上（`Tool::clicall_section`）。字在 `core/call/`（原来在 `software/basesystem/shell/call/`，字节没变）：
//!
//! - `describe.txt`：怎么用，`{tools}` 换成能调的名单；
//! - `example.txt`：`Example: {example}`，`{example}` 照跑命令的 shell 挑 `example-*.txt` 里的一行，造工具时挑一次；都用不上的
//!   不带这一行；
//! - `shapes.txt`：`{shapes}` 换成有结构化结果的那几件的样子，一件都没有的不带这一行。
//!
//! 换进去的都是登记来的字（工具名、工具自己写的样子、资源文件），照原样，不转义：转了引号，样子就看不懂了（同 `--schema`）。

use std::collections::BTreeMap;
use std::ffi::OsStr;
use std::path::Path;

use miyu_kernel::template::Template;

use miyu_tool::load::LoadError;

/// 那一段的几句，和这台机器上挑好的例子。
pub struct Section {
    describe: Template,
    example: Option<String>,
    shapes: Template,
}

/// 例子用哪一行：PowerShell 的用自带的；bash、zsh 的有 `jq` 用它，没有、有 `python3` 用 Python；都没有的不带（设计第九节第 2 条）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Example {
    /// PowerShell 的。
    PowerShell,
    /// bash、zsh 有 `jq` 的。
    Jq,
    /// bash、zsh 没有 `jq`、有 `python3` 的。
    Python,
}

impl Example {
    /// 照 shell 是不是 POSIX 那一路的（`posix`：bash、zsh 是，PowerShell 不是）和 `PATH`（`path`）挑。
    pub fn pick(posix: bool, path: Option<&OsStr>) -> Option<Example> {
        if !posix {
            return Some(Example::PowerShell);
        }
        if which(path, "jq") {
            return Some(Example::Jq);
        }
        which(path, "python3").then_some(Example::Python)
    }

    /// 资源文件的名字。
    fn file(self) -> &'static str {
        match self {
            Example::PowerShell => "example-powershell",
            Example::Jq => "example-jq",
            Example::Python => "example-python",
        }
    }
}

impl Section {
    /// 照资源目录 `resources` 读，例子照 `example` 挑好。
    ///
    /// # Errors
    ///
    /// 哪一句读不出来、换不进字段：写明是哪个文件。
    pub fn load(resources: &Path, example: Option<Example>) -> Result<Section, LoadError> {
        let text = |name: &str, fields: &[&str]| crate::text(resources, name, fields);
        let example = match example {
            Some(example) => {
                let line = crate::raw(resources, example.file())?;
                let template = text("example", &["example"])?;
                Some(fill(&template, "example", line.trim_end()))
            }
            None => None,
        };
        Ok(Section {
            describe: text("describe", &["tools"])?,
            example,
            shapes: text("shapes", &["shapes"])?,
        })
    }

    /// 接在说明后面的那一段：`tools` 是能调的名单，`shapes` 是结果的样子（空的不带那一行）。一行一句，例子、样子各一行。
    pub fn render(&self, tools: &str, shapes: &str) -> String {
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

/// 在 `path` 列的目录里找叫 `name` 的程序。相对路径的那几项不看（施工 R3-1 补）：照它找到的跟着进程的工作目录变，核心的工作
/// 目录是数据根。同 `shell` 找程序的那一个。
fn which(path: Option<&OsStr>, name: &str) -> bool {
    path.is_some_and(|path| {
        std::env::split_paths(path)
            .filter(|dir| dir.is_absolute())
            .any(|dir| dir.join(name).is_file())
    })
}

/// 照原样换进一个字段：读的时候试换过。
fn fill(template: &Template, name: &str, value: &str) -> String {
    template
        .fill(&BTreeMap::from([(name, value)]), str::to_string)
        .expect("读的时候试换过，字段都有")
}

#[cfg(test)]
mod tests;
