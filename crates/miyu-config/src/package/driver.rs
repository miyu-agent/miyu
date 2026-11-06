//! 扩展提供的模型驱动（施工 MD-1，设计 `66-扩展提供模型驱动.md` 第四节）：`[[driver]]` 一段一个，读成 [`DriverDecl`]。只查写法；
//! 供应商的配置照 `driver = "<包编号>/<驱动名>"` 用它，只有手写的才指向扩展（档案、npm 表、默认永远是内置的三种）。只有扩展包能
//! 写（`kinds::tables`）；写了它必须在 `[process] capabilities` 里声明 `model.driver`，声明了 `model.driver` 也必须写它
//! （[`check`]，读完整份以后查）。

use toml_edit::Item;

use super::capability::Capability;
use super::reader::{Reader, command_name, line_of};
use super::{Code, Problem};

/// 扩展声明的一个模型驱动。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DriverDecl {
    /// 驱动名：写法同命令名（小写字母开头，小写字母、数字、`-`，最多 32 个），一个包里不重。
    pub name: String,
    /// 缓存类型（设计 05 第七节）：影响压缩怎么算，`contract` 的要过一致性检查（MD-3）。
    pub cache: DriverCache,
    /// 要不要核心交 key（设计 66 第九节第 3 条）。
    pub key: DriverKey,
    /// 看得了图的：请求里的图交过去（设计 66 第五节）。
    pub images: bool,
    /// 这一段写在第几行。
    pub line: Option<usize>,
}

impl DriverDecl {
    /// 配置里的写法：`<包编号>/<驱动名>`，和内置的几种撞不上（它们不带 `/`）。
    pub fn reference(&self, package: &str) -> String {
        format!("{package}/{}", self.name)
    }
}

/// 驱动名的写法：同命令名（小写字母开头，小写字母、数字、`-`，最多 32 个）。配置里 `<包编号>/<驱动名>` 的后半照它查（施工 MD-1）。
pub fn driver_name(name: &str) -> bool {
    command_name(name)
}

/// 驱动的缓存类型。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DriverCache {
    /// 契约型：前缀不变就命中。
    Contract,
    /// 尽力而为。
    BestEffort,
    /// 没有缓存。
    None,
}

/// 驱动要不要 key。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DriverKey {
    /// 不要：配置里写了也不交。
    None,
    /// 可有可无。
    Optional,
    /// 一定要：没配的 `miyu check` 报错。
    Required,
}

/// `[[driver]]`：一段一个，驱动名在一个包里不重。
pub(super) fn read(reader: &Reader<'_>, node: &Item) -> Result<Vec<DriverDecl>, Problem> {
    let Some(array) = node.as_array_of_tables() else {
        return Err(reader.problem(
            Some(node),
            Code::NotATable,
            "driver",
            "driver is written as [[driver]], one table per driver".to_string(),
        ));
    };
    let mut drivers: Vec<DriverDecl> = Vec::new();
    for table in array.iter() {
        let line = table.span().map(|span| line_of(reader.text, span.start));
        reader.only(table, "driver", super::fields::DRIVER)?;
        let name = match table.get("name").and_then(Item::as_str) {
            Some(name) if command_name(name) => name.to_string(),
            Some(name) => {
                return Err(bad(
                    line,
                    &format!("driver.name {name:?}"),
                    format!(
                        "driver name {name:?} must start with a lowercase letter, use only lowercase letters, digits and -, at most 32"
                    ),
                ));
            }
            None => {
                return Err(bad(
                    line,
                    "driver.name",
                    "driver.name is missing or not text".to_string(),
                ));
            }
        };
        let at = format!("driver.{name}");
        if drivers.iter().any(|seen| seen.name == name) {
            return Err(bad(line, &at, format!("driver {name:?} is written twice")));
        }
        let cache = match table.get("cache").map(|item| item.as_str()) {
            None | Some(Some("best_effort")) => DriverCache::BestEffort,
            Some(Some("contract")) => DriverCache::Contract,
            Some(Some("none")) => DriverCache::None,
            Some(_) => {
                return Err(bad(
                    line,
                    &format!("{at}.cache"),
                    format!("{at}.cache must be \"contract\", \"best_effort\" or \"none\""),
                ));
            }
        };
        let key = match table.get("key").map(|item| item.as_str()) {
            None | Some(Some("optional")) => DriverKey::Optional,
            Some(Some("none")) => DriverKey::None,
            Some(Some("required")) => DriverKey::Required,
            Some(_) => {
                return Err(bad(
                    line,
                    &format!("{at}.key"),
                    format!("{at}.key must be \"none\", \"optional\" or \"required\""),
                ));
            }
        };
        let images = match table.get("images") {
            None => false,
            Some(item) => item.as_bool().ok_or_else(|| {
                bad(
                    line,
                    &format!("{at}.images"),
                    format!("{at}.images must be true or false"),
                )
            })?,
        };
        drivers.push(DriverDecl {
            name,
            cache,
            key,
            images,
            line,
        });
    }
    Ok(drivers)
}

/// 写了 `[[driver]]` 的要声明 `model.driver`，声明了 `model.driver` 的要写 `[[driver]]`（设计 66 第四节第 1 条）：报在第一段上，
/// 没写的报在整份上。
pub(super) fn check(drivers: &[DriverDecl], capabilities: &[Capability]) -> Result<(), Problem> {
    let declared = capabilities.contains(&Capability::ModelDriver);
    match drivers.first() {
        Some(first) if !declared => Err(bad(
            first.line,
            "driver",
            "[[driver]] needs the model.driver capability in [process] capabilities".to_string(),
        )),
        None if declared => Err(bad(
            None,
            "model.driver",
            "the model.driver capability needs at least one [[driver]]".to_string(),
        )),
        _ => Ok(()),
    }
}

/// 写错了的一处。
fn bad(line: Option<usize>, key: &str, message: String) -> Problem {
    Problem {
        line,
        code: Code::BadDriver,
        detail: key.to_string(),
        message,
    }
}

#[cfg(test)]
mod tests;
