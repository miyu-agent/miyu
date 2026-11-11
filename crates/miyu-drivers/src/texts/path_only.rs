//! 只有路径的附件写成的那一句（施工 AT-1，`docs/blueprint/drivers/openai-chat.md` 第 9 条）：超过上传上限的文件，头只把它在
//! 核心那台机器上的路径交上来（`kernel/blocks.md` 的 `path` 块），请求里没有内容可发，写一句说明附了什么、在哪、内容
//! 没带上。和模型能看什么无关：哪个模型都是这一句。
//!
//! 两份模板：模板没有能不填的字段，写了大小的、没写的各一份。以前造的快照里没有这两句，只写路径本身（照模板的规矩
//! 转义）：等于人在话里写了一个路径，不多出没登记的字。

use std::collections::BTreeMap;

use miyu_kernel::template::{Template, TemplateError, escape};

use super::{DriverTexts, render};

/// 只有路径的附件的两句（施工 AT-1）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct PathOnly {
    /// 没写大小的：字段是 `name`、`path`。
    bare: Template,
    /// 写了大小的：字段是 `name`、`size`、`path`。
    sized: Template,
}

/// 只有路径的附件的两句的原文（施工 AT-1）。
#[derive(Debug, Clone, Copy)]
pub struct PathOnlySources<'a> {
    /// `file-path-only.txt`。
    pub file_path_only: &'a str,
    /// `file-path-only-sized.txt`。
    pub file_path_only_sized: &'a str,
}

impl PathOnly {
    /// 读两份模板，拿字段试着换一次：没写大小的那一份只能要 `name`、`path`，写了的另加 `size`。
    pub(super) fn new(sources: PathOnlySources<'_>) -> Result<PathOnly, TemplateError> {
        let texts = PathOnly {
            bare: Template::parse(sources.file_path_only)?,
            sized: Template::parse(sources.file_path_only_sized)?,
        };
        texts.bare.render(&fields("", "", None))?;
        texts.sized.render(&fields("", "", Some("")))?;
        Ok(texts)
    }
}

impl DriverTexts {
    /// 只有路径的附件（施工 AT-1）换成的这一句：名字 `name`、路径 `path`，头写了大小 `size`（字节数）的带上。快照里
    /// 没有这两句的（以前造的），只写路径本身，照模板的规矩转义。
    pub fn path_only(&self, name: &str, path: &str, size: Option<u64>) -> String {
        let Some(texts) = &self.path_only else {
            return escape(path);
        };
        match size {
            Some(size) => {
                let size = size.to_string();
                render(&texts.sized, &fields(name, path, Some(&size)))
            }
            None => render(&texts.bare, &fields(name, path, None)),
        }
    }
}

/// 模板的字段：名字、路径，有大小的另加大小。
fn fields<'a>(name: &'a str, path: &'a str, size: Option<&'a str>) -> BTreeMap<&'a str, &'a str> {
    let mut fields = BTreeMap::from([("name", name), ("path", path)]);
    if let Some(size) = size {
        fields.insert("size", size);
    }
    fields
}
