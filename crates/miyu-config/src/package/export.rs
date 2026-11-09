//! `[export]`（施工 X-18；BUS-1 加 `provides`、`plugs`，设计 `67-总线.md` 第三节、B4、B6）：包对外给什么。
//!
//! - `methods`：给别的软件调的方法名，写法同后台页的方法（小写字母开头，小写字母、数字、`_`、`.`、`-`，1 到 64 个），不重复。
//! - `provides`：这个包在总线上提供的名字，一个名字装着的包里只有一家；`plugs`：这个包接进的名字，可以多家。名字是点分的几段，
//!   每段小写字母开头、小写字母、数字、`-`，整个不超过 64 个；同一张表里不重复，一个名字不能两样都写。
//! - 名字底下的方法照名字带前缀写进 `methods`（`stt` 的 `stt.transcribe`，短名只有一段，B10）：声明的每个名字至少要有一个，不然
//!   总线上什么都调不到。不在任何名字底下的方法照收（还没迁到总线的包在用），BUS-5
//!   收掉旧路时起拒。
//!
//! 没写的都是空的；只有扩展包能写（`kinds.rs`）。

use toml_edit::TableLike;

use super::reader::Reader;
use super::{Code, Problem};

/// 名字最多几个字符。
const NAME_MAX: usize = 64;

/// 包对外给什么。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Export {
    /// 给别的软件调的方法名，照写的先后。
    pub methods: Vec<String>,
    /// 在总线上提供的名字（一个名字一家）。
    pub provides: Vec<String>,
    /// 在总线上接进的名字（可以多家）。
    pub plugs: Vec<String>,
}

impl Export {
    /// 什么都没写。
    pub fn is_empty(&self) -> bool {
        self.methods.is_empty() && self.provides.is_empty() && self.plugs.is_empty()
    }

    /// `method` 写进了 `methods`。
    pub fn exports(&self, method: &str) -> bool {
        self.methods.iter().any(|one| one == method)
    }
}

/// 读 `[export]`。
pub(super) fn export(reader: &Reader<'_>, table: &dyn TableLike) -> Result<Export, Problem> {
    reader.only(table, "export", super::fields::EXPORT)?;
    let bad = |key: &str, name: &str, why: String| {
        reader.problem(table.get(key), Code::BadExport, name, why)
    };
    let methods = reader.texts(table, "export", "methods")?;
    for (index, name) in methods.iter().enumerate() {
        let why = if !method_name(name) {
            "is not a method name"
        } else if methods[..index].contains(name) {
            "is listed twice"
        } else {
            continue;
        };
        return Err(bad(
            "methods",
            name,
            format!("export.methods: {name:?} {why}"),
        ));
    }
    let provides = reader.texts(table, "export", "provides")?;
    let plugs = reader.texts(table, "export", "plugs")?;
    for (key, names) in [("provides", &provides), ("plugs", &plugs)] {
        for (index, name) in names.iter().enumerate() {
            let why = if !bus_name(name) {
                "is not a bus name: dot-separated parts of lowercase letters, digits and -"
                    .to_string()
            } else if names[..index].contains(name) {
                "is listed twice".to_string()
            } else if key == "plugs" && provides.contains(name) {
                "is in both provides and plugs".to_string()
            } else if !methods.iter().any(|method| under(method, name)) {
                format!(
                    "has no method in export.methods: write them as \"{name}.<method>\", <method> without dots"
                )
            } else {
                continue;
            };
            return Err(bad(key, name, format!("export.{key}: {name:?} {why}")));
        }
    }
    Ok(Export {
        methods,
        provides,
        plugs,
    })
}

/// 方法 `method` 在名字 `name` 底下：`<名字>.<短名>`，短名是一段、不带点（设计 67 B10）。所以 `stt.engine.transcribe` 只在
/// `stt.engine` 底下，不在 `stt` 底下。
pub fn under(method: &str, name: &str) -> bool {
    method
        .strip_prefix(name)
        .and_then(|rest| rest.strip_prefix('.'))
        .is_some_and(|rest| !rest.is_empty() && !rest.contains('.'))
}

/// 方法名的写法：小写字母开头，小写字母、数字、`_`、`.`、`-`，1 到 64 个（同 `package.methods` 登记的）。
fn method_name(name: &str) -> bool {
    name.len() <= NAME_MAX
        && name.starts_with(|c: char| c.is_ascii_lowercase())
        && name
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '_' | '.' | '-'))
}

/// 总线名字的写法：点分的几段，每段小写字母开头，小写字母、数字、`-`；整个不超过 64 个。
fn bus_name(name: &str) -> bool {
    name.len() <= NAME_MAX
        && name.split('.').all(|part| {
            part.starts_with(|c: char| c.is_ascii_lowercase())
                && part
                    .chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
        })
}

#[cfg(test)]
mod tests;
