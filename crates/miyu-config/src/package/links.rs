//! 包和别处的几条连线（施工 F-1，设计 `30-插件框架.md` 第五节、第六节，`docs/blueprint/packages.md`「格式」）：平台接入
//! `[connection]`、依赖的包 `[depends]`、`[recommends]`（施工 F-11 起什么种类都行）、小程序自己的 `[worker]`。

use toml_edit::{Item, TableLike};

use super::reader::{Reader, program_name};
use super::{Code, Problem};
use crate::secret::valid_name;

/// `[connection]`：这个扩展接的是哪几个通讯平台。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Connection {
    /// 平台名，写法同包的编号，例如 `qq`；照写的先后、不重复、至少一个。清单里写一个字的是一个，写列表的是几个（施工 IM-3，
    /// 接入通讯平台设计 07 第 4.6 节第 1 样：一个 im 接几个平台）。
    pub platforms: Vec<String>,
}

impl Connection {
    /// 接不接平台 `platform`。
    pub fn has(&self, platform: &str) -> bool {
        self.platforms.iter().any(|one| one == platform)
    }
}

/// `[worker]`：小程序怎么拉起。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Worker {
    /// 程序名，不带路径；只找 `miyu` 旁边的（`packages.md`「转交」）。
    pub program: String,
    /// 拉起时带的参数；没写的是空的。
    pub args: Vec<String>,
}

/// `[connection]`：`platform` 必写，一个字或者一列字（施工 IM-3）；列表不能空、不能重复。
pub(super) fn connection(
    reader: &Reader<'_>,
    table: &dyn TableLike,
    at: &Item,
) -> Result<Connection, Problem> {
    reader.only(table, "connection", super::fields::CONNECTION)?;
    let item = reader.required(table, at, "connection", "platform")?;
    let bad = || {
        reader.problem(
            Some(item),
            Code::BadPlatform,
            "connection.platform",
            "connection.platform must be a name or a non-empty list of distinct names; a name starts with a lowercase letter and uses only lowercase letters, digits, - and _".to_string(),
        )
    };
    let written: Vec<&str> = match (item.as_str(), item.as_array()) {
        (Some(one), _) => vec![one],
        (None, Some(list)) => list
            .iter()
            .map(|value| value.as_str().ok_or_else(bad))
            .collect::<Result<_, _>>()?,
        (None, None) => return Err(bad()),
    };
    let mut platforms: Vec<String> = Vec::new();
    for name in written {
        if !valid_name(name) || platforms.iter().any(|seen| seen == name) {
            return Err(bad());
        }
        platforms.push(name.to_string());
    }
    if platforms.is_empty() {
        return Err(bad());
    }
    Ok(Connection { platforms })
}

/// `[depends]`、`[recommends]`（`name` 是哪一张）：`packages` 是别的包的编号，什么种类都行（施工 F-11，照 pacman 只有一份包名）；
/// 以前写的 `workers` 照样认，读进同一份。不重复；没写的是空的。
pub(super) fn dependencies(
    reader: &Reader<'_>,
    table: &dyn TableLike,
    name: &str,
) -> Result<Vec<String>, Problem> {
    reader.only(table, name, super::fields::LINKS)?;
    let mut listed: Vec<String> = Vec::new();
    for key in ["packages", "workers"] {
        for id in reader.texts(table, name, key)? {
            let why = if !valid_name(&id) {
                "is not a package id"
            } else if listed.contains(&id) {
                "is listed twice"
            } else {
                listed.push(id);
                continue;
            };
            return Err(reader.problem(
                table.get(key),
                Code::BadDependency,
                &id,
                format!("{name}.{key}: {id:?} {why}"),
            ));
        }
    }
    Ok(listed)
}

/// `[export]`（施工 X-18）：`methods` 是给别的软件经 `package.call` 调的方法名，写法同后台页的方法（小写字母开头，小写字母、
/// 数字、`_`、`.`、`-`，1 到 64 个），不重复；没写的是空的。
pub(super) fn export(reader: &Reader<'_>, table: &dyn TableLike) -> Result<Vec<String>, Problem> {
    reader.only(table, "export", super::fields::EXPORT)?;
    let methods = reader.texts(table, "export", "methods")?;
    for (index, name) in methods.iter().enumerate() {
        let why = if !method_name(name) {
            "is not a method name"
        } else if methods[..index].contains(name) {
            "is listed twice"
        } else {
            continue;
        };
        return Err(reader.problem(
            table.get("methods"),
            Code::BadExport,
            name,
            format!("export.methods: {name:?} {why}"),
        ));
    }
    Ok(methods)
}

/// 方法名的写法：小写字母开头，小写字母、数字、`_`、`.`、`-`，1 到 64 个（同 `package.methods` 登记的）。
fn method_name(name: &str) -> bool {
    name.len() <= 64
        && name.starts_with(|c: char| c.is_ascii_lowercase())
        && name
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '_' | '.' | '-'))
}

/// `[worker]`：`program` 必写。
pub(super) fn worker(
    reader: &Reader<'_>,
    table: &dyn TableLike,
    at: &Item,
) -> Result<Worker, Problem> {
    reader.only(table, "worker", super::fields::WORKER)?;
    let item = reader.required(table, at, "worker", "program")?;
    let program = item
        .as_str()
        .filter(|program| program_name(program))
        .ok_or_else(|| {
            reader.problem(
                Some(item),
                Code::BadProgram,
                "worker.program",
                "worker.program must be a program name without a path".to_string(),
            )
        })?;
    Ok(Worker {
        program: program.to_string(),
        args: reader.texts(table, "worker", "args")?,
    })
}

#[cfg(test)]
mod tests;
