//! 扩展提供的模型驱动（施工 MD-1，设计 `66-扩展提供模型驱动.md` 第四节）：`[[driver]]` 一段一个，读成 [`DriverDecl`]。只查写法；
//! 供应商的配置照 `driver = "<包编号>/<驱动名>"` 用它，只有手写的才指向扩展（档案、npm 表、默认永远是内置的三种）。只有扩展包能
//! 写（`kinds::tables`）；写了它必须在 `[process] capabilities` 里声明 `model.driver`，声明了 `model.driver` 也必须写它
//! （[`check`]，读完整份以后查）。

use toml_edit::{Item, Table};

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
    /// 要不要供应商的地址（施工 FX-11）：同 `key` 的三种写法，没写的是可有可无。头照它和 `key` 只画这家真要的那几项
    /// （`model.list` 的 `needs`）。只给头看：配置里写了的地址照旧交给扩展（`miyu-session` 的 `route/extension.rs`）。
    pub base_url: DriverKey,
    /// 看得了图的：请求里的图交过去（设计 66 第五节）。
    pub images: bool,
    /// 读得了文件的：请求里的文件块（PDF、音频、视频这类）也交过去（施工 FX-17）。
    pub files: bool,
    /// 会让供应商自己跑的工具名（设计 71 第三节第 7 条）：装包确认屏照它列出来，交上来的不在里面的核心照收、记一行 `WARN`。
    /// 每个 1 到 64 个字节、不带控制字符、不重；没写的是空的。
    pub hosted: Vec<String>,
    /// 多久没动静才算断，秒（施工 DR-2）：等第一个字、两段之间都照它等，不比核心的空闲时限短（短的不收紧）；1 到
    /// 900（`QUIET_SECS_MOST`），没写的照核心的。供应商不流思考、头一个字要等一两分钟的驱动写它。
    pub quiet_secs: Option<u64>,
    /// 这一段写在第几行。
    pub line: Option<usize>,
}

impl DriverDecl {
    /// 配置里的写法：`<包编号>/<驱动名>`，和内置的几种撞不上（它们不带 `/`）。
    pub fn reference(&self, package: &str) -> String {
        format!("{package}/{}", self.name)
    }
}

/// 清单 `hosted` 里一个工具名最多几个字节（设计 71 第三节第 7 条）：供应商的工具名都很短，写长了多半是写错了。执行器记
/// 「没声明的名字说过没有」也照这个数（更长的一定没声明，不进表）。
pub const HOSTED_NAME_BYTES: usize = 64;

/// 清单 `quiet_secs` 至多多少秒（施工 DR-2）：十五分钟。总时限照它的十倍，再长的卡住了也要等太久才知道。
pub(super) const QUIET_SECS_MOST: u64 = 900;

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

/// 驱动要不要 key、要不要地址（施工 FX-11 起地址也用它）。
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
        let base_url = match table.get("base_url").map(|item| item.as_str()) {
            None | Some(Some("optional")) => DriverKey::Optional,
            Some(Some("none")) => DriverKey::None,
            Some(Some("required")) => DriverKey::Required,
            Some(_) => {
                return Err(bad(
                    line,
                    &format!("{at}.base_url"),
                    format!("{at}.base_url must be \"none\", \"optional\" or \"required\""),
                ));
            }
        };
        let images = switch(table, "images", line, &at)?;
        let files = switch(table, "files", line, &at)?;
        let hosted = hosted(table.get("hosted"), line, &at)?;
        let quiet_secs = match table.get("quiet_secs") {
            None => None,
            Some(item) => Some(
                item.as_integer()
                    .and_then(|secs| u64::try_from(secs).ok())
                    .filter(|secs| (1..=QUIET_SECS_MOST).contains(secs))
                    .ok_or_else(|| {
                        bad(
                            line,
                            &format!("{at}.quiet_secs"),
                            format!(
                                "{at}.quiet_secs must be a whole number of seconds from 1 to {QUIET_SECS_MOST}"
                            ),
                        )
                    })?,
            ),
        };
        drivers.push(DriverDecl {
            name,
            cache,
            key,
            base_url,
            images,
            files,
            hosted,
            quiet_secs,
            line,
        });
    }
    Ok(drivers)
}

/// 开关格（`images`、`files`）：`true`、`false`，没写的是 `false`。
fn switch(table: &Table, key: &str, line: Option<usize>, at: &str) -> Result<bool, Problem> {
    match table.get(key) {
        None => Ok(false),
        Some(item) => item.as_bool().ok_or_else(|| {
            bad(
                line,
                &format!("{at}.{key}"),
                format!("{at}.{key} must be true or false"),
            )
        }),
    }
}

/// `hosted`：工具名的数组，没写的是空的；每个 1 到 [`HOSTED_NAME_BYTES`] 个字节、不带控制字符、一段里不重（施工 HT-4）。
fn hosted(item: Option<&Item>, line: Option<usize>, at: &str) -> Result<Vec<String>, Problem> {
    let Some(item) = item else {
        return Ok(Vec::new());
    };
    let key = format!("{at}.hosted");
    let names = item
        .as_array()
        .and_then(|array| {
            array
                .iter()
                .map(|value| value.as_str().map(str::to_string))
                .collect::<Option<Vec<_>>>()
        })
        .ok_or_else(|| bad(line, &key, format!("{key} must be a list of tool names")))?;
    for (n, name) in names.iter().enumerate() {
        if name.is_empty() || name.len() > HOSTED_NAME_BYTES || name.chars().any(char::is_control) {
            return Err(bad(
                line,
                &key,
                format!(
                    "{key} {name:?} must be 1 to {HOSTED_NAME_BYTES} bytes without control characters"
                ),
            ));
        }
        if names[..n].contains(name) {
            return Err(bad(line, &key, format!("{key} {name:?} is written twice")));
        }
    }
    Ok(names)
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
