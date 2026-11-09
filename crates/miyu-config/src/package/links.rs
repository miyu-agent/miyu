//! 包和别处的几条连线（施工 F-1，设计 `30-插件框架.md` 第五节、第六节，`docs/blueprint/packages.md`「格式」）：平台接入
//! `[connection]`、依赖的包 `[depends]`、`[recommends]`（施工 F-11 起什么种类都行，PK-2 起能带版本下限）、小程序自己的 `[worker]`。

use toml_edit::{Item, TableLike};

use super::depend::Dependency;
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

/// `[depends]`、`[recommends]`（`name` 是哪一张）：`packages` 是别的包的编号，什么种类都行（施工 F-11，照 pacman 只有一份包名），
/// 施工 PK-2 起能带版本下限 `<编号>>=<版本>`（[`Dependency::parse`]）；以前写的 `workers` 照样认、读进同一份，只写编号。同一个
/// 编号不写两次（带不带下限都算）；没写的是空的。
pub(super) fn dependencies(
    reader: &Reader<'_>,
    table: &dyn TableLike,
    name: &str,
) -> Result<Vec<Dependency>, Problem> {
    reader.only(table, name, super::fields::LINKS)?;
    let mut listed: Vec<Dependency> = Vec::new();
    for key in ["packages", "workers"] {
        for written in reader.texts(table, name, key)? {
            let why = match Dependency::parse(&written) {
                Some(dep) if key == "workers" && dep.at_least.is_some() => "is not a package id",
                Some(dep) if listed.iter().any(|seen| seen.id == dep.id) => "is listed twice",
                Some(dep) => {
                    listed.push(dep);
                    continue;
                }
                None if written.contains(">=") => {
                    "is not a package id followed by >= and a version like 1.2.0"
                }
                None => "is not a package id",
            };
            return Err(reader.problem(
                table.get(key),
                Code::BadDependency,
                &written,
                format!("{name}.{key}: {written:?} {why}"),
            ));
        }
    }
    Ok(listed)
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
